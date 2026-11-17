//! TU-Varna timetables.
//!
//! The university publishes bachelor timetables on umo.tu-varna.bg as Excel "Save as web
//! page" files, one per specialty / course / group. A student picks the group in a form
//! that answers with a 302 to the file. We do the same on the server (the site sends no
//! CORS headers, so a browser cannot), then turn the Excel grid into a clean lesson list.
//!
//! Safety: only allow-listed hosts, only ports 80/443, DNS answers that point into private
//! networks are refused (SSRF guard), redirects are followed by hand and re-checked, the
//! body is capped, and results are cached so we stay gentle with the university server.

use crate::AppState;
use crate::session::User;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use scraper::{ElementRef, Html, Node, Selector};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Meta {
    pub year: String,
    pub semester: String,
    pub course: String,
    pub stream: String,
    pub specialty: String,
    pub group: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Lesson {
    /// 1 = Monday … 7 = Sunday
    pub day: u8,
    pub start: String,
    pub end: String,
    pub subject: String,
    /// lecture | lab | seminar | exercise | seminar_lab | project | other
    pub kind: String,
    /// The abbreviation as written, e.g. "л.у."
    pub kind_label: String,
    /// "1а", "1б" … or empty for the whole group.
    pub subgroup: String,
    /// all | odd | even
    pub parity: String,
    pub room: String,
    pub lecturer: String,
    pub hours: String,
    /// Other lines: notes, elective options, week ranges.
    pub extra: Vec<String>,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Timetable {
    pub meta: Meta,
    pub lessons: Vec<Lesson>,
    pub notes: Vec<String>,
}

// ---------------------------------------------------------------------------------------
// Parsing the Excel-exported grid
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Cell {
    row: usize,
    col: usize,
    rs: usize,
    cs: usize,
    text: String,
    grey: bool,
}

impl Cell {
    fn last_col(&self) -> usize {
        self.col + self.cs - 1
    }
    fn last_row(&self) -> usize {
        self.row + self.rs - 1
    }
}

#[derive(Debug, Clone, Copy)]
struct Slot {
    col: usize,
    cs: usize,
    start: u32,
    end: u32,
}

fn sel(s: &str) -> Selector {
    Selector::parse(s).expect("static selector")
}

fn norm_ws(s: &str) -> String {
    s.split(|c: char| c.is_whitespace() || c == '\u{a0}')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn cell_text(el: ElementRef) -> String {
    let mut s = String::new();
    for node in el.descendants() {
        match node.value() {
            Node::Text(t) => s.push_str(t),
            Node::Element(e) if e.name() == "br" => s.push(' '),
            _ => {}
        }
    }
    norm_ws(&s)
}

/// CSS classes whose background is a grey (Excel marks even-week classes in grey).
fn grey_classes(doc: &Html) -> HashSet<String> {
    let mut out = HashSet::new();
    for st in doc.select(&sel("style")) {
        let css: String = st.text().collect();
        for block in css.split('}') {
            let Some((name, body)) = block.split_once('{') else {
                continue;
            };
            let name = name.trim();
            if let Some(cls) = name.strip_prefix('.')
                && style_is_grey(body)
            {
                out.insert(cls.trim().to_string());
            }
        }
    }
    out
}

fn style_is_grey(style: &str) -> bool {
    let lower = style.to_ascii_lowercase();
    let Some(i) = lower.find("background:") else {
        return false;
    };
    let v = lower[i + 11..].trim_start();
    if v.starts_with("silver") || v.starts_with("gray") || v.starts_with("grey") {
        return true;
    }
    if let Some(hexv) = v.strip_prefix('#')
        && hexv.len() >= 6
    {
        let p = |a: usize| u8::from_str_radix(&hexv[a..a + 2], 16).ok();
        if let (Some(r), Some(g), Some(b)) = (p(0), p(2), p(4)) {
            let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
            return mx - mn <= 10 && (0x80..=0xE8).contains(&mx);
        }
    }
    false
}

fn grid(html: &str) -> Result<Vec<Cell>, String> {
    let doc = Html::parse_document(html);
    let grey = grey_classes(&doc);
    // Pick the table that holds the "ден/час" header (the timetable itself).
    let table = doc
        .select(&sel("table"))
        .find(|t| {
            t.text()
                .any(|s| s.contains("ден/час") || s.contains("ден / час"))
        })
        .ok_or("not_a_timetable")?;
    let mut occupied: HashSet<(usize, usize)> = HashSet::new();
    let mut cells = Vec::new();
    for (r, tr) in table.select(&sel("tr")).enumerate() {
        let mut c = 0usize;
        for td in tr
            .children()
            .filter_map(ElementRef::wrap)
            .filter(|e| matches!(e.value().name(), "td" | "th"))
        {
            while occupied.contains(&(r, c)) {
                c += 1;
            }
            let num = |a: &str| {
                td.value()
                    .attr(a)
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, 60)
            };
            let (rs, cs) = (num("rowspan"), num("colspan"));
            for dr in 0..rs {
                for dc in 0..cs {
                    occupied.insert((r + dr, c + dc));
                }
            }
            let text = cell_text(td);
            let is_grey = td
                .value()
                .attr("class")
                .map(|cl| cl.split_whitespace().any(|x| grey.contains(x)))
                .unwrap_or(false)
                || td.value().attr("style").map(style_is_grey).unwrap_or(false)
                || td
                    .value()
                    .attr("bgcolor")
                    .map(|b| style_is_grey(&format!("background:{b}")))
                    .unwrap_or(false);
            if !text.is_empty() {
                cells.push(Cell {
                    row: r,
                    col: c,
                    rs,
                    cs,
                    text,
                    grey: is_grey,
                });
            }
            c += cs;
        }
    }
    Ok(cells)
}

/// "715 - 800", "7 15 - 8 00" or "7:15-8:00" → minutes since midnight.
fn parse_range(t: &str) -> Option<(u32, u32)> {
    let runs: Vec<&str> = t
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .collect();
    let hm = |h: &str, m: &str| -> Option<u32> {
        let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
        (h < 24 && m < 60).then_some(h * 60 + m)
    };
    let split = |s: &str| -> Option<u32> {
        if s.len() < 3 || s.len() > 4 {
            return None;
        }
        hm(&s[..s.len() - 2], &s[s.len() - 2..])
    };
    match runs.as_slice() {
        [a, b] => Some((split(a)?, split(b)?)),
        [h1, m1, h2, m2] => Some((hm(h1, m1)?, hm(h2, m2)?)),
        _ => None,
    }
}

/// "12,45", "7,30", "13.30", "13:30" → minutes.
fn parse_marker(t: &str) -> Option<u32> {
    let t = t.trim();
    let (h, m) = t.split_once([',', '.', ':'])?;
    if h.is_empty() || h.len() > 2 || m.len() != 2 {
        return None;
    }
    let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

pub fn hhmm(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

fn day_number(t: &str) -> Option<u8> {
    let l = t.to_lowercase();
    let l = l.trim();
    const DAYS: [(&str, u8); 7] = [
        ("пон", 1),
        ("вто", 2),
        ("сря", 3),
        ("чет", 4),
        ("пет", 5),
        ("съб", 6),
        ("нед", 7),
    ];
    DAYS.iter()
        .find(|(p, _)| l.starts_with(p))
        .map(|(_, n)| *n)
        .filter(|_| l.chars().count() <= 12)
}

const TITLES: [&str; 12] = [
    "проф",
    "доц",
    "гл. ас",
    "гл.ас",
    "ас.",
    "ас ",
    "д-р",
    "преп",
    "ст. преп",
    "ст.преп",
    "инж",
    "хон",
];

fn is_lecturer(t: &str) -> bool {
    let l = t.to_lowercase();
    TITLES.iter().any(|p| l.starts_with(p))
}

fn is_room(t: &str) -> bool {
    let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let up = compact.to_uppercase();
    let rest = up
        .strip_prefix("НУК")
        .or_else(|| up.strip_prefix("NUK"))
        .unwrap_or(&up);
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    if !(2..=4).contains(&digits) {
        return false;
    }
    let tail: String = rest.chars().skip(digits).collect();
    tail.chars().count() <= 4 && tail.chars().all(|c| c.is_alphabetic())
}

fn is_hours(t: &str) -> bool {
    let c: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    let digits = c.chars().take_while(|c| c.is_ascii_digit()).count();
    digits > 0 && matches!(&c[digits..], "ч." | "ч" | "часа")
}

const KINDS: [(&str, &str); 11] = [
    ("с.у.+л.у.", "seminar_lab"),
    ("л.у.+с.у.", "seminar_lab"),
    ("к.пр.", "project"),
    ("л.у.", "lab"),
    ("с.у.", "seminar"),
    ("лаб.", "lab"),
    ("упр.", "exercise"),
    ("сем.", "seminar"),
    ("л.", "lecture"),
    ("у.", "exercise"),
    ("с.", "seminar"),
];

struct Head {
    parity: &'static str,
    subgroup: String,
    kind: &'static str,
    kind_label: String,
    rest: String,
}

/// Peel parity, sub-group and type off the front of a lesson text.
fn parse_head(text: &str) -> Head {
    let mut t = text.trim().to_string();
    let mut parity = "all";
    // "(нечетна седмица)", "(четна с.)"
    if let Some(open) = t.find('(') {
        let after = t[open + 1..].to_lowercase();
        let odd = after.starts_with("нечетна");
        let even = after.starts_with("четна");
        if (odd || even)
            && let Some(close) = t[open..].find(')')
        {
            parity = if odd { "odd" } else { "even" };
            t = format!("{}{}", &t[..open], &t[open + close + 1..]);
            t = t.trim().to_string();
        }
    }
    // "1а)", "2б)"
    let mut subgroup = String::new();
    if let Some(close) = t.find(')') {
        let pre = &t[..close];
        let n = pre.chars().count();
        if (2..=3).contains(&n)
            && pre.chars().next().is_some_and(|c| c.is_ascii_digit())
            && pre.chars().last().is_some_and(|c| c.is_alphabetic())
        {
            subgroup = pre.to_string();
            t = t[close + 1..].trim().to_string();
        }
    }
    let mut kind = "other";
    let mut kind_label = String::new();
    let lower = t.to_lowercase();
    for (label, k) in KINDS {
        if lower.starts_with(label) {
            kind = k;
            let n = label.chars().count();
            kind_label = t.chars().take(n).collect();
            t = t.chars().skip(n).collect::<String>().trim().to_string();
            break;
        }
    }
    Head {
        parity,
        subgroup,
        kind,
        kind_label,
        rest: t,
    }
}

fn looks_like_lesson(text: &str) -> bool {
    let h = parse_head(text);
    h.parity != "all" || !h.subgroup.is_empty() || h.kind != "other"
}

struct Draft {
    head: Cell,
    lines: Vec<Cell>,
    bottom: usize,
}

fn overlap(a0: usize, a1: usize, b0: usize, b1: usize) -> usize {
    let lo = a0.max(b0);
    let hi = a1.min(b1);
    if hi >= lo { hi - lo + 1 } else { 0 }
}

pub fn parse(html: &str) -> Result<Timetable, String> {
    let cells = grid(html)?;

    // 1. Time slots from the "ден/час" header row.
    let hdr = cells
        .iter()
        .find(|c| c.text.starts_with("ден"))
        .ok_or("no_header")?
        .row;
    let slots: Vec<Slot> = cells
        .iter()
        .filter(|c| c.row == hdr && c.col > 0)
        .filter_map(|c| {
            parse_range(&c.text).map(|(s, e)| Slot {
                col: c.col,
                cs: c.cs,
                start: s,
                end: e,
            })
        })
        .collect();
    if slots.len() < 4 {
        return Err("no_time_slots".into());
    }
    let slot_of = |col: usize| slots.iter().find(|s| col >= s.col && col < s.col + s.cs);
    // A half-column inside a slot: the first half starts on time, the second 15 min later;
    // a lesson ending in a first half ends 30 min into the slot (matches the "12,45" style
    // markers the university writes next to such lessons).
    let col_start =
        |col: usize| slot_of(col).map(|s| if col == s.col { s.start } else { s.start + 15 });
    let col_end = |col: usize| {
        slot_of(col).map(|s| {
            if col + 1 == s.col + s.cs {
                s.end
            } else {
                s.start + 30
            }
        })
    };

    // 2. Meta from the label row ("учебна година", "семестър" …) and the row above it.
    let mut meta = Meta::default();
    if let Some(lr) = cells
        .iter()
        .find(|c| c.text.contains("учебна година"))
        .map(|c| c.row)
        && lr > 0
    {
        for label in cells.iter().filter(|c| c.row == lr) {
            let val = cells
                .iter()
                .find(|v| v.row < lr && v.last_row() >= lr - 1 && v.col == label.col);
            let Some(val) = val else { continue };
            let v = val.text.clone();
            let l = label.text.to_lowercase();
            if l.contains("година") {
                meta.year = v;
            } else if l.contains("семест") {
                meta.semester = v;
            } else if l.contains("курс") {
                meta.course = v;
            } else if l.contains("поток") {
                meta.stream = v;
            } else if l.contains("спец") {
                meta.specialty = v;
            } else if l.contains("група") {
                meta.group = v;
            }
        }
    }

    // 3. Day blocks.
    let days: Vec<(u8, usize, usize)> = cells
        .iter()
        .filter(|c| c.col == 0 && c.row > hdr)
        .filter_map(|c| day_number(&c.text).map(|d| (d, c.row, c.last_row())))
        .collect();
    if days.is_empty() {
        return Err("no_days".into());
    }
    let last_day_row = days.iter().map(|d| d.2).max().unwrap_or(hdr);

    // 4. Lessons, day by day.
    let mut lessons = Vec::new();
    for &(day, r0, r1) in &days {
        let mut in_day: Vec<&Cell> = cells
            .iter()
            .filter(|c| c.col > 0 && c.row >= r0 && c.row <= r1)
            .collect();
        in_day.sort_by_key(|c| (c.row, c.col));
        let (markers, content): (Vec<&Cell>, Vec<&Cell>) = in_day
            .into_iter()
            .partition(|c| parse_marker(&c.text).is_some());

        let mut drafts: Vec<Draft> = Vec::new();
        for c in content {
            let starts_new = c.rs >= 2 || looks_like_lesson(&c.text);
            // The lesson directly above this cell in the same columns, if any.
            let above = drafts
                .iter_mut()
                .filter(|d| {
                    d.bottom < c.row
                        && overlap(d.head.col, d.head.last_col(), c.col, c.last_col()) * 2
                            >= c.cs.min(d.head.cs)
                })
                .max_by_key(|d| d.bottom);
            match above {
                Some(d) if !starts_new => {
                    d.bottom = c.last_row();
                    d.lines.push(c.clone());
                }
                _ => drafts.push(Draft {
                    head: c.clone(),
                    lines: vec![],
                    bottom: c.last_row(),
                }),
            }
        }

        for d in drafts {
            let h = &d.head;
            let mut start = col_start(h.col).unwrap_or(0);
            let mut end = col_end(h.last_col()).unwrap_or(0);
            let aligned_start = slot_of(h.col).map(|s| s.col == h.col).unwrap_or(true);
            let aligned_end = slot_of(h.last_col())
                .map(|s| s.col + s.cs == h.last_col() + 1)
                .unwrap_or(true);
            for m in &markers {
                let rows_meet = m.row <= d.bottom && m.last_row() >= h.row;
                if !rows_meet {
                    continue;
                }
                let v = parse_marker(&m.text).unwrap_or(0);
                if !aligned_start && m.last_col() + 1 == h.col {
                    start = v;
                }
                if !aligned_end && h.last_col() + 1 == m.col {
                    end = v;
                }
            }
            if end <= start {
                end = start + 90;
            }
            lessons.push(build_lesson(day, start, end, h, &d.lines));
        }
    }
    lessons.sort_by(|a, b| (a.day, &a.start, &a.subgroup).cmp(&(b.day, &b.start, &b.subgroup)));

    // 5. Footnotes under the grid.
    let notes = cells
        .iter()
        .filter(|c| c.row > last_day_row && !c.text.to_lowercase().starts_with("забележка"))
        .map(|c| c.text.clone())
        .collect();

    Ok(Timetable {
        meta,
        lessons,
        notes,
    })
}

fn build_lesson(day: u8, start: u32, end: u32, head: &Cell, lines: &[Cell]) -> Lesson {
    let h = parse_head(&head.text);
    let mut parity = h.parity;
    if parity == "all" && head.grey {
        parity = "even";
    }
    let mut room = String::new();
    let mut lecturer = String::new();
    let mut hours = String::new();
    let mut extra = Vec::new();

    let mut segs: Vec<String> = h
        .rest
        .split(" - ")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let mut subject = if segs.is_empty() {
        String::new()
    } else {
        segs.remove(0)
    };
    // "ООП 1ч." → subject "ООП", hours "1ч."
    if let Some(pos) = subject.rfind(' ') {
        let tail = subject[pos + 1..].to_string();
        let tail2 = subject[..pos]
            .rsplit_once(' ')
            .map(|(_, n)| format!("{n} {tail}"));
        if is_hours(&tail) {
            hours = tail;
            subject = subject[..pos].trim().to_string();
        } else if let Some(t2) = tail2.filter(|t| is_hours(t)) {
            hours = t2.clone();
            subject = subject[..subject.len() - t2.len()].trim().to_string();
        }
    }
    // "л.у.Web. диз. - 513ІІЕ" has no space after the type; nothing else to do.
    for s in segs {
        if is_lecturer(&s) && lecturer.is_empty() {
            lecturer = s;
        } else if is_room(&s) && room.is_empty() {
            room = s;
        } else if is_hours(&s) && hours.is_empty() {
            hours = s;
        } else if s.contains("с.)") || s.contains("седм") {
            // "(1-7с.) 412Е, (8-15с.) 823М": rooms change by week.
            if room.is_empty() {
                room = s;
            } else {
                extra.push(s);
            }
        } else {
            extra.push(s);
        }
    }
    for l in lines {
        let t = l.text.trim().to_string();
        if is_lecturer(&t) && lecturer.is_empty() {
            lecturer = t;
        } else if is_room(&t) && room.is_empty() {
            room = t;
        } else {
            extra.push(t);
        }
    }
    if subject.is_empty() {
        subject = if !extra.is_empty() {
            extra.remove(0)
        } else {
            head.text.clone()
        };
    }
    Lesson {
        day,
        start: hhmm(start),
        end: hhmm(end),
        subject: subject.trim_end_matches(':').trim().to_string(),
        kind: h.kind.to_string(),
        kind_label: h.kind_label,
        subgroup: h.subgroup,
        parity: parity.to_string(),
        room,
        lecturer,
        hours,
        extra,
        raw: std::iter::once(head.text.as_str())
            .chain(lines.iter().map(|l| l.text.as_str()))
            .collect::<Vec<_>>()
            .join(" / "),
    }
}

/// The <select> lists of the university's group form.
pub fn parse_options(html: &str) -> Value {
    let doc = Html::parse_document(html);
    let mut out = serde_json::Map::new();
    for name in ["spec", "kurs", "grupa"] {
        let list: Vec<Value> = doc
            .select(&sel(&format!("select[name=\"{name}\"] option")))
            .filter_map(|o| {
                let v = o.value().attr("value")?.trim().to_string();
                let label = norm_ws(&o.text().collect::<String>());
                (!v.is_empty() && v.len() <= 6 && v.bytes().all(|b| b.is_ascii_digit()))
                    .then(|| json!({"value": v, "label": label}))
            })
            .collect();
        out.insert(name.into(), Value::Array(list));
    }
    Value::Object(out)
}

// ---------------------------------------------------------------------------------------
// Fetching (SSRF-safe)
// ---------------------------------------------------------------------------------------

pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_broadcast()
                || v.is_documentation()
                || v.is_unspecified()
                || v.is_multicast()
                || o[0] == 0
                || (o[0] == 100 && (64..128).contains(&o[1]))
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19))
                || o[0] >= 240)
        }
        IpAddr::V6(v) => {
            if let Some(v4) = v.to_ipv4_mapped() {
                return is_public_ip(IpAddr::V4(v4));
            }
            let s = v.segments();
            !(v.is_loopback()
                || v.is_unspecified()
                || v.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00
                || (s[0] & 0xffc0) == 0xfe80)
        }
    }
}

/// DNS resolver that drops private, loopback and link-local answers.
struct PublicOnlyResolver;

impl reqwest::dns::Resolve for PublicOnlyResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|a| is_public_ip(a.ip()))
                .collect();
            if addrs.is_empty() {
                return Err("host resolves only to non-public addresses".into());
            }
            Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

pub fn host_allowed(u: &url::Url, allowed: &[String]) -> bool {
    if !matches!(u.scheme(), "http" | "https") || !u.username().is_empty() || u.password().is_some()
    {
        return false;
    }
    if let Some(p) = u.port()
        && p != 80
        && p != 443
    {
        return false;
    }
    let Some(url::Host::Domain(h)) = u.host() else {
        return false;
    };
    let h = h.to_ascii_lowercase();
    allowed.iter().any(|a| {
        let a = a.trim().trim_start_matches('.').to_ascii_lowercase();
        h == a || h.ends_with(&format!(".{a}"))
    })
}

pub struct Fetcher {
    client: reqwest::Client,
    cache: Mutex<HashMap<String, (Instant, Value)>>,
}

impl Fetcher {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .dns_resolver(Arc::new(PublicOnlyResolver))
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(8))
            .timeout(Duration::from_secs(20))
            .user_agent(concat!(
                "TUV-Hub/",
                env!("CARGO_PKG_VERSION"),
                " (student timetable reader)"
            ))
            .build()
            .expect("http client");
        Self {
            client,
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn cached(&self, key: &str, ttl: Duration) -> Option<Value> {
        let c = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        c.get(key)
            .filter(|(t, _)| t.elapsed() < ttl)
            .map(|(_, v)| v.clone())
    }

    fn store(&self, key: String, v: Value) {
        let mut c = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if c.len() > 2000 {
            c.clear();
        }
        c.insert(key, (Instant::now(), v));
    }

    /// GET a page, following up to 3 redirects, each re-checked against the allow-list.
    async fn get_text(
        &self,
        url: &url::Url,
        allowed: &[String],
        max: usize,
    ) -> Result<(url::Url, String), String> {
        let mut cur = url.clone();
        for _ in 0..4 {
            if !host_allowed(&cur, allowed) {
                return Err("host_not_allowed".into());
            }
            let res = self
                .client
                .get(cur.clone())
                .send()
                .await
                .map_err(|e| net_err(&e))?;
            if res.status().is_redirection() {
                cur = next_location(&cur, res.headers())?;
                continue;
            }
            if !res.status().is_success() {
                return Err(format!("upstream_{}", res.status().as_u16()));
            }
            let charset = res
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            let bytes = read_capped(res, max).await?;
            return Ok((cur, decode(&bytes, &charset)));
        }
        Err("too_many_redirects".into())
    }

    /// Submit the university's group form and return where it points.
    async fn resolve(
        &self,
        form: &url::Url,
        allowed: &[String],
        spec: &str,
        kurs: &str,
        grupa: &str,
    ) -> Result<url::Url, String> {
        if !host_allowed(form, allowed) {
            return Err("host_not_allowed".into());
        }
        let res = self
            .client
            .post(form.clone())
            .form(&[("spec", spec), ("kurs", kurs), ("grupa", grupa)])
            .send()
            .await
            .map_err(|e| net_err(&e))?;
        if !res.status().is_redirection() {
            return Err("no_timetable_for_group".into());
        }
        let to = next_location(form, res.headers())?;
        if !host_allowed(&to, allowed) {
            return Err("host_not_allowed".into());
        }
        Ok(to)
    }
}

fn next_location(base: &url::Url, h: &reqwest::header::HeaderMap) -> Result<url::Url, String> {
    let loc = h
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .ok_or("bad_redirect")?;
    base.join(loc).map_err(|_| "bad_redirect".to_string())
}

fn net_err(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "upstream_timeout".into()
    } else {
        "upstream_unreachable".into()
    }
}

async fn read_capped(res: reqwest::Response, max: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut stream = res.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| net_err(&e))?;
        if out.len() + chunk.len() > max {
            return Err("too_large".into());
        }
        out.extend_from_slice(&chunk);
    }
    Ok(out)
}

fn decode(bytes: &[u8], content_type: &str) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let enc = if content_type.contains("1251")
        || content_type.is_empty()
        || !content_type.contains("utf")
    {
        encoding_rs::WINDOWS_1251
    } else {
        encoding_rs::UTF_8
    };
    enc.decode(bytes).0.into_owned()
}

// ---------------------------------------------------------------------------------------
// HTTP handlers
// ---------------------------------------------------------------------------------------

fn err(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error": code}))).into_response()
}

fn form_url(st: &AppState, semester: &str) -> Result<url::Url, Response> {
    let comp = match semester {
        "winter" => "com_razp_zimen_sem&Itemid=12",
        "summer" => "com_razp_leten_sem&Itemid=29",
        _ => return Err(err(StatusCode::BAD_REQUEST, "bad_semester")),
    };
    let base = st.cfg.schedule.umo_base.trim_end_matches('/');
    url::Url::parse(&format!("{base}/index.php?option={comp}"))
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "bad_config"))
}

fn upstream_status(code: &str) -> StatusCode {
    match code {
        "host_not_allowed" | "bad_url" => StatusCode::BAD_REQUEST,
        "no_timetable_for_group"
        | "not_a_timetable"
        | "no_header"
        | "no_time_slots"
        | "no_days" => StatusCode::UNPROCESSABLE_ENTITY,
        "upstream_timeout" => StatusCode::GATEWAY_TIMEOUT,
        _ => StatusCode::BAD_GATEWAY,
    }
}

fn gate(st: &AppState, headers: &HeaderMap, ip: IpAddr) -> Result<User, Response> {
    if !st.limiter.allow(ip, crate::security::Bucket::Schedule) {
        return Err(crate::security::too_many());
    }
    crate::require_user(st, headers).map(|(_, u)| u)
}

#[derive(Deserialize)]
pub struct OptionsReq {
    pub semester: String,
}

pub async fn options(
    State(st): State<Arc<AppState>>,
    axum::Extension(ip): axum::Extension<crate::security::ClientIp>,
    headers: HeaderMap,
    Json(req): Json<OptionsReq>,
) -> Response {
    if let Err(r) = gate(&st, &headers, ip.0) {
        return r;
    }
    let url = match form_url(&st, &req.semester) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let key = format!("opts:{url}");
    if let Some(v) = st.fetcher.cached(&key, Duration::from_secs(6 * 3600)) {
        return Json(v).into_response();
    }
    match st
        .fetcher
        .get_text(&url, &st.cfg.schedule.allowed_hosts, 1_000_000)
        .await
    {
        Ok((_, html)) => {
            let v = parse_options(&html);
            if v["spec"].as_array().is_none_or(|a| a.is_empty()) {
                return err(StatusCode::BAD_GATEWAY, "form_changed");
            }
            st.fetcher.store(key, v.clone());
            Json(v).into_response()
        }
        Err(e) => err(upstream_status(&e), &e),
    }
}

#[derive(Deserialize)]
pub struct ResolveReq {
    pub semester: String,
    pub spec: String,
    pub kurs: String,
    pub grupa: String,
}

pub async fn resolve(
    State(st): State<Arc<AppState>>,
    axum::Extension(ip): axum::Extension<crate::security::ClientIp>,
    headers: HeaderMap,
    Json(req): Json<ResolveReq>,
) -> Response {
    if let Err(r) = gate(&st, &headers, ip.0) {
        return r;
    }
    let ok = |s: &str| !s.is_empty() && s.len() <= 4 && s.bytes().all(|b| b.is_ascii_digit());
    if !ok(&req.spec) || !ok(&req.kurs) || !ok(&req.grupa) {
        return err(StatusCode::BAD_REQUEST, "bad_choice");
    }
    let form = match form_url(&st, &req.semester) {
        Ok(u) => u,
        Err(r) => return r,
    };
    let rkey = format!(
        "res:{}:{}:{}:{}",
        req.semester, req.spec, req.kurs, req.grupa
    );
    let ttl = Duration::from_secs(st.cfg.schedule.cache_minutes * 60);
    let target = match st
        .fetcher
        .cached(&rkey, ttl)
        .and_then(|v| v.as_str().map(String::from))
    {
        Some(u) => url::Url::parse(&u).ok(),
        None => None,
    };
    let target = match target {
        Some(t) => t,
        None => match st
            .fetcher
            .resolve(
                &form,
                &st.cfg.schedule.allowed_hosts,
                &req.spec,
                &req.kurs,
                &req.grupa,
            )
            .await
        {
            Ok(t) => {
                st.fetcher.store(rkey, Value::String(t.to_string()));
                t
            }
            Err(e) => return err(upstream_status(&e), &e),
        },
    };
    load(&st, target).await
}

#[derive(Deserialize)]
pub struct FetchReq {
    pub url: String,
}

pub async fn fetch(
    State(st): State<Arc<AppState>>,
    axum::Extension(ip): axum::Extension<crate::security::ClientIp>,
    headers: HeaderMap,
    Json(req): Json<FetchReq>,
) -> Response {
    if let Err(r) = gate(&st, &headers, ip.0) {
        return r;
    }
    let Ok(u) = url::Url::parse(req.url.trim()) else {
        return err(StatusCode::BAD_REQUEST, "bad_url");
    };
    if req.url.len() > 2000 || !host_allowed(&u, &st.cfg.schedule.allowed_hosts) {
        return err(StatusCode::BAD_REQUEST, "host_not_allowed");
    }
    load(&st, u).await
}

async fn load(st: &AppState, url: url::Url) -> Response {
    let key = format!("tt:{url}");
    let ttl = Duration::from_secs(st.cfg.schedule.cache_minutes * 60);
    if let Some(v) = st.fetcher.cached(&key, ttl) {
        return Json(v).into_response();
    }
    match st
        .fetcher
        .get_text(
            &url,
            &st.cfg.schedule.allowed_hosts,
            st.cfg.schedule.max_bytes,
        )
        .await
    {
        Ok((final_url, html)) => match parse(&html) {
            Ok(tt) => {
                let v = json!({
                    "url": final_url.to_string(),
                    "fetched_at": crate::util::unix_now(),
                    "timetable": tt,
                });
                st.fetcher.store(key, v.clone());
                Json(v).into_response()
            }
            Err(e) => err(upstream_status(&e), &e),
        },
        Err(e) => err(upstream_status(&e), &e),
    }
}

impl Default for Fetcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------------------
// Tests against the real TU-Varna files (tests/fixtures, fetched 2026-11-12)
// ---------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const WINTER: &str = include_str!("../tests/fixtures/winter_kst_2_1.htm");
    const SUMMER: &str = include_str!("../tests/fixtures/summer_kst_2_1.htm");
    const FORM: &str = include_str!("../tests/fixtures/form_winter.html");

    fn find<'a>(tt: &'a Timetable, day: u8, needle: &str) -> &'a Lesson {
        tt.lessons
            .iter()
            .find(|l| l.day == day && l.raw.contains(needle))
            .unwrap_or_else(|| panic!("no lesson on day {day} containing {needle}"))
    }

    #[test]
    fn winter_meta_and_counts() {
        let tt = parse(WINTER).unwrap();
        assert_eq!(tt.meta.year, "2026/2027");
        assert_eq!(tt.meta.semester, "ЗИМЕН");
        assert_eq!(tt.meta.specialty, "КСТ");
        assert_eq!(tt.meta.group, "1");
        assert_eq!(tt.meta.stream, "V");
        // 20 lesson blocks on the sheet (counted by hand from the grid dump: 4+5+3+4+4).
        assert_eq!(
            tt.lessons.len(),
            20,
            "{:#?}",
            tt.lessons.iter().map(|l| &l.raw).collect::<Vec<_>>()
        );
        assert!(tt.notes.iter().any(|n| n.contains("четна седмица")));
    }

    #[test]
    fn winter_lecture_with_lecturer_and_room_below() {
        let tt = parse(WINTER).unwrap();
        let ok = find(&tt, 1, "л. ОК");
        assert_eq!((ok.start.as_str(), ok.end.as_str()), ("13:15", "15:00"));
        assert_eq!(ok.kind, "lecture");
        assert_eq!(ok.subject, "ОК");
        assert_eq!(ok.lecturer, "гл. ас. д-р Ил. Бойчев");
        assert_eq!(ok.room, "114 M");
    }

    #[test]
    fn winter_subgroups_markers_and_week_rooms() {
        let tt = parse(WINTER).unwrap();
        let up = find(&tt, 1, "1а) УП");
        assert_eq!(up.subgroup, "1а");
        assert_eq!(
            (up.start.as_str(), up.end.as_str()),
            ("11:15", "12:45"),
            "end comes from the 12,45 cell"
        );
        assert_eq!(up.hours, "2ч.");
        assert!(up.room.contains("412Е") && up.room.contains("823М"));
        let web = find(&tt, 1, "1б) л.у.Web");
        assert_eq!(web.subgroup, "1б");
        assert_eq!(web.kind, "lab");
        assert_eq!(web.room, "513ІІЕ");
        let ssp = find(&tt, 3, "ССП");
        assert_eq!(
            ssp.start, "13:30",
            "start comes from the 13,30 cell on the left"
        );
        assert_eq!(ssp.end, "15:00");
    }

    #[test]
    fn winter_parity_and_inline_lecturer() {
        let tt = parse(WINTER).unwrap();
        let web = find(&tt, 2, "л. Web дизайн");
        assert_eq!(web.parity, "odd");
        assert_eq!(web.subject, "Web дизайн");
        assert_eq!(web.lecturer, "доц. Г. Маринова");
        assert_eq!(web.room, "201М");
        let usp = find(&tt, 4, "(четна седмица) л.у. УСП");
        assert_eq!(usp.parity, "even");
        assert_eq!(usp.subject, "УСП");
        assert_eq!(usp.room, "404ТВ");
        let ds = find(&tt, 3, "с.у. ДС");
        assert_eq!(ds.kind, "seminar");
        assert_eq!(
            ds.room, "НУК 316",
            "room sits two rows below, under an empty lecturer row"
        );
        let lds = find(&tt, 3, "л. ДС");
        assert_eq!(lds.lecturer, "доц. Ив. Пенев");
        assert_eq!(lds.room, "212 Е");
    }

    #[test]
    fn summer_markers_electives_and_grey() {
        let tt = parse(SUMMER).unwrap();
        assert_eq!(tt.meta.semester, "ЛЕТЕН");
        let ps = find(&tt, 1, "1б) л.у. ПС");
        assert_eq!(ps.start, "07:30", "7,30 marker on the left");
        let sp = find(&tt, 1, "1б) с.у.+л.у. СП");
        assert_eq!(sp.kind, "seminar_lab");
        assert_eq!(sp.end, "17:30", "17,30 marker on the right");
        let ev = find(&tt, 2, "(четна с.)");
        assert_eq!(ev.parity, "even");
        let elective = tt
            .lessons
            .iter()
            .filter(|l| l.day == 3 && l.raw.starts_with("к.пр. по избор"))
            .count();
        assert_eq!(
            elective, 2,
            "two elective course-project blocks on Wednesday"
        );
        let ds = tt
            .lessons
            .iter()
            .find(|l| l.day == 3 && l.raw.contains("ДС") && l.kind == "project")
            .unwrap();
        assert_eq!(ds.room, "301ТВ");
    }

    #[test]
    fn options_from_real_form() {
        let v = parse_options(FORM);
        let spec = v["spec"].as_array().unwrap();
        assert!(
            spec.len() > 50,
            "about 75 specialties expected, got {}",
            spec.len()
        );
        assert!(
            spec.iter()
                .any(|o| o["value"] == "20" && o["label"] == "КСТ")
        );
        assert_eq!(v["kurs"].as_array().unwrap()[0]["label"], "I");
        assert!(!v["grupa"].as_array().unwrap().is_empty());
    }

    #[test]
    fn non_timetable_is_refused() {
        assert_eq!(
            parse("<html><body><table><tr><td>x</td></tr></table></body></html>").unwrap_err(),
            "not_a_timetable"
        );
    }

    #[test]
    fn helpers() {
        assert_eq!(parse_range("715 - 800"), Some((435, 480)));
        assert_eq!(parse_range("7 15 - 8 00"), Some((435, 480)));
        assert_eq!(parse_range("10:15-11:00"), Some((615, 660)));
        assert_eq!(parse_marker("12,45"), Some(765));
        assert_eq!(parse_marker("2ч."), None);
        assert!(
            is_room("404ТВ")
                && is_room("НУК 316")
                && is_room("114 M")
                && is_room("513II E")
                && is_room("207ІЕ")
        );
        assert!(!is_room("ОК") && !is_room("доц. Живомиров") && !is_room("2026/2027"));
        assert!(is_lecturer("гл. ас. д-р Ил. Бойчев") && is_lecturer("доц. Живомиров"));
        assert!(is_hours("2ч.") && is_hours("4 ч.") && !is_hours("404ТВ"));
        assert!(style_is_grey("background:#BFBFBF;") && !style_is_grey("background:white"));
        assert!(!style_is_grey("background:#FFFF00"));
    }

    #[test]
    fn ssrf_guard() {
        let allowed = vec!["tu-varna.bg".to_string()];
        let ok = |s: &str| host_allowed(&url::Url::parse(s).unwrap(), &allowed);
        assert!(ok("http://umo.tu-varna.bg/components/x.htm"));
        assert!(ok("https://tu-varna.bg/"));
        assert!(!ok("http://tu-varna.bg.evil.com/"), "suffix trick");
        assert!(!ok("http://eviltu-varna.bg/"), "no dot boundary");
        assert!(!ok("http://127.0.0.1/"), "IP literal");
        assert!(!ok("http://umo.tu-varna.bg:8080/"), "odd port");
        assert!(!ok("http://user:pw@umo.tu-varna.bg/"), "credentials in URL");
        assert!(!ok("file:///etc/passwd"));
        assert!(!is_public_ip("10.0.0.1".parse().unwrap()));
        assert!(
            !is_public_ip("169.254.169.254".parse().unwrap()),
            "cloud metadata"
        );
        assert!(!is_public_ip("::ffff:127.0.0.1".parse().unwrap()));
        assert!(!is_public_ip("fd00::1".parse().unwrap()));
        assert!(is_public_ip("195.216.228.7".parse().unwrap()));
    }

    #[test]
    fn decode_falls_back_to_cp1251() {
        let (bytes, _, _) = encoding_rs::WINDOWS_1251.encode("понеделник");
        assert_eq!(
            decode(&bytes, "text/html; charset=windows-1251"),
            "понеделник"
        );
    }
}

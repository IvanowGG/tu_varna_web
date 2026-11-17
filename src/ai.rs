//! The document assistant.
//!
//! With an Anthropic API key in config.toml it streams a draft from Claude (raw HTTP:
//! Rust has no official Anthropic SDK). Without a key it streams a structured template,
//! so the feature still helps offline and on a free host.
//!
//! Spend control: signed-in students only, a per-student hourly quota, a site-wide daily
//! cap, input size limits, and the browser can stop a draft (we then close the upstream).

use crate::AppState;
use crate::session::User;
use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const API: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";

#[derive(Deserialize, Clone)]
pub struct GenReq {
    pub action: String,
    pub kind: String,
    #[serde(default)]
    pub topic: String,
    #[serde(default)]
    pub details: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub length: String,
    #[serde(default)]
    pub tone: String,
    /// The current document, for improve / continue / summarize.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub include_profile: bool,
}

const ACTIONS: [&str; 7] = [
    "write",
    "outline",
    "improve",
    "continue",
    "summarize",
    "questions",
    "translate",
];
const KINDS: [&str; 11] = [
    "essay",
    "report",
    "coursework",
    "lab",
    "summary",
    "email",
    "cv",
    "presentation",
    "letter",
    "notes",
    "other",
];

impl GenReq {
    pub fn validate(&mut self) -> Result<(), &'static str> {
        if !ACTIONS.contains(&self.action.as_str()) {
            return Err("bad_action");
        }
        if !KINDS.contains(&self.kind.as_str()) {
            return Err("bad_kind");
        }
        if !matches!(self.language.as_str(), "bg" | "en") {
            self.language = "bg".into();
        }
        if !matches!(self.length.as_str(), "short" | "medium" | "long") {
            self.length = "medium".into();
        }
        if !matches!(self.tone.as_str(), "academic" | "neutral" | "friendly") {
            self.tone = "academic".into();
        }
        if self.topic.chars().count() > 300
            || self.details.chars().count() > 6000
            || self.text.chars().count() > 60_000
        {
            return Err("too_long");
        }
        let needs_text = matches!(
            self.action.as_str(),
            "improve" | "continue" | "summarize" | "questions" | "translate"
        );
        if needs_text && self.text.trim().is_empty() {
            return Err("no_text");
        }
        if !needs_text && self.topic.trim().is_empty() {
            return Err("no_topic");
        }
        Ok(())
    }
}

/// Site-wide daily counter.
#[derive(Default)]
pub struct DayCounter(Mutex<(u64, u32)>);

impl DayCounter {
    pub fn take(&self, cap: u32) -> bool {
        let today = crate::util::unix_now() / 86400;
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if g.0 != today {
            *g = (today, 0);
        }
        if g.1 >= cap {
            return false;
        }
        g.1 += 1;
        true
    }
}

fn kind_name(kind: &str, bg: bool) -> &'static str {
    match (kind, bg) {
        ("essay", true) => "есе",
        ("essay", false) => "essay",
        ("report", true) => "реферат",
        ("report", false) => "report",
        ("coursework", true) => "курсова работа",
        ("coursework", false) => "coursework",
        ("lab", true) => "протокол от лабораторно упражнение",
        ("lab", false) => "lab report",
        ("summary", true) => "конспект",
        ("summary", false) => "study summary",
        ("email", true) => "официален имейл до преподавател",
        ("email", false) => "formal e-mail to a lecturer",
        ("cv", true) => "автобиография (CV)",
        ("cv", false) => "CV",
        ("presentation", true) => "план на презентация (слайдове)",
        ("presentation", false) => "presentation outline (slides)",
        ("letter", true) => "заявление / молба",
        ("letter", false) => "formal application letter",
        ("notes", true) => "учебни бележки",
        ("notes", false) => "study notes",
        (_, true) => "документ",
        (_, false) => "document",
    }
}

pub fn system_prompt() -> &'static str {
    "You are the writing assistant inside a study hub for students of the Technical University of Varna \
(TU-Varna, Bulgaria). You write complete, ready-to-use academic and practical documents.\n\n\
Output rules:\n\
- Reply with the document only, in Markdown: '#' for the title, '##' and '###' for sections, '-' for \
bullets, '1.' for numbered lists, **bold** for key terms. No preamble, no closing remarks, no code fences \
around the document.\n\
- Write in the requested language. For Bulgarian use correct literary Bulgarian and Bulgarian academic \
conventions (e.g. 'Увод', 'Изложение', 'Заключение', 'Използвана литература').\n\
- Be accurate. Never invent sources, quotes, statistics, page numbers or URLs. Where a citation is \
needed write a clear placeholder such as [източник] / [source] so the student adds a real one.\n\
- Where the student must add their own data (measurements, names, dates) write a placeholder in square \
brackets, e.g. [фак. №] or [измерена стойност].\n\
- Match the requested length: short ≈ 300-500 words, medium ≈ 800-1200 words, long ≈ 1800-2500 words \
(for outlines and e-mails, keep it proportionate).\n\
- Help the student learn: explanations should be clear enough that they understand and can defend the \
text. Do not help with cheating during an exam."
}

pub fn user_prompt(r: &GenReq, user: &User) -> String {
    let bg = r.language == "bg";
    let kind = kind_name(&r.kind, bg);
    let lang = if bg { "Bulgarian" } else { "English" };
    let mut p = String::new();
    let task = match r.action.as_str() {
        "write" => format!("Write a complete {kind} ({}).", r.kind),
        "outline" => format!("Write a detailed outline (headings with 1-2 line notes under each) for a {kind}."),
        "improve" => "Improve the document below: fix grammar and style, make the structure clearer, keep the \
author's meaning and facts. Return the full improved document."
            .to_string(),
        "continue" => "Continue the document below from where it stops, in the same style. Return only the new \
part."
            .to_string(),
        "summarize" => "Write a clear study summary (конспект) of the text below: key ideas, definitions, \
formulas, and a short list of likely exam questions."
            .to_string(),
        "questions" => "Write 10 exam-style self-test questions about the text below, each followed by a short \
model answer."
            .to_string(),
        "translate" => format!("Translate the document below into {lang}, keeping the Markdown structure."),
        _ => String::new(),
    };
    p.push_str(&format!(
        "Task: {task}\nLanguage: {lang}\nLength: {}\nTone: {}\n",
        r.length, r.tone
    ));
    if !r.topic.trim().is_empty() {
        p.push_str(&format!("Topic: {}\n", r.topic.trim()));
    }
    if !r.details.trim().is_empty() {
        p.push_str(&format!(
            "\nThe student's notes and requirements:\n<notes>\n{}\n</notes>\n",
            r.details.trim()
        ));
    }
    if r.include_profile {
        p.push_str(&format!(
            "\nFor title pages / signatures use: name: {}; department/specialty: {}; faculty number: {}; \
university: Технически университет – Варна.\n",
            or_placeholder(&user.name, "[име]"),
            or_placeholder(&user.department, "[специалност]"),
            or_placeholder(&user.employee_id, "[фак. №]")
        ));
    }
    if !r.text.trim().is_empty() {
        p.push_str(&format!(
            "\nThe document:\n<document>\n{}\n</document>\n",
            r.text.trim()
        ));
    }
    p
}

fn or_placeholder(s: &str, ph: &str) -> String {
    if s.trim().is_empty() {
        ph.to_string()
    } else {
        s.trim().to_string()
    }
}

fn supports_fallbacks(model: &str) -> bool {
    [
        "claude-fable-5-2",
        "claude-fable-5-1",
        "claude-mythos-5-2",
        "claude-mythos-5-1",
        "claude-opus-5-5",
        "claude-opus-5",
        "claude-sonnet-5-5",
    ]
    .contains(&model)
}

pub fn request_body(cfg: &crate::config::Ai, r: &GenReq, user: &User) -> Value {
    let mut body = json!({
        "model": cfg.model,
        "max_tokens": cfg.max_output_tokens,
        "stream": true,
        "system": system_prompt(),
        "output_config": {"effort": cfg.effort},
        "messages": [{"role": "user", "content": user_prompt(r, user)}],
    });
    if supports_fallbacks(&cfg.model) {
        // On a safety-classifier decline the API retries on a fallback model in the same call.
        body["fallbacks"] = json!("default");
    }
    body
}

fn sse(event: &str, data: &Value) -> axum::body::Bytes {
    axum::body::Bytes::from(format!("event: {event}\ndata: {data}\n\n"))
}

fn err(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error": code}))).into_response()
}

/// POST /api/ai/generate - streams Server-Sent Events: `status`, `text`, `done`, `error`.
pub async fn generate(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(mut req): Json<GenReq>,
) -> Response {
    let (sid, user) = match crate::require_user(&st, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    if let Err(e) = req.validate() {
        return err(StatusCode::BAD_REQUEST, e);
    }

    let (tx, rx) = mpsc::channel::<axum::body::Bytes>(64);

    if !st.cfg.ai_enabled() {
        // Offline: a structured template, streamed in pieces like a real draft.
        let text = crate::templates::template(&req);
        tokio::spawn(async move {
            let _ = tx.send(sse("status", &json!({"mode": "template"}))).await;
            for chunk in text.split_inclusive('\n').collect::<Vec<_>>().chunks(3) {
                if tx.send(sse("text", &json!(chunk.concat()))).await.is_err() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(12)).await;
            }
            let _ = tx
                .send(sse(
                    "done",
                    &json!({"stop": "end_turn", "mode": "template"}),
                ))
                .await;
        });
        return stream_response(rx);
    }

    // Quotas: per student per hour, and for the whole site per day.
    let cap = st.cfg.ai.per_user_per_hour as usize;
    let allowed = st
        .sessions
        .with(&sid, |s| {
            let hour_ago = Instant::now() - Duration::from_secs(3600);
            while s.ai_calls.front().is_some_and(|t| *t < hour_ago) {
                s.ai_calls.pop_front();
            }
            if s.ai_calls.len() >= cap {
                return false;
            }
            s.ai_calls.push_back(Instant::now());
            true
        })
        .unwrap_or(false);
    if !allowed {
        return err(StatusCode::TOO_MANY_REQUESTS, "hourly_limit");
    }
    if !st.ai_day.take(st.cfg.ai.per_day_total) {
        return err(StatusCode::TOO_MANY_REQUESTS, "daily_limit");
    }

    let body = request_body(&st.cfg.ai, &req, &user);
    let mut rb = st
        .http
        .post(API)
        .header("x-api-key", st.cfg.ai.api_key.trim())
        .header("anthropic-version", API_VERSION)
        .header(header::CONTENT_TYPE, "application/json")
        .timeout(Duration::from_secs(600))
        .json(&body);
    if body.get("fallbacks").is_some() {
        rb = rb.header("anthropic-beta", "server-side-fallback-2026-07-01");
    }

    tokio::spawn(async move {
        let _ = tx.send(sse("status", &json!({"mode": "ai"}))).await;
        let res = match rb.send().await {
            Ok(r) => r,
            Err(_) => {
                let _ = tx
                    .send(sse("error", &json!({"code": "ai_unreachable"})))
                    .await;
                return;
            }
        };
        if !res.status().is_success() {
            let status = res.status().as_u16();
            let detail: Value = res.json().await.unwrap_or(Value::Null);
            eprintln!(
                "[ai] upstream {status}: {}",
                detail["error"]["type"].as_str().unwrap_or("?")
            );
            let code = match status {
                429 | 529 => "ai_busy",
                401 | 403 => "ai_key",
                _ => "ai_error",
            };
            let _ = tx.send(sse("error", &json!({"code": code}))).await;
            return;
        }
        let mut stream = res.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();
        let mut stop = String::new();
        let mut sent_thinking = false;
        while let Some(chunk) = stream.next().await {
            let Ok(chunk) = chunk else {
                let _ = tx
                    .send(sse("error", &json!({"code": "ai_interrupted"})))
                    .await;
                return;
            };
            buf.extend_from_slice(&chunk);
            while let Some(pos) = find_event_end(&buf) {
                let raw: Vec<u8> = buf.drain(..pos).collect();
                let text = String::from_utf8_lossy(&raw);
                for ev in parse_sse_data(&text) {
                    match ev["type"].as_str().unwrap_or_default() {
                        "content_block_start"
                            if ev["content_block"]["type"] == "thinking" && !sent_thinking =>
                        {
                            sent_thinking = true;
                            if tx
                                .send(sse("status", &json!({"mode": "thinking"})))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                        "content_block_delta" if ev["delta"]["type"] == "text_delta" => {
                            if tx.send(sse("text", &ev["delta"]["text"])).await.is_err() {
                                // The student pressed Stop / closed the tab: dropping the
                                // stream closes the upstream request too.
                                return;
                            }
                        }
                        "message_delta" => {
                            if let Some(s) = ev["delta"]["stop_reason"].as_str() {
                                stop = s.to_string();
                            }
                        }
                        "error" => {
                            let code = if ev["error"]["type"] == "overloaded_error" {
                                "ai_busy"
                            } else {
                                "ai_error"
                            };
                            let _ = tx.send(sse("error", &json!({"code": code}))).await;
                            return;
                        }
                        _ => {}
                    }
                }
            }
        }
        if stop == "refusal" {
            let _ = tx.send(sse("error", &json!({"code": "refusal"}))).await;
        } else {
            let _ = tx
                .send(sse("done", &json!({"stop": stop, "mode": "ai"})))
                .await;
        }
    });
    stream_response(rx)
}

/// Index just past the first blank line ("\n\n" or "\r\n\r\n").
pub fn find_event_end(buf: &[u8]) -> Option<usize> {
    for i in 0..buf.len() {
        if buf[i..].starts_with(b"\n\n") {
            return Some(i + 2);
        }
        if buf[i..].starts_with(b"\r\n\r\n") {
            return Some(i + 4);
        }
    }
    None
}

pub fn parse_sse_data(event: &str) -> Vec<Value> {
    event
        .lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .filter_map(|d| serde_json::from_str::<Value>(d.trim()).ok())
        .collect()
}

fn stream_response(rx: mpsc::Receiver<axum::body::Bytes>) -> Response {
    let s = futures_util::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|b| (Ok::<_, Infallible>(b), rx))
    });
    let mut res = Response::new(Body::from_stream(s));
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert("x-accel-buffering", HeaderValue::from_static("no"));
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(action: &str) -> GenReq {
        GenReq {
            action: action.into(),
            kind: "essay".into(),
            topic: "Облачни технологии".into(),
            details: String::new(),
            language: "xx".into(),
            length: "".into(),
            tone: "".into(),
            text: String::new(),
            include_profile: true,
        }
    }

    #[test]
    fn validation() {
        let mut r = req("write");
        assert!(r.validate().is_ok());
        assert_eq!(
            (r.language.as_str(), r.length.as_str(), r.tone.as_str()),
            ("bg", "medium", "academic")
        );
        let mut r = req("improve");
        assert_eq!(r.validate(), Err("no_text"));
        let mut r = req("hack");
        assert_eq!(r.validate(), Err("bad_action"));
        let mut r = req("write");
        r.details = "x".repeat(6001);
        assert_eq!(r.validate(), Err("too_long"));
        let mut r = req("write");
        r.topic = " ".into();
        assert_eq!(r.validate(), Err("no_topic"));
    }

    #[test]
    fn body_shape_matches_current_api() {
        let cfg = crate::config::Ai::default();
        let u = User {
            name: "Иван".into(),
            ..User::default()
        };
        let mut r = req("write");
        r.validate().unwrap();
        let b = request_body(&cfg, &r, &u);
        assert_eq!(b["model"], "claude-opus-5-5");
        assert_eq!(b["stream"], true);
        assert_eq!(b["output_config"]["effort"], "medium");
        assert_eq!(b["fallbacks"], "default");
        assert!(
            b.get("temperature").is_none(),
            "sampling params are rejected by API version 2026-10-21"
        );
        assert!(
            b.get("thinking").is_none(),
            "Opus 5.5 thinks adaptively by default; disabled would be a 400"
        );
        let prompt = b["messages"][0]["content"].as_str().unwrap();
        assert!(prompt.contains("Иван") && prompt.contains("[фак. №]"));
        let mut cfg2 = cfg.clone();
        cfg2.model = "claude-haiku-5-5".into();
        assert!(request_body(&cfg2, &r, &u).get("fallbacks").is_none());
    }

    #[test]
    fn sse_parsing() {
        let raw = b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\nevent: x";
        let end = find_event_end(raw).unwrap();
        let evs = parse_sse_data(std::str::from_utf8(&raw[..end]).unwrap());
        assert_eq!(evs[0]["delta"]["text"], "Hi");
        assert!(find_event_end(b"event: x\ndata: {}").is_none());
        assert_eq!(find_event_end(b"a\r\n\r\nb"), Some(5));
    }

    #[test]
    fn day_cap() {
        let d = DayCounter::default();
        assert!(d.take(2) && d.take(2));
        assert!(!d.take(2));
    }
}

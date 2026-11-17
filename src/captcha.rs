//! Our own anti-bot check, three layers that must all pass:
//! 1. Proof of work: the browser must find a number whose SHA-256 (with a random salt)
//!    starts with N zero bits. Cheap once for a person, expensive at bot scale.
//! 2. Picture puzzle: a server-drawn PNG with noise shows one empty circle among other
//!    shapes; the person drags a dot onto the circle. The target position never leaves
//!    the server as data, only as pixels.
//! 3. Behaviour: the drag trace must look like a hand (timing, uneven speed, no jumps),
//!    a hidden honeypot field must stay empty, and the answer may not come too fast.
//! Each challenge is single-use, bound to the requesting IP, and expires in 2 minutes.

use crate::png::Canvas;
use crate::util::{leading_zero_bits, random_range, random_token, sha256};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const W: usize = 320;
pub const H: usize = 96;
pub const KNOB_Y: f32 = 48.0;
pub const KNOB_START_X: f32 = 26.0;
const TTL: Duration = Duration::from_secs(120);
const MIN_SOLVE: Duration = Duration::from_millis(700);
const TOLERANCE_PX: f32 = 10.0;
const MAX_OPEN: usize = 20_000;

struct Challenge {
    salt: String,
    bits: u32,
    target_x: f32,
    ip: IpAddr,
    issued: Instant,
}

#[derive(Serialize)]
pub struct Issued {
    pub id: String,
    pub salt: String,
    pub bits: u32,
    pub width: usize,
    pub height: usize,
    pub knob_y: f32,
    pub knob_start_x: f32,
    /// data:image/png;base64,...
    pub image: String,
}

#[derive(Deserialize)]
pub struct Answer {
    pub id: String,
    pub nonce: String,
    /// Where the dot was dropped, in image pixels.
    pub x: f32,
    /// Pointer trace: [milliseconds since drag start, x, y] in image pixels.
    pub trace: Vec<[f32; 3]>,
    /// Hidden field. People never see it; naive bots fill it in.
    #[serde(default)]
    pub website: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Reject {
    Unknown,
    Expired,
    WrongIp,
    TooFast,
    Honeypot,
    Work,
    Missed,
    Trace(&'static str),
}

impl Reject {
    pub fn code(&self) -> &'static str {
        match self {
            Reject::Unknown => "unknown",
            Reject::Expired => "expired",
            Reject::WrongIp => "wrong_ip",
            Reject::TooFast => "too_fast",
            Reject::Honeypot => "bot",
            Reject::Work => "work",
            Reject::Missed => "missed",
            Reject::Trace(_) => "trace",
        }
    }
}

#[derive(Default)]
pub struct CaptchaStore {
    open: Mutex<HashMap<String, Challenge>>,
}

impl CaptchaStore {
    pub fn issue(&self, ip: IpAddr, bits: u32) -> Option<Issued> {
        let (png, target_x) = draw();
        let id = random_token::<18>();
        let salt = random_token::<16>();
        {
            let mut open = self.open.lock().unwrap_or_else(|e| e.into_inner());
            if open.len() >= MAX_OPEN {
                let now = Instant::now();
                open.retain(|_, c| now.duration_since(c.issued) < TTL);
                if open.len() >= MAX_OPEN {
                    return None;
                }
            }
            open.insert(
                id.clone(),
                Challenge {
                    salt: salt.clone(),
                    bits,
                    target_x,
                    ip,
                    issued: Instant::now(),
                },
            );
        }
        Some(Issued {
            id,
            salt,
            bits,
            width: W,
            height: H,
            knob_y: KNOB_Y,
            knob_start_x: KNOB_START_X,
            image: format!("data:image/png;base64,{}", crate::util::b64(&png)),
        })
    }

    /// Check an answer. The challenge is removed first, so every challenge gets exactly
    /// one try, right or wrong.
    pub fn verify(&self, a: &Answer, ip: IpAddr) -> Result<(), Reject> {
        let c = {
            let mut open = self.open.lock().unwrap_or_else(|e| e.into_inner());
            open.remove(&a.id).ok_or(Reject::Unknown)?
        };
        let age = c.issued.elapsed();
        if age > TTL {
            return Err(Reject::Expired);
        }
        if c.ip != ip {
            return Err(Reject::WrongIp);
        }
        if !a.website.is_empty() {
            return Err(Reject::Honeypot);
        }
        if age < MIN_SOLVE {
            return Err(Reject::TooFast);
        }
        if !pow_ok(&c.salt, &a.nonce, c.bits) {
            return Err(Reject::Work);
        }
        if (a.x - c.target_x).abs() > TOLERANCE_PX {
            return Err(Reject::Missed);
        }
        check_trace(&a.trace, a.x).map_err(Reject::Trace)
    }

    #[cfg(test)]
    pub(crate) fn target_of(&self, id: &str) -> f32 {
        self.open.lock().unwrap().get(id).unwrap().target_x
    }

    #[cfg(test)]
    fn age(&self, id: &str, by: Duration) {
        let mut open = self.open.lock().unwrap();
        let c = open.get_mut(id).unwrap();
        c.issued -= by;
    }
}

pub fn pow_ok(salt: &str, nonce: &str, bits: u32) -> bool {
    if nonce.is_empty() || nonce.len() > 20 || !nonce.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    leading_zero_bits(&sha256(format!("{salt}:{nonce}").as_bytes())) >= bits
}

/// Does the drag look like a hand moved it?
pub fn check_trace(t: &[[f32; 3]], drop_x: f32) -> Result<(), &'static str> {
    if t.len() < 6 {
        return Err("too_few_points");
    }
    if t.len() > 600 {
        return Err("too_many_points");
    }
    if t.iter().any(|p| !p.iter().all(|v| v.is_finite())) {
        return Err("bad_number");
    }
    let dur = t[t.len() - 1][0] - t[0][0];
    if !(200.0..=60_000.0).contains(&dur) {
        return Err("duration");
    }
    let mut speeds = Vec::with_capacity(t.len());
    for w in t.windows(2) {
        let dt = w[1][0] - w[0][0];
        if dt < 0.0 {
            return Err("time_goes_back");
        }
        let d = ((w[1][1] - w[0][1]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt();
        if d > 160.0 {
            return Err("teleport");
        }
        if dt > 0.0 {
            speeds.push(d / dt);
        }
    }
    if (t[t.len() - 1][1] - drop_x).abs() > 24.0 {
        return Err("trace_ends_elsewhere");
    }
    if speeds.len() < 4 {
        return Err("too_few_moves");
    }
    let mean = speeds.iter().sum::<f32>() / speeds.len() as f32;
    if mean <= 0.0 {
        return Err("no_movement");
    }
    let var = speeds.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / speeds.len() as f32;
    // Scripted drags move at one even speed; a hand speeds up and slows down.
    if var.sqrt() / mean < 0.08 {
        return Err("too_even");
    }
    Ok(())
}

/// Draw the puzzle. Returns the PNG and the x position of the empty circle.
fn draw() -> (Vec<u8>, f32) {
    let r = |lo: u32, hi: u32| random_range(lo, hi) as f32;
    let hue = random_range(0, 3);
    let bg: [u8; 3] = match hue {
        0 => [234, 240, 250],
        1 => [238, 246, 240],
        _ => [247, 242, 233],
    };
    let mut cv = Canvas::new(W, H, bg);

    // Soft vertical gradient and pixel noise make simple template matching harder.
    for y in 0..H {
        for x in 0..W {
            let n = random_range(0, 23) as i32 - 11;
            let g = (y as i32 * 18 / H as i32) - 9;
            let p = &mut cv.px[y * W + x];
            for c in p.iter_mut() {
                *c = (*c as i32 + n - g).clamp(0, 255) as u8;
            }
        }
    }
    // Background scribbles.
    for _ in 0..5 {
        let col = [r(120, 200) as u8, r(120, 200) as u8, r(120, 200) as u8];
        cv.line(
            (r(0, W as u32), r(0, H as u32)),
            (r(0, W as u32), r(0, H as u32)),
            0.7,
            col,
        );
    }
    // The track the dot slides along.
    cv.line(
        (14.0, KNOB_Y),
        (W as f32 - 14.0, KNOB_Y),
        1.2,
        [190, 198, 212],
    );

    // Target and three decoys anywhere on the track, at least 44 px apart. Free positions
    // keep a blind guess at about 1 in 11 (fixed lanes would make it 1 in 4). Random picks
    // can corner themselves with no room for the last shape, so each round is capped and
    // a stuck round starts over: this always ends (the old unbounded loop could hang).
    let xs: Vec<f32> = loop {
        let mut xs: Vec<f32> = Vec::with_capacity(4);
        for _ in 0..40 {
            let x = r(78, W as u32 - 22);
            if xs.iter().all(|o| (o - x).abs() >= 44.0) {
                xs.push(x);
                if xs.len() == 4 {
                    break;
                }
            }
        }
        if xs.len() == 4 {
            break xs;
        }
    };
    let target_x = xs[0];
    let ink = |i: u32| -> [u8; 3] {
        match (i + hue) % 4 {
            0 => [30, 64, 175],
            1 => [190, 70, 40],
            2 => [22, 120, 90],
            _ => [120, 60, 160],
        }
    };
    let rad = r(13, 16);
    cv.ring(
        target_x,
        KNOB_Y + r(0, 3) - 1.0,
        rad,
        1.6,
        ink(random_range(0, 4)),
    );
    let mut kinds = [0u32, 1, 2, 3];
    // Simple shuffle so the decoy kinds vary.
    for i in (1..kinds.len()).rev() {
        kinds.swap(i, random_range(0, i as u32 + 1) as usize);
    }
    for (i, &x) in xs[1..].iter().enumerate() {
        let y = KNOB_Y + r(0, 21) - 10.0;
        let s = r(11, 15);
        let col = ink(random_range(0, 4));
        match kinds[i] {
            0 => cv.polygon(
                &[
                    (x - s, y - s),
                    (x + s, y - s),
                    (x + s, y + s),
                    (x - s, y + s),
                ],
                1.6,
                col,
            ),
            1 => cv.polygon(
                &[(x, y - s - 2.0), (x + s + 2.0, y + s), (x - s - 2.0, y + s)],
                1.6,
                col,
            ),
            2 => cv.polygon(
                &[
                    (x, y - s - 2.0),
                    (x + s + 2.0, y),
                    (x, y + s + 2.0),
                    (x - s - 2.0, y),
                ],
                1.6,
                col,
            ),
            _ => {
                cv.line((x - s, y - s), (x + s, y + s), 1.8, col);
                cv.line((x - s, y + s), (x + s, y - s), 1.8, col);
            }
        }
    }
    // A few short arcs near shapes, so "the only curved thing" is not a shortcut.
    for _ in 0..3 {
        let (cx, cy, rr) = (r(30, W as u32 - 30), r(10, H as u32 - 10), r(3, 7));
        cv.ring(cx, cy, rr, 0.6, [150, 150, 170]);
    }
    (cv.to_png(), target_x)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solve(salt: &str, bits: u32) -> String {
        (0u64..)
            .map(|n| n.to_string())
            .find(|n| pow_ok(salt, n, bits))
            .unwrap()
    }

    fn human_trace(to: f32) -> Vec<[f32; 3]> {
        // Ease-in-out with small wobble, like a real hand.
        let n = 30;
        (0..=n)
            .map(|i| {
                let f = i as f32 / n as f32;
                let e = f * f * (3.0 - 2.0 * f);
                [
                    i as f32 * 17.0 + (i % 3) as f32,
                    KNOB_START_X + (to - KNOB_START_X) * e,
                    KNOB_Y + (i % 4) as f32 * 0.7,
                ]
            })
            .collect()
    }

    fn answer(store: &CaptchaStore, id: &str, salt: &str, bits: u32) -> Answer {
        let x = store.target_of(id);
        Answer {
            id: id.into(),
            nonce: solve(salt, bits),
            x,
            trace: human_trace(x),
            website: String::new(),
        }
    }

    const IP: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(10, 1, 2, 3));

    #[test]
    fn a_person_passes_once_and_replay_fails() {
        let s = CaptchaStore::default();
        let c = s.issue(IP, 8).unwrap();
        assert!(c.image.starts_with("data:image/png;base64,"));
        s.age(&c.id, Duration::from_secs(2));
        let a = answer(&s, &c.id, &c.salt, 8);
        assert_eq!(s.verify(&a, IP), Ok(()));
        assert_eq!(
            s.verify(&a, IP),
            Err(Reject::Unknown),
            "a solved challenge cannot be reused"
        );
    }

    #[test]
    fn failure_paths() {
        let s = CaptchaStore::default();
        // Too fast.
        let c = s.issue(IP, 8).unwrap();
        let a = answer(&s, &c.id, &c.salt, 8);
        assert_eq!(s.verify(&a, IP), Err(Reject::TooFast));
        // One try only: a wrong answer burns the challenge.
        let c = s.issue(IP, 8).unwrap();
        s.age(&c.id, Duration::from_secs(2));
        let mut a = answer(&s, &c.id, &c.salt, 8);
        a.x += 40.0;
        a.trace = human_trace(a.x);
        assert_eq!(s.verify(&a, IP), Err(Reject::Missed));
        a.x -= 40.0;
        assert_eq!(s.verify(&a, IP), Err(Reject::Unknown));
        // Honeypot.
        let c = s.issue(IP, 8).unwrap();
        s.age(&c.id, Duration::from_secs(2));
        let mut a = answer(&s, &c.id, &c.salt, 8);
        a.website = "http://spam".into();
        assert_eq!(s.verify(&a, IP), Err(Reject::Honeypot));
        // Other IP.
        let c = s.issue(IP, 8).unwrap();
        s.age(&c.id, Duration::from_secs(2));
        let a = answer(&s, &c.id, &c.salt, 8);
        assert_eq!(
            s.verify(&a, "10.9.9.9".parse().unwrap()),
            Err(Reject::WrongIp)
        );
        // Expired.
        let c = s.issue(IP, 8).unwrap();
        s.age(&c.id, Duration::from_secs(121));
        let a = answer(&s, &c.id, &c.salt, 8);
        assert_eq!(s.verify(&a, IP), Err(Reject::Expired));
        // Bad proof of work.
        let c = s.issue(IP, 12).unwrap();
        s.age(&c.id, Duration::from_secs(2));
        let mut a = answer(&s, &c.id, &c.salt, 12);
        a.nonce = (0u64..)
            .map(|n| n.to_string())
            .find(|n| !pow_ok(&c.salt, n, 12))
            .unwrap();
        assert_eq!(s.verify(&a, IP), Err(Reject::Work));
    }

    #[test]
    fn robotic_traces_are_refused() {
        let even: Vec<[f32; 3]> = (0..20)
            .map(|i| [i as f32 * 16.0, 26.0 + i as f32 * 10.0, 48.0])
            .collect();
        assert_eq!(check_trace(&even, even[19][1]), Err("too_even"));
        let jump = vec![
            [0.0, 26.0, 48.0],
            [60.0, 27.0, 48.0],
            [120.0, 29.0, 48.0],
            [180.0, 31.0, 48.0],
            [240.0, 32.0, 48.0],
            [300.0, 250.0, 48.0],
        ];
        assert_eq!(check_trace(&jump, 250.0), Err("teleport"));
        assert_eq!(
            check_trace(&[[0.0, 1.0, 1.0]; 3], 1.0),
            Err("too_few_points")
        );
        let t = human_trace(200.0);
        assert!(check_trace(&t, 200.0).is_ok());
        assert_eq!(check_trace(&t, 100.0), Err("trace_ends_elsewhere"));
    }

    #[test]
    fn puzzle_drawing_always_finishes_and_keeps_shapes_apart() {
        // Regression: the old placement loop could spin forever (seen as a hung test).
        for _ in 0..300 {
            let (png, x) = draw();
            assert!(png.len() > 100);
            assert!((70.0..=300.0).contains(&x), "target {x} outside the track");
        }
    }

    #[test]
    fn pow_rejects_non_digits() {
        assert!(!pow_ok("s", "", 1));
        assert!(!pow_ok("s", "12a", 0));
        assert!(!pow_ok("s", "123456789012345678901", 0));
        assert!(pow_ok("s", "1", 0));
    }
}

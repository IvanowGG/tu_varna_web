//! TU-Varna Student Hub - one small server: web app, Microsoft sign-in, timetables,
//! document assistant, Word export, captcha and security, all in one binary.

pub mod ai;
pub mod assets;
pub mod auth;
pub mod captcha;
pub mod config;
pub mod docx;
pub mod png;
pub mod schedule;
pub mod security;
pub mod session;
pub mod templates;
pub mod util;

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower_http::compression::CompressionLayer;

pub use config::Config;
use security::{Bucket, ClientIp};
use session::User;

pub struct AppState {
    pub cfg: Config,
    pub sessions: session::SessionStore,
    pub captcha: captcha::CaptchaStore,
    pub limiter: security::RateLimiter,
    /// For Microsoft and Anthropic (fixed, trusted hosts).
    pub http: reqwest::Client,
    /// For timetables (allow-listed hosts, SSRF guard).
    pub fetcher: schedule::Fetcher,
    pub ai_day: ai::DayCounter,
    pub started: Instant,
}

impl AppState {
    pub fn new(cfg: Config) -> Arc<Self> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent(concat!("TUV-Hub/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("http client");
        Arc::new(Self {
            cfg,
            sessions: Default::default(),
            captcha: Default::default(),
            limiter: Default::default(),
            http,
            fetcher: schedule::Fetcher::new(),
            ai_day: Default::default(),
            started: Instant::now(),
        })
    }
}

/// The signed-in student, or a 401 response.
pub fn require_user(st: &AppState, headers: &HeaderMap) -> Result<(String, User), Response> {
    let deny = || (StatusCode::UNAUTHORIZED, Json(json!({"error": "sign_in"}))).into_response();
    let sid = session::cookie_id(headers).ok_or_else(deny)?;
    let user = st
        .sessions
        .with(&sid, |s| s.user.clone())
        .flatten()
        .ok_or_else(deny)?;
    Ok((sid, user))
}

async fn health(State(st): State<Arc<AppState>>) -> Response {
    Json(json!({"ok": true, "uptime_s": st.started.elapsed().as_secs(), "sessions": st.sessions.len()})).into_response()
}

async fn captcha_new(
    State(st): State<Arc<AppState>>,
    axum::Extension(ip): axum::Extension<ClientIp>,
) -> Response {
    if !st.limiter.allow(ip.0, Bucket::Captcha) {
        return security::too_many();
    }
    match st.captcha.issue(ip.0, st.cfg.captcha.difficulty_bits) {
        Some(c) => {
            let mut r = Json(c).into_response();
            r.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            r
        }
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "busy"})),
        )
            .into_response(),
    }
}

async fn captcha_verify(
    State(st): State<Arc<AppState>>,
    axum::Extension(ip): axum::Extension<ClientIp>,
    headers: HeaderMap,
    Json(a): Json<captcha::Answer>,
) -> Response {
    if !st.limiter.allow(ip.0, Bucket::Captcha) {
        return security::too_many();
    }
    if let Err(e) = st.captcha.verify(&a, ip.0) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "captcha", "reason": e.code()})),
        )
            .into_response();
    }
    // Mark the session as "a person" for 10 minutes (enough to finish signing in).
    let until = Instant::now() + Duration::from_secs(600);
    let existing = session::cookie_id(&headers).filter(|sid| {
        st.sessions
            .with(sid, |s| s.human_until = Some(until))
            .is_some()
    });
    let mut res = Json(json!({"ok": true})).into_response();
    if existing.is_none() {
        let Some(sid) = st.sessions.create() else {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "busy"})),
            )
                .into_response();
        };
        st.sessions.with(&sid, |s| s.human_until = Some(until));
        res.headers_mut().insert(
            header::SET_COOKIE,
            session::set_cookie(&sid, st.cfg.is_https()),
        );
    }
    res
}

pub fn router(st: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/me", get(auth::me))
        .route("/api/captcha", get(captcha_new))
        .route("/api/captcha/verify", post(captcha_verify))
        .route("/auth/login", get(auth::login))
        .route("/auth/callback", get(auth::callback))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/demo", post(auth::demo))
        .route("/api/ms/calendar", get(auth::calendar))
        .route("/api/schedule/options", post(schedule::options))
        .route("/api/schedule/resolve", post(schedule::resolve))
        .route("/api/schedule/fetch", post(schedule::fetch))
        .route("/api/ai/generate", post(ai::generate))
        .route("/api/export/docx", post(docx::export))
        .fallback(assets::serve)
        .layer(DefaultBodyLimit::max(512 * 1024))
        .layer(CompressionLayer::new())
        .layer(from_fn_with_state(st.clone(), security::origin_guard))
        .layer(from_fn_with_state(st.clone(), security::general_limit))
        .layer(from_fn_with_state(st.clone(), security::security_headers))
        .layer(from_fn_with_state(st.clone(), security::client_ip_layer))
        .with_state(st)
}

/// Background housekeeping: drop expired sessions every 10 minutes.
pub fn spawn_sweeper(st: Arc<AppState>) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(600));
        loop {
            tick.tick().await;
            st.sessions.sweep();
        }
    });
}

#[cfg(test)]
mod http_tests {
    //! End-to-end: the real router on a real socket, driven like a browser would.
    use super::*;
    use std::net::SocketAddr;

    async fn start() -> (String, Arc<AppState>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let mut cfg = Config::default();
        cfg.bind = addr.to_string();
        cfg.public_url = format!("http://{addr}");
        cfg.captcha.difficulty_bits = 10;
        let st = AppState::new(cfg);
        let app = router(st.clone());
        tokio::spawn(async move {
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .unwrap();
        });
        (format!("http://{addr}"), st)
    }

    fn cookie_from(r: &reqwest::Response) -> Option<String> {
        r.headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .find(|v| v.starts_with("tuv_sid=") && !v.starts_with("tuv_sid=;"))
            .map(|v| v.split(';').next().unwrap().to_string())
    }

    #[tokio::test]
    async fn page_and_headers() {
        let (base, _) = start().await;
        let c = reqwest::Client::new();
        let r = c.get(&base).send().await.unwrap();
        assert_eq!(r.status(), 200);
        let h = r.headers().clone();
        assert_eq!(h["content-security-policy"], security::CSP);
        assert_eq!(h["x-content-type-options"], "nosniff");
        assert_eq!(h["x-frame-options"], "DENY");
        assert!(
            h.get("strict-transport-security").is_none(),
            "no HSTS on plain http"
        );
        let body = r.text().await.unwrap();
        assert!(body.contains("data-theme=\"auto\""));
        let r = c
            .get(&base)
            .header("cookie", "tuv_prefs=dark.en")
            .send()
            .await
            .unwrap();
        let body = r.text().await.unwrap();
        assert!(body.contains("data-theme=\"dark\"") && body.contains("<html lang=\"en\""));
        let r = c
            .get(&base)
            .header("cookie", "tuv_prefs=\"><script>.x")
            .send()
            .await
            .unwrap();
        assert!(
            r.text().await.unwrap().contains("data-theme=\"auto\""),
            "unknown prefs fall back safely"
        );
        let r = c.get(format!("{base}/api/nope")).send().await.unwrap();
        assert_eq!(r.status(), 404);
        let r = c.get(format!("{base}/../Cargo.toml")).send().await.unwrap();
        assert_eq!(r.status(), 404);
    }

    #[tokio::test]
    async fn protected_endpoints_refuse_strangers() {
        let (base, _) = start().await;
        let c = reqwest::Client::new();
        // No Origin header → cross-site guard.
        let r = c
            .post(format!("{base}/api/schedule/fetch"))
            .json(&json!({"url": "http://umo.tu-varna.bg/"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 403);
        // Foreign Origin → refused.
        let r = c
            .post(format!("{base}/api/ai/generate"))
            .header("origin", "https://evil.example")
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 403);
        // Same origin but not signed in → 401.
        let r = c
            .post(format!("{base}/api/schedule/fetch"))
            .header("origin", &base)
            .json(&json!({"url": "http://umo.tu-varna.bg/"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 401);
        // Demo sign-in without a solved captcha → refused.
        let r = c
            .post(format!("{base}/auth/demo"))
            .header("origin", &base)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 403);
        // Microsoft sign-in without captcha → bounced home with a reason.
        let r = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
            .get(format!("{base}/auth/login"))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 303);
        assert!(
            r.headers()["location"]
                .to_str()
                .unwrap()
                .contains("auth_error=")
        );
        // A wrong captcha answer is refused and burns the challenge.
        let ch: serde_json::Value = c
            .get(format!("{base}/api/captcha"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let r = c
            .post(format!("{base}/api/captcha/verify"))
            .header("origin", &base)
            .json(&json!({"id": ch["id"], "nonce": "1", "x": 5.0, "trace": []}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 400);
    }

    #[tokio::test]
    async fn full_student_journey() {
        let (base, st) = start().await;
        let c = reqwest::Client::new();
        let me: serde_json::Value = c
            .get(format!("{base}/api/me"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(me["signed_in"], false);
        assert_eq!(me["auth"]["demo"], true);

        // 1. Captcha, solved like the browser does.
        let ch: serde_json::Value = c
            .get(format!("{base}/api/captcha"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let id = ch["id"].as_str().unwrap();
        let salt = ch["salt"].as_str().unwrap();
        let target = st.captcha.target_of(id);
        let nonce = (0u64..)
            .map(|n| n.to_string())
            .find(|n| captcha::pow_ok(salt, n, 10))
            .unwrap();
        let trace: Vec<[f32; 3]> = (0..=30)
            .map(|i| {
                let f = i as f32 / 30.0;
                let e = f * f * (3.0 - 2.0 * f);
                [
                    i as f32 * 18.0,
                    captcha::KNOB_START_X + (target - captcha::KNOB_START_X) * e,
                    48.0 + (i % 3) as f32,
                ]
            })
            .collect();
        tokio::time::sleep(Duration::from_millis(800)).await;
        let r = c
            .post(format!("{base}/api/captcha/verify"))
            .header("origin", &base)
            .json(&json!({"id": id, "nonce": nonce, "x": target, "trace": trace, "website": ""}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let pre = cookie_from(&r).expect("captcha gives a session cookie");

        // 2. Demo sign-in rotates the session id.
        let r = c
            .post(format!("{base}/auth/demo"))
            .header("origin", &base)
            .header("cookie", &pre)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let sid = cookie_from(&r).unwrap();
        assert_ne!(sid, pre, "session id must change at sign-in");
        let r = c
            .get(format!("{base}/api/me"))
            .header("cookie", &pre)
            .send()
            .await
            .unwrap();
        assert_eq!(
            r.json::<serde_json::Value>().await.unwrap()["signed_in"],
            false,
            "old id is dead"
        );
        let me: serde_json::Value = c
            .get(format!("{base}/api/me"))
            .header("cookie", &sid)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(me["signed_in"], true);
        assert_eq!(me["user"]["name"], "Демо Студент");
        assert_eq!(me["storage_key"].as_str().unwrap().len(), 16);

        // 3. Document assistant without an API key → template stream.
        let r = c
            .post(format!("{base}/api/ai/generate"))
            .header("origin", &base)
            .header("cookie", &sid)
            .json(&json!({"action": "write", "kind": "lab", "topic": "Закон на Ом", "language": "bg"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        assert!(
            r.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with("text/event-stream")
        );
        let body = r.text().await.unwrap();
        assert!(
            body.contains("event: text")
                && body.contains("event: done")
                && body.contains("Закон на Ом")
        );

        // 4. Word export.
        let r = c
            .post(format!("{base}/api/export/docx"))
            .header("origin", &base)
            .header("cookie", &sid)
            .json(&json!({"title": "Протокол", "markdown": "# Протокол\n\nТекст"}))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        assert!(
            r.headers()["content-disposition"]
                .to_str()
                .unwrap()
                .contains("filename*=UTF-8''")
        );
        assert_eq!(&r.bytes().await.unwrap()[..2], b"PK");

        // 5. Timetable fetch refuses anything outside tu-varna.bg (SSRF guard).
        for bad in [
            "http://127.0.0.1:1/",
            "http://169.254.169.254/latest",
            "http://tu-varna.bg.evil.com/",
            "file:///c:/windows/win.ini",
        ] {
            let r = c
                .post(format!("{base}/api/schedule/fetch"))
                .header("origin", &base)
                .header("cookie", &sid)
                .json(&json!({"url": bad}))
                .send()
                .await
                .unwrap();
            assert_eq!(r.status(), 400, "{bad} must be refused");
        }

        // 6. Sign out kills the session.
        let r = c
            .post(format!("{base}/auth/logout"))
            .header("origin", &base)
            .header("cookie", &sid)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let me: serde_json::Value = c
            .get(format!("{base}/api/me"))
            .header("cookie", &sid)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(me["signed_in"], false);
    }
}

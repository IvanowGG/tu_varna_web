//! Microsoft (university account) sign-in.
//!
//! OAuth 2.0 authorization code flow with PKCE, run entirely on the server:
//! the browser only ever sees Microsoft's own login page and our opaque session cookie.
//! After sign-in we read the student's profile from Microsoft Graph once, keep the access
//! token in RAM for at most its lifetime (for calendar refresh), and never store it on disk.

use crate::AppState;
use crate::security::{Bucket, ClientIp};
use crate::session::{self, MsToken, PendingAuth, User};
use crate::util::{b64, b64url, b64url_decode, ct_eq, random_token, sha256, sha256_hex, unix_now};
use axum::Json;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};

const GRAPH: &str = "https://graph.microsoft.com/v1.0";
const LOGIN: &str = "https://login.microsoftonline.com";
const PENDING_TTL: Duration = Duration::from_secs(10 * 60);

fn redirect_error(code: &str) -> Response {
    Redirect::to(&format!("/?auth_error={code}")).into_response()
}

fn scopes(st: &AppState) -> String {
    scope_string(&st.cfg.microsoft)
}

/// What the sign-in asks Microsoft for: only the basics (name, e-mail, profile incl.
/// department/speciality via User.Read). Students can approve these themselves, so sign-in
/// never stops at "Need admin approval". Extra permissions only if listed in `extra_scopes`.
/// (Asking for ".default" = every permission on the app blocked the WHOLE sign-in when one
/// of them, e.g. Team.ReadBasic.All, needed TU-Varna IT approval.)
pub(crate) fn scope_string(ms: &crate::config::Microsoft) -> String {
    let mut s = vec![
        "openid".to_string(),
        "profile".into(),
        "email".into(),
        "User.Read".into(),
    ];
    for x in &ms.extra_scopes {
        let x = x.trim();
        if !x.is_empty() && !s.iter().any(|y| y.eq_ignore_ascii_case(x)) {
            s.push(x.to_string());
        }
    }
    s.join(" ")
}

/// GET /auth/login - start sign-in. Needs a passed captcha.
pub async fn login(
    State(st): State<Arc<AppState>>,
    axum::Extension(ip): axum::Extension<ClientIp>,
    headers: HeaderMap,
) -> Response {
    if !st.limiter.allow(ip.0, Bucket::Auth) {
        return redirect_error("rate_limited");
    }
    if !st.cfg.microsoft_enabled() {
        return redirect_error("not_configured");
    }
    let Some(sid) = session::cookie_id(&headers) else {
        return redirect_error("captcha");
    };
    let verifier = random_token::<48>();
    let pending = PendingAuth {
        state: random_token::<24>(),
        verifier: verifier.clone(),
        nonce: random_token::<24>(),
        created: Instant::now(),
    };
    let ok = st
        .sessions
        .with(&sid, |s| {
            if !s.is_human() {
                return false;
            }
            s.pending = Some(pending.clone());
            true
        })
        .unwrap_or(false);
    if !ok {
        return redirect_error("captcha");
    }
    let challenge = b64url(&sha256(verifier.as_bytes()));
    let u = authorize_url(
        &st.cfg.microsoft,
        &st.cfg.redirect_uri(),
        &pending,
        &challenge,
    );
    Redirect::to(u.as_str()).into_response()
}

/// The Microsoft sign-in address.
pub(crate) fn authorize_url(
    ms: &crate::config::Microsoft,
    redirect_uri: &str,
    pending: &PendingAuth,
    challenge: &str,
) -> url::Url {
    let mut u = url::Url::parse(&format!(
        "{LOGIN}/{}/oauth2/v2.0/authorize",
        ms.tenant.trim()
    ))
    .expect("static url");
    {
        let mut q = u.query_pairs_mut();
        q.append_pair("client_id", ms.client_id.trim())
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("response_mode", "query")
            .append_pair("scope", &scope_string(ms))
            .append_pair("state", &pending.state)
            .append_pair("nonce", &pending.nonce)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("prompt", "select_account");
        if let Some(d) = ms.allowed_domains.first() {
            q.append_pair("domain_hint", d);
        }
    }
    u
}

#[derive(Deserialize)]
pub struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Turn Microsoft's error into our code, which the page explains to the student.
pub(crate) fn callback_error(error: &str, description: Option<&str>) -> &'static str {
    let d = description.unwrap_or_default();
    // AADSTS90094 / 900941 / 65001: an administrator must approve the app (user consent off).
    let needs_admin = ["AADSTS90094", "AADSTS900941", "AADSTS65001"]
        .iter()
        .any(|c| d.contains(c));
    match error {
        _ if needs_admin => "consent",
        "consent_required" | "interaction_required" => "consent",
        "access_denied" => "cancelled",
        _ => "microsoft",
    }
}

/// GET /auth/callback - Microsoft sends the student back here.
pub async fn callback(
    State(st): State<Arc<AppState>>,
    axum::Extension(ip): axum::Extension<ClientIp>,
    headers: HeaderMap,
    Query(q): Query<Callback>,
) -> Response {
    if !st.limiter.allow(ip.0, Bucket::Auth) {
        return redirect_error("rate_limited");
    }
    let Some(sid) = session::cookie_id(&headers) else {
        return redirect_error("session");
    };
    // Take the pending sign-in out of the session: one use only.
    let pending = st.sessions.with(&sid, |s| s.pending.take()).flatten();
    let Some(pending) = pending else {
        return redirect_error("session");
    };
    if pending.created.elapsed() > PENDING_TTL {
        return redirect_error("expired");
    }
    if let Some(e) = q.error.as_deref() {
        let code = callback_error(e, q.error_description.as_deref());
        eprintln!("[auth] Microsoft returned {e} -> {code}");
        return redirect_error(code);
    }
    let (Some(code), Some(state)) = (q.code, q.state) else {
        return redirect_error("microsoft");
    };
    if !ct_eq(state.as_bytes(), pending.state.as_bytes()) {
        return redirect_error("state");
    }

    // Exchange the code (with the PKCE verifier) for tokens.
    let tenant = st.cfg.microsoft.tenant.trim().to_string();
    let redirect_uri = st.cfg.redirect_uri();
    let scope = scopes(&st);
    let mut form: Vec<(&str, &str)> = vec![
        ("client_id", st.cfg.microsoft.client_id.trim()),
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", &redirect_uri),
        ("code_verifier", &pending.verifier),
        ("scope", &scope),
    ];
    let secret = st.cfg.microsoft.client_secret.trim().to_string();
    if !secret.is_empty() {
        form.push(("client_secret", &secret));
    }
    let tok = match st
        .http
        .post(format!("{LOGIN}/{tenant}/oauth2/v2.0/token"))
        .form(&form)
        .send()
        .await
    {
        Ok(r) => {
            let ok = r.status().is_success();
            let v: Value = r.json().await.unwrap_or(Value::Null);
            if !ok {
                eprintln!(
                    "[auth] token exchange refused: {}",
                    v["error"].as_str().unwrap_or("?")
                );
                return redirect_error("token");
            }
            v
        }
        Err(_) => return redirect_error("network"),
    };
    let access = tok["access_token"].as_str().unwrap_or_default().to_string();
    let id_token = tok["id_token"].as_str().unwrap_or_default();
    let expires_in = tok["expires_in"].as_u64().unwrap_or(3600).min(24 * 3600);
    let granted: Vec<String> = tok["scope"]
        .as_str()
        .unwrap_or_default()
        .split(' ')
        .map(String::from)
        .collect();
    if access.is_empty() {
        return redirect_error("token");
    }

    // The ID token came straight from Microsoft over TLS (OIDC Core 3.1.3.7 permits TLS
    // server validation instead of a signature check here); we still check every claim.
    let claims = match id_claims(id_token) {
        Some(c) => c,
        None => return redirect_error("id_token"),
    };
    if let Err(e) = check_claims(
        &claims,
        &st.cfg.microsoft.client_id,
        &pending.nonce,
        &st.cfg.microsoft.tenant,
    ) {
        eprintln!("[auth] id_token rejected: {e}");
        return redirect_error("id_token");
    }

    let mut user = match read_profile(&st.http, &access).await {
        Some(u) => u,
        None => return redirect_error("graph"),
    };
    user.tenant_id = claims["tid"].as_str().unwrap_or_default().to_string();
    if !domain_allowed(&user, &st.cfg.microsoft.allowed_domains) {
        return redirect_error("domain");
    }
    user.granted = granted
        .iter()
        .filter(|s| s.contains('.') || s.contains('/'))
        .map(|s| s.rsplit('/').next().unwrap_or(s).to_string())
        .collect();
    user.source = "microsoft".into();

    // New session id after sign-in (stops session fixation).
    let Some(new_sid) = st.sessions.rotate(&sid) else {
        return redirect_error("session");
    };
    st.sessions.with(&new_sid, |s| {
        s.user = Some(user);
        s.ms_token = Some(MsToken {
            access_token: access,
            expires: Instant::now() + Duration::from_secs(expires_in),
        });
        s.human_until = None;
    });
    let mut res = Redirect::to("/#/dashboard").into_response();
    res.headers_mut().insert(
        header::SET_COOKIE,
        session::set_cookie(&new_sid, st.cfg.is_https()),
    );
    res
}

pub fn id_claims(jwt: &str) -> Option<Value> {
    let mut parts = jwt.split('.');
    let (_h, p, _s) = (parts.next()?, parts.next()?, parts.next()?);
    serde_json::from_slice(&b64url_decode(p)?).ok()
}

pub fn check_claims(
    c: &Value,
    client_id: &str,
    nonce: &str,
    tenant: &str,
) -> Result<(), &'static str> {
    if c["aud"].as_str() != Some(client_id.trim()) {
        return Err("aud");
    }
    match c["nonce"].as_str() {
        Some(n) if ct_eq(n.as_bytes(), nonce.as_bytes()) => {}
        _ => return Err("nonce"),
    }
    let now = unix_now();
    if c["exp"].as_u64().unwrap_or(0) + 300 < now {
        return Err("expired");
    }
    let iss = c["iss"].as_str().unwrap_or_default();
    if !iss.starts_with("https://login.microsoftonline.com/") {
        return Err("iss");
    }
    // A tenant GUID in the config pins the university's directory exactly.
    let t = tenant.trim();
    if t.len() == 36
        && t.chars().filter(|ch| *ch == '-').count() == 4
        && c["tid"].as_str() != Some(t)
    {
        return Err("tid");
    }
    Ok(())
}

pub fn domain_allowed(u: &User, allowed: &[String]) -> bool {
    if allowed.is_empty() {
        return true;
    }
    let addr = if u.email.contains('@') {
        &u.email
    } else {
        &u.upn
    };
    let Some(domain) = addr.rsplit('@').next().map(|d| d.to_ascii_lowercase()) else {
        return false;
    };
    allowed.iter().any(|a| {
        let a = a.trim().to_ascii_lowercase();
        domain == a || domain.ends_with(&format!(".{a}"))
    })
}

async fn graph(http: &reqwest::Client, token: &str, path: &str) -> Option<reqwest::Response> {
    let r = http
        .get(format!("{GRAPH}{path}"))
        .bearer_auth(token)
        .send()
        .await
        .ok()?;
    r.status().is_success().then_some(r)
}

async fn read_profile(http: &reqwest::Client, token: &str) -> Option<User> {
    let me: Value = graph(
        http,
        token,
        "/me?$select=id,displayName,givenName,surname,mail,userPrincipalName,jobTitle,department,officeLocation,employeeId,companyName",
    )
    .await?
    .json()
    .await
    .ok()?;
    let s = |k: &str| crate::util::clip(me[k].as_str().unwrap_or_default(), 200);
    let mut u = User {
        id: s("id"),
        name: s("displayName"),
        given_name: s("givenName"),
        email: s("mail"),
        upn: s("userPrincipalName"),
        job_title: s("jobTitle"),
        department: s("department"),
        office: s("officeLocation"),
        employee_id: s("employeeId"),
        company: s("companyName"),
        ..User::default()
    };
    if u.id.is_empty() {
        return None;
    }
    if u.email.is_empty() {
        u.email = u.upn.clone();
    }
    if let Some(r) = graph(http, token, "/me/photos/64x64/$value").await {
        let ct = r
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/jpeg")
            .to_string();
        if let Ok(bytes) = r.bytes().await
            && bytes.len() < 150_000
            && (ct == "image/jpeg" || ct == "image/png")
        {
            u.photo = format!("data:{ct};base64,{}", b64(&bytes));
        }
    }
    Some(u)
}

/// POST /auth/logout
pub async fn logout(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Some(sid) = session::cookie_id(&headers) {
        st.sessions.destroy(&sid);
    }
    let ms = st.cfg.microsoft_enabled().then(|| {
        format!(
            "{LOGIN}/{}/oauth2/v2.0/logout?post_logout_redirect_uri={}",
            st.cfg.microsoft.tenant.trim(),
            url::form_urlencoded::byte_serialize(st.cfg.public_url.as_bytes()).collect::<String>()
        )
    });
    let mut res = Json(json!({"ok": true, "microsoft_logout": ms})).into_response();
    res.headers_mut()
        .insert(header::SET_COOKIE, session::clear_cookie(st.cfg.is_https()));
    res
}

/// POST /auth/demo - only on your own PC with Microsoft sign-in off (see Config::demo_enabled).
pub async fn demo(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if !st.cfg.demo_enabled() {
        return (StatusCode::NOT_FOUND, Json(json!({"error": "not_found"}))).into_response();
    }
    let Some(sid) = session::cookie_id(&headers) else {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "captcha"}))).into_response();
    };
    if !st.sessions.with(&sid, |s| s.is_human()).unwrap_or(false) {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "captcha"}))).into_response();
    }
    let Some(new_sid) = st.sessions.rotate(&sid) else {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "session"}))).into_response();
    };
    st.sessions.with(&new_sid, |s| {
        s.user = Some(User {
            id: "demo-student".into(),
            name: "Демо Студент".into(),
            given_name: "Демо".into(),
            email: "demo.student@tu-varna.bg".into(),
            upn: "demo.student@tu-varna.bg".into(),
            job_title: "Студент".into(),
            department: "ФИТА · КСТ".into(),
            office: "Варна".into(),
            employee_id: "23621000".into(),
            company: "Технически университет – Варна".into(),
            source: "demo".into(),
            ..User::default()
        });
        s.human_until = None;
    });
    let mut res = Json(json!({"ok": true})).into_response();
    res.headers_mut().insert(
        header::SET_COOKIE,
        session::set_cookie(&new_sid, st.cfg.is_https()),
    );
    res
}

/// GET /api/me - who is signed in, plus what the site can do.
pub async fn me(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let user = session::cookie_id(&headers)
        .and_then(|sid| st.sessions.with(&sid, |s| s.user.clone()).flatten());
    let storage_key = user
        .as_ref()
        .map(|u| sha256_hex(format!("tuv-hub:{}", u.id).as_bytes())[..16].to_string());
    let mut res = Json(json!({
        "signed_in": user.is_some(),
        "user": user,
        "storage_key": storage_key,
        "auth": {"microsoft": st.cfg.microsoft_enabled(), "demo": st.cfg.demo_enabled()},
        "ai": {"enabled": st.cfg.ai_enabled(), "model": if st.cfg.ai_enabled() { st.cfg.ai.model.as_str() } else { "" }},
    }))
    .into_response();
    res.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    res
}

#[derive(Deserialize)]
pub struct CalQuery {
    days: Option<u32>,
}

/// GET /api/ms/calendar?days=14 - the student's Outlook/Teams calendar, if granted.
pub async fn calendar(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<CalQuery>,
) -> Response {
    let (sid, _) = match crate::require_user(&st, &headers) {
        Ok(x) => x,
        Err(r) => return r,
    };
    let token = st.sessions.with(&sid, |s| s.ms_token.clone()).flatten();
    let Some(token) = token.filter(|t| t.expires > Instant::now() + Duration::from_secs(30)) else {
        return (
            StatusCode::CONFLICT,
            Json(json!({"error": "sign_in_again"})),
        )
            .into_response();
    };
    let days = q.days.unwrap_or(14).clamp(1, 60) as u64;
    let now = unix_now();
    let fmt = |t: u64| iso_utc(t);
    let path = format!(
        "/me/calendarView?startDateTime={}&endDateTime={}&$select=subject,start,end,location,isOnlineMeeting,onlineMeeting&$orderby=start/dateTime&$top=200",
        fmt(now - 3600),
        fmt(now + days * 86400)
    );
    let r = st
        .http
        .get(format!("{GRAPH}{path}"))
        .bearer_auth(&token.access_token)
        .header("Prefer", "outlook.timezone=\"FLE Standard Time\"")
        .send()
        .await;
    match r {
        Ok(r) if r.status().is_success() => {
            let v: Value = r.json().await.unwrap_or(Value::Null);
            let events: Vec<Value> = v["value"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|e| {
                            json!({
                                "subject": e["subject"],
                                "start": e["start"]["dateTime"],
                                "end": e["end"]["dateTime"],
                                "location": e["location"]["displayName"],
                                "online": e["isOnlineMeeting"],
                                "join_url": e["onlineMeeting"]["joinUrl"],
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Json(json!({"events": events})).into_response()
        }
        Ok(r) if r.status() == StatusCode::FORBIDDEN || r.status() == StatusCode::NOT_FOUND => (
            StatusCode::CONFLICT,
            Json(json!({"error": "no_calendar_access"})),
        )
            .into_response(),
        _ => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "graph_unavailable"})),
        )
            .into_response(),
    }
}

/// Unix seconds → "YYYY-MM-DDTHH:MM:SSZ" (civil-from-days, no time crate needed).
pub fn iso_utc(t: u64) -> String {
    let days = (t / 86400) as i64;
    let secs = t % 86400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_in_address_uses_account_picker_and_university_domain() {
        let ms = crate::config::Microsoft {
            client_id: "app".into(),
            ..Default::default()
        };
        let p = PendingAuth {
            state: "st".into(),
            verifier: "v".into(),
            nonce: "n".into(),
            created: Instant::now(),
        };
        let q: std::collections::HashMap<String, String> =
            authorize_url(&ms, "http://localhost:8080/auth/callback", &p, "c")
                .query_pairs()
                .into_owned()
                .collect();
        assert_eq!(q["prompt"], "select_account");
        assert_eq!(q["domain_hint"], "tu-varna.bg");
        assert_eq!(q["scope"], "openid profile email User.Read");
        assert_eq!(q["state"], "st");
        assert_eq!(q["code_challenge_method"], "S256");
    }

    #[test]
    fn microsoft_errors_map_to_what_the_student_must_do() {
        // "Need admin approval" (user consent off in the tenant) -> IT must approve.
        let admin =
            "AADSTS90094: An administrator of Technical University Varna has set a policy...";
        assert_eq!(callback_error("access_denied", Some(admin)), "consent");
        assert_eq!(
            callback_error(
                "access_denied",
                Some("AADSTS900941: Administrator consent is required.")
            ),
            "consent"
        );
        assert_eq!(callback_error("consent_required", None), "consent");
        assert_eq!(callback_error("interaction_required", None), "consent");
        // Pressed Cancel / declined the permission screen.
        assert_eq!(
            callback_error(
                "access_denied",
                Some("AADSTS65004: User declined to consent")
            ),
            "cancelled"
        );
        assert_eq!(callback_error("access_denied", None), "cancelled");
        assert_eq!(callback_error("server_error", Some("x")), "microsoft");
    }

    #[test]
    fn sign_in_asks_only_for_basics_students_can_approve_themselves() {
        // Regression: ".default" asked for every permission on the app; one needing TU-Varna IT
        // approval (Team.ReadBasic.All) then blocked the whole sign-in.
        let ms = crate::config::Microsoft::default();
        let s = scope_string(&ms);
        assert_eq!(s, "openid profile email User.Read");
        assert!(!s.contains(".default") && !s.contains("Team."));
        // An explicit extra list is added once, without duplicates.
        let ms = crate::config::Microsoft {
            extra_scopes: vec![" Calendars.Read ".into(), "user.read".into(), "".into()],
            ..Default::default()
        };
        assert_eq!(
            scope_string(&ms),
            "openid profile email User.Read Calendars.Read"
        );
    }

    fn jwt(claims: Value) -> String {
        format!("e30.{}.sig", b64url(claims.to_string().as_bytes()))
    }

    #[test]
    fn claims_checked() {
        let tid = "31886941-8a86-4f93-8f42-d140eaea36ad";
        let good = json!({"aud": "app", "nonce": "n1", "exp": unix_now() + 600, "iss": format!("https://login.microsoftonline.com/{tid}/v2.0"), "tid": tid});
        let c = id_claims(&jwt(good.clone())).unwrap();
        assert_eq!(check_claims(&c, "app", "n1", tid), Ok(()));
        assert_eq!(check_claims(&c, "other", "n1", tid), Err("aud"));
        assert_eq!(
            check_claims(&c, "app", "n2", tid),
            Err("nonce"),
            "replayed token from another sign-in"
        );
        let mut wrong_tenant = good.clone();
        wrong_tenant["tid"] = json!("00000000-0000-0000-0000-000000000000");
        assert_eq!(
            check_claims(&wrong_tenant, "app", "n1", tid),
            Err("tid"),
            "account from another university"
        );
        let mut old = good.clone();
        old["exp"] = json!(unix_now() - 3600);
        assert_eq!(check_claims(&old, "app", "n1", tid), Err("expired"));
        let mut iss = good;
        iss["iss"] = json!("https://evil.example/");
        assert_eq!(check_claims(&iss, "app", "n1", tid), Err("iss"));
        assert!(id_claims("garbage").is_none());
    }

    #[test]
    fn domains() {
        let mut u = User {
            email: "ivan@tu-varna.bg".into(),
            ..User::default()
        };
        let allowed = vec!["tu-varna.bg".to_string()];
        assert!(domain_allowed(&u, &allowed));
        u.email = "ivan@students.tu-varna.bg".into();
        assert!(domain_allowed(&u, &allowed));
        u.email = "ivan@gmail.com".into();
        assert!(!domain_allowed(&u, &allowed));
        u.email = "ivan@eviltu-varna.bg".into();
        assert!(!domain_allowed(&u, &allowed));
        u.email = String::new();
        u.upn = "ivan@tu-varna.bg".into();
        assert!(domain_allowed(&u, &allowed));
    }

    #[test]
    fn iso_dates() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_794_441_600), "2026-11-12T00:00:00Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z");
    }
}

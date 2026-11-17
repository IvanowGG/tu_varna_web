//! Security headers, the cross-site request guard, client IP and rate limits.

use crate::AppState;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

/// Content-Security-Policy: no inline script, nothing loaded from other sites.
/// Microsoft sign-in is a top-level navigation, so it needs no CSP entry.
pub const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; \
img-src 'self' data: blob:; font-src 'self'; connect-src 'self'; worker-src 'self'; \
frame-src 'none'; frame-ancestors 'none'; object-src 'none'; base-uri 'none'; \
form-action 'self'; manifest-src 'self'";

/// The client's address, put on every request by [`client_ip_layer`].
#[derive(Clone, Copy, Debug)]
pub struct ClientIp(pub IpAddr);

pub async fn client_ip_layer(
    State(st): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let ip = if st.cfg.trust_proxy {
        forwarded_ip(req.headers()).unwrap_or(peer)
    } else {
        peer
    };
    req.extensions_mut().insert(ClientIp(ip));
    next.run(req).await
}

/// The last X-Forwarded-For entry is the one our own proxy added; earlier ones are
/// whatever the client sent, so they cannot be trusted.
fn forwarded_ip(h: &HeaderMap) -> Option<IpAddr> {
    let v = h.get("x-forwarded-for")?.to_str().ok()?;
    v.rsplit(',').next()?.trim().parse().ok()
}

pub async fn security_headers(
    State(st): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    h.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=(), usb=()"),
    );
    h.insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );
    h.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    if st.cfg.is_https() {
        h.insert(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    res
}

/// Blocks cross-site requests that change state (CSRF). Every POST must come from a
/// page of this site: the browser's Origin header must equal our own origin.
pub async fn origin_guard(State(st): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let m = req.method();
    if m == Method::GET || m == Method::HEAD || m == Method::OPTIONS {
        return next.run(req).await;
    }
    if !same_origin(req.headers(), &st.cfg.origin()) {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({"error": "cross_site"})),
        )
            .into_response();
    }
    next.run(req).await
}

pub fn same_origin(h: &HeaderMap, ours: &str) -> bool {
    if let Some(site) = h.get("sec-fetch-site").and_then(|v| v.to_str().ok())
        && site != "same-origin"
        && site != "none"
    {
        return false;
    }
    match h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        Some(o) => o == ours || loopback_alias(o, ours),
        // All current browsers send Origin on POST; a request without it is not from our page.
        None => false,
    }
}

/// On a local install, `localhost`, `127.0.0.1` and `[::1]` are the same machine, so a page
/// opened at any of them is our own page (same scheme and port required). Only applies when
/// our own address is a loopback one: a real deployment gets no aliases.
fn loopback_alias(theirs: &str, ours: &str) -> bool {
    let (Ok(a), Ok(b)) = (url::Url::parse(theirs), url::Url::parse(ours)) else {
        return false;
    };
    let local = |u: &url::Url| match u.host() {
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    };
    local(&a)
        && local(&b)
        && a.scheme() == b.scheme()
        && a.port_or_known_default() == b.port_or_known_default()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bucket {
    General,
    Captcha,
    Auth,
    Schedule,
    Export,
}

impl Bucket {
    /// Allowed requests per minute per IP.
    fn per_minute(self) -> u32 {
        match self {
            Bucket::General => 240,
            Bucket::Captcha => 20,
            Bucket::Auth => 20,
            Bucket::Schedule => 30,
            Bucket::Export => 30,
        }
    }
}

/// Fixed one-minute windows per (IP, bucket). Small, lock-protected, self-cleaning.
#[derive(Default)]
pub struct RateLimiter {
    map: Mutex<HashMap<(IpAddr, Bucket), (u64, u32)>>,
}

impl RateLimiter {
    pub fn allow(&self, ip: IpAddr, b: Bucket) -> bool {
        self.allow_at(ip, b, crate::util::unix_now() / 60)
    }

    pub fn allow_at(&self, ip: IpAddr, b: Bucket, window: u64) -> bool {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        if map.len() > 100_000 {
            map.retain(|_, (w, _)| *w == window);
        }
        let entry = map.entry((ip, b)).or_insert((window, 0));
        if entry.0 != window {
            *entry = (window, 0);
        }
        if entry.1 >= b.per_minute() {
            return false;
        }
        entry.1 += 1;
        true
    }
}

pub fn too_many() -> Response {
    let mut r = (
        StatusCode::TOO_MANY_REQUESTS,
        axum::Json(serde_json::json!({"error": "rate_limited"})),
    )
        .into_response();
    r.headers_mut()
        .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
    r
}

pub async fn general_limit(State(st): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let ip = req
        .extensions()
        .get::<ClientIp>()
        .map(|c| c.0)
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    if !st.limiter.allow(ip, Bucket::General) {
        return too_many();
    }
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_blocks_after_quota_and_resets_next_minute() {
        let rl = RateLimiter::default();
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        for _ in 0..20 {
            assert!(rl.allow_at(ip, Bucket::Captcha, 5));
        }
        assert!(
            !rl.allow_at(ip, Bucket::Captcha, 5),
            "21st captcha request in one minute must be refused"
        );
        let other: IpAddr = "10.0.0.2".parse().unwrap();
        assert!(
            rl.allow_at(other, Bucket::Captcha, 5),
            "another IP is not affected"
        );
        assert!(
            rl.allow_at(ip, Bucket::Captcha, 6),
            "next minute starts fresh"
        );
    }

    #[test]
    fn origin_check() {
        let mut h = HeaderMap::new();
        assert!(
            !same_origin(&h, "http://localhost:8080"),
            "no Origin header = refused"
        );
        h.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://localhost:8080"),
        );
        assert!(same_origin(&h, "http://localhost:8080"));
        h.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://evil.example"),
        );
        assert!(!same_origin(&h, "http://localhost:8080"));
        h.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://localhost:8080"),
        );
        h.insert("sec-fetch-site", HeaderValue::from_static("cross-site"));
        assert!(!same_origin(&h, "http://localhost:8080"));
    }

    #[test]
    fn local_addresses_are_one_site_but_only_locally() {
        let with = |o: &'static str| {
            let mut h = HeaderMap::new();
            h.insert(header::ORIGIN, HeaderValue::from_static(o));
            h
        };
        // Server says localhost; the student typed 127.0.0.1 or [::1]: same machine, same site.
        assert!(same_origin(
            &with("http://127.0.0.1:8080"),
            "http://localhost:8080"
        ));
        assert!(same_origin(
            &with("http://[::1]:8080"),
            "http://localhost:8080"
        ));
        assert!(same_origin(
            &with("http://localhost:8080"),
            "http://127.0.0.1:8080"
        ));
        // Other port, other scheme or a real host stay refused.
        assert!(!same_origin(
            &with("http://127.0.0.1:9999"),
            "http://localhost:8080"
        ));
        assert!(!same_origin(
            &with("https://127.0.0.1:8080"),
            "http://localhost:8080"
        ));
        assert!(!same_origin(
            &with("http://evil.example:8080"),
            "http://localhost:8080"
        ));
        assert!(!same_origin(
            &with("http://localhost.evil.example:8080"),
            "http://localhost:8080"
        ));
        // A real deployment gets no aliases at all.
        assert!(!same_origin(
            &with("http://127.0.0.1"),
            "https://hub.example.bg"
        ));
        assert!(!same_origin(
            &with("https://127.0.0.1"),
            "https://hub.example.bg"
        ));
    }

    #[test]
    fn forwarded_uses_last_hop() {
        let mut h = HeaderMap::new();
        h.insert(
            "x-forwarded-for",
            HeaderValue::from_static("1.1.1.1, 9.9.9.9"),
        );
        assert_eq!(forwarded_ip(&h), Some("9.9.9.9".parse().unwrap()));
    }
}

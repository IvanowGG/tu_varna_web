//! Server-side sessions kept in RAM. The browser holds only a random id in an HttpOnly
//! cookie; Microsoft tokens never leave the server and are never written to disk.

use crate::util::{random_token, sha256_hex};
use axum::http::{HeaderMap, HeaderValue, header};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const COOKIE: &str = "tuv_sid";
const IDLE: Duration = Duration::from_secs(12 * 3600);
const ABSOLUTE: Duration = Duration::from_secs(3 * 24 * 3600);
const MAX_SESSIONS: usize = 50_000;

#[derive(Clone, Debug, Serialize, Default)]
pub struct User {
    pub id: String,
    pub name: String,
    pub given_name: String,
    pub email: String,
    pub upn: String,
    pub job_title: String,
    pub department: String,
    pub office: String,
    pub employee_id: String,
    pub company: String,
    pub tenant_id: String,
    /// Small profile photo as a data: URL, or empty.
    pub photo: String,
    /// "microsoft" or "demo".
    pub source: String,
    /// Extra Graph permissions the student granted at sign-in.
    pub granted: Vec<String>,
}

#[derive(Clone)]
pub struct MsToken {
    pub access_token: String,
    pub expires: Instant,
}

#[derive(Clone)]
pub struct PendingAuth {
    pub state: String,
    pub verifier: String,
    pub nonce: String,
    pub created: Instant,
}

#[derive(Clone)]
pub struct Session {
    pub created: Instant,
    pub last_seen: Instant,
    pub user: Option<User>,
    pub ms_token: Option<MsToken>,
    pub human_until: Option<Instant>,
    pub pending: Option<PendingAuth>,
    pub ai_calls: VecDeque<Instant>,
}

impl Session {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            created: now,
            last_seen: now,
            user: None,
            ms_token: None,
            human_until: None,
            pending: None,
            ai_calls: VecDeque::new(),
        }
    }

    pub fn is_human(&self) -> bool {
        self.human_until.is_some_and(|t| t > Instant::now())
    }
}

/// Sessions are stored under the SHA-256 of the cookie value, so even a memory dump
/// of the server does not reveal usable cookies.
#[derive(Default)]
pub struct SessionStore {
    map: Mutex<HashMap<String, Session>>,
}

impl SessionStore {
    fn key(id: &str) -> String {
        sha256_hex(id.as_bytes())
    }

    /// Create a fresh session and return its cookie value, or None when full.
    pub fn create(&self) -> Option<String> {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        if map.len() >= MAX_SESSIONS {
            Self::sweep_locked(&mut map);
            if map.len() >= MAX_SESSIONS {
                return None;
            }
        }
        let id = random_token::<32>();
        map.insert(Self::key(&id), Session::new());
        Some(id)
    }

    /// Run `f` on a live session; updates last-seen. Returns None if the id is unknown/expired.
    pub fn with<R>(&self, id: &str, f: impl FnOnce(&mut Session) -> R) -> Option<R> {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let k = Self::key(id);
        let now = Instant::now();
        let alive = {
            let s = map.get(&k)?;
            now.duration_since(s.last_seen) < IDLE && now.duration_since(s.created) < ABSOLUTE
        };
        if !alive {
            map.remove(&k);
            return None;
        }
        let s = map.get_mut(&k)?;
        s.last_seen = now;
        Some(f(s))
    }

    /// New id for the same session (after sign-in) so a cookie planted before sign-in is useless.
    pub fn rotate(&self, id: &str) -> Option<String> {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let s = map.remove(&Self::key(id))?;
        let new_id = random_token::<32>();
        map.insert(Self::key(&new_id), s);
        Some(new_id)
    }

    pub fn destroy(&self, id: &str) {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(&Self::key(id));
    }

    pub fn sweep(&self) -> usize {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        Self::sweep_locked(&mut map);
        map.len()
    }

    fn sweep_locked(map: &mut HashMap<String, Session>) {
        let now = Instant::now();
        map.retain(|_, s| {
            now.duration_since(s.last_seen) < IDLE && now.duration_since(s.created) < ABSOLUTE
        });
    }

    pub fn len(&self) -> usize {
        self.map.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

/// Read the session id from the Cookie header.
pub fn cookie_id(h: &HeaderMap) -> Option<String> {
    for v in h.get_all(header::COOKIE) {
        let Ok(s) = v.to_str() else { continue };
        for part in s.split(';') {
            let part = part.trim();
            if let Some(val) = part.strip_prefix(COOKIE).and_then(|r| r.strip_prefix('='))
                && !val.is_empty()
                && val.len() <= 64
            {
                return Some(val.to_string());
            }
        }
    }
    None
}

pub fn set_cookie(id: &str, secure: bool) -> HeaderValue {
    let sec = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE}={id}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{sec}",
        ABSOLUTE.as_secs()
    ))
    .expect("cookie value is ascii")
}

pub fn clear_cookie(secure: bool) -> HeaderValue {
    let sec = if secure { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{sec}"
    ))
    .expect("cookie value is ascii")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_rotate_destroy() {
        let st = SessionStore::default();
        let id = st.create().unwrap();
        assert_eq!(id.len(), 43);
        st.with(&id, |s| {
            s.human_until = Some(Instant::now() + Duration::from_secs(60))
        })
        .unwrap();
        let new_id = st.rotate(&id).unwrap();
        assert!(
            st.with(&id, |_| ()).is_none(),
            "old id must stop working after rotation"
        );
        assert!(
            st.with(&new_id, |s| s.is_human()).unwrap(),
            "state carries over"
        );
        st.destroy(&new_id);
        assert!(st.with(&new_id, |_| ()).is_none());
    }

    #[test]
    fn cookie_parsing_ignores_lookalikes() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("tuv_sidx=bad; tuv_prefs=a; tuv_sid=good123"),
        );
        assert_eq!(cookie_id(&h).as_deref(), Some("good123"));
    }

    #[test]
    fn cookie_flags() {
        let v = set_cookie("abc", true);
        let s = v.to_str().unwrap();
        assert!(s.contains("HttpOnly") && s.contains("SameSite=Lax") && s.contains("Secure"));
    }
}

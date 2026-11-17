//! Settings, read from one TOML file. Secrets live only in that file (never in git).

use serde::Deserialize;
use std::net::SocketAddr;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Address to listen on, e.g. "127.0.0.1:8080" or "0.0.0.0:8080".
    pub bind: String,
    /// The address students open in the browser, e.g. "https://hub.example.com".
    /// Used for the Microsoft redirect, cookie security and the Origin check.
    pub public_url: String,
    /// Set true only when running behind a reverse proxy you control (nginx, Caddy,
    /// Cloudflare). Then the client IP is read from the last X-Forwarded-For entry.
    pub trust_proxy: bool,
    pub microsoft: Microsoft,
    pub schedule: Schedule,
    pub ai: Ai,
    pub captcha: Captcha,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Microsoft {
    /// Application (client) ID from the Entra app registration. Empty = sign-in off.
    pub client_id: String,
    /// Client secret ("Web" platform). Leave empty for a public client
    /// ("Mobile and desktop" platform + "Allow public client flows").
    pub client_secret: String,
    /// Tenant: the university domain, its tenant GUID, or "organizations".
    pub tenant: String,
    /// Only accounts whose e-mail domain is in this list may sign in. Empty = any.
    pub allowed_domains: Vec<String>,
    /// Extra Microsoft Graph permissions beyond User.Read (default: none). Each one is asked
    /// for at every sign-in; one that needs admin approval blocks sign-in for everybody.
    pub extra_scopes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Schedule {
    /// Where TU-Varna publishes bachelor timetables (the group form lives here).
    pub umo_base: String,
    /// Hosts the server may fetch schedules from (each also allows its subdomains).
    pub allowed_hosts: Vec<String>,
    pub cache_minutes: u64,
    pub max_bytes: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Ai {
    /// Anthropic API key. Empty = the assistant works from built-in templates only.
    pub api_key: String,
    pub model: String,
    /// low | medium | high | xhigh | max
    pub effort: String,
    pub max_output_tokens: u32,
    /// Requests one signed-in student may make per hour.
    pub per_user_per_hour: u32,
    /// Requests the whole site may make per day (spend cap).
    pub per_day_total: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Captcha {
    /// Proof-of-work difficulty in bits. 16 ≈ 0.2 s on a laptop, 18 ≈ 1 s, 20 ≈ 4 s.
    pub difficulty_bits: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".into(),
            public_url: "http://localhost:8080".into(),
            trust_proxy: false,
            microsoft: Microsoft::default(),
            schedule: Schedule::default(),
            ai: Ai::default(),
            captcha: Captcha::default(),
        }
    }
}

impl Default for Microsoft {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            client_secret: String::new(),
            // TU-Varna's Entra tenant (from login.microsoftonline.com/tu-varna.bg discovery).
            tenant: "31886941-8a86-4f93-8f42-d140eaea36ad".into(),
            allowed_domains: vec!["tu-varna.bg".into()],
            extra_scopes: vec![],
        }
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            umo_base: "http://umo.tu-varna.bg".into(),
            allowed_hosts: vec!["tu-varna.bg".into()],
            cache_minutes: 15,
            max_bytes: 4_000_000,
        }
    }
}

impl Default for Ai {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "claude-opus-5-5".into(),
            effort: "medium".into(),
            max_output_tokens: 16000,
            per_user_per_hour: 20,
            per_day_total: 400,
        }
    }
}

impl Default for Captcha {
    fn default() -> Self {
        Self {
            difficulty_bits: 17,
        }
    }
}

impl Config {
    /// Read the file. A missing file gives the defaults (local trial mode).
    pub fn load(path: &str) -> Result<Config, String> {
        let cfg = match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str::<Config>(&text).map_err(|e| format!("{path}: {e}"))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("[config] {path} not found - using defaults (local trial mode)");
                Config::default()
            }
            Err(e) => return Err(format!("{path}: {e}")),
        };
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.bind
            .parse::<SocketAddr>()
            .map_err(|_| format!("bind '{}' is not ip:port", self.bind))?;
        let u =
            url::Url::parse(&self.public_url).map_err(|_| "public_url is not a URL".to_string())?;
        if u.scheme() != "http" && u.scheme() != "https" {
            return Err("public_url must start with http:// or https://".into());
        }
        if !matches!(
            self.ai.effort.as_str(),
            "low" | "medium" | "high" | "xhigh" | "max"
        ) {
            return Err("ai.effort must be low, medium, high, xhigh or max".into());
        }
        if !(8..=26).contains(&self.captcha.difficulty_bits) {
            return Err("captcha.difficulty_bits must be between 8 and 26".into());
        }
        if self.schedule.allowed_hosts.is_empty() {
            return Err("schedule.allowed_hosts must list at least one host".into());
        }
        let umo = url::Url::parse(&self.schedule.umo_base)
            .map_err(|_| "schedule.umo_base is not a URL".to_string())?;
        if !crate::schedule::host_allowed(&umo, &self.schedule.allowed_hosts) {
            return Err("schedule.umo_base must be on one of schedule.allowed_hosts".into());
        }
        if self.microsoft_enabled() && self.microsoft.tenant.trim().is_empty() {
            return Err("microsoft.tenant is empty".into());
        }
        Ok(())
    }

    /// "https://host[:port]" with no trailing slash.
    pub fn origin(&self) -> String {
        match url::Url::parse(&self.public_url) {
            Ok(u) => u.origin().ascii_serialization(),
            Err(_) => self.public_url.trim_end_matches('/').to_string(),
        }
    }

    pub fn is_https(&self) -> bool {
        self.public_url.starts_with("https://")
    }

    pub fn microsoft_enabled(&self) -> bool {
        !self.microsoft.client_id.trim().is_empty()
    }

    /// The demo sign-in exists only for trying the site on your own PC: Microsoft sign-in
    /// must be off AND the server must listen on a loopback address only.
    pub fn demo_enabled(&self) -> bool {
        !self.microsoft_enabled()
            && self
                .bind
                .parse::<SocketAddr>()
                .map(|a| a.ip().is_loopback())
                .unwrap_or(false)
    }

    pub fn ai_enabled(&self) -> bool {
        !self.ai.api_key.trim().is_empty()
    }

    pub fn redirect_uri(&self) -> String {
        format!("{}/auth/callback", self.public_url.trim_end_matches('/'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_loads_and_matches_defaults() {
        // config.example.toml is what the owner copies; unknown keys are refused, so it must parse.
        let c: Config = toml::from_str(include_str!("../config.example.toml")).unwrap();
        c.validate().unwrap();
        let d = Config::default();
        assert_eq!((&c.bind, &c.public_url), (&d.bind, &d.public_url));
        assert_eq!(c.microsoft.tenant, d.microsoft.tenant);
        assert_eq!(c.ai.model, d.ai.model);
        assert_eq!(c.captcha.difficulty_bits, d.captcha.difficulty_bits);
        assert!(!c.microsoft_enabled() && !c.ai_enabled());
    }

    #[test]
    fn defaults_are_valid_and_demo_only_on_loopback() {
        let c = Config::default();
        c.validate().unwrap();
        assert!(c.demo_enabled());
        let mut open = c.clone();
        open.bind = "0.0.0.0:8080".into();
        assert!(
            !open.demo_enabled(),
            "demo sign-in must never be reachable from the network"
        );
        let mut ms = c.clone();
        ms.microsoft.client_id = "x".into();
        assert!(
            !ms.demo_enabled(),
            "demo sign-in must be off once Microsoft is configured"
        );
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let err = toml::from_str::<Config>("bindd = \"1\"").unwrap_err();
        assert!(err.to_string().contains("bindd"));
    }

    #[test]
    fn origin_strips_path() {
        let c = Config {
            public_url: "https://hub.example.com/".into(),
            ..Config::default()
        };
        assert_eq!(c.origin(), "https://hub.example.com");
        assert!(c.is_https());
    }
}

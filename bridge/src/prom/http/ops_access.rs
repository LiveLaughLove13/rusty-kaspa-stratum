//! Optional hardening for `/api/config` (bearer token, CSRF header, localhost-only, POST rate limit).
//!
//! All checks are **opt-in via environment variables** so default behavior stays unchanged.
//!
//! | Variable | Effect |
//! |----------|--------|
//! | `RKSTRATUM_OPS_BEARER_TOKEN` | If set, `GET`/`POST /api/config` require `Authorization: Bearer <exact token>`. |
//! | `RKSTRATUM_HTTP_CSRF_SECRET` | If set, `POST /api/config` requires `X-Rkstratum-Csrf: <exact secret>`. |
//! | `RKSTRATUM_HTTP_LOCALHOST_CONFIG_ONLY=1` | Only loopback clients may call `/api/config` (GET or POST). |
//! | `RKSTRATUM_HTTP_POST_CONFIG_RATE_PER_MIN` | If set to a positive integer, caps `POST /api/config` per source IP per sliding 60s window. |
//!
//! **TLS:** Terminate TLS in front of the bridge (e.g. nginx, Caddy, cloud LB); this stack serves plain HTTP by design.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

static OPS_BEARER: LazyLock<Option<String>> =
    LazyLock::new(|| std::env::var("RKSTRATUM_OPS_BEARER_TOKEN").ok().filter(|s| !s.is_empty()));

static OPS_CSRF: LazyLock<Option<String>> =
    LazyLock::new(|| std::env::var("RKSTRATUM_HTTP_CSRF_SECRET").ok().filter(|s| !s.is_empty()));

static LOCALHOST_ONLY: LazyLock<bool> =
    LazyLock::new(|| matches!(std::env::var("RKSTRATUM_HTTP_LOCALHOST_CONFIG_ONLY").as_deref(), Ok("1") | Ok("true")));

static RATE_PER_MIN: LazyLock<Option<u32>> =
    LazyLock::new(|| std::env::var("RKSTRATUM_HTTP_POST_CONFIG_RATE_PER_MIN").ok().and_then(|s| s.parse().ok()).filter(|&n| n > 0));

static POST_CONFIG_HITS: LazyLock<Mutex<HashMap<String, Vec<Instant>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn header_value<'a>(request: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{}:", name);
    for line in request.lines() {
        let line = line.trim_end_matches('\r');
        if line.len() >= prefix.len() && line[..prefix.len()].eq_ignore_ascii_case(prefix.as_str()) {
            return Some(line[prefix.len()..].trim_start());
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigRouteDeny {
    Unauthorized,
    ForbiddenLocalhost,
    ForbiddenCsrf,
    RateLimited,
}

impl ConfigRouteDeny {
    pub(crate) fn json_body(self) -> &'static str {
        match self {
            Self::Unauthorized => {
                r#"{"success":false,"message":"Missing or invalid Authorization bearer (set RKSTRATUM_OPS_BEARER_TOKEN on server and send Authorization: Bearer <token>)."}"#
            }
            Self::ForbiddenLocalhost => {
                r#"{"success":false,"message":"Config API is restricted to localhost (RKSTRATUM_HTTP_LOCALHOST_CONFIG_ONLY)."}"#
            }
            Self::ForbiddenCsrf => {
                r#"{"success":false,"message":"Missing or invalid X-Rkstratum-Csrf (set RKSTRATUM_HTTP_CSRF_SECRET on server)."}"#
            }
            Self::RateLimited => r#"{"success":false,"message":"POST /api/config rate limit exceeded."}"#,
        }
    }

    pub(crate) fn status_code(self) -> u16 {
        match self {
            Self::RateLimited => 429,
            Self::Unauthorized => 401,
            Self::ForbiddenLocalhost | Self::ForbiddenCsrf => 403,
        }
    }
}

/// `is_post` distinguishes `POST /api/config` (CSRF + rate limit) from `GET /api/config`.
pub(crate) fn check_config_route_access(request: &str, peer_ip: IpAddr, is_post: bool) -> Result<(), ConfigRouteDeny> {
    if *LOCALHOST_ONLY && !peer_ip.is_loopback() {
        return Err(ConfigRouteDeny::ForbiddenLocalhost);
    }
    if let Some(ref token) = *OPS_BEARER {
        let expected = format!("Bearer {}", token);
        match header_value(request, "Authorization") {
            Some(v) if v == expected.as_str() => {}
            _ => return Err(ConfigRouteDeny::Unauthorized),
        }
    }
    if is_post {
        if let Some(ref secret) = *OPS_CSRF {
            match header_value(request, "X-Rkstratum-Csrf") {
                Some(v) if v == secret.as_str() => {}
                _ => return Err(ConfigRouteDeny::ForbiddenCsrf),
            }
        }
        if let Some(limit) = *RATE_PER_MIN {
            let now = Instant::now();
            let key = peer_ip.to_string();
            let mut map = POST_CONFIG_HITS.lock();
            let v = map.entry(key).or_default();
            v.retain(|t| now.duration_since(*t) < Duration::from_secs(60));
            if v.len() >= limit as usize {
                return Err(ConfigRouteDeny::RateLimited);
            }
            v.push(now);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_value_finds_authorization() {
        let r = "GET /x HTTP/1.1\r\nAuthorization: Bearer abc\r\n\r\n";
        assert_eq!(header_value(r, "Authorization"), Some("Bearer abc"));
    }
}

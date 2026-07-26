use std::fmt;
use std::time::Duration;

use zeroize::Zeroizing;

use crate::QualificationError;

pub const QUALIFICATION_REQUEST_VERSION: u32 = 1;
const MIN_TIMEOUT_MS: u64 = 100;
const MAX_TIMEOUT_MS: u64 = 120_000;
const MAX_ENDPOINT_BYTES: usize = 2_048;
const MAX_TOKEN_BYTES: usize = 4_096;
const MAX_CA_BYTES: usize = 64 * 1024;

pub struct QualificationRequest {
    version: u32,
    endpoint: String,
    access_token: Zeroizing<String>,
    local_ca_der: Option<Zeroizing<Vec<u8>>>,
    timeout: Duration,
}

impl QualificationRequest {
    pub fn new(
        endpoint: impl Into<String>,
        access_token: impl Into<String>,
        local_ca_der: Option<Vec<u8>>,
        timeout_ms: u64,
    ) -> Result<Self, QualificationError> {
        Self::with_version(
            QUALIFICATION_REQUEST_VERSION,
            endpoint,
            access_token,
            local_ca_der,
            timeout_ms,
        )
    }

    pub fn with_version(
        version: u32,
        endpoint: impl Into<String>,
        access_token: impl Into<String>,
        local_ca_der: Option<Vec<u8>>,
        timeout_ms: u64,
    ) -> Result<Self, QualificationError> {
        let endpoint = endpoint.into();
        let access_token = access_token.into();
        if version != QUALIFICATION_REQUEST_VERSION
            || !is_loopback_https_endpoint(&endpoint)
            || access_token.len() < 32
            || access_token.len() > MAX_TOKEN_BYTES
            || !access_token
                .bytes()
                .all(|byte| (0x21..=0x7e).contains(&byte))
            || !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&timeout_ms)
            || local_ca_der.as_ref().is_some_and(|certificate| {
                certificate.is_empty() || certificate.len() > MAX_CA_BYTES
            })
        {
            return Err(QualificationError::invalid_request());
        }
        Ok(Self {
            version,
            endpoint,
            access_token: Zeroizing::new(access_token),
            local_ca_der: local_ca_der.map(Zeroizing::new),
            timeout: Duration::from_millis(timeout_ms),
        })
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub(crate) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) fn access_token_copy(&self) -> String {
        self.access_token.to_string()
    }

    pub(crate) fn local_ca_der_copy(&self) -> Option<Vec<u8>> {
        self.local_ca_der.as_ref().map(|value| value.to_vec())
    }

    pub(crate) fn timeout(&self) -> Duration {
        self.timeout
    }
}

impl fmt::Debug for QualificationRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualificationRequest")
            .field("version", &self.version)
            .field("endpoint", &"[validated loopback https endpoint]")
            .field("access_token", &"[redacted]")
            .field("local_ca_configured", &self.local_ca_der.is_some())
            .field("timeout", &self.timeout)
            .finish()
    }
}

fn is_loopback_https_endpoint(endpoint: &str) -> bool {
    if endpoint.is_empty()
        || endpoint.len() > MAX_ENDPOINT_BYTES
        || endpoint.bytes().any(|byte| byte.is_ascii_whitespace())
        || !endpoint.starts_with("https://")
    {
        return false;
    }
    let remainder = &endpoint["https://".len()..];
    if remainder.contains(['@', '?', '#']) {
        return false;
    }
    let authority = remainder.strip_suffix('/').unwrap_or(remainder);
    if authority.is_empty() || authority.contains('/') {
        return false;
    }
    if authority == "localhost" || authority == "127.0.0.1" || authority == "[::1]" {
        return true;
    }
    if let Some(port) = authority.strip_prefix("localhost:") {
        return valid_port(port);
    }
    if let Some(port) = authority.strip_prefix("127.0.0.1:") {
        return valid_port(port);
    }
    if let Some(port) = authority.strip_prefix("[::1]:") {
        return valid_port(port);
    }
    false
}

fn valid_port(value: &str) -> bool {
    value
        .parse::<u16>()
        .is_ok_and(|port| port != 0 && value.bytes().all(|byte| byte.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token() -> String {
        "q".repeat(48)
    }

    #[test]
    fn accepts_only_explicit_loopback_https_endpoints() {
        for endpoint in [
            "https://localhost",
            "https://localhost:8443",
            "https://127.0.0.1:443/",
            "https://[::1]:9443",
        ] {
            QualificationRequest::new(endpoint, token(), None, 1_000)
                .expect("valid loopback endpoint");
        }

        for endpoint in [
            "http://localhost:8080",
            "https://example.com",
            "https://localhost.evil.test",
            "https://user@localhost",
            "https://localhost/path",
            "https://localhost?token=x",
            "https://localhost#fragment",
            "https://127.0.0.2",
            "https://localhost:0",
        ] {
            assert!(QualificationRequest::new(endpoint, token(), None, 1_000).is_err());
        }
    }

    #[test]
    fn rejects_noncanonical_version_token_ca_and_timeout() {
        assert!(
            QualificationRequest::with_version(2, "https://localhost", token(), None, 1_000)
                .is_err()
        );
        assert!(QualificationRequest::new("https://localhost", "short", None, 1_000).is_err());
        assert!(QualificationRequest::new(
            "https://localhost",
            format!("{} x", "q".repeat(32)),
            None,
            1_000,
        )
        .is_err());
        assert!(QualificationRequest::new(
            "https://localhost",
            format!("{}\u{7f}", "q".repeat(32)),
            None,
            1_000,
        )
        .is_err());
        assert!(
            QualificationRequest::new("https://localhost", token(), Some(Vec::new()), 1_000,)
                .is_err()
        );
        assert!(QualificationRequest::new("https://localhost", token(), None, 99).is_err());
    }

    #[test]
    fn debug_redacts_all_transient_inputs() {
        let request = QualificationRequest::new(
            "https://localhost:9443",
            token(),
            Some(vec![1, 2, 3]),
            1_000,
        )
        .expect("request");
        let debug = format!("{request:?}");
        assert!(!debug.contains("9443"));
        assert!(!debug.contains(&token()));
        assert!(!debug.contains("[1, 2, 3]"));
    }
}

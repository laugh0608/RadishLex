use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

use crate::remote::{
    SyncRemoteError, SyncRemoteMethod, SyncRemoteRequest, SyncRemoteResponse, SyncRemoteTransport,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_ADDITIONAL_ROOT_CERTIFICATES: usize = 8;
const MAX_ROOT_CERTIFICATE_BYTES: usize = 64 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub struct HttpSyncRemoteTransport {
    endpoint: HttpEndpoint,
    timeout: Duration,
    access_token: Option<BearerAccessToken>,
    device_identity: Option<HttpDeviceIdentity>,
    additional_root_certificates: Vec<Vec<u8>>,
}

impl fmt::Debug for HttpSyncRemoteTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpSyncRemoteTransport")
            .field("endpoint", &self.endpoint)
            .field("timeout", &self.timeout)
            .field("access_token_configured", &self.access_token.is_some())
            .field(
                "device_identity_configured",
                &self.device_identity.is_some(),
            )
            .field(
                "additional_root_certificate_count",
                &self.additional_root_certificates.len(),
            )
            .finish()
    }
}

impl HttpSyncRemoteTransport {
    pub fn new(base_url: impl Into<String>) -> Result<Self, SyncRemoteError> {
        Self::with_timeout(base_url, DEFAULT_TIMEOUT)
    }

    pub fn with_timeout(
        base_url: impl Into<String>,
        timeout: Duration,
    ) -> Result<Self, SyncRemoteError> {
        if timeout.is_zero() {
            return invalid_request("http transport timeout must be greater than zero");
        }
        Ok(Self {
            endpoint: HttpEndpoint::parse(&base_url.into())?,
            timeout,
            access_token: None,
            device_identity: None,
            additional_root_certificates: Vec::new(),
        })
    }

    pub fn with_bearer_access_token(
        mut self,
        access_token: impl Into<String>,
    ) -> Result<Self, SyncRemoteError> {
        self.access_token = Some(BearerAccessToken::new(access_token.into())?);
        Ok(self)
    }

    pub fn with_device_identity(
        mut self,
        device_id: impl Into<String>,
    ) -> Result<Self, SyncRemoteError> {
        self.device_identity = Some(HttpDeviceIdentity::new(device_id.into())?);
        Ok(self)
    }

    /// Adds one DER-encoded trust anchor for the lifetime of this transport.
    ///
    /// This is intended for local HTTPS qualification with an explicit local
    /// CA. The certificate stays in memory and is never persisted by the
    /// transport. Public HTTPS continues to use the compiled Mozilla roots.
    pub fn with_additional_root_certificate_der(
        mut self,
        certificate_der: impl Into<Vec<u8>>,
    ) -> Result<Self, SyncRemoteError> {
        if self.endpoint.scheme != HttpScheme::Https {
            return invalid_request(
                "https transport root certificate requires an https:// base_url",
            );
        }
        let certificate_der = certificate_der.into();
        if certificate_der.is_empty() || certificate_der.len() > MAX_ROOT_CERTIFICATE_BYTES {
            return invalid_request("https transport root certificate must be 1..=65536 bytes");
        }
        if self.additional_root_certificates.len() >= MAX_ADDITIONAL_ROOT_CERTIFICATES {
            return invalid_request("https transport accepts at most 8 additional roots");
        }
        let mut roots = RootCertStore::empty();
        roots
            .add(CertificateDer::from(certificate_der.clone()))
            .map_err(|_| {
                invalid_request_value("https transport root certificate is not valid DER")
            })?;
        self.additional_root_certificates.push(certificate_der);
        Ok(self)
    }

    pub fn base_url(&self) -> String {
        self.endpoint.base_url()
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn has_bearer_access_token(&self) -> bool {
        self.access_token.is_some()
    }

    pub fn has_device_identity(&self) -> bool {
        self.device_identity.is_some()
    }

    pub fn is_https(&self) -> bool {
        self.endpoint.scheme == HttpScheme::Https
    }
}

impl SyncRemoteTransport for HttpSyncRemoteTransport {
    fn send(&self, request: SyncRemoteRequest) -> Result<SyncRemoteResponse, SyncRemoteError> {
        let path = self
            .endpoint
            .request_target(request.path(), request.query())?;
        validate_request_headers(&request)?;
        let stream = TcpStream::connect((self.endpoint.connect_host.as_str(), self.endpoint.port))
            .map_err(|error| transport_error(format!("connect failed: {error}")))?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(|error| transport_error(format!("set read timeout failed: {error}")))?;
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(|error| transport_error(format!("set write timeout failed: {error}")))?;

        let response = match self.endpoint.scheme {
            HttpScheme::Http => self.exchange(stream, &path, &request)?,
            HttpScheme::Https => self.exchange_tls(stream, &path, &request)?,
        };
        parse_response(&response)
    }
}

impl HttpSyncRemoteTransport {
    fn exchange<S: Read + Write>(
        &self,
        mut stream: S,
        path: &str,
        request: &SyncRemoteRequest,
    ) -> Result<Vec<u8>, SyncRemoteError> {
        write_request(
            &mut stream,
            &self.endpoint,
            path,
            self.access_token.as_ref(),
            self.device_identity.as_ref(),
            request,
        )?;
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|_| transport_error("read response failed"))?;
        Ok(response)
    }

    fn exchange_tls(
        &self,
        stream: TcpStream,
        path: &str,
        request: &SyncRemoteRequest,
    ) -> Result<Vec<u8>, SyncRemoteError> {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        for certificate in &self.additional_root_certificates {
            roots
                .add(CertificateDer::from(certificate.clone()))
                .map_err(|_| transport_error("tls root configuration failed"))?;
        }
        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let server_name = ServerName::try_from(self.endpoint.connect_host.clone())
            .map_err(|_| invalid_request_value("https transport server name is invalid"))?;
        let connection = ClientConnection::new(Arc::new(config), server_name)
            .map_err(|_| transport_error("tls client initialization failed"))?;
        self.exchange(StreamOwned::new(connection, stream), path, request)
            .map_err(|error| match error {
                SyncRemoteError::Transport { .. } => transport_error("tls exchange failed"),
                other => other,
            })
    }
}

#[derive(Clone, PartialEq, Eq)]
struct BearerAccessToken(String);

impl BearerAccessToken {
    fn new(access_token: String) -> Result<Self, SyncRemoteError> {
        if access_token.len() < 32 {
            return invalid_request("http transport bearer access token must be at least 32 bytes");
        }
        if access_token.chars().any(char::is_whitespace) {
            return invalid_request(
                "http transport bearer access token must not contain whitespace",
            );
        }
        Ok(Self(access_token))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for BearerAccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted bearer access token]")
    }
}

#[derive(Clone, PartialEq, Eq)]
struct HttpDeviceIdentity(String);

impl HttpDeviceIdentity {
    fn new(device_id: String) -> Result<Self, SyncRemoteError> {
        if device_id.is_empty() || device_id.len() > 128 {
            return invalid_request("http transport device identity must be 1..=128 bytes");
        }
        if !device_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return invalid_request(
                "http transport device identity contains unsupported characters",
            );
        }
        Ok(Self(device_id))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for HttpDeviceIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[configured device identity]")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpScheme {
    Http,
    Https,
}

impl HttpScheme {
    const fn default_port(self) -> u16 {
        match self {
            Self::Http => 80,
            Self::Https => 443,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HttpEndpoint {
    scheme: HttpScheme,
    host_header: String,
    connect_host: String,
    port: u16,
    base_path: String,
}

impl HttpEndpoint {
    fn parse(raw: &str) -> Result<Self, SyncRemoteError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return invalid_request("http transport base_url cannot be empty");
        }
        if raw.contains('?') || raw.contains('#') {
            return invalid_request("http transport base_url cannot contain query or fragment");
        }
        let (scheme, rest) = if let Some(rest) = raw.strip_prefix("http://") {
            (HttpScheme::Http, rest)
        } else if let Some(rest) = raw.strip_prefix("https://") {
            (HttpScheme::Https, rest)
        } else {
            return invalid_request("http transport requires http:// or https:// base_url");
        };
        let (authority, path) = match rest.split_once('/') {
            Some((authority, path)) => (authority, path),
            None => (rest, ""),
        };
        if authority.is_empty() {
            return invalid_request("http transport base_url host cannot be empty");
        }
        if authority.contains('@') {
            return invalid_request("http transport base_url must not contain credentials");
        }
        let (connect_host, host_header, port) = parse_authority(authority, scheme.default_port())?;
        let base_path = normalize_base_path(path)?;
        Ok(Self {
            scheme,
            host_header,
            connect_host,
            port,
            base_path,
        })
    }

    fn base_url(&self) -> String {
        let path = if self.base_path.is_empty() {
            ""
        } else {
            &self.base_path
        };
        format!("{}://{}{}", self.scheme.as_str(), self.host_header, path)
    }

    fn request_target(
        &self,
        request_path: &str,
        query: &[(String, String)],
    ) -> Result<String, SyncRemoteError> {
        if !request_path.starts_with('/') {
            return invalid_request("http transport request path must start with '/'");
        }
        if request_path.contains("://") {
            return invalid_request("http transport request path must be relative to base_url");
        }
        if request_path.contains('?') || request_path.contains('#') {
            return invalid_request("http transport request path cannot contain query or fragment");
        }
        if request_path.chars().any(char::is_whitespace) {
            return invalid_request("http transport request path cannot contain whitespace");
        }
        let path = if self.base_path.is_empty() {
            request_path.to_owned()
        } else if request_path == "/" {
            self.base_path.clone()
        } else {
            format!("{}{}", self.base_path, request_path)
        };
        if query.is_empty() {
            return Ok(path);
        }
        let query = query
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("&");
        Ok(format!("{path}?{query}"))
    }
}

fn parse_authority(
    authority: &str,
    default_port: u16,
) -> Result<(String, String, u16), SyncRemoteError> {
    if let Some(rest) = authority.strip_prefix('[') {
        let Some((host, suffix)) = rest.split_once(']') else {
            return invalid_request("http transport IPv6 host must use bracket notation");
        };
        if host.trim().is_empty() {
            return invalid_request("http transport IPv6 host cannot be empty");
        }
        let port = if suffix.is_empty() {
            default_port
        } else {
            let Some(port_text) = suffix.strip_prefix(':') else {
                return invalid_request("http transport IPv6 host suffix is invalid");
            };
            parse_port(port_text)?
        };
        let host_header = if port == default_port {
            format!("[{host}]")
        } else {
            format!("[{host}]:{port}")
        };
        return Ok((host.to_owned(), host_header, port));
    }

    if authority.contains('[') || authority.contains(']') {
        return invalid_request("http transport host bracket notation is invalid");
    }
    if authority.matches(':').count() > 1 {
        return invalid_request("http transport IPv6 host must use bracket notation");
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port_text)) => (host, parse_port(port_text)?),
        None => (authority, default_port),
    };
    if host.trim().is_empty() {
        return invalid_request("http transport host cannot be empty");
    }
    if host.contains('/') || host.chars().any(char::is_whitespace) {
        return invalid_request("http transport host is invalid");
    }
    let host_header = if port == default_port {
        host.to_owned()
    } else {
        format!("{host}:{port}")
    };
    Ok((host.to_owned(), host_header, port))
}

fn parse_port(port_text: &str) -> Result<u16, SyncRemoteError> {
    if port_text.is_empty() {
        return invalid_request("http transport port cannot be empty");
    }
    let port = port_text
        .parse::<u16>()
        .map_err(|_| invalid_request_value("http transport port must be a valid u16"))?;
    if port == 0 {
        return invalid_request("http transport port must be greater than zero");
    }
    Ok(port)
}

fn normalize_base_path(path: &str) -> Result<String, SyncRemoteError> {
    if path.is_empty() {
        return Ok(String::new());
    }
    if path.contains('\\') || path.chars().any(char::is_whitespace) {
        return invalid_request("http transport base_url path is invalid");
    }
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("/{trimmed}"))
    }
}

fn write_request<W: Write>(
    stream: &mut W,
    endpoint: &HttpEndpoint,
    path: &str,
    access_token: Option<&BearerAccessToken>,
    device_identity: Option<&HttpDeviceIdentity>,
    request: &SyncRemoteRequest,
) -> Result<(), SyncRemoteError> {
    let method = match request.method() {
        SyncRemoteMethod::Get => "GET",
        SyncRemoteMethod::Post => "POST",
    };
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\n\
         Host: {}\r\n\
         User-Agent: radishlex-ime-sync/0.1\r\n\
         Accept: application/json, application/octet-stream\r\n\
         Connection: close\r\n\
         Content-Length: {}\r\n",
        endpoint.host_header,
        request.body().len(),
    )
    .map_err(|error| transport_error(format!("write request failed: {error}")))?;
    if let Some(content_type) = request.content_type() {
        write!(stream, "Content-Type: {content_type}\r\n")
            .map_err(|error| transport_error(format!("write request failed: {error}")))?;
    }
    if let Some(token) = access_token {
        write!(stream, "Authorization: Bearer {}\r\n", token.as_str())
            .map_err(|error| transport_error(format!("write request failed: {error}")))?;
    }
    if let Some(device_identity) = device_identity {
        write!(
            stream,
            "X-RadishLex-Device-ID: {}\r\n",
            device_identity.as_str()
        )
        .map_err(|error| transport_error(format!("write request failed: {error}")))?;
    }
    stream
        .write_all(b"\r\n")
        .map_err(|error| transport_error(format!("write request failed: {error}")))?;
    stream
        .write_all(request.body())
        .map_err(|error| transport_error(format!("write request failed: {error}")))?;
    stream
        .flush()
        .map_err(|error| transport_error(format!("flush request failed: {error}")))?;
    Ok(())
}

fn validate_request_headers(request: &SyncRemoteRequest) -> Result<(), SyncRemoteError> {
    if let Some(content_type) = request.content_type() {
        if content_type.trim().is_empty() {
            return invalid_request("http transport content-type cannot be empty");
        }
        if content_type.contains('\r') || content_type.contains('\n') {
            return invalid_request("http transport content-type cannot contain line breaks");
        }
    }
    Ok(())
}

fn parse_response(bytes: &[u8]) -> Result<SyncRemoteResponse, SyncRemoteError> {
    let header_end = find_header_end(bytes).ok_or_else(|| SyncRemoteError::InvalidResponse {
        message: "http response is missing header terminator".to_owned(),
    })?;
    let header_bytes = &bytes[..header_end];
    let mut body = bytes[header_end + 4..].to_vec();
    let header_text =
        std::str::from_utf8(header_bytes).map_err(|_| SyncRemoteError::InvalidResponse {
            message: "http response headers are not utf-8".to_owned(),
        })?;
    let mut lines = header_text.split("\r\n");
    let status = parse_status_line(lines.next().unwrap_or_default())?;
    let mut content_type = None;
    let mut content_length = None;
    let mut chunked = false;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            return invalid_response("http response header is invalid");
        };
        let name = name.trim();
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-type") {
            content_type = Some(value.to_owned());
        } else if name.eq_ignore_ascii_case("content-length") {
            content_length =
                Some(
                    value
                        .parse::<usize>()
                        .map_err(|_| SyncRemoteError::InvalidResponse {
                            message: "http response content-length is invalid".to_owned(),
                        })?,
                );
        } else if name.eq_ignore_ascii_case("transfer-encoding")
            && value
                .split(',')
                .any(|part| part.trim().eq_ignore_ascii_case("chunked"))
        {
            chunked = true;
        }
    }
    if chunked {
        body = decode_chunked_body(&body)?;
    } else if let Some(expected_len) = content_length {
        if body.len() < expected_len {
            return invalid_response("http response body is shorter than content-length");
        }
        body.truncate(expected_len);
    }
    Ok(SyncRemoteResponse::new(status, content_type, body))
}

fn parse_status_line(line: &str) -> Result<u16, SyncRemoteError> {
    let mut parts = line.split_whitespace();
    let Some(version) = parts.next() else {
        return invalid_response("http response status line is empty");
    };
    if !version.starts_with("HTTP/1.") {
        return invalid_response("http response version is unsupported");
    }
    let Some(status_text) = parts.next() else {
        return invalid_response("http response status code is missing");
    };
    status_text
        .parse::<u16>()
        .map_err(|_| SyncRemoteError::InvalidResponse {
            message: "http response status code is invalid".to_owned(),
        })
}

fn decode_chunked_body(body: &[u8]) -> Result<Vec<u8>, SyncRemoteError> {
    let mut cursor = 0;
    let mut decoded = Vec::new();
    loop {
        let line_end =
            find_crlf(&body[cursor..]).ok_or_else(|| SyncRemoteError::InvalidResponse {
                message: "chunked response is missing chunk size terminator".to_owned(),
            })? + cursor;
        let size_line = std::str::from_utf8(&body[cursor..line_end]).map_err(|_| {
            SyncRemoteError::InvalidResponse {
                message: "chunked response size is not utf-8".to_owned(),
            }
        })?;
        let size_text = size_line
            .split_once(';')
            .map_or(size_line, |(size, _)| size);
        let size = usize::from_str_radix(size_text.trim(), 16).map_err(|_| {
            SyncRemoteError::InvalidResponse {
                message: "chunked response size is invalid".to_owned(),
            }
        })?;
        cursor = line_end + 2;
        if size == 0 {
            return Ok(decoded);
        }
        if body.len() < cursor + size + 2 {
            return invalid_response("chunked response body is truncated");
        }
        decoded.extend_from_slice(&body[cursor..cursor + size]);
        cursor += size;
        if body.get(cursor..cursor + 2) != Some(b"\r\n") {
            return invalid_response("chunked response is missing chunk terminator");
        }
        cursor += 2;
    }
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

fn find_crlf(bytes: &[u8]) -> Option<usize> {
    bytes.windows(2).position(|window| window == b"\r\n")
}

fn invalid_request<T>(message: impl Into<String>) -> Result<T, SyncRemoteError> {
    Err(SyncRemoteError::InvalidRequest {
        message: message.into(),
    })
}

fn invalid_request_value(message: impl Into<String>) -> SyncRemoteError {
    SyncRemoteError::InvalidRequest {
        message: message.into(),
    }
}

fn invalid_response<T>(message: impl Into<String>) -> Result<T, SyncRemoteError> {
    Err(SyncRemoteError::InvalidResponse {
        message: message.into(),
    })
}

fn transport_error(message: impl Into<String>) -> SyncRemoteError {
    SyncRemoteError::Transport {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::test_support::{response_for, signed_object};
    use crate::{LatestObjectConflictMetadata, SyncRemoteClient, SyncServerErrorCode};
    use base64ct::Encoding;
    use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
    use rustls::pki_types::PrivatePkcs8KeyDer;
    use rustls::{ServerConfig, ServerConnection};
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::thread;

    const TEST_ACCESS_TOKEN: &str = "test-access-token-12345678901234567890";

    #[test]
    fn http_transport_sends_request_and_reads_json_response() {
        let Some(server) = TestHttpServer::try_spawn(|request| {
            assert_eq!(request.method, "POST");
            assert_eq!(
                request.path,
                "/api/v1/domains/domain-a/objects/object-a/versions"
            );
            assert_eq!(request.header("host"), Some(request.host_header.as_str()));
            assert_eq!(request.header("content-type"), Some("application/json"));
            assert_eq!(request.body, br#"{"payload":"encrypted"}"#);
            TestHttpResponse::json(201, br#"{"ok":true}"#)
        }) else {
            return;
        };
        let transport =
            HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(2))
                .expect("transport");

        let response = transport
            .send(SyncRemoteRequest::new(
                SyncRemoteMethod::Post,
                "/api/v1/domains/domain-a/objects/object-a/versions",
                Some("application/json".to_owned()),
                br#"{"payload":"encrypted"}"#.to_vec(),
            ))
            .expect("response");

        assert_eq!(response.status, 201);
        assert_eq!(response.content_type.as_deref(), Some("application/json"));
        assert_eq!(response.body, br#"{"ok":true}"#);
        server.join();
    }

    #[test]
    fn http_transport_sends_bearer_access_token_without_leaking_debug() {
        let Some(server) = TestHttpServer::try_spawn(|request| {
            let expected = format!("Bearer {TEST_ACCESS_TOKEN}");
            assert_eq!(request.header("authorization"), Some(expected.as_str()));
            TestHttpResponse::json(200, br#"{"ok":true}"#)
        }) else {
            return;
        };
        let transport =
            HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(2))
                .expect("transport")
                .with_bearer_access_token(TEST_ACCESS_TOKEN)
                .expect("access token");

        let debug = format!("{transport:?}");
        assert!(debug.contains("access_token_configured"));
        assert!(!debug.contains(TEST_ACCESS_TOKEN));

        let response = transport
            .send(SyncRemoteRequest::new(
                SyncRemoteMethod::Get,
                "/api/v1/domains/domain-a/state",
                None,
                Vec::new(),
            ))
            .expect("response");

        assert_eq!(response.status, 200);
        server.join();
    }

    #[test]
    fn http_transport_sends_validated_device_identity() {
        let Some(server) = TestHttpServer::try_spawn(|request| {
            assert_eq!(request.header("x-radishlex-device-id"), Some("device-b"));
            TestHttpResponse::json(200, br#"{"ok":true}"#)
        }) else {
            return;
        };
        let transport =
            HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(2))
                .expect("transport")
                .with_device_identity("device-b")
                .expect("device identity");
        assert!(transport.has_device_identity());

        let response = transport
            .send(SyncRemoteRequest::new(
                SyncRemoteMethod::Get,
                "/api/v1/domains/domain-a/state",
                None,
                Vec::new(),
            ))
            .expect("response");
        assert_eq!(response.status, 200);
        server.join();
    }

    #[test]
    fn http_transport_preserves_base_path_and_decodes_chunked_payload() {
        let Some(server) = TestHttpServer::try_spawn(|request| {
            assert_eq!(request.method, "GET");
            assert_eq!(
                request.path,
                "/radishlex/api/v1/domains/domain-a/objects/object-a/versions/1/payload"
            );
            TestHttpResponse::chunked(
                200,
                "application/octet-stream",
                &[b"encr".as_slice(), b"ypted".as_slice()],
            )
        }) else {
            return;
        };
        let transport = HttpSyncRemoteTransport::with_timeout(
            server.base_url_with_path("/radishlex"),
            Duration::from_secs(2),
        )
        .expect("transport");

        let response = transport
            .send(SyncRemoteRequest::new(
                SyncRemoteMethod::Get,
                "/api/v1/domains/domain-a/objects/object-a/versions/1/payload",
                None,
                Vec::new(),
            ))
            .expect("response");

        assert_eq!(response.status, 200);
        assert_eq!(
            response.content_type.as_deref(),
            Some("application/octet-stream")
        );
        assert_eq!(response.body, b"encrypted");
        server.join();
    }

    #[test]
    fn http_transport_integrates_with_remote_client_upload_and_payload_download() {
        let (object, manifest) = signed_object();
        let response_body = serde_json::to_vec(&response_for(&object)).expect("response json");
        let upload_response = response_body.clone();
        let metadata_response = response_body;
        let payload = object.envelope.encrypted_payload.clone();
        let expected_payload = payload.clone();
        let Some(server) = TestHttpServer::try_spawn_many(vec![
            Box::new(move |request| {
                assert_eq!(request.method, "POST");
                assert_eq!(
                    request.path,
                    "/api/v1/domains/domain-a/objects/object-a/versions"
                );
                let body = String::from_utf8(request.body).expect("request body");
                assert!(body.contains("\"payload\""));
                assert!(!body.contains("plaintext"));
                assert!(!body.contains("input_code"));
                assert!(!body.contains("reading"));
                TestHttpResponse::json(201, &upload_response)
            }),
            Box::new(move |request| {
                assert_eq!(request.method, "GET");
                assert_eq!(
                    request.path,
                    "/api/v1/domains/domain-a/objects/object-a/versions/1"
                );
                TestHttpResponse::json(200, &metadata_response)
            }),
            Box::new(move |request| {
                assert_eq!(request.method, "GET");
                assert_eq!(
                    request.path,
                    "/api/v1/domains/domain-a/objects/object-a/versions/1/payload"
                );
                TestHttpResponse::with_body(200, "application/octet-stream", &payload)
            }),
        ]) else {
            return;
        };
        let transport =
            HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(2))
                .expect("transport");
        let client = SyncRemoteClient::new(transport);

        let uploaded = client
            .upload_object_version("domain-a", &object, &manifest)
            .expect("upload");
        let downloaded = client
            .object_payload("domain-a", "object-a", 1)
            .expect("payload");

        assert_eq!(uploaded.object_id, "object-a");
        assert_eq!(downloaded.object.object_id, "object-a");
        assert_eq!(downloaded.payload, expected_payload);
        server.join();
    }

    #[test]
    fn http_transport_integrates_with_remote_client_server_error_mapping() {
        let Some(server) = TestHttpServer::try_spawn(|request| {
            assert_eq!(request.method, "GET");
            assert_eq!(
                request.path,
                "/api/v1/domains/domain-a/objects/object-a/versions/1"
            );
            TestHttpResponse::json(
                409,
                br#"{"error_code":"conflict_stale_base_version","message":"base version is stale","retryable":false,"server_time_ms":123,"latest_version":3,"latest_ciphertext_hash":"latest-hash"}"#,
            )
        }) else {
            return;
        };
        let transport =
            HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(2))
                .expect("transport");
        let client = SyncRemoteClient::new(transport);

        let error = client
            .object_version("domain-a", "object-a", 1)
            .expect_err("server error");

        match error {
            SyncRemoteError::Server {
                status,
                code,
                latest,
                ..
            } => {
                assert_eq!(status, 409);
                assert_eq!(code, SyncServerErrorCode::ConflictStaleBaseVersion);
                assert_eq!(
                    latest,
                    Some(LatestObjectConflictMetadata {
                        version: 3,
                        ciphertext_hash: Some("latest-hash".to_owned()),
                    })
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
        server.join();
    }

    #[test]
    fn https_transport_verifies_explicit_local_root_and_redacts_certificate() {
        let Some(server) = TestHttpsServer::try_spawn(|request| {
            assert_eq!(request.method, "GET");
            assert_eq!(request.path, "/api/v1/domains/domain-a/state");
            TestHttpResponse::json(200, br#"{"ok":true}"#)
        }) else {
            return;
        };
        let root_der = server.root_certificate_der().to_vec();
        let transport =
            HttpSyncRemoteTransport::with_timeout(server.base_url(), Duration::from_secs(2))
                .expect("https transport")
                .with_additional_root_certificate_der(root_der.clone())
                .expect("local root");

        assert!(transport.is_https());
        let debug = format!("{transport:?}");
        assert!(debug.contains("additional_root_certificate_count"));
        assert!(!debug.contains(&base64ct::Base64::encode_string(&root_der)));

        let response = transport
            .send(SyncRemoteRequest::new(
                SyncRemoteMethod::Get,
                "/api/v1/domains/domain-a/state",
                None,
                Vec::new(),
            ))
            .expect("https response");
        assert_eq!(response.status, 200);
        server.join();
    }

    #[test]
    fn https_transport_rejects_untrusted_certificate_and_wrong_hostname() {
        let Some(untrusted_server) =
            TestHttpsServer::try_spawn(|_| TestHttpResponse::json(200, br#"{"unexpected":true}"#))
        else {
            return;
        };
        let untrusted = HttpSyncRemoteTransport::with_timeout(
            untrusted_server.base_url(),
            Duration::from_secs(2),
        )
        .expect("https transport")
        .send(SyncRemoteRequest::new(
            SyncRemoteMethod::Get,
            "/api/v1/domains/domain-a/state",
            None,
            Vec::new(),
        ))
        .expect_err("untrusted local certificate must fail");
        assert_eq!(
            untrusted,
            SyncRemoteError::Transport {
                message: "tls exchange failed".to_owned(),
            }
        );
        untrusted_server.join();

        let Some(wrong_host_server) =
            TestHttpsServer::try_spawn(|_| TestHttpResponse::json(200, br#"{"unexpected":true}"#))
        else {
            return;
        };
        let wrong_host = HttpSyncRemoteTransport::with_timeout(
            wrong_host_server.ip_base_url(),
            Duration::from_secs(2),
        )
        .expect("https transport")
        .with_additional_root_certificate_der(wrong_host_server.root_certificate_der().to_vec())
        .expect("local root")
        .send(SyncRemoteRequest::new(
            SyncRemoteMethod::Get,
            "/api/v1/domains/domain-a/state",
            None,
            Vec::new(),
        ))
        .expect_err("certificate hostname mismatch must fail");
        assert_eq!(
            wrong_host,
            SyncRemoteError::Transport {
                message: "tls exchange failed".to_owned(),
            }
        );
        wrong_host_server.join();
    }

    #[test]
    fn http_transport_rejects_unsupported_or_ambiguous_urls() {
        let https = HttpSyncRemoteTransport::new("https://example.test").expect("https url");
        assert!(https.is_https());
        assert_eq!(https.base_url(), "https://example.test");
        assert!(matches!(
            HttpSyncRemoteTransport::new("ftp://example.test"),
            Err(SyncRemoteError::InvalidRequest { .. })
        ));
        assert!(matches!(
            HttpSyncRemoteTransport::new("http://example.test/path?token=secret"),
            Err(SyncRemoteError::InvalidRequest { .. })
        ));
        assert!(matches!(
            HttpSyncRemoteTransport::new("http://user:pass@example.test"),
            Err(SyncRemoteError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn https_transport_rejects_invalid_or_excessive_local_roots() {
        let transport = HttpSyncRemoteTransport::new("https://localhost").expect("transport");
        assert!(matches!(
            transport
                .clone()
                .with_additional_root_certificate_der(Vec::new()),
            Err(SyncRemoteError::InvalidRequest { .. })
        ));
        assert!(matches!(
            transport
                .clone()
                .with_additional_root_certificate_der(vec![0x01, 0x02]),
            Err(SyncRemoteError::InvalidRequest { .. })
        ));
        assert!(matches!(
            transport
                .clone()
                .with_additional_root_certificate_der(vec![0u8; 65 * 1024]),
            Err(SyncRemoteError::InvalidRequest { .. })
        ));

        let mut root_params =
            CertificateParams::new(Vec::<String>::new()).expect("test root params");
        root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let root_key = KeyPair::generate().expect("generate test root key");
        let root_der = root_params
            .self_signed(&root_key)
            .expect("sign test root")
            .der()
            .to_vec();
        let mut transport = transport;
        for _ in 0..MAX_ADDITIONAL_ROOT_CERTIFICATES {
            transport = transport
                .with_additional_root_certificate_der(root_der.clone())
                .expect("root within count limit");
        }
        assert!(matches!(
            transport.with_additional_root_certificate_der(root_der),
            Err(SyncRemoteError::InvalidRequest { .. })
        ));

        assert!(matches!(
            HttpSyncRemoteTransport::new("http://localhost")
                .expect("http transport")
                .with_additional_root_certificate_der(vec![0x01]),
            Err(SyncRemoteError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn http_transport_rejects_invalid_bearer_access_token() {
        let transport = HttpSyncRemoteTransport::new("http://example.test").expect("transport");
        assert!(matches!(
            transport.clone().with_bearer_access_token("short-token"),
            Err(SyncRemoteError::InvalidRequest { .. })
        ));
        assert!(matches!(
            transport.with_bearer_access_token("test-access-token-12345678901234567890\n"),
            Err(SyncRemoteError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn http_transport_rejects_invalid_device_identity() {
        let transport = HttpSyncRemoteTransport::new("http://example.test").expect("transport");
        assert!(matches!(
            transport.clone().with_device_identity(""),
            Err(SyncRemoteError::InvalidRequest { .. })
        ));
        assert!(matches!(
            transport.with_device_identity("device-b\r\nX-Leak: wrapped"),
            Err(SyncRemoteError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn http_transport_rejects_ambiguous_request_parts() {
        let transport = HttpSyncRemoteTransport::new("http://127.0.0.1:7319").expect("transport");

        let bad_path = transport.send(SyncRemoteRequest::new(
            SyncRemoteMethod::Get,
            "/api/v1/domains/domain-a/state?plaintext=true",
            None,
            Vec::new(),
        ));
        assert!(matches!(
            bad_path,
            Err(SyncRemoteError::InvalidRequest { .. })
        ));

        let bad_content_type = transport.send(SyncRemoteRequest::new(
            SyncRemoteMethod::Post,
            "/api/v1/domains/domain-a/objects/object-a/versions",
            Some("application/json\r\nX-Leak: payload".to_owned()),
            Vec::new(),
        ));
        assert!(matches!(
            bad_content_type,
            Err(SyncRemoteError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn http_transport_errors_do_not_include_request_body() {
        let transport =
            HttpSyncRemoteTransport::with_timeout("http://127.0.0.1:1", Duration::from_millis(50))
                .expect("transport");

        let error = transport
            .send(SyncRemoteRequest::new(
                SyncRemoteMethod::Post,
                "/api/v1/domains/domain-a/objects/object-a/versions",
                Some("application/json".to_owned()),
                br#"{"payload":"sensitive encrypted bytes"}"#.to_vec(),
            ))
            .expect_err("connect fails");

        let debug = format!("{error:?}");
        assert!(!debug.contains("sensitive encrypted bytes"));
        assert!(!debug.contains("payload"));
    }

    struct TestHttpServer {
        host: String,
        port: u16,
        handle: thread::JoinHandle<()>,
    }

    struct TestHttpsServer {
        port: u16,
        root_certificate_der: Vec<u8>,
        handle: thread::JoinHandle<()>,
    }

    impl TestHttpsServer {
        fn try_spawn(
            handler: impl FnOnce(TestHttpRequest) -> TestHttpResponse + Send + 'static,
        ) -> Option<Self> {
            let listener = match TcpListener::bind("127.0.0.1:0") {
                Ok(listener) => listener,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
                Err(error) => panic!("bind test https server: {error}"),
            };
            let mut root_params =
                CertificateParams::new(Vec::<String>::new()).expect("test root certificate params");
            root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
            let root_key = KeyPair::generate().expect("generate test root key");
            let root_certificate = root_params
                .self_signed(&root_key)
                .expect("sign test root certificate");
            let server_params = CertificateParams::new(vec!["localhost".to_owned()])
                .expect("test server certificate params");
            let server_key = KeyPair::generate().expect("generate test server key");
            let server_certificate = server_params
                .signed_by(&server_key, &root_certificate, &root_key)
                .expect("sign test server certificate");
            let root_certificate_der = root_certificate.der().to_vec();
            let private_key = PrivatePkcs8KeyDer::from(server_key.serialize_der());
            let config = ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![server_certificate.der().clone()], private_key.into())
                .expect("test tls config");
            let port = listener.local_addr().expect("local addr").port();
            let handle = thread::spawn(move || {
                let (stream, _) = listener.accept().expect("accept");
                let connection =
                    ServerConnection::new(Arc::new(config)).expect("test tls connection");
                let mut stream = StreamOwned::new(connection, stream);
                if stream.conn.complete_io(&mut stream.sock).is_err() {
                    return;
                }
                let request = read_test_request(&mut stream);
                let response = handler(request);
                stream.write_all(&response.bytes).expect("write response");
                stream.conn.send_close_notify();
                stream.flush().expect("flush response");
            });
            Some(Self {
                port,
                root_certificate_der,
                handle,
            })
        }

        fn base_url(&self) -> String {
            format!("https://localhost:{}", self.port)
        }

        fn ip_base_url(&self) -> String {
            format!("https://127.0.0.1:{}", self.port)
        }

        fn root_certificate_der(&self) -> &[u8] {
            &self.root_certificate_der
        }

        fn join(self) {
            self.handle.join().expect("server thread");
        }
    }

    type TestHttpHandler = Box<dyn FnOnce(TestHttpRequest) -> TestHttpResponse + Send>;

    impl TestHttpServer {
        fn try_spawn(
            handler: impl FnOnce(TestHttpRequest) -> TestHttpResponse + Send + 'static,
        ) -> Option<Self> {
            Self::try_spawn_many(vec![Box::new(handler)])
        }

        fn try_spawn_many(handlers: Vec<TestHttpHandler>) -> Option<Self> {
            let listener = match TcpListener::bind("127.0.0.1:0") {
                Ok(listener) => listener,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
                Err(error) => panic!("bind test http server: {error}"),
            };
            let addr = listener.local_addr().expect("local addr");
            let host = addr.ip().to_string();
            let port = addr.port();
            let handle = thread::spawn(move || {
                for handler in handlers {
                    let (mut stream, _) = listener.accept().expect("accept");
                    let request = read_test_request(&mut stream);
                    let response = handler(request);
                    stream.write_all(&response.bytes).expect("write response");
                }
            });
            Some(Self { host, port, handle })
        }

        fn base_url(&self) -> String {
            format!("http://{}:{}", self.host, self.port)
        }

        fn base_url_with_path(&self, path: &str) -> String {
            format!("{}{}", self.base_url(), path)
        }

        fn join(self) {
            self.handle.join().expect("server thread");
        }
    }

    #[derive(Debug)]
    struct TestHttpRequest {
        method: String,
        path: String,
        host_header: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl TestHttpRequest {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
        }
    }

    struct TestHttpResponse {
        bytes: Vec<u8>,
    }

    impl TestHttpResponse {
        fn json(status: u16, body: &[u8]) -> Self {
            Self::with_body(status, "application/json", body)
        }

        fn with_body(status: u16, content_type: &str, body: &[u8]) -> Self {
            let mut bytes = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .into_bytes();
            bytes.extend_from_slice(body);
            Self { bytes }
        }

        fn chunked(status: u16, content_type: &str, chunks: &[&[u8]]) -> Self {
            let mut bytes = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: {content_type}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
            )
            .into_bytes();
            for chunk in chunks {
                bytes.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
                bytes.extend_from_slice(chunk);
                bytes.extend_from_slice(b"\r\n");
            }
            bytes.extend_from_slice(b"0\r\n\r\n");
            Self { bytes }
        }
    }

    fn read_test_request<R: Read>(stream: &mut R) -> TestHttpRequest {
        let mut reader = BufReader::new(stream);
        let mut first_line = String::new();
        reader.read_line(&mut first_line).expect("request line");
        let mut parts = first_line.split_whitespace();
        let method = parts.next().expect("method").to_owned();
        let path = parts.next().expect("path").to_owned();
        let mut headers = Vec::new();
        let mut content_length = 0usize;
        let mut host_header = String::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("header line");
            if line == "\r\n" {
                break;
            }
            let (name, value) = line.trim_end().split_once(':').expect("header");
            let value = value.trim().to_owned();
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.parse().expect("content-length");
            }
            if name.eq_ignore_ascii_case("host") {
                host_header = value.clone();
            }
            headers.push((name.to_owned(), value));
        }
        let mut body = vec![0u8; content_length];
        reader.read_exact(&mut body).expect("body");
        TestHttpRequest {
            method,
            path,
            host_header,
            headers,
            body,
        }
    }
}

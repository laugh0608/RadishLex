use std::env;
use std::fs;
use std::time::Duration;

use radishlex_ime_sync::{
    HttpSyncRemoteTransport, SyncRemoteMethod, SyncRemoteRequest, SyncRemoteTransport,
};

const RUN_ENV: &str = "RADISHLEX_RUN_LOCAL_HTTPS_TRANSPORT_TEST";
const ENDPOINT_ENV: &str = "RADISHLEX_LOCAL_HTTPS_ENDPOINT";
const ACCESS_TOKEN_ENV: &str = "RADISHLEX_LOCAL_HTTPS_ACCESS_TOKEN";
const ROOT_DER_PATH_ENV: &str = "RADISHLEX_LOCAL_HTTPS_ROOT_DER_PATH";

#[test]
fn rust_transport_reaches_local_caddy_with_verified_tls_and_bearer_gate() {
    if env::var(RUN_ENV).as_deref() != Ok("1") {
        return;
    }

    let endpoint = required_env(ENDPOINT_ENV);
    let access_token = required_env(ACCESS_TOKEN_ENV);
    let root_der_path = required_env(ROOT_DER_PATH_ENV);
    let root_der = fs::read(root_der_path).expect("read temporary local HTTPS root certificate");

    let base = HttpSyncRemoteTransport::with_timeout(endpoint, Duration::from_secs(5))
        .expect("construct local HTTPS transport")
        .with_additional_root_certificate_der(root_der)
        .expect("configure temporary local HTTPS root");
    assert!(base.is_https());

    let request = || {
        SyncRemoteRequest::new(
            SyncRemoteMethod::Get,
            "/api/v1/domains/rust-local-https-probe/state",
            None,
            Vec::new(),
        )
    };
    let unauthorized = base
        .send(request())
        .expect("receive bearer-gated local HTTPS response");
    assert_eq!(unauthorized.status, 401);
    assert_error_code(&unauthorized.body, "unauthenticated");

    let authorized = base
        .with_bearer_access_token(access_token.clone())
        .expect("configure in-memory bearer access token")
        .send(request())
        .expect("receive authorized local HTTPS response");
    assert_eq!(authorized.status, 404);
    assert_error_code(&authorized.body, "not_found");
    assert!(!String::from_utf8_lossy(&authorized.body).contains(&access_token));
}

fn required_env(name: &str) -> String {
    env::var(name)
        .unwrap_or_else(|_| panic!("required local HTTPS test environment is missing: {name}"))
}

fn assert_error_code(body: &[u8], expected: &str) {
    let value: serde_json::Value =
        serde_json::from_slice(body).expect("local HTTPS response must be JSON");
    assert_eq!(
        value.get("error_code").and_then(serde_json::Value::as_str),
        Some(expected)
    );
}

#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import ssl
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from typing import Any


FORMAT = "sync_connection_health.v1"
REDACTION_POLICY = "summary_only_no_endpoint_tokens_or_response_body"
DEFAULT_PROBE_DOMAIN_ID = "radishlex-local-probe"


class ConnectionHealthError(RuntimeError):
    pass


@dataclass(frozen=True)
class EndpointClassification:
    endpoint_status: str
    transport_mode: str
    blocker: str
    parsed_url: urllib.parse.ParseResult | None


@dataclass(frozen=True)
class ProbeResult:
    http_status: int
    error_code: str
    network_error: str


def classify_endpoint(endpoint: str) -> EndpointClassification:
    candidate = endpoint.strip()
    if not candidate:
        return EndpointClassification("not_configured", "not_configured", "server_endpoint_missing", None)

    parsed = urllib.parse.urlparse(candidate)
    if not parsed.scheme or not parsed.hostname:
        return EndpointClassification("invalid", "invalid_endpoint", "server_endpoint_invalid", None)
    if parsed.username or parsed.password:
        return EndpointClassification(
            "invalid_userinfo",
            "invalid_endpoint",
            "server_endpoint_userinfo_forbidden",
            None,
        )
    if parsed.query:
        return EndpointClassification("invalid_query", "invalid_endpoint", "server_endpoint_query_forbidden", None)
    if parsed.fragment:
        return EndpointClassification(
            "invalid_fragment",
            "invalid_endpoint",
            "server_endpoint_fragment_forbidden",
            None,
        )
    if parsed.scheme not in {"http", "https"}:
        return EndpointClassification(
            "invalid_scheme",
            "invalid_endpoint",
            "server_endpoint_scheme_unsupported",
            None,
        )

    hostname = parsed.hostname.lower()
    is_local = hostname in {"localhost", "127.0.0.1", "::1"}
    if parsed.scheme == "https" and is_local:
        return EndpointClassification("configured", "local_https", "none", parsed)
    if parsed.scheme == "http" and is_local:
        return EndpointClassification("configured", "local_http", "none", parsed)
    if parsed.scheme == "https":
        return EndpointClassification("configured", "external_https", "none", parsed)
    return EndpointClassification("configured", "remote_http", "remote_plain_http_forbidden", parsed)


def summary_for_configuration(
    *,
    endpoint: EndpointClassification,
    access_token_present: bool,
    local_insecure_tls: bool,
) -> dict[str, Any] | None:
    if endpoint.blocker != "none":
        connection_status = "not_configured" if endpoint.endpoint_status == "not_configured" else "configuration_invalid"
        return base_summary(
            endpoint=endpoint,
            access_token_present=access_token_present,
            connection_status=connection_status,
            auth_status="not_checked",
            server_state_status="not_checked_configuration_blocked",
            http_status=0,
            http_status_class="not_checked",
            last_remote_error_code=endpoint.blocker,
            local_insecure_tls=local_insecure_tls,
        )
    if endpoint.transport_mode == "remote_http":
        return base_summary(
            endpoint=endpoint,
            access_token_present=access_token_present,
            connection_status="configuration_invalid",
            auth_status="not_checked",
            server_state_status="not_checked_unsupported_transport",
            http_status=0,
            http_status_class="not_checked",
            last_remote_error_code=endpoint.blocker,
            local_insecure_tls=local_insecure_tls,
        )
    return None


def probe_server_state(
    *,
    parsed_url: urllib.parse.ParseResult,
    access_token: str,
    probe_domain_id: str,
    timeout_seconds: float,
    allow_local_insecure_tls: bool,
) -> ProbeResult:
    base_url = urllib.parse.urlunparse(
        (
            parsed_url.scheme,
            parsed_url.netloc,
            parsed_url.path.rstrip("/"),
            "",
            "",
            "",
        )
    )
    probe_path = f"/api/v1/domains/{urllib.parse.quote(probe_domain_id)}/state"
    url = f"{base_url}{probe_path}"
    request = urllib.request.Request(url)
    if access_token:
        request.add_header("Authorization", f"Bearer {access_token}")

    context = None
    if parsed_url.scheme == "https" and allow_local_insecure_tls:
        context = ssl._create_unverified_context()

    try:
        with urllib.request.urlopen(request, timeout=timeout_seconds, context=context) as response:
            body = response.read(8192).decode("utf-8", errors="replace")
            return ProbeResult(int(response.status), error_code_from_body(body), "")
    except urllib.error.HTTPError as exc:
        body = exc.read(8192).decode("utf-8", errors="replace")
        return ProbeResult(int(exc.code), error_code_from_body(body), "")
    except ssl.SSLError:
        return ProbeResult(0, "tls_error", "tls_error")
    except OSError:
        return ProbeResult(0, "network_unreachable", "network_unreachable")


def error_code_from_body(body: str) -> str:
    try:
        decoded = json.loads(body)
    except json.JSONDecodeError:
        return "none"
    if not isinstance(decoded, dict):
        return "none"
    error_code = decoded.get("error_code")
    return error_code if isinstance(error_code, str) and error_code else "none"


def summary_for_probe(
    *,
    endpoint: EndpointClassification,
    access_token_present: bool,
    result: ProbeResult,
    local_insecure_tls: bool,
) -> dict[str, Any]:
    if result.network_error:
        return base_summary(
            endpoint=endpoint,
            access_token_present=access_token_present,
            connection_status=result.network_error,
            auth_status="not_checked",
            server_state_status="not_checked_network_unreachable",
            http_status=0,
            http_status_class="network_error",
            last_remote_error_code=result.error_code,
            local_insecure_tls=local_insecure_tls,
        )

    if result.http_status == 401 and result.error_code == "unauthenticated":
        return base_summary(
            endpoint=endpoint,
            access_token_present=access_token_present,
            connection_status="reachable",
            auth_status="failed" if access_token_present else "required",
            server_state_status="auth_gate_reachable",
            http_status=result.http_status,
            http_status_class="client_error",
            last_remote_error_code=result.error_code,
            local_insecure_tls=local_insecure_tls,
        )
    if result.http_status == 404 and result.error_code == "not_found":
        return base_summary(
            endpoint=endpoint,
            access_token_present=access_token_present,
            connection_status="reachable",
            auth_status="accepted" if access_token_present else "not_required_for_local_probe",
            server_state_status="domain_missing_expected",
            http_status=result.http_status,
            http_status_class="client_error",
            last_remote_error_code=result.error_code,
            local_insecure_tls=local_insecure_tls,
        )
    if 200 <= result.http_status < 300:
        return base_summary(
            endpoint=endpoint,
            access_token_present=access_token_present,
            connection_status="reachable",
            auth_status="accepted" if access_token_present else "not_required_for_local_probe",
            server_state_status="domain_state_returned",
            http_status=result.http_status,
            http_status_class="success",
            last_remote_error_code="none",
            local_insecure_tls=local_insecure_tls,
        )

    return base_summary(
        endpoint=endpoint,
        access_token_present=access_token_present,
        connection_status="reachable_with_unexpected_status",
        auth_status="unknown",
        server_state_status="unexpected_response_status",
        http_status=result.http_status,
        http_status_class=http_status_class(result.http_status),
        last_remote_error_code=result.error_code,
        local_insecure_tls=local_insecure_tls,
    )


def base_summary(
    *,
    endpoint: EndpointClassification,
    access_token_present: bool,
    connection_status: str,
    auth_status: str,
    server_state_status: str,
    http_status: int,
    http_status_class: str,
    last_remote_error_code: str,
    local_insecure_tls: bool,
) -> dict[str, Any]:
    return {
        "format": FORMAT,
        "redaction_policy": REDACTION_POLICY,
        "probe": "domain_state_read",
        "endpoint_status": endpoint.endpoint_status,
        "transport_mode": endpoint.transport_mode,
        "access_token_status": "configured" if access_token_present else "not_configured",
        "connection_status": connection_status,
        "auth_status": auth_status,
        "server_state_status": server_state_status,
        "http_status": http_status,
        "http_status_class": http_status_class,
        "last_remote_error_code": last_remote_error_code,
        "local_insecure_tls": "allowed" if local_insecure_tls else "system_trust",
    }


def http_status_class(status: int) -> str:
    if status <= 0:
        return "not_checked"
    if 200 <= status < 300:
        return "success"
    if 300 <= status < 400:
        return "redirect"
    if 400 <= status < 500:
        return "client_error"
    if 500 <= status < 600:
        return "server_error"
    return "unexpected_status"


def print_summary_text(summary: dict[str, Any]) -> None:
    print(FORMAT)
    for key in (
        "redaction_policy",
        "probe",
        "endpoint_status",
        "transport_mode",
        "access_token_status",
        "connection_status",
        "auth_status",
        "server_state_status",
        "http_status",
        "http_status_class",
        "last_remote_error_code",
        "local_insecure_tls",
    ):
        print(f"{key}: {summary[key]}")


def run_probe(args: argparse.Namespace) -> dict[str, Any]:
    endpoint_value = args.endpoint_option or args.endpoint or os.environ.get("RADISHLEX_SYNC_ENDPOINT", "")
    access_token = os.environ.get(args.access_token_env, "") if args.access_token_env else ""
    endpoint = classify_endpoint(endpoint_value)
    access_token_present = bool(access_token)
    configuration_summary = summary_for_configuration(
        endpoint=endpoint,
        access_token_present=access_token_present,
        local_insecure_tls=args.allow_local_insecure_tls,
    )
    if configuration_summary is not None:
        return configuration_summary
    if endpoint.parsed_url is None:
        raise ConnectionHealthError("endpoint parser returned no URL for a valid endpoint")

    result = probe_server_state(
        parsed_url=endpoint.parsed_url,
        access_token=access_token,
        probe_domain_id=args.probe_domain_id,
        timeout_seconds=args.timeout_seconds,
        allow_local_insecure_tls=args.allow_local_insecure_tls,
    )
    return summary_for_probe(
        endpoint=endpoint,
        access_token_present=access_token_present,
        result=result,
        local_insecure_tls=args.allow_local_insecure_tls,
    )


def assert_no_sensitive_output(summary: dict[str, Any], secret: str) -> None:
    rendered = json.dumps(summary, ensure_ascii=False, sort_keys=True)
    forbidden = [
        secret,
        "Authorization",
        "Bearer ",
        "payload_bytes",
    ]
    for marker in forbidden:
        if marker and marker in rendered:
            raise AssertionError(f"summary leaked forbidden marker: {marker}")


def run_self_test() -> None:
    userinfo = classify_endpoint("https://user:token@localhost:7319")
    assert userinfo.blocker == "server_endpoint_userinfo_forbidden"
    query = classify_endpoint("https://localhost:7319?token=secret")
    assert query.blocker == "server_endpoint_query_forbidden"
    external_http = classify_endpoint("http://sync.example.invalid")
    assert external_http.transport_mode == "remote_http"
    assert external_http.blocker == "remote_plain_http_forbidden"
    local_https = classify_endpoint("https://localhost:7319")
    assert local_https.transport_mode == "local_https"

    unauthorized = summary_for_probe(
        endpoint=local_https,
        access_token_present=False,
        result=ProbeResult(401, "unauthenticated", ""),
        local_insecure_tls=False,
    )
    assert unauthorized["auth_status"] == "required"
    accepted = summary_for_probe(
        endpoint=local_https,
        access_token_present=True,
        result=ProbeResult(404, "not_found", ""),
        local_insecure_tls=True,
    )
    assert accepted["auth_status"] == "accepted"
    assert accepted["server_state_status"] == "domain_missing_expected"
    assert_no_sensitive_output(accepted, "secret-token-value")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run a read-only sync server connection health probe and print a non-sensitive summary."
    )
    parser.add_argument("endpoint", nargs="?", help="Sync server endpoint. Env fallback: RADISHLEX_SYNC_ENDPOINT.")
    parser.add_argument("--endpoint", dest="endpoint_option", help="Sync server endpoint.")
    parser.add_argument(
        "--access-token-env",
        default="RADISHLEX_SYNC_ACCESS_TOKEN",
        help="Environment variable that contains the bearer token. Default: RADISHLEX_SYNC_ACCESS_TOKEN.",
    )
    parser.add_argument(
        "--probe-domain-id",
        default=DEFAULT_PROBE_DOMAIN_ID,
        help=f"Opaque domain id used for the read-only state probe. Default: {DEFAULT_PROBE_DOMAIN_ID}.",
    )
    parser.add_argument("--timeout-seconds", type=float, default=3.0, help="HTTP probe timeout. Default: 3.")
    parser.add_argument(
        "--allow-local-insecure-tls",
        action="store_true",
        help="Allow local HTTPS probes with self-signed or internal TLS certificates.",
    )
    parser.add_argument("--summary-json", action="store_true", help="Print JSON summary.")
    parser.add_argument("--summary-text", action="store_true", help="Print text summary. This is the default.")
    parser.add_argument("--self-test", action="store_true", help="Run offline classifier self-test.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        print("Sync server connection health self-test passed.")
        return 0

    try:
        summary = run_probe(args)
    except ConnectionHealthError as exc:
        print(f"connection health probe failed: {exc}", file=sys.stderr)
        return 1

    if args.summary_json:
        print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    else:
        print_summary_text(summary)

    return 0 if summary["connection_status"] == "reachable" else 1


if __name__ == "__main__":
    raise SystemExit(main())

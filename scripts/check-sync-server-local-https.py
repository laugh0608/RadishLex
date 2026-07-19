#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from secrets import token_hex, token_urlsafe
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
COMPOSE_FILE = REPO_ROOT / "deploy" / "sync-server" / "docker-compose.local.yaml"
CONNECTION_HEALTH = REPO_ROOT / "scripts" / "check-sync-server-connection-health.py"
FORMAT = "local_sync_https_smoke.v1"
DEFAULT_TIMEOUT_SECONDS = 90


class LocalHttpsSmokeError(RuntimeError):
    pass


def run_command(
    command: list[str],
    *,
    capture: bool = False,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            command,
            cwd=REPO_ROOT,
            env=env,
            text=True,
            capture_output=capture,
            check=False,
        )
    except FileNotFoundError as exc:
        raise LocalHttpsSmokeError(f"{command[0]} is required for local HTTPS smoke.") from exc


def run_required(
    command: list[str],
    *,
    capture: bool = False,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    result = run_command(command, capture=capture, env=env)
    if result.returncode != 0:
        raise LocalHttpsSmokeError(
            f"command failed with exit code {result.returncode}: {' '.join(command)}"
        )
    return result


def choose_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        try:
            sock.bind(("127.0.0.1", 0))
        except PermissionError:
            return 20000 + (int(token_hex(2), 16) % 20000)
        return int(sock.getsockname()[1])


def write_env(path: Path, project_name: str, port: int, access_token: str) -> None:
    path.write_text(
        "\n".join(
            (
                f"COMPOSE_PROJECT_NAME={project_name}",
                f"RADISHLEX_SYNC_PORT={port}",
                "RADISHLEX_SYNC_PUBLIC_HOST=localhost",
                f"RADISHLEX_SYNC_ACCESS_TOKEN={access_token}",
                "",
            )
        ),
        encoding="utf-8",
    )
    path.chmod(0o600)


def compose_command(project_name: str, env_path: Path, *args: str) -> list[str]:
    return [
        "docker",
        "compose",
        "-p",
        project_name,
        "-f",
        str(COMPOSE_FILE),
        "--env-file",
        str(env_path),
        *args,
    ]


def read_probe_summary(endpoint: str, access_token: str | None) -> dict[str, Any] | None:
    command = [
        sys.executable,
        str(CONNECTION_HEALTH),
        "--endpoint",
        endpoint,
        "--allow-local-insecure-tls",
        "--summary-json",
    ]
    env = os.environ.copy()
    if access_token is None:
        env.pop("RADISHLEX_SYNC_ACCESS_TOKEN", None)
    else:
        env["RADISHLEX_SYNC_ACCESS_TOKEN"] = access_token
    result = run_command(command, capture=True, env=env)
    try:
        decoded = json.loads(result.stdout)
    except json.JSONDecodeError:
        return None
    return decoded if isinstance(decoded, dict) else None


def probe_is_expected(
    summary: dict[str, Any] | None,
    *,
    auth_status: str,
    http_status: int,
    error_code: str,
) -> bool:
    return bool(
        summary
        and summary.get("format") == "sync_connection_health.v1"
        and summary.get("transport_mode") == "local_https"
        and summary.get("connection_status") == "reachable"
        and summary.get("auth_status") == auth_status
        and summary.get("http_status") == http_status
        and summary.get("last_remote_error_code") == error_code
        and summary.get("local_insecure_tls") == "allowed"
    )


def wait_for_https(endpoint: str, access_token: str, timeout_seconds: int) -> None:
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        unauthorized = read_probe_summary(endpoint, None)
        if probe_is_expected(
            unauthorized,
            auth_status="required",
            http_status=401,
            error_code="unauthenticated",
        ):
            authorized = read_probe_summary(endpoint, access_token)
            if probe_is_expected(
                authorized,
                auth_status="accepted",
                http_status=404,
                error_code="not_found",
            ):
                return
        time.sleep(1)
    raise LocalHttpsSmokeError("local HTTPS endpoint did not reach the expected bearer-gated state")


def inspect_container(project_name: str, env_path: Path, service: str) -> dict[str, Any]:
    container_id = run_required(
        compose_command(project_name, env_path, "ps", "-q", service),
        capture=True,
    ).stdout.strip()
    if not container_id:
        raise LocalHttpsSmokeError(f"local HTTPS service is not running: {service}")
    result = run_required(["docker", "inspect", container_id], capture=True)
    decoded = json.loads(result.stdout)
    if not isinstance(decoded, list) or len(decoded) != 1 or not isinstance(decoded[0], dict):
        raise LocalHttpsSmokeError(f"unexpected docker inspect result for {service}")
    return decoded[0]


def assert_container_hardening(project_name: str, env_path: Path) -> None:
    server = inspect_container(project_name, env_path, "sync-server")
    server_config = server.get("Config", {})
    server_host = server.get("HostConfig", {})
    if server_config.get("User") != "10001:10001":
        raise LocalHttpsSmokeError("sync-server container is not using the fixed non-root identity")
    if server_host.get("ReadonlyRootfs") is not True:
        raise LocalHttpsSmokeError("sync-server root filesystem is not read-only")
    if "ALL" not in (server_host.get("CapDrop") or []):
        raise LocalHttpsSmokeError("sync-server container did not drop all capabilities")
    if "no-new-privileges:true" not in (server_host.get("SecurityOpt") or []):
        raise LocalHttpsSmokeError("sync-server container is missing no-new-privileges")

    gateway = inspect_container(project_name, env_path, "sync-gateway")
    gateway_host = gateway.get("HostConfig", {})
    if gateway_host.get("ReadonlyRootfs") is not True:
        raise LocalHttpsSmokeError("sync-gateway root filesystem is not read-only")
    if "no-new-privileges:true" not in (gateway_host.get("SecurityOpt") or []):
        raise LocalHttpsSmokeError("sync-gateway container is missing no-new-privileges")
    bindings = gateway_host.get("PortBindings", {}).get("7319/tcp", [])
    if not bindings or any(binding.get("HostIp") != "127.0.0.1" for binding in bindings):
        raise LocalHttpsSmokeError("local HTTPS gateway is not restricted to loopback")


def assert_logs_redacted(project_name: str, env_path: Path, access_token: str) -> None:
    result = run_required(
        compose_command(project_name, env_path, "logs", "--no-color", "--tail=250"),
        capture=True,
    )
    logs = result.stdout + result.stderr
    for marker in (
        access_token,
        "Authorization:",
        "Bearer ",
        "encrypted_payload",
        "signature bytes",
        "wrapped material",
        "recovery material",
    ):
        if marker in logs:
            raise LocalHttpsSmokeError(f"local HTTPS logs contain forbidden marker: {marker!r}")


def print_diagnostics(project_name: str, env_path: Path, access_token: str) -> None:
    result = run_command(
        compose_command(project_name, env_path, "logs", "--no-color", "--tail=120"),
        capture=True,
    )
    output = (result.stdout + result.stderr).replace(access_token, "<redacted-access-token>").strip()
    if output:
        print(output, file=sys.stderr)


def remove_project(project_name: str, env_path: Path) -> None:
    result = run_command(
        compose_command(project_name, env_path, "down", "--volumes", "--remove-orphans"),
        capture=True,
    )
    if result.returncode != 0:
        raise LocalHttpsSmokeError("failed to remove the isolated local HTTPS compose project")
    containers = run_required(
        [
            "docker",
            "ps",
            "-aq",
            "--filter",
            f"label=com.docker.compose.project={project_name}",
        ],
        capture=True,
    ).stdout.strip()
    volumes = run_required(
        [
            "docker",
            "volume",
            "ls",
            "-q",
            "--filter",
            f"label=com.docker.compose.project={project_name}",
        ],
        capture=True,
    ).stdout.strip()
    if containers or volumes:
        raise LocalHttpsSmokeError("isolated local HTTPS compose resources were not fully removed")


def run_smoke(args: argparse.Namespace) -> int:
    project_name = f"radishlex-local-https-{token_hex(4)}"
    access_token = token_urlsafe(48)
    port = args.port if args.port is not None else choose_port()
    endpoint = f"https://localhost:{port}"

    with tempfile.TemporaryDirectory(prefix="radishlex-local-https.") as temp_dir:
        env_path = Path(temp_dir) / "local-https.env"
        write_env(env_path, project_name, port, access_token)
        started = False
        primary_error: Exception | None = None
        try:
            run_required(compose_command(project_name, env_path, "config"), capture=True)
            if args.config_only:
                print("Local HTTPS compose config passed.")
                return 0
            started = True
            run_required(compose_command(project_name, env_path, "up", "--build", "-d"))
            wait_for_https(endpoint, access_token, args.timeout_seconds)
            assert_container_hardening(project_name, env_path)
            assert_logs_redacted(project_name, env_path, access_token)
        except Exception as exc:  # noqa: BLE001
            primary_error = exc
            if started:
                print_diagnostics(project_name, env_path, access_token)
        finally:
            if started:
                try:
                    remove_project(project_name, env_path)
                except Exception as cleanup_exc:  # noqa: BLE001
                    if primary_error is None:
                        primary_error = cleanup_exc
                    else:
                        primary_error = LocalHttpsSmokeError(
                            "local HTTPS smoke failed; isolated project cleanup also failed"
                        )
        if primary_error is not None:
            raise primary_error

    print(FORMAT)
    print("transport: local_https")
    print("tls_handshake: passed")
    print("bearer_unauthenticated_401: passed")
    print("bearer_authorized_backend_response: passed")
    print("loopback_only: passed")
    print("container_hardening: passed")
    print("log_redaction: passed")
    print("isolated_cleanup: passed")
    return 0


def run_self_test() -> None:
    accepted = {
        "format": "sync_connection_health.v1",
        "transport_mode": "local_https",
        "connection_status": "reachable",
        "auth_status": "accepted",
        "http_status": 404,
        "last_remote_error_code": "not_found",
        "local_insecure_tls": "allowed",
    }
    assert probe_is_expected(
        accepted,
        auth_status="accepted",
        http_status=404,
        error_code="not_found",
    )
    rejected = dict(accepted, transport_mode="external_https")
    assert not probe_is_expected(
        rejected,
        auth_status="accepted",
        http_status=404,
        error_code="not_found",
    )
    compose = COMPOSE_FILE.read_text(encoding="utf-8")
    for contract in (
        '"127.0.0.1:${RADISHLEX_SYNC_PORT:-7319}:7319"',
        'RADISHLEX_SYNC_ACCESS_TOKEN: "${RADISHLEX_SYNC_ACCESS_TOKEN:-}"',
        'RADISHLEX_SYNC_PORT: "7319"',
        "read_only: true",
        "cap_drop:\n      - ALL",
        "no-new-privileges:true",
    ):
        assert contract in compose


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run an isolated local Compose + Caddy internal TLS sync server smoke."
    )
    parser.add_argument("--config-only", action="store_true", help="Only validate the isolated Compose config.")
    parser.add_argument("--self-test", action="store_true", help="Run offline contract self-tests.")
    parser.add_argument("--port", type=int, help="Loopback port. Default: choose an available port.")
    parser.add_argument(
        "--timeout-seconds",
        type=int,
        default=DEFAULT_TIMEOUT_SECONDS,
        help=f"Seconds to wait for local HTTPS readiness. Default: {DEFAULT_TIMEOUT_SECONDS}.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        run_self_test()
        print("Local HTTPS smoke self-test passed.")
        return 0
    try:
        return run_smoke(args)
    except (LocalHttpsSmokeError, json.JSONDecodeError) as exc:
        print(f"local HTTPS smoke failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

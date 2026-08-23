#!/usr/bin/env python3
from __future__ import annotations

import base64
import hashlib
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Protocol

import l6_utm_canonical_input_bindings as transfer_bindings
import l6_utm_guest_network_ready as network_ready
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer
import l6_v4_canonical_input_resolution_probe as resolution_probe


EVIDENCE_FORMAT = "radishlex-linux-l6-utm-canonical-input-resolution-v1"
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_canonical_input_resolution.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_canonical_input_resolution_bindings.py"
)
PROBE_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_v4_canonical_input_resolution_probe.py"
)
EVIDENCE_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_canonical_input_resolution_evidence.py"
)
REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256 = (
    "d1090e8d4fd31e0793bf382e8bdc718aa42fdaba38213efe71c5595e8926dc09"
)
REQUIRED_PRIOR_TRANSFER_REPOSITORY_HEAD = (
    "6a23c377ccf2862ec504576a1a6ba3bfa4ca6298"
)
REQUIRED_TRANSFER_ATTEMPT_ID = "d75818f-v4-input-20260823-v1"
REQUIRED_SOURCE_BUNDLE_SIZE = 92825600
REQUIRED_SOURCE_BUNDLE_SHA256 = (
    "7bbeb2915ee737ffae162608e735bf6d160de6e149317a42dcccab3462c2403c"
)
REQUIRED_FIRST_RESULT_STDERR_SIZE = 234
REQUIRED_FIRST_RESULT_STDERR_SHA256 = (
    "b734f75c39fbdc6a4251c2c081706822b3552e202e79021920c2bd7ac6b33de2"
)
REQUIRED_TRANSFER_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "target-handles-preflight.json",
    "network-evidence-live-readback-1.json",
    "network-evidence-live-readback-2.json",
    "guest-root-create.json",
    "guest-installer-push.json",
    "guest-installer-normalize.json",
    "guest-installer-readback.json",
    "source-bundle-push.json",
    "source-bundle-readback-1.json",
    "source-bundle-readback-2.json",
    "guest-installer.json",
    "guest-transfer-evidence-readback-1.json",
    "source-bundle-postflight.json",
    "terminal.json",
)
HEX_64 = re.compile(r"[0-9a-f]{64}")


class ResolutionBindingRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    prior_network_root: Path
    prior_network_manifest_sha256: str
    prior_transfer_root: Path
    prior_transfer_manifest_sha256: str
    source_bundle_path: Path
    source_bundle_size: int
    source_bundle_sha256: str
    transfer_attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path


@dataclass(frozen=True)
class ResolutionBinding:
    evidence: dict[str, object]
    network: transfer_bindings.NetworkBinding
    installer_bytes: bytes
    probe_bytes: bytes


NetworkValidator = Callable[
    [ResolutionBindingRequest], transfer_bindings.NetworkBinding
]


def validate_resolution_bindings(
    request: ResolutionBindingRequest,
    *,
    network_validator: NetworkValidator | None = None,
) -> ResolutionBinding:
    validate_network = network_validator or transfer_bindings.validate_transfer_bindings
    network = validate_network(request)
    repository_head = start_control._run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if repository_head != request.expected_repository_head:
        raise ValueError("repository-head-drift")
    if start_control._run_git(
        request.repository_root, ("status", "--porcelain")
    ):
        raise ValueError("repository-not-clean")

    identities: dict[str, object] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "control"),
        (BINDINGS_RELATIVE_PATH, "bindings"),
        (PROBE_RELATIVE_PATH, "probe"),
        (EVIDENCE_RELATIVE_PATH, "evidence"),
        (transfer_bindings.CONTROL_RELATIVE_PATH, "transfer_control"),
        (transfer_bindings.BINDINGS_RELATIVE_PATH, "transfer_bindings"),
        (transfer_bindings.GUEST_INSTALLER_RELATIVE_PATH, "guest_installer"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)

    manifest = request.prior_transfer_root / "files.sha256"
    if network_ready._sha256_file(manifest) != request.prior_transfer_manifest_sha256:
        raise ValueError("prior-transfer-manifest-drift")
    entries = start_control._verify_sha256_manifest(
        request.prior_transfer_root, manifest
    )
    if entries != len(REQUIRED_TRANSFER_ENTRY_NAMES):
        raise ValueError("prior-transfer-entry-count-invalid")
    if _manifest_names(manifest) != REQUIRED_TRANSFER_ENTRY_NAMES:
        raise ValueError("prior-transfer-entry-names-invalid")

    prior_request = network_ready._read_json(
        request.prior_transfer_root / "request.json"
    )
    expected_authorization = {
        "canonical_input_transfer": True,
        "exact_source_bundle_identity": True,
        "no_operation_transaction_stop_or_retry": True,
        "private_staging_and_atomic_switch": True,
    }
    expected_request_keys = {
        "attempt_id",
        "authorization",
        "command_timeout_seconds",
        "expected_repository_head",
        "format",
        "guest_final_input_root",
        "guest_root",
        "prior_network_manifest_sha256",
        "source_bundle_path_sha256",
        "source_bundle_sha256",
        "source_bundle_size",
        "target_name",
        "target_package_path_sha256",
        "target_uuid",
        "transfer_timeout_seconds",
    }
    if (
        set(prior_request) != expected_request_keys
        or prior_request.get("format") != transfer_bindings.EVIDENCE_FORMAT
        or prior_request.get("expected_repository_head")
        != REQUIRED_PRIOR_TRANSFER_REPOSITORY_HEAD
        or prior_request.get("attempt_id") != request.transfer_attempt_id
        or prior_request.get("authorization") != expected_authorization
        or prior_request.get("prior_network_manifest_sha256")
        != request.prior_network_manifest_sha256
        or prior_request.get("source_bundle_size") != request.source_bundle_size
        or prior_request.get("source_bundle_sha256")
        != request.source_bundle_sha256
        or prior_request.get("source_bundle_path_sha256")
        != _sha256_text(str(request.source_bundle_path))
        or prior_request.get("target_uuid") != request.target_uuid
        or prior_request.get("target_name") != request.target_name
        or prior_request.get("target_package_path_sha256")
        != _sha256_text(str(request.target_package_path))
        or prior_request.get("command_timeout_seconds") != 60
        or prior_request.get("transfer_timeout_seconds") != 600
    ):
        raise ValueError("prior-transfer-request-semantics-invalid")
    guest_root = prior_request.get("guest_root")
    if guest_root != (
        "/var/tmp/radishlex-l6-v4-input-transfer-"
        f"{request.transfer_attempt_id}"
    ) or prior_request.get("guest_final_input_root") != str(
        guest_installer.FINAL_INPUT_ROOT
    ):
        raise ValueError("prior-transfer-guest-paths-invalid")

    prior_terminal = network_ready._read_json(
        request.prior_transfer_root / "terminal.json"
    )
    required_terminal = {
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "bundle_push_invocations": 1,
        "bundle_readback_invocations": 2,
        "file_pull_invocations": 6,
        "file_push_invocations": 2,
        "format": transfer_bindings.EVIDENCE_FORMAT,
        "guest_exec_invocations": 3,
        "guest_installer_invocations": 1,
        "guest_terminal_outcome": None,
        "input_root": str(guest_installer.FINAL_INPUT_ROOT),
        "operation_id": "not-generated",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": (
            "guest-transfer-evidence-readback-1:"
            "guest-transfer-evidence-readback-1-stderr-not-empty"
        ),
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
    }
    if prior_terminal != required_terminal:
        raise ValueError("prior-transfer-terminal-semantics-invalid")

    installer_bytes = (
        request.repository_root / transfer_bindings.GUEST_INSTALLER_RELATIVE_PATH
    ).read_bytes()
    installer_readback = network_ready._read_json(
        request.prior_transfer_root / "guest-installer-readback.json"
    )
    if installer_readback.get("argv") != [
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        f"{guest_root}/install-canonical-input.py",
    ] or _successful_observation_payload(installer_readback) != installer_bytes:
        raise ValueError("prior-transfer-installer-readback-drift")
    installer_observation = network_ready._read_json(
        request.prior_transfer_root / "guest-installer.json"
    )
    expected_installer_argv = [
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        f"{guest_root}/install-canonical-input.py",
        "--attempt-id",
        request.transfer_attempt_id,
        "--transfer-root",
        guest_root,
        "--bundle-path",
        f"{guest_root}/canonical-input.ustar.incoming",
        "--expected-bundle-size",
        str(request.source_bundle_size),
        "--expected-bundle-sha256",
        request.source_bundle_sha256,
    ]
    if not _observation_is_empty_success(
        installer_observation, expected_argv=expected_installer_argv
    ):
        raise ValueError("prior-transfer-installer-observation-invalid")

    first_result = network_ready._read_json(
        request.prior_transfer_root
        / "guest-transfer-evidence-readback-1.json"
    )
    _validate_missing_first_result(
        first_result,
        expected_argv=[
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            f"{guest_root}/transfer.evidence.json",
        ],
    )
    for index in (1, 2):
        _validate_bundle_readback(
            network_ready._read_json(
                request.prior_transfer_root
                / f"source-bundle-readback-{index}.json"
            ),
            request,
            expected_argv=[
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                f"{guest_root}/canonical-input.ustar.incoming",
            ],
        )
    source_postflight = network_ready._read_json(
        request.prior_transfer_root / "source-bundle-postflight.json"
    )
    if source_postflight != {
        "descriptor_unchanged": True,
        "format": transfer_bindings.EVIDENCE_FORMAT,
        "inventory_unchanged": True,
        "sha256": request.source_bundle_sha256,
        "size": request.source_bundle_size,
    }:
        raise ValueError("prior-transfer-source-postflight-invalid")

    probe_path = request.repository_root / PROBE_RELATIVE_PATH
    probe_bytes = probe_path.read_bytes()
    if not probe_bytes or len(probe_bytes) > resolution_probe.MAX_SMALL_FILE_BYTES:
        raise ValueError("resolution-probe-size-invalid")
    return ResolutionBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "prior_network_manifest_sha256": request.prior_network_manifest_sha256,
            "prior_transfer_entries_verified": entries,
            "prior_transfer_manifest_sha256": (
                request.prior_transfer_manifest_sha256
            ),
            "prior_transfer_outcome": "state-indeterminate",
            "prior_transfer_reason": required_terminal["reason"],
            "repository_clean": True,
            "repository_head": repository_head,
            "source_bundle_sha256": request.source_bundle_sha256,
            "source_bundle_size": request.source_bundle_size,
            "transfer_attempt_id": request.transfer_attempt_id,
        },
        network=network,
        installer_bytes=installer_bytes,
        probe_bytes=probe_bytes,
    )


def _manifest_names(path: Path) -> tuple[str, ...]:
    names: list[str] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        parts = line.split("  ", 1)
        if len(parts) != 2 or not HEX_64.fullmatch(parts[0]):
            raise ValueError("prior-transfer-manifest-format-invalid")
        names.append(parts[1])
    return tuple(names)


def _successful_observation_payload(value: dict[str, object]) -> bytes:
    stdout = value.get("stdout")
    stderr = value.get("stderr")
    if not isinstance(stdout, dict) or not isinstance(stderr, dict):
        raise ValueError("prior-transfer-observation-shape-invalid")
    try:
        payload = base64.b64decode(
            str(stdout["prefix_base64"]), validate=True
        )
    except (KeyError, ValueError) as exc:
        raise ValueError("prior-transfer-observation-base64-invalid") from exc
    if (
        value.get("exit_code") != 0
        or value.get("timed_out") is not False
        or stdout.get("truncated") is not False
        or stdout.get("total_bytes") != len(payload)
        or stdout.get("sha256") != hashlib.sha256(payload).hexdigest()
        or stderr.get("total_bytes") != 0
        or stderr.get("truncated") is not False
    ):
        raise ValueError("prior-transfer-observation-metadata-invalid")
    return payload


def _observation_is_empty_success(
    value: dict[str, object], *, expected_argv: list[str]
) -> bool:
    stdout = value.get("stdout")
    stderr = value.get("stderr")
    empty_sha = hashlib.sha256(b"").hexdigest()
    return (
        value.get("argv") == expected_argv
        and value.get("exit_code") == 0
        and value.get("timed_out") is False
        and isinstance(stdout, dict)
        and isinstance(stderr, dict)
        and stdout.get("total_bytes") == 0
        and stdout.get("sha256") == empty_sha
        and stdout.get("truncated") is False
        and stderr.get("total_bytes") == 0
        and stderr.get("sha256") == empty_sha
        and stderr.get("truncated") is False
    )


def _validate_missing_first_result(
    value: dict[str, object], *, expected_argv: list[str]
) -> None:
    stdout = value.get("stdout")
    stderr = value.get("stderr")
    if (
        value.get("argv") != expected_argv
        or value.get("exit_code") != 0
        or value.get("timed_out") is not False
        or not isinstance(stdout, dict)
        or not isinstance(stderr, dict)
        or stdout.get("total_bytes") != 0
        or stdout.get("sha256") != hashlib.sha256(b"").hexdigest()
        or stdout.get("truncated") is not False
        or stderr.get("total_bytes") != REQUIRED_FIRST_RESULT_STDERR_SIZE
        or stderr.get("sha256") != REQUIRED_FIRST_RESULT_STDERR_SHA256
        or stderr.get("truncated") is not False
    ):
        raise ValueError("prior-transfer-first-result-readback-invalid")


def _validate_bundle_readback(
    value: dict[str, object],
    request: ResolutionBindingRequest,
    *,
    expected_argv: list[str],
) -> None:
    observation = value.get("observation")
    if value.get("format") != transfer_bindings.EVIDENCE_FORMAT or not isinstance(
        observation, dict
    ):
        raise ValueError("prior-transfer-bundle-readback-shape-invalid")
    stdout = observation.get("stdout")
    stderr = observation.get("stderr")
    if (
        observation.get("argv") != expected_argv
        or not isinstance(stdout, dict)
        or not isinstance(stderr, dict)
        or observation.get("exit_code") != 0
        or observation.get("timed_out") is not False
        or stdout.get("total_bytes") != request.source_bundle_size
        or stdout.get("sha256") != request.source_bundle_sha256
        or stdout.get("truncated") is not True
        or stderr.get("total_bytes") != 0
        or stderr.get("truncated") is not False
    ):
        raise ValueError("prior-transfer-bundle-readback-invalid")


def _sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()

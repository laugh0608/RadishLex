#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_resume_bindings as resume_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-backend-resolution-v1"
)
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_backend_resolution.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_backend_resolution_bindings.py"
)
REQUIRED_PRIOR_FAILURE_MANIFEST_SHA256 = (
    "a436677c28a65e6abb995f0d038086e09ccec6edb3a0c5f510d5ff192638adc7"
)
REQUIRED_PRIOR_FAILURE_REPOSITORY_HEAD = (
    "7309313d97a5dd579e254b88bb34feba0af00de7"
)
REQUIRED_BACKEND_RESOLUTION_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-backend-resolution-20260824-v1"
)
REQUIRED_PRIOR_FAILURE_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class BackendResolutionBindingRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    prior_network_root: Path
    prior_network_manifest_sha256: str
    prior_transfer_root: Path
    prior_transfer_manifest_sha256: str
    prior_resolution_root: Path
    prior_resolution_manifest_sha256: str
    prior_preflight_root: Path
    prior_preflight_manifest_sha256: str
    prior_checkpoint_root: Path
    prior_checkpoint_manifest_sha256: str
    prior_resume_failure_root: Path
    prior_resume_failure_manifest_sha256: str
    source_bundle_path: Path
    source_bundle_size: int
    source_bundle_sha256: str
    transfer_attempt_id: str
    resolution_attempt_id: str
    preflight_attempt_id: str
    checkpoint_attempt_id: str
    resume_attempt_id: str
    backend_resolution_attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path


@dataclass(frozen=True)
class BackendResolutionBinding:
    evidence: dict[str, object]
    resume: resume_bindings.ResumeBinding


def validate_backend_resolution_bindings(
    request: BackendResolutionBindingRequest,
) -> BackendResolutionBinding:
    upstream = resume_bindings.validate_resume_bindings(request)
    identities: dict[str, object] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "backend_resolution_control"),
        (BINDINGS_RELATIVE_PATH, "backend_resolution_bindings"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)

    manifest = request.prior_resume_failure_root / "files.sha256"
    if (
        request.prior_resume_failure_manifest_sha256
        != REQUIRED_PRIOR_FAILURE_MANIFEST_SHA256
        or network_ready._sha256_file(manifest)
        != REQUIRED_PRIOR_FAILURE_MANIFEST_SHA256
    ):
        raise ValueError("prior-resume-failure-manifest-identity-invalid")
    entries = start_control._verify_sha256_manifest(
        request.prior_resume_failure_root, manifest
    )
    if entries != len(REQUIRED_PRIOR_FAILURE_ENTRY_NAMES):
        raise ValueError("prior-resume-failure-entry-count-invalid")
    _validate_prior_failure_semantics(request)

    return BackendResolutionBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "backend_resolution_attempt_id": (
                request.backend_resolution_attempt_id
            ),
            "prior_resume_failure_entries_verified": entries,
            "prior_resume_failure_manifest_sha256": (
                request.prior_resume_failure_manifest_sha256
            ),
            "prior_resume_failure_outcome": "precondition-rejected",
            "prior_resume_failure_reason": (
                "target-handles-preflight:target-handles-preflight-exit-1"
            ),
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "resume_attempt_id": request.resume_attempt_id,
            "source_bundle_sha256": request.source_bundle_sha256,
            "source_bundle_size": request.source_bundle_size,
            "target_uuid": request.target_uuid,
        },
        resume=upstream,
    )


def _validate_prior_failure_semantics(
    request: BackendResolutionBindingRequest,
) -> None:
    root = request.prior_resume_failure_root
    manifest = root / "files.sha256"
    if _manifest_names(manifest) != REQUIRED_PRIOR_FAILURE_ENTRY_NAMES:
        raise ValueError("prior-resume-failure-entry-names-invalid")

    prior_request = network_ready._read_json(root / "request.json")
    required_request = {
        "authorization": {
            "existing_operation_secret_hash_only": True,
            "install_artifacts_staged_exact_resume": True,
            "no_retry_cleanup_stop_quit_or_next_checkpoint": True,
            "one_resume_and_one_postflight": True,
        },
        "checkpoint_attempt_id": request.checkpoint_attempt_id,
        "command_timeout_seconds": 60,
        "evidence_settle_seconds": 10,
        "expected_repository_head": REQUIRED_PRIOR_FAILURE_REPOSITORY_HEAD,
        "format": resume_bindings.EVIDENCE_FORMAT,
        "guest_resume_root": (
            "/var/tmp/radishlex-l6-v4-install-artifacts-staged-resume-"
            f"{request.resume_attempt_id}"
        ),
        "prior_checkpoint_manifest_sha256": (
            request.prior_checkpoint_manifest_sha256
        ),
        "prior_network_manifest_sha256": request.prior_network_manifest_sha256,
        "prior_preflight_manifest_sha256": (
            request.prior_preflight_manifest_sha256
        ),
        "prior_resolution_manifest_sha256": (
            request.prior_resolution_manifest_sha256
        ),
        "prior_transfer_manifest_sha256": (
            request.prior_transfer_manifest_sha256
        ),
        "resume_attempt_id": request.resume_attempt_id,
        "resume_timeout_seconds": 1500,
        "source_bundle_path_sha256": network_ready._sha256_text(
            str(request.source_bundle_path)
        ),
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_package_path_sha256": network_ready._sha256_text(
            str(request.target_package_path)
        ),
        "target_uuid": request.target_uuid,
    }
    if prior_request != required_request:
        raise ValueError("prior-resume-failure-request-semantics-invalid")

    prior_binding = network_ready._read_json(root / "binding-preflight.json")
    required_binding = {
        "bindings_sha256": (
            "747dacb2d8748496815617825307bd4726584d8b37e86bc4eeaf2bb5ca936663"
        ),
        "checkpoint_attempt_id": request.checkpoint_attempt_id,
        "control_sha256": (
            "280ca0d78a0276a54dcf61a733c35ccd854587336dc95b4844f745698001f3a8"
        ),
        "driver_sha256": (
            "0b649ecd7b90a8368cf40a3887660cade5fa90cf9119bd7baba0904bcad3d05b"
        ),
        "evidence_sha256": (
            "c65798326e7548ea08ffd5e046c6929fb3fb212a0efb5e93c2ef7b02fa6958f1"
        ),
        "format": resume_bindings.EVIDENCE_FORMAT,
        "prior_checkpoint_entries_verified": 49,
        "prior_checkpoint_manifest_sha256": (
            request.prior_checkpoint_manifest_sha256
        ),
        "prior_checkpoint_outcome": "checkpoint-prepared",
        "repository_clean": True,
        "repository_head": REQUIRED_PRIOR_FAILURE_REPOSITORY_HEAD,
        "resume_attempt_id": request.resume_attempt_id,
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_uuid": request.target_uuid,
    }
    if prior_binding != required_binding:
        raise ValueError("prior-resume-failure-binding-semantics-invalid")

    source_preflight = network_ready._read_json(
        root / "source-bundle-preflight.json"
    )
    descriptor = source_preflight.get("descriptor")
    inventory = source_preflight.get("inventory")
    if (
        source_preflight.get("format")
        != "radishlex-linux-l6-utm-canonical-input-transfer-v1"
        or source_preflight.get("inventory_count") != 12
        or not isinstance(descriptor, dict)
        or descriptor.get("mode") != "0600"
        or descriptor.get("owner_uid") != 501
        or descriptor.get("owner_gid") != 20
        or descriptor.get("sha256") != request.source_bundle_sha256
        or descriptor.get("size") != request.source_bundle_size
        or not isinstance(inventory, list)
        or len(inventory) != 12
    ):
        raise ValueError("prior-resume-failure-source-preflight-invalid")

    target = network_ready._read_json(root / "target-files-preflight.json")
    descriptors = target.get("target_descriptors")
    if (
        target.get("format")
        != "radishlex-linux-l6-utm-guest-network-ready-v1"
        or target.get("target_config_sha256")
        != network_ready.REQUIRED_TARGET_CONFIG_SHA256
        or target.get("target_package_name") != request.target_package_path.name
        or target.get("target_package_path_sha256")
        != network_ready._sha256_text(str(request.target_package_path))
        or not isinstance(descriptors, dict)
        or set(descriptors) != {"config", "efi", "qcow2"}
    ):
        raise ValueError("prior-resume-failure-target-preflight-invalid")

    host = network_ready._read_json(root / "host-process-preflight.json")
    observation = host.get("observation")
    if (
        host.get("format") != resume_bindings.EVIDENCE_FORMAT
        or host.get("relevant_process_count") != 0
        or not isinstance(observation, dict)
        or observation.get("argv") != list(launch_transport.PROCESS_COMMAND)
        or observation.get("exit_code") != 0
        or observation.get("timed_out") is not False
        or not _empty_observation_stream(observation.get("stderr"))
        or not _complete_observation_stream(observation.get("stdout"))
    ):
        raise ValueError("prior-resume-failure-host-preflight-invalid")

    source_postflight = network_ready._read_json(
        root / "source-bundle-postflight.json"
    )
    if source_postflight != {
        "descriptor_unchanged": True,
        "format": "radishlex-linux-l6-utm-canonical-input-transfer-v1",
        "inventory_unchanged": True,
        "sha256": request.source_bundle_sha256,
        "size": request.source_bundle_size,
    }:
        raise ValueError("prior-resume-failure-source-postflight-invalid")

    terminal = network_ready._read_json(root / "terminal.json")
    required_terminal = {
        "automatic_cleanup": "not-performed",
        "automatic_next_checkpoint": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "checkpoint_attempt_id": request.checkpoint_attempt_id,
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "format": resume_bindings.EVIDENCE_FORMAT,
        "guest_exec_invocations": 0,
        "guest_resume_outcome": None,
        "maintenance_resume_invocations": 0,
        "operation_id": "existing-secret-hash-only",
        "operation_id_sha256": resume_bindings.REQUIRED_OPERATION_ID_SHA256,
        "outcome": "precondition-rejected",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "postflight_invocations": 0,
        "reason": "target-handles-preflight:target-handles-preflight-exit-1",
        "resume_attempt_id": request.resume_attempt_id,
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "artifacts-staged-preserved",
    }
    if terminal != required_terminal:
        raise ValueError("prior-resume-failure-terminal-semantics-invalid")


def _empty_observation_stream(value: object) -> bool:
    return (
        isinstance(value, dict)
        and value.get("sha256")
        == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        and value.get("total_bytes") == 0
        and value.get("truncated") is False
    )


def _complete_observation_stream(value: object) -> bool:
    return (
        isinstance(value, dict)
        and value.get("sha256")
        == "e9988018390d125437cfc1e9b83beb604a6748c587e0028e9b308aa9f94ce07c"
        and value.get("total_bytes") == 31605
        and value.get("truncated") is False
    )


def _manifest_names(path: Path) -> tuple[str, ...]:
    names: list[str] = []
    try:
        lines = path.read_text(encoding="ascii").splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        raise ValueError("prior-resume-failure-manifest-unreadable") from exc
    for line in lines:
        parts = line.split("  ", 1)
        if len(parts) != 2:
            raise ValueError("prior-resume-failure-manifest-line-invalid")
        names.append(parts[1])
    return tuple(names)

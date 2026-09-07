#!/usr/bin/env python3
from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_canonical_input_bindings as transfer_bindings
import l6_utm_install_artifacts_staged_checkpoint as checkpoint_control
import l6_utm_install_artifacts_staged_checkpoint_bindings as checkpoint_bindings
import l6_utm_install_artifacts_staged_checkpoint_evidence as checkpoint_evidence
import l6_utm_guest_network_ready as network_ready
import l6_utm_start_once as start_control
import l6_v4_install_artifacts_staged_resume_driver as resume_driver


EVIDENCE_FORMAT = "radishlex-linux-l6-utm-install-artifacts-staged-resume-v1"
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_install_artifacts_staged_resume.py"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_install_artifacts_staged_resume_bindings.py"
)
DRIVER_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_v4_install_artifacts_staged_resume_driver.py"
)
EVIDENCE_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_install_artifacts_staged_resume_evidence.py"
)
REQUIRED_PRIOR_CHECKPOINT_MANIFEST_SHA256 = (
    "3aca0576eb91ae2916374d1f175d98276758ccf594f2e48152eb7814ec60a4f7"
)
REQUIRED_PRIOR_CHECKPOINT_REPOSITORY_HEAD = (
    "7bf6e04b829e5b3931cc14987487993239958e18"
)
REQUIRED_RESUME_ATTEMPT_ID = resume_driver.EXPECTED_RESUME_ATTEMPT_ID
REQUIRED_CHECKPOINT_ATTEMPT_ID = resume_driver.EXPECTED_CHECKPOINT_ATTEMPT_ID
REQUIRED_OPERATION_ID_SHA256 = resume_driver.EXPECTED_OPERATION_ID_SHA256
REQUIRED_CHECKPOINT_SHA256 = resume_driver.EXPECTED_CHECKPOINT_SHA256
REQUIRED_CRASH_STATE_SHA256 = resume_driver.EXPECTED_CRASH_STATE_SHA256
REQUIRED_RECEIPT_SHA256 = resume_driver.EXPECTED_RECEIPT_SHA256
REQUIRED_CHECKPOINT_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "target-handles-preflight.json",
    "network-evidence-live-readback-1.json",
    "network-evidence-live-readback-2.json",
    "negative-preflight-evidence-live-readback-1.json",
    "negative-preflight-evidence-live-readback-2.json",
    "guest-checkpoint-root-create.json",
    "guest-checkpoint-driver-push.json",
    "guest-checkpoint-driver-chown.json",
    "guest-checkpoint-driver-chmod.json",
    "guest-checkpoint-driver-publish.json",
    "guest-checkpoint-driver-readback.json",
    "guest-install-artifacts-staged-checkpoint.json",
    "guest-checkpoint-marker-readback.json",
    "guest-checkpoint-evidence-readback-1.json",
    "guest-checkpoint-evidence-readback-2.json",
    "guest-checkpoint-evidence.json",
    "guest-checkpoint-phase-readback.json",
    "guest-identity-readback-1.json",
    "guest-identity-readback-2.json",
    "transfer-setup-readback-1.json",
    "transfer-setup-readback-2.json",
    "mutation-preflight-readback-1.json",
    "mutation-preflight-readback-2.json",
    "crash-result-readback-1.json",
    "crash-result-readback-2.json",
    "checkpoint-readback-1.json",
    "checkpoint-readback-2.json",
    "crash-state-readback-1.json",
    "crash-state-readback-2.json",
    "case-preflight-stdout-readback-1.json",
    "case-preflight-stdout-readback-2.json",
    "case-preflight-stderr-readback-1.json",
    "case-preflight-stderr-readback-2.json",
    "case-crash-stdout-readback-1.json",
    "case-crash-stdout-readback-2.json",
    "case-crash-stderr-readback-1.json",
    "case-crash-stderr-readback-2.json",
    "case-inspect-crash-stdout-readback-1.json",
    "case-inspect-crash-stdout-readback-2.json",
    "case-inspect-crash-stderr-readback-1.json",
    "case-inspect-crash-stderr-readback-2.json",
    "network-evidence-postflight-readback.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class ResumeBindingRequest(Protocol):
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
    source_bundle_path: Path
    source_bundle_size: int
    source_bundle_sha256: str
    transfer_attempt_id: str
    resolution_attempt_id: str
    preflight_attempt_id: str
    checkpoint_attempt_id: str
    resume_attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path


@dataclass(frozen=True)
class ResumeBinding:
    evidence: dict[str, object]
    network: transfer_bindings.NetworkBinding
    driver_bytes: bytes
    boot_id_sha256: str
    operation_id_sha256: str
    checkpoint_sha256: str
    crash_state_sha256: str
    receipt_sha256: str
    live_artifacts: dict[str, tuple[str, bytes]]


def validate_resume_bindings(request: ResumeBindingRequest) -> ResumeBinding:
    upstream = checkpoint_bindings.validate_checkpoint_bindings(request)
    identities: dict[str, object] = {}
    for relative_path, label in (
        (CONTROL_RELATIVE_PATH, "control"),
        (BINDINGS_RELATIVE_PATH, "bindings"),
        (EVIDENCE_RELATIVE_PATH, "evidence"),
        (DRIVER_RELATIVE_PATH, "driver"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)

    manifest = request.prior_checkpoint_root / "files.sha256"
    if (
        request.prior_checkpoint_manifest_sha256
        != REQUIRED_PRIOR_CHECKPOINT_MANIFEST_SHA256
        or network_ready._sha256_file(manifest)
        != REQUIRED_PRIOR_CHECKPOINT_MANIFEST_SHA256
    ):
        raise ValueError("prior-checkpoint-manifest-identity-invalid")
    entries = start_control._verify_sha256_manifest(
        request.prior_checkpoint_root, manifest
    )
    if entries != len(REQUIRED_CHECKPOINT_ENTRY_NAMES):
        raise ValueError("prior-checkpoint-entry-count-invalid")
    if _manifest_names(manifest) != REQUIRED_CHECKPOINT_ENTRY_NAMES:
        raise ValueError("prior-checkpoint-entry-names-invalid")

    prior_request = network_ready._read_json(
        request.prior_checkpoint_root / "request.json"
    )
    expected_authorization = {
        "generate_one_operation_id": True,
        "install_artifacts_staged_checkpoint": True,
        "no_resume_retry_cleanup_stop_or_quit": True,
        "one_acceptance_checkpoint_and_process_group_termination": True,
    }
    if (
        prior_request.get("format") != checkpoint_control.EVIDENCE_FORMAT
        or prior_request.get("expected_repository_head")
        != REQUIRED_PRIOR_CHECKPOINT_REPOSITORY_HEAD
        or prior_request.get("authorization") != expected_authorization
        or prior_request.get("checkpoint_attempt_id")
        != request.checkpoint_attempt_id
        or prior_request.get("preflight_attempt_id") != request.preflight_attempt_id
        or prior_request.get("resolution_attempt_id")
        != request.resolution_attempt_id
        or prior_request.get("transfer_attempt_id") != request.transfer_attempt_id
        or prior_request.get("prior_network_manifest_sha256")
        != request.prior_network_manifest_sha256
        or prior_request.get("prior_preflight_manifest_sha256")
        != request.prior_preflight_manifest_sha256
        or prior_request.get("prior_resolution_manifest_sha256")
        != request.prior_resolution_manifest_sha256
        or prior_request.get("prior_transfer_manifest_sha256")
        != request.prior_transfer_manifest_sha256
        or prior_request.get("source_bundle_size") != request.source_bundle_size
        or prior_request.get("source_bundle_sha256") != request.source_bundle_sha256
        or prior_request.get("target_uuid") != request.target_uuid
        or prior_request.get("target_name") != request.target_name
    ):
        raise ValueError("prior-checkpoint-request-semantics-invalid")

    prior_terminal = network_ready._read_json(
        request.prior_checkpoint_root / "terminal.json"
    )
    required_terminal = {
        "acceptance_invocations": 1,
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "checkpoint_attempt_id": request.checkpoint_attempt_id,
        "checkpoint_invocations": 1,
        "file_pull_invocations": 34,
        "file_push_invocations": 1,
        "format": checkpoint_control.EVIDENCE_FORMAT,
        "guest_checkpoint_outcome": "checkpoint-prepared",
        "guest_exec_invocations": 5,
        "maintenance_resume_invocations": 0,
        "operation_id": "generated-once-hash-only",
        "operation_id_sha256": REQUIRED_OPERATION_ID_SHA256,
        "outcome": "checkpoint-prepared",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "preflight_attempt_id": request.preflight_attempt_id,
        "reason": "one-shot-install-artifacts-staged-checkpoint-passed",
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "artifacts-staged-checkpoint-only",
    }
    if prior_terminal != required_terminal:
        raise ValueError("prior-checkpoint-terminal-semantics-invalid")

    terminal_payloads = tuple(
        transfer_bindings._prior_readback_bytes(
            request.prior_checkpoint_root
            / f"guest-checkpoint-evidence-readback-{index}.json"
        )
        for index in (1, 2)
    )
    if terminal_payloads[0] != terminal_payloads[1]:
        raise ValueError("prior-checkpoint-terminal-readback-drift")
    terminal_binding = type(
        "CheckpointEvidenceBinding",
        (),
        {
            "boot_id_sha256": upstream.boot_id_sha256,
            "preflight_evidence_bytes": upstream.preflight_evidence_bytes,
        },
    )()
    parsed_terminal = checkpoint_evidence.parse_driver_terminal(
        terminal_payloads[0], request, terminal_binding
    )
    persisted_terminal = network_ready._read_json(
        request.prior_checkpoint_root / "guest-checkpoint-evidence.json"
    )
    if parsed_terminal != persisted_terminal:
        raise ValueError("prior-checkpoint-guest-terminal-drift")
    if (
        parsed_terminal.get("operation_id_sha256")
        != REQUIRED_OPERATION_ID_SHA256
        or parsed_terminal.get("checkpoint_sha256") != REQUIRED_CHECKPOINT_SHA256
        or parsed_terminal.get("crash_state_sha256")
        != REQUIRED_CRASH_STATE_SHA256
        or parsed_terminal.get("guest_identity_sha256")
        != resume_driver.EXPECTED_GUEST_IDENTITY_SHA256
        or parsed_terminal.get("mutation_preflight_sha256")
        != resume_driver.EXPECTED_MUTATION_PREFLIGHT_SHA256
    ):
        raise ValueError("prior-checkpoint-guest-terminal-identity-invalid")

    artifacts: dict[str, bytes] = {}
    for name in (
        "guest-identity",
        "transfer-setup",
        "mutation-preflight",
        "crash-result",
        "checkpoint",
        "crash-state",
        "case-preflight-stdout",
        "case-preflight-stderr",
        "case-crash-stdout",
        "case-crash-stderr",
        "case-inspect-crash-stdout",
        "case-inspect-crash-stderr",
    ):
        payloads = tuple(
            transfer_bindings._prior_readback_bytes(
                request.prior_checkpoint_root / f"{name}-readback-{index}.json"
            )
            for index in (1, 2)
        )
        if payloads[0] != payloads[1]:
            raise ValueError(f"prior-checkpoint-{name}-readback-drift")
        artifacts[name] = payloads[0]
    checkpoint_evidence.validate_checkpoint_artifacts(
        artifacts, parsed_terminal, terminal_binding
    )
    crash_state = checkpoint_evidence.parse_fields(
        artifacts["crash-state"], "crash-state"
    )
    if (
        hashlib.sha256(artifacts["checkpoint"]).hexdigest()
        != REQUIRED_CHECKPOINT_SHA256
        or hashlib.sha256(artifacts["crash-state"]).hexdigest()
        != REQUIRED_CRASH_STATE_SHA256
        or crash_state.get("receipt_sha256") != REQUIRED_RECEIPT_SHA256
        or crash_state.get("receipt_size")
        != str(resume_driver.EXPECTED_RECEIPT_SIZE)
        or crash_state.get("dpkg_log_sha256")
        != resume_driver.EXPECTED_DPKG_LOG_SHA256
    ):
        raise ValueError("prior-checkpoint-artifact-identity-invalid")

    driver_bytes = (request.repository_root / DRIVER_RELATIVE_PATH).read_bytes()
    if not driver_bytes or len(driver_bytes) > resume_driver.MAX_DRIVER_BYTES:
        raise ValueError("resume-driver-size-invalid")
    operation_hash = REQUIRED_OPERATION_ID_SHA256
    live_artifacts = {
        "guest-identity": (
            "/var/tmp/radishlex-l6-crash-install-artifacts-staged-guest-identity.evidence.txt",
            artifacts["guest-identity"],
        ),
        "mutation-preflight": (
            "/var/tmp/radishlex-l6-crash-install-artifacts-staged-output/"
            "mutation-preflight.evidence.txt",
            artifacts["mutation-preflight"],
        ),
        "checkpoint": (
            "/var/tmp/radishlex-l6-evidence/checkpoints/"
            f"install_artifacts_staged-{operation_hash[:16]}.json",
            artifacts["checkpoint"],
        ),
        "crash-state": (
            "/var/tmp/radishlex-l6-crash-install-artifacts-staged-output/"
            "crash-state.evidence.txt",
            artifacts["crash-state"],
        ),
    }
    return ResumeBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "checkpoint_attempt_id": request.checkpoint_attempt_id,
            "prior_checkpoint_entries_verified": entries,
            "prior_checkpoint_manifest_sha256": (
                request.prior_checkpoint_manifest_sha256
            ),
            "prior_checkpoint_outcome": "checkpoint-prepared",
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "resume_attempt_id": request.resume_attempt_id,
            "source_bundle_sha256": request.source_bundle_sha256,
            "source_bundle_size": request.source_bundle_size,
            "target_uuid": request.target_uuid,
        },
        network=upstream.network,
        driver_bytes=driver_bytes,
        boot_id_sha256=upstream.boot_id_sha256,
        operation_id_sha256=REQUIRED_OPERATION_ID_SHA256,
        checkpoint_sha256=REQUIRED_CHECKPOINT_SHA256,
        crash_state_sha256=REQUIRED_CRASH_STATE_SHA256,
        receipt_sha256=REQUIRED_RECEIPT_SHA256,
        live_artifacts=live_artifacts,
    )


def _manifest_names(path: Path) -> tuple[str, ...]:
    names: list[str] = []
    for line in path.read_text(encoding="ascii").splitlines():
        parts = line.split("  ", 1)
        if len(parts) != 2:
            raise ValueError("prior-checkpoint-manifest-line-invalid")
        names.append(parts[1])
    return tuple(names)

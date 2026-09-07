#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import re
from typing import Protocol

import l6_v4_install_artifacts_staged_checkpoint_driver as checkpoint_driver


REQUIRED_TARGET_UUID = "50B75F88-493D-42C0-A1DC-054DEC478038"
HEX_64 = re.compile(r"[0-9a-f]{64}")


class CheckpointEvidenceError(ValueError):
    pass


class CheckpointRequest(Protocol):
    checkpoint_attempt_id: str
    preflight_attempt_id: str
    guest_checkpoint_root: str


class CheckpointBinding(Protocol):
    boot_id_sha256: str
    preflight_evidence_bytes: bytes


class Pull(Protocol):
    def __call__(
        self, path: str, name: str, *, allow_empty: bool = False
    ) -> bytes: ...


def parse_driver_terminal(
    payload: bytes,
    request: CheckpointRequest,
    binding: CheckpointBinding,
) -> dict[str, object]:
    try:
        value = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise CheckpointEvidenceError(
            "guest-checkpoint-evidence-json-invalid"
        ) from exc
    if (
        not isinstance(value, dict)
        or checkpoint_driver.canonical_json(value) != payload
        or value.get("format") != checkpoint_driver.EVIDENCE_FORMAT
        or value.get("checkpoint_attempt_id") != request.checkpoint_attempt_id
        or value.get("phase") not in {
            "arguments",
            "negative-preflight-binding",
            "case-setup",
            "case-preflight",
            "checkpoint",
            "inspect-crash",
            "complete",
        }
        or value.get("automatic_cleanup") != "not-performed"
        or value.get("automatic_quit") != "not-performed"
        or value.get("automatic_retry") != "not-performed"
        or value.get("automatic_stop") != "not-performed"
        or value.get("maintenance_resume_invocations") != 0
    ):
        raise CheckpointEvidenceError("guest-checkpoint-evidence-shape-invalid")
    if value.get("outcome") == "checkpoint-prepared":
        required = {
            "acceptance_invocations": 1,
            "boot_id_sha256": binding.boot_id_sha256,
            "case_crash_invocations": 1,
            "case_inspect_crash_invocations": 1,
            "case_preflight_invocations": 1,
            "checkpoint_invocations": 1,
            "dpkg_mutation": "not-performed",
            "negative_preflight_outcome": "passed",
            "operation_id": "generated-once-hash-only",
            "phase": "complete",
            "preflight_attempt_id": request.preflight_attempt_id,
            "reason": "one-shot-install-artifacts-staged-checkpoint-passed",
            "transaction": "artifacts-staged-checkpoint-only",
        }
        if any(value.get(key) != expected for key, expected in required.items()):
            raise CheckpointEvidenceError(
                "guest-checkpoint-evidence-success-invalid"
            )
        for key in (
            "checkpoint_sha256",
            "crash_result_sha256",
            "crash_state_sha256",
            "guest_identity_sha256",
            "mutation_preflight_sha256",
            "operation_id_sha256",
        ):
            if not HEX_64.fullmatch(str(value.get(key))):
                raise CheckpointEvidenceError(
                    f"guest-checkpoint-evidence-{key}-invalid"
                )
    elif value.get("outcome") == "checkpoint-rejected":
        if value.get("operation_id") != "not-generated" or value.get(
            "transaction"
        ) != "not-performed":
            raise CheckpointEvidenceError(
                "guest-checkpoint-evidence-rejected-invalid"
            )
    elif value.get("outcome") == "state-indeterminate":
        if value.get("operation_id") != "generated-not-exported":
            raise CheckpointEvidenceError(
                "guest-checkpoint-evidence-indeterminate-invalid"
            )
    else:
        raise CheckpointEvidenceError("guest-checkpoint-evidence-outcome-invalid")
    if not isinstance(value.get("reason"), str) or not value["reason"]:
        raise CheckpointEvidenceError("guest-checkpoint-evidence-reason-invalid")
    return value


def pull_checkpoint_artifacts(
    request: CheckpointRequest,
    terminal: dict[str, object],
    pull: Pull,
) -> dict[str, bytes]:
    operation_hash = str(terminal["operation_id_sha256"])
    paths = {
        "guest-identity": (
            "/var/tmp/radishlex-l6-crash-install-artifacts-staged-guest-identity.evidence.txt"
        ),
        "transfer-setup": (
            "/var/tmp/radishlex-l6-crash-install-artifacts-staged-output/"
            "transfer-setup.evidence.txt"
        ),
        "mutation-preflight": (
            "/var/tmp/radishlex-l6-crash-install-artifacts-staged-output/"
            "mutation-preflight.evidence.txt"
        ),
        "crash-result": (
            "/var/tmp/radishlex-l6-crash-install-artifacts-staged-output/"
            "crash-result.evidence.txt"
        ),
        "checkpoint": (
            "/var/tmp/radishlex-l6-evidence/checkpoints/"
            f"install_artifacts_staged-{operation_hash[:16]}.json"
        ),
        "crash-state": (
            "/var/tmp/radishlex-l6-crash-install-artifacts-staged-output/"
            "crash-state.evidence.txt"
        ),
        "case-preflight-stdout": f"{request.guest_checkpoint_root}/case-preflight.stdout",
        "case-preflight-stderr": f"{request.guest_checkpoint_root}/case-preflight.stderr",
        "case-crash-stdout": f"{request.guest_checkpoint_root}/case-crash.stdout",
        "case-crash-stderr": f"{request.guest_checkpoint_root}/case-crash.stderr",
        "case-inspect-crash-stdout": (
            f"{request.guest_checkpoint_root}/case-inspect-crash.stdout"
        ),
        "case-inspect-crash-stderr": (
            f"{request.guest_checkpoint_root}/case-inspect-crash.stderr"
        ),
    }
    artifacts: dict[str, bytes] = {}
    for name, path in paths.items():
        payloads = [
            pull(
                path,
                f"{name}-readback-{index}",
                allow_empty=name.endswith("stderr"),
            )
            for index in (1, 2)
        ]
        artifacts[name] = require_equal_payloads(payloads, name)
    return artifacts


def validate_checkpoint_artifacts(
    artifacts: dict[str, bytes],
    terminal: dict[str, object],
    binding: CheckpointBinding,
) -> None:
    for name, expected in {
        "case-preflight-stdout": b"install_artifacts_staged_preflight_outcome=passed\n",
        "case-preflight-stderr": b"",
        "case-crash-stdout": b"install_artifacts_staged_crash_outcome=checkpoint_recorded\n",
        "case-crash-stderr": b"",
        "case-inspect-crash-stdout": b"install_artifacts_staged_crash_state_outcome=passed\n",
        "case-inspect-crash-stderr": b"",
    }.items():
        if artifacts[name] != expected:
            raise CheckpointEvidenceError(f"{name}-invalid")
    identities = {
        "guest-identity": "guest_identity_sha256",
        "mutation-preflight": "mutation_preflight_sha256",
        "crash-result": "crash_result_sha256",
        "checkpoint": "checkpoint_sha256",
        "crash-state": "crash_state_sha256",
    }
    for artifact, terminal_key in identities.items():
        if hashlib.sha256(artifacts[artifact]).hexdigest() != terminal[terminal_key]:
            raise CheckpointEvidenceError(f"{artifact}-terminal-hash-mismatch")
    guest = parse_fields(artifacts["guest-identity"], "guest-identity")
    setup = parse_fields(artifacts["transfer-setup"], "transfer-setup")
    mutation = parse_fields(artifacts["mutation-preflight"], "mutation-preflight")
    crash = parse_fields(artifacts["crash-result"], "crash-result")
    state = parse_fields(artifacts["crash-state"], "crash-state")
    operation_hash = str(terminal["operation_id_sha256"])
    if (
        guest.get("clone_uuid") != REQUIRED_TARGET_UUID
        or guest.get("boot_id_sha256") != binding.boot_id_sha256
        or guest.get("operation_id") != "not-generated"
        or setup.get("operation_id") != "not-generated"
        or setup.get("negative_preflight_sha256")
        != hashlib.sha256(binding.preflight_evidence_bytes).hexdigest()
        or mutation.get("operation_id") != "not-generated"
        or mutation.get("mutation_preflight") != "passed"
        or mutation.get("dpkg_status_sha256")
        != checkpoint_driver.EXPECTED_DPKG_STATUS_SHA256
        or crash.get("operation_id_sha256") != operation_hash
        or crash.get("preflight_sha256")
        != hashlib.sha256(artifacts["mutation-preflight"]).hexdigest()
        or crash.get("acceptance_invocations") != "1"
        or crash.get("checkpoint_count") != "1"
        or crash.get("crash_result") != "passed"
        or state.get("operation_id_sha256") != operation_hash
        or state.get("receipt")
        != "install|not_applicable|artifacts_staged|chain-1"
        or state.get("guard") != "present-valid-unlocked"
        or state.get("dpkg_mutation_executed") != "false"
        or state.get("dpkg_status_sha256") != mutation.get("dpkg_status_sha256")
        or state.get("dpkg_log_sha256") != mutation.get("dpkg_log_sha256")
        or state.get("manager_startup") != checkpoint_driver.EXPECTED_STARTUP
        or state.get("fcitx_startup") != checkpoint_driver.EXPECTED_STARTUP
        or state.get("user_xdg") != "absent"
        or state.get("product_processes") != "absent"
        or state.get("network") != "loopback-only-main-routes-empty"
        or state.get("crash_state") != "passed"
    ):
        raise CheckpointEvidenceError("checkpoint-artifact-semantics-invalid")
    try:
        checkpoint = json.loads(artifacts["checkpoint"])
    except json.JSONDecodeError as exc:
        raise CheckpointEvidenceError("checkpoint-json-invalid") from exc
    operation = checkpoint.get("operation") if isinstance(checkpoint, dict) else None
    termination = checkpoint.get("termination") if isinstance(checkpoint, dict) else None
    if (
        not isinstance(checkpoint, dict)
        or checkpoint.get("format") != "radishlex-linux-l6-checkpoint-evidence-v1"
        or checkpoint.get("repository_commit")
        != checkpoint_driver.EXPECTED_REPOSITORY_COMMIT
        or checkpoint.get("scenario") != "install_artifacts_staged"
        or checkpoint.get("checkpoint") != "artifacts_staged"
        or checkpoint.get("guest_identity_sha256")
        != terminal["guest_identity_sha256"]
        or not isinstance(operation, dict)
        or operation.get("operation_id_sha256") != operation_hash
        or operation.get("matrix_operation") != "install_source"
        or not isinstance(termination, dict)
        or termination.get("process_group") != "terminated"
        or termination.get("signal") != "sigkill"
        or termination.get("process_group_member_count") != 0
        or termination.get("dpkg_child") != "absent"
    ):
        raise CheckpointEvidenceError("checkpoint-envelope-semantics-invalid")


def parse_fields(payload: bytes, label: str) -> dict[str, str]:
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise CheckpointEvidenceError(f"{label}-not-utf8") from exc
    if not text.endswith("\n") or "\r" in text or "\x00" in text:
        raise CheckpointEvidenceError(f"{label}-not-canonical")
    value: dict[str, str] = {}
    for line in text.splitlines():
        key, separator, field = line.partition("=")
        if not separator or not key or key in value:
            raise CheckpointEvidenceError(f"{label}-invalid")
        value[key] = field
    return value


def require_equal_payloads(payloads: list[bytes], label: str) -> bytes:
    if len(payloads) != 2 or payloads[0] != payloads[1]:
        raise CheckpointEvidenceError(f"{label}-double-readback-drift")
    return payloads[0]

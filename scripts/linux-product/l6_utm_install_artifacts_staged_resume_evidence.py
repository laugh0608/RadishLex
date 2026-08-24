#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import re
from typing import Protocol

import l6_v4_install_artifacts_staged_resume_driver as resume_driver


HEX_64 = re.compile(r"[0-9a-f]{64}")
RAW_OPERATION_TOKEN = re.compile(rb"(?<![0-9a-f])[0-9a-f]{32}(?![0-9a-f])")


class ResumeEvidenceError(ValueError):
    pass


class ResumeRequest(Protocol):
    resume_attempt_id: str
    checkpoint_attempt_id: str
    guest_resume_root: str


class ResumeBinding(Protocol):
    boot_id_sha256: str
    operation_id_sha256: str
    checkpoint_sha256: str
    crash_state_sha256: str


class Pull(Protocol):
    def __call__(
        self, path: str, name: str, *, allow_empty: bool = False
    ) -> bytes: ...


def parse_driver_terminal(
    payload: bytes,
    request: ResumeRequest,
    binding: ResumeBinding,
) -> dict[str, object]:
    try:
        value = json.loads(payload)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ResumeEvidenceError("guest-resume-evidence-json-invalid") from exc
    if (
        not isinstance(value, dict)
        or resume_driver.canonical_json(value) != payload
        or value.get("format") != resume_driver.EVIDENCE_FORMAT
        or value.get("resume_attempt_id") != request.resume_attempt_id
        or value.get("checkpoint_attempt_id") != request.checkpoint_attempt_id
        or value.get("phase") not in {
            "arguments",
            "checkpoint-binding",
            "resume-readiness",
            "terminal-case",
            "resume",
            "postflight",
            "complete",
        }
        or value.get("automatic_cleanup") != "not-performed"
        or value.get("automatic_quit") != "not-performed"
        or value.get("automatic_retry") != "not-performed"
        or value.get("automatic_stop") != "not-performed"
        or value.get("operation_id") != "existing-secret-hash-only"
        or value.get("operation_id_sha256") != binding.operation_id_sha256
    ):
        raise ResumeEvidenceError("guest-resume-evidence-shape-invalid")
    if value.get("outcome") == "resume-completed":
        required = {
            "boot_id_sha256": binding.boot_id_sha256,
            "checkpoint_sha256": binding.checkpoint_sha256,
            "maintenance_resume_invocations": 1,
            "phase": "complete",
            "postflight_invocations": 1,
            "reason": "one-shot-install-artifacts-staged-resume-passed",
            "transaction": "completed",
        }
        if any(value.get(key) != expected for key, expected in required.items()):
            raise ResumeEvidenceError("guest-resume-evidence-success-invalid")
        for key in (
            "dpkg_delta_sha256",
            "dpkg_log_sha256",
            "receipt_sha256",
            "resume_result_sha256",
            "terminal_case_sha256",
            "terminal_postflight_sha256",
        ):
            if not HEX_64.fullmatch(str(value.get(key))):
                raise ResumeEvidenceError(
                    f"guest-resume-evidence-{key}-invalid"
                )
        for key in ("dpkg_delta_size", "dpkg_log_size", "receipt_size"):
            if not isinstance(value.get(key), int) or value[key] < 0:
                raise ResumeEvidenceError(
                    f"guest-resume-evidence-{key}-invalid"
                )
    elif value.get("outcome") == "resume-rejected":
        if (
            value.get("maintenance_resume_invocations") != 0
            or value.get("postflight_invocations") != 0
            or value.get("transaction") != "artifacts-staged-preserved"
        ):
            raise ResumeEvidenceError("guest-resume-evidence-rejected-invalid")
    elif value.get("outcome") == "state-indeterminate":
        if value.get("maintenance_resume_invocations") != 1 or value.get(
            "transaction"
        ) != "state-indeterminate":
            raise ResumeEvidenceError(
                "guest-resume-evidence-indeterminate-invalid"
            )
    else:
        raise ResumeEvidenceError("guest-resume-evidence-outcome-invalid")
    if not isinstance(value.get("reason"), str) or not value["reason"]:
        raise ResumeEvidenceError("guest-resume-evidence-reason-invalid")
    if RAW_OPERATION_TOKEN.search(payload):
        raise ResumeEvidenceError("guest-resume-evidence-contains-raw-operation-id")
    return value


def pull_resume_artifacts(
    request: ResumeRequest,
    terminal: dict[str, object],
    pull: Pull,
) -> dict[str, bytes]:
    del terminal
    output = "/var/tmp/radishlex-l6-crash-install-artifacts-staged-output"
    paths = {
        "case-resume-stdout": f"{request.guest_resume_root}/case-resume.stdout",
        "case-resume-stderr": f"{request.guest_resume_root}/case-resume.stderr",
        "case-postflight-stdout": (
            f"{request.guest_resume_root}/case-postflight.stdout"
        ),
        "case-postflight-stderr": (
            f"{request.guest_resume_root}/case-postflight.stderr"
        ),
        "maintenance-resume-stdout": f"{output}/resume.stdout",
        "maintenance-resume-stderr": f"{output}/resume.stderr",
        "resume-result": f"{output}/resume-result.evidence.txt",
        "terminal-postflight": f"{output}/terminal-postflight.evidence.txt",
        "dpkg-delta": f"{output}/dpkg-delta.txt",
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


def validate_resume_artifacts(
    artifacts: dict[str, bytes],
    terminal: dict[str, object],
    binding: ResumeBinding,
) -> None:
    exact = {
        "case-resume-stdout": (
            b"install_artifacts_staged_resume_outcome=completed\n"
        ),
        "case-resume-stderr": b"",
        "case-postflight-stdout": (
            b"install_artifacts_staged_postflight_outcome=passed\n"
        ),
        "case-postflight-stderr": b"",
        "maintenance-resume-stdout": b"maintenance_outcome=completed\n",
        "maintenance-resume-stderr": b"",
    }
    for name, expected in exact.items():
        if artifacts.get(name) != expected:
            raise ResumeEvidenceError(f"{name}-invalid")
    for name, payload in artifacts.items():
        if RAW_OPERATION_TOKEN.search(payload):
            raise ResumeEvidenceError(f"{name}-contains-raw-operation-id")
    resume = parse_fields(artifacts["resume-result"], "resume-result")
    required_resume = {
        "format": "radishlex-linux-l6-install-artifacts-staged-resume-result-v1",
        "operation_id_sha256": binding.operation_id_sha256,
        "crash_state_sha256": binding.crash_state_sha256,
        "maintenance_invocations": "1",
        "maintenance_exit_code": "0",
        "maintenance_stdout": "maintenance_outcome=completed",
        "maintenance_stderr": "empty",
        "resume_result": "passed",
    }
    if resume != required_resume:
        raise ResumeEvidenceError("resume-result-semantics-invalid")
    postflight = parse_fields(
        artifacts["terminal-postflight"], "terminal-postflight"
    )
    required_postflight = {
        "format": "radishlex-linux-l6-install-artifacts-staged-terminal-postflight-v1",
        "clone_uuid": resume_driver.EXPECTED_TARGET_UUID,
        "boot_id_sha256": binding.boot_id_sha256,
        "guest_identity_sha256": resume_driver.EXPECTED_GUEST_IDENTITY_SHA256,
        "snapshot_identity_sha256": resume_driver.EXPECTED_SNAPSHOT_SHA256,
        "operation_id_sha256": binding.operation_id_sha256,
        "checkpoint_sha256": binding.checkpoint_sha256,
        "acceptance_invocations": "1",
        "maintenance_resume_invocations": "1",
        "operation": "install_source|install|not_applicable|completed|chain-1",
        "package": "radishlex|arm64|26.7.1+38-1|install-ok-installed",
        "target_package_sha256": resume_driver.EXPECTED_SOURCE_PACKAGE_SHA256,
        "target_evidence_sha256": resume_driver.EXPECTED_SOURCE_EVIDENCE_SHA256,
        "dpkg_status_sha256": resume_driver.EXPECTED_SOURCE_STATUS_SHA256,
        "dpkg_audit": "clean",
        "dpkg_verify": "clean",
        "dpkg_mutation_executed": "true",
        "installed_payload_md5_inventory": (
            f"{resume_driver.EXPECTED_SOURCE_MD5_SHA256}|28|verified"
        ),
        "dependency_count": "20",
        "fonts": "dejavu+noto-cjk|family+owner-satisfied",
        "manifest": resume_driver.EXPECTED_SOURCE_MANIFEST_SHA256,
        "ffi": f"{resume_driver.EXPECTED_SOURCE_FFI_SHA256}|distinct-inodes",
        "manager_startup": resume_driver.EXPECTED_ALLOWED_STARTUP,
        "fcitx_startup": resume_driver.EXPECTED_ALLOWED_STARTUP,
        "user_xdg": "absent",
        "product_processes": "absent",
        "network": "loopback-only-main-routes-empty",
        "manual_recovery_required": "false",
        "install_artifacts_staged_completed": "true",
        "terminal_postflight": "passed",
    }
    if any(postflight.get(key) != value for key, value in required_postflight.items()):
        raise ResumeEvidenceError("terminal-postflight-semantics-invalid")
    if (
        hashlib.sha256(artifacts["resume-result"]).hexdigest()
        != terminal["resume_result_sha256"]
        or hashlib.sha256(artifacts["terminal-postflight"]).hexdigest()
        != terminal["terminal_postflight_sha256"]
        or hashlib.sha256(artifacts["dpkg-delta"]).hexdigest()
        != terminal["dpkg_delta_sha256"]
        or len(artifacts["dpkg-delta"]) != terminal["dpkg_delta_size"]
    ):
        raise ResumeEvidenceError("resume-artifact-terminal-hash-mismatch")
    delta_hash, separator, delta_size = postflight.get("dpkg_delta", "").partition(
        "|"
    )
    if (
        not separator
        or delta_hash != terminal["dpkg_delta_sha256"]
        or not delta_size.isdigit()
        or int(delta_size) != terminal["dpkg_delta_size"]
    ):
        raise ResumeEvidenceError("dpkg-delta-postflight-identity-invalid")
    receipt_hash, separator, receipt_size = postflight.get("receipt", "").partition(
        "|"
    )
    if (
        not separator
        or receipt_hash != terminal["receipt_sha256"]
        or not receipt_size.isdigit()
        or int(receipt_size) != terminal["receipt_size"]
    ):
        raise ResumeEvidenceError("receipt-postflight-identity-invalid")
    if (
        postflight.get("dpkg_log_sha256") != terminal["dpkg_log_sha256"]
        or not postflight.get("dpkg_log_size", "").isdigit()
        or int(postflight["dpkg_log_size"]) != terminal["dpkg_log_size"]
    ):
        raise ResumeEvidenceError("dpkg-log-postflight-identity-invalid")


def parse_fields(payload: bytes, label: str) -> dict[str, str]:
    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise ResumeEvidenceError(f"{label}-not-utf8") from exc
    if not text.endswith("\n") or "\r" in text or "\x00" in text:
        raise ResumeEvidenceError(f"{label}-not-canonical")
    value: dict[str, str] = {}
    for line in text.splitlines():
        key, separator, field = line.partition("=")
        if not separator or not key or key in value:
            raise ResumeEvidenceError(f"{label}-invalid")
        value[key] = field
    return value


def require_equal_payloads(payloads: list[bytes], label: str) -> bytes:
    if len(payloads) != 2 or payloads[0] != payloads[1]:
        raise ResumeEvidenceError(f"{label}-double-readback-drift")
    return payloads[0]

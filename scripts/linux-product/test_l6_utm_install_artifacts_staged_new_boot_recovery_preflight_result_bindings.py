#!/usr/bin/env python3
from __future__ import annotations

import contextlib
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_new_boot_recovery_preflight_result_bindings as bindings
import test_l6_utm_install_artifacts_staged_new_boot_recovery_preflight as recovery_test


class RecoveryPreflightResultBindingTests(unittest.TestCase):
    def test_manifest_binds_all_thirty_eight_members_before_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_recovery_preflight_root,
                bindings.REQUIRED_ENTRY_NAMES,
            )
            request.prior_recovery_preflight_manifest_sha256 = manifest_sha256

            with patched_validation(manifest_sha256):
                result = bindings.validate_recovery_preflight_result_bindings(
                    request
                )

            self.assertEqual(
                result.evidence["prior_recovery_preflight_entries_verified"],
                38,
            )
            self.assertEqual(result.guest_recovery_outcome, "recovery-rejected")
            self.assertEqual(result.guest_probe_transport_exit_code, 0)

    def test_manifest_member_order_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_recovery_preflight_root,
                tuple(reversed(bindings.REQUIRED_ENTRY_NAMES)),
            )
            request.prior_recovery_preflight_manifest_sha256 = manifest_sha256

            with patched_validation(manifest_sha256):
                with self.assertRaisesRegex(
                    ValueError,
                    "prior-recovery-preflight-entry-names-invalid",
                ):
                    bindings.validate_recovery_preflight_result_bindings(request)

    def test_guest_rejection_and_phase_override_transport_exit_zero(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_recovery_preflight_root
            root.mkdir(mode=0o700)
            write_json(
                root / "guest-recovery-preflight-once.json",
                {
                    "format": bindings.recovery_bindings.EVIDENCE_FORMAT.replace(
                        "binding-v1", "v1"
                    ),
                    "observation": observation(
                        argv=bindings._probe_argv(request)
                    ),
                    "raw_output_persisted": False,
                },
            )
            for name, sha256, size in (
                (
                    "guest-recovery-marker-readback.json",
                    bindings.REQUIRED_MARKER_SHA256,
                    bindings.REQUIRED_MARKER_SIZE,
                ),
                (
                    "guest-recovery-result-readback-1.json",
                    bindings.REQUIRED_RESULT_SHA256,
                    bindings.REQUIRED_RESULT_SIZE,
                ),
                (
                    "guest-recovery-result-readback-2.json",
                    bindings.REQUIRED_RESULT_SHA256,
                    bindings.REQUIRED_RESULT_SIZE,
                ),
                (
                    "guest-recovery-phase-readback.json",
                    bindings.REQUIRED_PHASE_SHA256,
                    bindings.REQUIRED_PHASE_SIZE,
                ),
            ):
                guest_name = {
                    "guest-recovery-marker-readback.json": "attempt.marker.json",
                    "guest-recovery-result-readback-1.json": (
                        "preflight.evidence.json"
                    ),
                    "guest-recovery-result-readback-2.json": (
                        "preflight.evidence.json"
                    ),
                    "guest-recovery-phase-readback.json": "phase.json",
                }[name]
                write_json(
                    root / name,
                    observation(
                        argv=[
                            "utmctl",
                            "file",
                            "pull",
                            request.target_uuid,
                            f"{bindings._guest_control_root()}/{guest_name}",
                        ],
                        sha256=sha256,
                        size=size,
                    ),
                )
            write_json(root / "guest-recovery-result.json", guest_result(request))

            bindings._validate_guest_result(request, root)

            value = guest_result(request)
            value["reason"] = "synthetic-drift"
            write_json(root / "guest-recovery-result.json", value)
            with self.assertRaisesRegex(
                ValueError, "prior-recovery-preflight-guest-result-invalid"
            ):
                bindings._validate_guest_result(request, root)

    def test_terminal_freezes_indeterminate_host_interpretation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_recovery_preflight_root
            root.mkdir(mode=0o700)
            value = host_terminal(request)
            write_json(root / "terminal.json", value)

            bindings._validate_terminal(request, root)

            value["automatic_retry"] = "performed"
            write_json(root / "terminal.json", value)
            with self.assertRaisesRegex(
                ValueError, "prior-recovery-preflight-terminal-invalid"
            ):
                bindings._validate_terminal(request, root)

    def test_forbidden_command_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in bindings.REQUIRED_ENTRY_NAMES:
                write_json(root / name, {})
            write_json(
                root / "guest-recovery-root-create.json",
                {"argv": ["utmctl", "status", "synthetic-target"]},
            )

            with self.assertRaisesRegex(
                ValueError, "prior-recovery-preflight-forbidden-command"
            ):
                bindings._validate_no_forbidden_commands(root)


def make_request(root: Path) -> SimpleNamespace:
    base = recovery_test.make_request(root)
    return SimpleNamespace(
        **base.__dict__,
        prior_recovery_preflight_root=root / "prior-recovery-preflight",
        prior_recovery_preflight_manifest_sha256=(
            bindings.REQUIRED_PRIOR_MANIFEST_SHA256
        ),
        prior_recovery_preflight_attempt_id=bindings.REQUIRED_PRIOR_ATTEMPT_ID,
    )


@contextlib.contextmanager
def patched_validation(manifest_sha256: str):
    semantic_names = (
        "_validate_request",
        "_validate_binding",
        "_validate_source_target",
        "_validate_host_identity",
        "_validate_delivery",
        "_validate_guest_result",
        "_validate_terminal",
        "_validate_no_forbidden_commands",
    )
    with contextlib.ExitStack() as stack:
        stack.enter_context(
            mock.patch.object(
                bindings, "REQUIRED_PRIOR_MANIFEST_SHA256", manifest_sha256
            )
        )
        stack.enter_context(
            mock.patch.object(
                bindings.recovery_bindings.result_bindings,
                "validate_boot_classification_result_bindings",
                return_value=SimpleNamespace(),
            )
        )
        stack.enter_context(
            mock.patch.object(
                bindings.network_ready, "_require_committed_regular"
            )
        )
        for name in semantic_names:
            stack.enter_context(mock.patch.object(bindings, name))
        yield


def write_manifest(root: Path, names: tuple[str, ...]) -> str:
    root.mkdir(mode=0o700)
    for name in set(names):
        path = root / name
        path.write_text("{}\n", encoding="utf-8")
        path.chmod(0o600)
    manifest = root / "files.sha256"
    manifest.write_text(
        "".join(f"{sha256(root / name)}  {name}\n" for name in names),
        encoding="ascii",
    )
    manifest.chmod(0o600)
    return sha256(manifest)


def guest_result(request: SimpleNamespace) -> dict[str, object]:
    return {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "current_boot_id_sha256": (
            bindings.recovery_bindings.REQUIRED_CURRENT_BOOT_ID_SHA256
        ),
        "dpkg_mutation_executed": False,
        "format": bindings.guest_probe.EVIDENCE_FORMAT,
        "guard_profile": "not-observed",
        "maintenance_resume_invocations": 0,
        "operation_id": "existing-receipt-hash-only",
        "operation_id_sha256": bindings.guest_probe.EXPECTED_OPERATION_ID_SHA256,
        "outcome": "recovery-rejected",
        "phase": "persistent-transaction",
        "prior_boot_id_sha256": (
            bindings.recovery_bindings.REQUIRED_PRIOR_BOOT_ID_SHA256
        ),
        "reason": "ResumeDriverError:directory-identity-invalid:state-root",
        "target_uuid": request.target_uuid,
        "transaction": "artifacts-staged-preserved-no-resume",
    }


def host_terminal(request: SimpleNamespace) -> dict[str, object]:
    return {
        "agent_readiness_invocations": 1,
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": bindings.REQUIRED_BACKEND_PID,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 6,
        "file_push_invocations": 2,
        "format": bindings.recovery_bindings.EVIDENCE_FORMAT.replace(
            "binding-v1", "v1"
        ),
        "guest_exec_invocations": 8,
        "guest_probe_invocations": 1,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 0,
        "maintenance_resume_invocations": 0,
        "operation_id": "existing-receipt-hash-only",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": "guest-recovery-phase-readback:probe-exit-evidence-mismatch",
        "recovery_preflight_attempt_id": bindings.REQUIRED_PRIOR_ATTEMPT_ID,
        "result_readback_invocations": 2,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }


def observation(
    *,
    argv: list[str] | None = None,
    sha256: str | None = None,
    size: int = 0,
) -> dict[str, object]:
    empty_sha256 = hashlib.sha256(b"").hexdigest()
    return {
        "argv": argv or ["synthetic"],
        "exit_code": 0,
        "stderr": {
            "sha256": empty_sha256,
            "total_bytes": 0,
            "truncated": False,
        },
        "stdout": {
            "sha256": sha256 or empty_sha256,
            "total_bytes": size,
            "truncated": False,
        },
        "timed_out": False,
    }


def write_json(path: Path, value: dict[str, object]) -> None:
    path.write_text(json.dumps(value, sort_keys=True) + "\n", encoding="utf-8")
    path.chmod(0o600)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


if __name__ == "__main__":
    unittest.main()

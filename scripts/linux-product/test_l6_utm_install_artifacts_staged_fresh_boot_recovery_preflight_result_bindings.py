#!/usr/bin/env python3
from __future__ import annotations

import contextlib
import hashlib
import json
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight_result_bindings as bindings
import l6_utm_install_artifacts_staged_fresh_boot_recovery_result_resolution as resolution_control
import l6_utm_install_artifacts_staged_new_boot_recovery_preflight as recovery_control
import test_l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight as recovery_test


class FreshBootRecoveryPreflightResultBindingTests(unittest.TestCase):
    def test_manifest_binds_all_thirty_five_members_before_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_fresh_boot_recovery_preflight_root,
                bindings.REQUIRED_ENTRY_NAMES,
            )
            request = replace(
                request,
                prior_fresh_boot_recovery_preflight_manifest_sha256=(
                    manifest_sha256
                ),
            )

            with patched_validation(manifest_sha256):
                result = bindings.validate_fresh_boot_recovery_preflight_result_bindings(
                    request
                )

            self.assertEqual(
                result.evidence[
                    "prior_fresh_boot_recovery_preflight_entries_verified"
                ],
                35,
            )
            self.assertEqual(
                result.evidence["guest_recovery_outcome"], "not-observed"
            )
            self.assertEqual(
                result.evidence["result_authority"],
                "marker-present-result-not-observed",
            )

    def test_manifest_member_order_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_fresh_boot_recovery_preflight_root,
                tuple(reversed(bindings.REQUIRED_ENTRY_NAMES)),
            )
            request = replace(
                request,
                prior_fresh_boot_recovery_preflight_manifest_sha256=(
                    manifest_sha256
                ),
            )

            with patched_validation(manifest_sha256), self.assertRaisesRegex(
                ValueError,
                "prior-fresh-boot-recovery-preflight-entry-set-invalid",
            ):
                bindings.validate_fresh_boot_recovery_preflight_result_bindings(
                    request
                )

    def test_probe_marker_and_missing_first_result_are_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            upstream = make_upstream(request)
            root = request.prior_fresh_boot_recovery_preflight_root
            root.mkdir(mode=0o700)
            probe_observation = observation(
                argv=list(recovery_control.probe_argv(request, upstream))
            )
            write_json(
                root / "guest-recovery-preflight-once.json",
                {
                    "format": recovery_control.EVIDENCE_FORMAT,
                    "observation": probe_observation,
                    "raw_output_persisted": False,
                },
            )
            marker = recovery_control.marker_bytes(request, upstream)
            write_json(
                root / "guest-recovery-marker-readback.json",
                observation(
                    argv=[
                        "utmctl",
                        "file",
                        "pull",
                        request.target_uuid,
                        request.guest_marker_path,
                    ],
                    stdout_sha256=hashlib.sha256(marker).hexdigest(),
                    stdout_size=len(marker),
                ),
            )
            write_json(
                root / "guest-recovery-result-readback-1.json",
                observation(
                    argv=[
                        "utmctl",
                        "file",
                        "pull",
                        request.target_uuid,
                        request.guest_terminal_path,
                    ],
                    stderr_sha256=bindings.RESULT_MISSING_STDERR_SHA256,
                    stderr_size=bindings.RESULT_MISSING_STDERR_SIZE,
                ),
            )

            bindings._validate_probe_and_missing_result(request, upstream, root)

            value = read_json(root / "guest-recovery-result-readback-1.json")
            value["stderr"]["total_bytes"] = 1
            write_json(root / "guest-recovery-result-readback-1.json", value)
            with self.assertRaisesRegex(
                ValueError,
                "prior-fresh-boot-recovery-preflight-missing-result-invalid",
            ):
                bindings._validate_probe_and_missing_result(
                    request, upstream, root
                )

    def test_terminal_and_forbidden_action_boundary_are_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            upstream = make_upstream(request)
            root = request.prior_fresh_boot_recovery_preflight_root
            root.mkdir(mode=0o700)
            value = terminal(request, upstream)
            write_json(root / "terminal.json", value)

            bindings._validate_terminal(request, upstream, root)

            value["maintenance_resume_invocations"] = 1
            write_json(root / "terminal.json", value)
            with self.assertRaisesRegex(
                ValueError,
                "prior-fresh-boot-recovery-preflight-terminal-invalid",
            ):
                bindings._validate_terminal(request, upstream, root)

            for name in bindings.REQUIRED_ENTRY_NAMES:
                write_json(root / name, {})
            write_json(
                root / "guest-recovery-root-create.json",
                {"argv": ["utmctl", "status", request.target_uuid]},
            )
            with self.assertRaisesRegex(
                ValueError,
                "prior-fresh-boot-recovery-preflight-forbidden-command",
            ):
                bindings._validate_no_forbidden_commands(root)


def make_request(
    root: Path,
) -> resolution_control.FreshBootRecoveryResultResolutionRequest:
    base = recovery_test.make_request(root)
    values = dict(base.__dict__)
    values.update(
        output_root=root / "fresh-boot-recovery-result-resolution-output",
        prior_fresh_boot_recovery_preflight_root=(
            root / "prior-fresh-boot-recovery-preflight"
        ),
        prior_fresh_boot_recovery_preflight_manifest_sha256=(
            bindings.REQUIRED_PRIOR_MANIFEST_SHA256
        ),
        prior_fresh_boot_recovery_preflight_attempt_id=(
            bindings.REQUIRED_PRIOR_ATTEMPT_ID
        ),
        result_resolution_attempt_id=resolution_control.REQUIRED_ATTEMPT_ID,
        authorized_install_artifacts_staged_fresh_boot_recovery_result_resolution=True,
        authorized_bound_prior_fresh_boot_recovery_preflight_result=True,
        authorized_two_existing_recovery_result_readbacks=True,
        authorized_one_existing_recovery_phase_readback=True,
        authorized_no_inventory_start_status_probe_push_exec_resume_dpkg_retry_stop_or_quit=True,
    )
    return resolution_control.FreshBootRecoveryResultResolutionRequest(**values)


def make_upstream(request: SimpleNamespace) -> SimpleNamespace:
    current = recovery_test.make_binding(request)
    current.evidence = {
        "format": bindings.bindings.EVIDENCE_FORMAT,
        "repository_clean": True,
        "repository_head": request.expected_repository_head,
    }
    return current


@contextlib.contextmanager
def patched_validation(manifest_sha256: str):
    request_head = "a" * 40
    upstream = SimpleNamespace(
        evidence={
            "format": bindings.bindings.EVIDENCE_FORMAT,
            "repository_clean": True,
            "repository_head": request_head,
        },
        probe_bytes=b"probe",
        resume_driver_bytes=b"driver",
        prior_boot_id_sha256="b" * 64,
        current_boot_id_sha256="c" * 64,
        prior_backend_pid=42,
    )
    semantic_names = (
        "_validate_request_and_binding",
        "_validate_host_identity",
        "_validate_delivery",
        "_validate_probe_and_missing_result",
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
                bindings.bindings,
                "validate_fresh_boot_recovery_preflight_bindings",
                return_value=upstream,
            )
        )
        stack.enter_context(
            mock.patch.object(bindings.network_ready, "_require_committed_regular")
        )
        stack.enter_context(
            mock.patch.object(bindings.legacy_result, "_validate_source_target")
        )
        stack.enter_context(
            mock.patch.object(bindings.runtime_control, "_require_no_raw_operation_id")
        )
        stack.enter_context(
            mock.patch.object(
                bindings,
                "_validate_request_and_binding",
                return_value=request_head,
            )
        )
        stack.enter_context(
            mock.patch.object(
                bindings,
                "_validate_host_identity",
                return_value=("d" * 64, 378),
            )
        )
        for name in semantic_names[2:]:
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


def terminal(request: SimpleNamespace, upstream: SimpleNamespace) -> dict[str, object]:
    return {
        "agent_readiness_invocations": 1,
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": upstream.prior_backend_pid,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 4,
        "file_push_invocations": 2,
        "format": recovery_control.EVIDENCE_FORMAT,
        "guest_exec_invocations": 8,
        "guest_probe_invocations": 1,
        "guest_probe_transport_exit_code": 0,
        "guest_recovery_outcome": "not-observed",
        "identity_observation_count": 3,
        "inventory_probe_invocations": 0,
        "maintenance_resume_invocations": 0,
        "operation_id": "existing-receipt-hash-only",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": (
            "guest-recovery-result-readback-1:"
            "guest-recovery-result-readback-1-stderr-not-empty"
        ),
        "recovery_preflight_attempt_id": bindings.REQUIRED_PRIOR_ATTEMPT_ID,
        "result_readback_invocations": 1,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }


def observation(
    *,
    argv: list[str],
    stdout_sha256: str | None = None,
    stdout_size: int = 0,
    stderr_sha256: str | None = None,
    stderr_size: int = 0,
) -> dict[str, object]:
    empty = hashlib.sha256(b"").hexdigest()
    return {
        "argv": argv,
        "exit_code": 0,
        "stderr": {
            "sha256": stderr_sha256 or empty,
            "total_bytes": stderr_size,
            "truncated": False,
        },
        "stdout": {
            "sha256": stdout_sha256 or empty,
            "total_bytes": stdout_size,
            "truncated": False,
        },
        "timed_out": False,
    }


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def write_json(path: Path, value: dict[str, object]) -> None:
    path.write_text(
        json.dumps(value, sort_keys=True) + "\n", encoding="utf-8"
    )
    path.chmod(0o600)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


if __name__ == "__main__":
    unittest.main()

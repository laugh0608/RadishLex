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

import l6_utm_install_artifacts_staged_transaction_state_resolution_result_bindings as bindings
import l6_utm_install_artifacts_staged_transaction_state_result_resolution as resolution_control
import test_l6_utm_install_artifacts_staged_transaction_state_result_resolution as resolution_test


class TransactionStateResolutionResultBindingTests(unittest.TestCase):
    def test_manifest_binds_all_sixty_seven_members_before_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_transaction_state_resolution_root,
                bindings.REQUIRED_ENTRY_NAMES,
            )
            request = replace(
                request,
                prior_transaction_state_resolution_manifest_sha256=(
                    manifest_sha256
                ),
            )

            with patched_validation(manifest_sha256):
                result = (
                    bindings.validate_transaction_state_resolution_result_bindings(
                        request
                    )
                )

            self.assertEqual(
                result.evidence[
                    "prior_transaction_state_resolution_entries_verified"
                ],
                67,
            )
            self.assertEqual(
                result.evidence["result_authority"],
                "marker-present-result-not-observed",
            )
            self.assertEqual(result.prior_backend_pid, 36343)

    def test_manifest_member_order_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_transaction_state_resolution_root,
                tuple(reversed(bindings.REQUIRED_ENTRY_NAMES)),
            )
            request = replace(
                request,
                prior_transaction_state_resolution_manifest_sha256=(
                    manifest_sha256
                ),
            )

            with patched_validation(manifest_sha256), self.assertRaisesRegex(
                ValueError,
                "prior-transaction-state-resolution-entry-set-invalid",
            ):
                bindings.validate_transaction_state_resolution_result_bindings(
                    request
                )

    def test_recorded_execution_head_is_separate_from_successor_head(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_transaction_state_resolution_root
            root.mkdir(mode=0o700)
            upstream = SimpleNamespace(
                evidence={
                    "format": bindings.bindings.EVIDENCE_FORMAT,
                    "repository_clean": True,
                    "repository_head": request.expected_repository_head,
                }
            )
            recorded = (
                bindings.control.TransactionStateResolutionRequest.as_json(
                    request
                )
            )
            recorded["expected_repository_head"] = (
                bindings.REQUIRED_PRIOR_REPOSITORY_HEAD
            )
            write_json(root / "request.json", recorded)
            expected_binding = dict(upstream.evidence)
            expected_binding["repository_head"] = (
                bindings.REQUIRED_PRIOR_REPOSITORY_HEAD
            )
            write_json(root / "binding-preflight.json", expected_binding)

            head = bindings._validate_request_and_binding(
                request, upstream, root
            )

            self.assertEqual(head, bindings.REQUIRED_PRIOR_REPOSITORY_HEAD)
            recorded["expected_repository_head"] = request.expected_repository_head
            write_json(root / "request.json", recorded)
            with self.assertRaisesRegex(
                ValueError, "prior-transaction-state-resolution-head-invalid"
            ):
                bindings._validate_request_and_binding(request, upstream, root)

    def test_terminal_and_forbidden_action_boundary_are_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_transaction_state_resolution_root
            root.mkdir(mode=0o700)
            upstream = SimpleNamespace(
                evidence={"prior_transaction": "state-indeterminate"}
            )
            value = terminal(request)
            write_json(root / "terminal.json", value)

            bindings._validate_terminal(request, upstream, root)

            value["maintenance_resume_invocations"] = 1
            write_json(root / "terminal.json", value)
            with self.assertRaisesRegex(
                ValueError,
                "prior-transaction-state-resolution-terminal-invalid",
            ):
                bindings._validate_terminal(request, upstream, root)

            for name in bindings.REQUIRED_ENTRY_NAMES:
                write_json(root / name, {})
            write_json(
                root / "guest-control-root-create.json",
                {"argv": ["utmctl", "status", request.target_uuid]},
            )
            with self.assertRaisesRegex(
                ValueError,
                "prior-transaction-state-resolution-forbidden-command",
            ):
                bindings._validate_no_forbidden_commands(root)


def make_request(
    root: Path,
) -> resolution_control.TransactionStateResultResolutionRequest:
    return resolution_test.make_request(root)


@contextlib.contextmanager
def patched_validation(manifest_sha256: str):
    upstream = SimpleNamespace(
        evidence={
            "format": bindings.bindings.EVIDENCE_FORMAT,
            "prior_transaction": "state-indeterminate",
            "repository_clean": True,
            "repository_head": "b" * 40,
        },
        baseline_inventory=(),
        probe_bytes=b"probe",
    )
    semantic_names = (
        "_validate_request_and_binding",
        "_validate_source_target",
        "_validate_inventory",
        "_validate_process_evidence",
        "_validate_handle_evidence",
        "_validate_start_and_readiness",
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
                "validate_transaction_state_resolution_bindings",
                return_value=upstream,
            )
        )
        stack.enter_context(
            mock.patch.object(bindings.network_ready, "_require_committed_regular")
        )
        stack.enter_context(
            mock.patch.object(
                bindings.runtime_control, "_require_no_raw_operation_id"
            )
        )
        returns = {
            "_validate_request_and_binding": "a" * 40,
            "_validate_process_evidence": ({"role": "qemu"},),
            "_validate_handle_evidence": ("d" * 64, 378),
            "_validate_probe_and_missing_result": "e" * 64,
        }
        for name in semantic_names:
            stack.enter_context(
                mock.patch.object(bindings, name, return_value=returns.get(name))
            )
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


def terminal(request: SimpleNamespace) -> dict[str, object]:
    return {
        "agent_readiness_invocations": 10,
        "agent_readiness_state": "ready",
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": 36343,
        "boot_classification": "not-generated",
        "boot_start_attempt_id": request.transaction_state_resolution_attempt_id,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 3,
        "file_push_invocations": 1,
        "foreground_start_invocations": 1,
        "format": bindings.control.EVIDENCE_FORMAT,
        "guest_exec_invocations": 13,
        "guest_probe_invocations": 1,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 1,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": None,
        "operation_id": "not-read-or-generated",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "attempted-once-as-potential-backend-reactivation",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "readiness_identity_observation_count": 10,
        "reason": "guest-result-readback-1:guest-result-readback-1-stderr-not-empty",
        "result_readback_invocations": 1,
        "runtime_poll_count": 1,
        "target_handles_terminal": "present",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "state-indeterminate",
    }


def write_json(path: Path, value: dict[str, object]) -> None:
    path.write_text(
        json.dumps(value, sort_keys=True) + "\n", encoding="utf-8"
    )
    path.chmod(0o600)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
from __future__ import annotations

import base64
import contextlib
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_transaction_state_result_resolution_result_bindings as bindings
import test_l6_utm_install_artifacts_staged_transaction_state_result_resolution as resolution_test


class RequestProxy:
    def __init__(self, base, root: Path) -> None:
        self._base = base
        self.prior_transaction_state_result_resolution_root = (
            root / "prior-transaction-state-result-resolution"
        )
        self.prior_transaction_state_result_resolution_manifest_sha256 = (
            bindings.REQUIRED_PRIOR_MANIFEST_SHA256
        )
        self.prior_transaction_state_result_resolution_attempt_id = (
            bindings.REQUIRED_PRIOR_ATTEMPT_ID
        )

    def __getattr__(self, name: str):
        return getattr(self._base, name)


class TransactionStateResultResolutionResultBindingTests(unittest.TestCase):
    def test_manifest_binds_all_twenty_one_members_before_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_transaction_state_result_resolution_root,
                bindings.REQUIRED_ENTRY_NAMES,
            )
            request.prior_transaction_state_result_resolution_manifest_sha256 = (
                manifest_sha256
            )

            with patched_validation(manifest_sha256):
                result = (
                    bindings.validate_transaction_state_result_resolution_result_bindings(
                        request
                    )
                )

            self.assertEqual(
                result.evidence[
                    "prior_transaction_state_result_resolution_entries_verified"
                ],
                21,
            )
            self.assertEqual(result.evidence["transaction"], "completed")
            self.assertEqual(result.prior_backend_pid, 36343)

    def test_manifest_member_order_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_transaction_state_result_resolution_root,
                tuple(reversed(bindings.REQUIRED_ENTRY_NAMES)),
            )
            request.prior_transaction_state_result_resolution_manifest_sha256 = (
                manifest_sha256
            )

            with patched_validation(manifest_sha256), self.assertRaisesRegex(
                ValueError,
                "prior-transaction-state-result-resolution-entry-set-invalid",
            ):
                bindings.validate_transaction_state_result_resolution_result_bindings(
                    request
                )

    def test_recorded_execution_head_is_separate_from_successor_head(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_transaction_state_result_resolution_root
            root.mkdir(mode=0o700)
            upstream = make_upstream()
            recorded = (
                bindings.control.TransactionStateResultResolutionRequest.as_json(
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

            self.assertEqual(
                bindings._validate_request_and_binding(request, upstream, root),
                bindings.REQUIRED_PRIOR_REPOSITORY_HEAD,
            )

            recorded["expected_repository_head"] = request.expected_repository_head
            write_json(root / "request.json", recorded)
            with self.assertRaisesRegex(
                ValueError,
                "prior-transaction-state-result-resolution-head-invalid",
            ):
                bindings._validate_request_and_binding(request, upstream, root)

    def test_handles_keep_exact_prior_pid_and_byte_signature(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_transaction_state_result_resolution_root
            root.mkdir(mode=0o700)
            upstream = make_upstream()
            names = (
                "target-handle-pid-discovery.json",
                "target-handle-pid-confirmation-001.json",
                "target-handle-pid-confirmation-002.json",
                "target-handle-pid-confirmation-003.json",
                "target-handle-pid-terminal.json",
            )
            for index, name in enumerate(names):
                argv = (
                    bindings.network_ready._lsof_argv(request)
                    if index == 0
                    else bindings.runtime_control.targeted_lsof_argv(
                        request, bindings.REQUIRED_BACKEND_PID
                    )
                )
                write_json(root / name, handle_value(argv))

            self.assertEqual(
                bindings._validate_handle_evidence(request, upstream, root),
                (
                    bindings.REQUIRED_HANDLE_SHA256,
                    bindings.REQUIRED_HANDLE_SIZE,
                ),
            )

            drift = handle_value(
                bindings.runtime_control.targeted_lsof_argv(
                    request, bindings.REQUIRED_BACKEND_PID
                )
            )
            drift["observation"]["stdout"]["sha256"] = "d" * 64
            write_json(root / "target-handle-pid-terminal.json", drift)
            with self.assertRaisesRegex(
                ValueError,
                "prior-transaction-state-result-resolution-handle-drift",
            ):
                bindings._validate_handle_evidence(request, upstream, root)

    def test_double_result_and_completed_phase_are_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_transaction_state_result_resolution_root
            root.mkdir(mode=0o700)
            upstream = make_upstream()
            payload = completed_payload(request, upstream.probe_sha256)
            self.assertEqual(
                hashlib.sha256(payload).hexdigest(),
                bindings.REQUIRED_GUEST_RESULT_SHA256,
            )
            result_argv = (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                request.guest_result_path,
            )
            for index in (1, 2):
                write_observation(
                    root
                    / f"existing-transaction-state-result-readback-{index}.json",
                    result_argv,
                    payload,
                )
            resolution = (
                bindings.control.prior_control.parse_transaction_state_result(
                    payload, request, upstream.probe_sha256
                )
            )
            write_json(
                root / "existing-transaction-state-result.json",
                {
                    "evidence": resolution.evidence,
                    "format": bindings.control.EVIDENCE_FORMAT,
                    "outcome": resolution.outcome,
                    "reason": resolution.reason,
                    "transaction": resolution.transaction,
                },
            )
            phase = bindings.control.prior_control.guest_probe.canonical_json(
                {
                    "format": (
                        bindings.control.prior_control.guest_probe.PHASE_FORMAT
                    ),
                    "phase": "completed",
                }
            )
            write_observation(
                root / "existing-transaction-state-phase-readback.json",
                (
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    request.guest_transaction_phase_path,
                ),
                phase,
            )

            result = bindings._validate_result_and_phase(
                request, upstream, root
            )

            self.assertEqual(result.transaction, "completed")
            write_observation(
                root / "existing-transaction-state-result-readback-2.json",
                result_argv,
                payload + b"\n",
            )
            with self.assertRaisesRegex(
                ValueError,
                "prior-transaction-state-result-resolution-result-drift",
            ):
                bindings._validate_result_and_phase(request, upstream, root)

    def test_terminal_refuses_any_mutation_count(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_transaction_state_result_resolution_root
            root.mkdir(mode=0o700)
            value = terminal(request)
            write_json(root / "terminal.json", value)

            bindings._validate_terminal(request, root)

            value["maintenance_resume_invocations"] = 1
            write_json(root / "terminal.json", value)
            with self.assertRaisesRegex(
                ValueError,
                "prior-transaction-state-result-resolution-terminal-invalid",
            ):
                bindings._validate_terminal(request, root)


def make_request(root: Path) -> RequestProxy:
    base = resolution_test.make_request(root)
    return RequestProxy(base, root)


def make_upstream():
    return SimpleNamespace(
        evidence={
            "format": bindings.control.EVIDENCE_FORMAT,
            "repository_clean": True,
            "repository_head": "b" * 40,
        },
        prior_backend_pid=bindings.REQUIRED_BACKEND_PID,
        prior_handle_sha256=bindings.REQUIRED_HANDLE_SHA256,
        prior_handle_size=bindings.REQUIRED_HANDLE_SIZE,
        probe_sha256=(
            "e706d6225d69407fff354f37efd7e28a5221fc7cb7fd772b09536fe46accd0e3"
        ),
    )


@contextlib.contextmanager
def patched_validation(manifest_sha256: str):
    upstream = make_upstream()
    semantic_names = (
        "_validate_request_and_binding",
        "_validate_source_target",
        "_validate_process_evidence",
        "_validate_handle_evidence",
        "_validate_result_and_phase",
        "_validate_terminal",
        "_validate_command_inventory",
    )
    returns = {
        "_validate_request_and_binding": "a" * 40,
        "_validate_process_evidence": bindings.REQUIRED_EXCLUDED_GENERIC_QEMU,
        "_validate_handle_evidence": (
            bindings.REQUIRED_HANDLE_SHA256,
            bindings.REQUIRED_HANDLE_SIZE,
        ),
        "_validate_result_and_phase": SimpleNamespace(transaction="completed"),
    }
    with contextlib.ExitStack() as stack:
        stack.enter_context(
            mock.patch.object(
                bindings, "REQUIRED_PRIOR_MANIFEST_SHA256", manifest_sha256
            )
        )
        stack.enter_context(
            mock.patch.object(
                bindings.control,
                "validate_transaction_state_result_resolution_bindings",
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
        for name in semantic_names:
            stack.enter_context(
                mock.patch.object(bindings, name, return_value=returns.get(name))
            )
        yield


def completed_payload(request: RequestProxy, probe_sha256: str) -> bytes:
    return bindings.control.prior_control.guest_probe.canonical_json(
        {
            "attempt_id": request.transaction_state_resolution_attempt_id,
            "automatic_cleanup": "not-performed",
            "automatic_retry": "not-performed",
            "boot_id_sha256": bindings.REQUIRED_BOOT_ID_SHA256,
            "dpkg_command_profile": "audit-plus-verify-read-only",
            "dpkg_log_sha256": bindings.REQUIRED_DPKG_LOG_SHA256,
            "dpkg_log_size": bindings.REQUIRED_DPKG_LOG_SIZE,
            "dpkg_mutation": "not-performed",
            "dpkg_status_sha256": bindings.REQUIRED_DPKG_STATUS_SHA256,
            "format": bindings.control.prior_control.guest_probe.EVIDENCE_FORMAT,
            "guard_profile": "absent-after-reboot",
            "maintenance_invocations": 0,
            "operation_id": "hash-only",
            "operation_id_sha256": (
                bindings.control.prior_control.guest_probe.EXPECTED_OPERATION_ID_SHA256
            ),
            "outcome": "completed",
            "package_profile": "installed-verified",
            "probe_sha256": probe_sha256,
            "product_state_write": "not-performed",
            "reason": "read-only-transaction-state-observed",
            "receipt_sha256": bindings.REQUIRED_RECEIPT_SHA256,
            "receipt_size": bindings.REQUIRED_RECEIPT_SIZE,
            "resume_invocations": 0,
            "startup_profile": "allowed",
            "target_uuid": request.target_uuid,
            "transaction": "completed",
        }
    )


def handle_value(argv: tuple[str, ...]) -> dict[str, object]:
    return {
        "backend_command": "QEMULauncher",
        "backend_pid": bindings.REQUIRED_BACKEND_PID,
        "efi_handle_count": 1,
        "format": bindings.runtime_control.EVIDENCE_FORMAT,
        "observation": {
            "argv": list(argv),
            "exit_code": 0,
            "stderr": empty_stream(),
            "stdout": {
                "sha256": bindings.REQUIRED_HANDLE_SHA256,
                "total_bytes": bindings.REQUIRED_HANDLE_SIZE,
                "truncated": False,
            },
            "timed_out": False,
        },
        "process_record_count": 1,
        "qcow2_handle_count": 1,
        "state": "present",
    }


def terminal(request: RequestProxy) -> dict[str, object]:
    return {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": bindings.REQUIRED_BACKEND_PID,
        "business_guest_action": "existing-result-readback-only",
        "file_pull_invocations": 3,
        "file_push_invocations": 0,
        "format": bindings.control.EVIDENCE_FORMAT,
        "guest_exec_invocations": 0,
        "guest_probe_invocations": 0,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 0,
        "maintenance_resume_invocations": 0,
        "operation_id": "hash-only",
        "outcome": "transaction-completed",
        "phase_readback_invocations": 1,
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "prior_transaction_state_resolution_attempt_id": (
            request.prior_transaction_state_resolution_attempt_id
        ),
        "reason": "deferred-readback-transaction-completed",
        "result_readback_invocations": 2,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_excluded_generic_qemu_process_count": 1,
        "terminal_relevant_host_process_count": 0,
        "transaction": "completed",
        "transaction_state_result_resolution_attempt_id": (
            request.transaction_state_result_resolution_attempt_id
        ),
    }


def write_manifest(root: Path, names: tuple[str, ...]) -> str:
    root.mkdir(mode=0o700)
    for name in set(names):
        write_json(root / name, {})
    manifest = root / "files.sha256"
    manifest.write_text(
        "".join(f"{sha256(root / name)}  {name}\n" for name in names),
        encoding="ascii",
    )
    manifest.chmod(0o600)
    return sha256(manifest)


def write_observation(path: Path, argv: tuple[str, ...], payload: bytes) -> None:
    write_json(
        path,
        {
            "argv": list(argv),
            "exit_code": 0,
            "stderr": {
                **empty_stream(),
                "prefix_base64": "",
                "prefix_utf8": "",
            },
            "stdout": {
                "prefix_base64": base64.b64encode(payload).decode("ascii"),
                "prefix_utf8": payload.decode("utf-8"),
                "sha256": hashlib.sha256(payload).hexdigest(),
                "total_bytes": len(payload),
                "truncated": False,
            },
            "timed_out": False,
        },
    )


def empty_stream() -> dict[str, object]:
    return {
        "sha256": (
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        ),
        "total_bytes": 0,
        "truncated": False,
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

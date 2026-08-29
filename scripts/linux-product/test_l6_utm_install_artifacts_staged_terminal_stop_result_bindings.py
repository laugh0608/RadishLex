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

import l6_utm_install_artifacts_staged_terminal_stop_result_bindings as bindings
import test_l6_utm_install_artifacts_staged_guest_agent_resolution as guest_agent_test
import test_l6_utm_install_artifacts_staged_runtime_resolution as runtime_test
import test_l6_utm_install_artifacts_staged_terminal_stop as terminal_stop_test


class RequestProxy:
    def __init__(self, base, root: Path) -> None:
        self._base = base
        self.prior_terminal_stop_root = root / "prior-terminal-stop"
        self.prior_terminal_stop_manifest_sha256 = (
            bindings.REQUIRED_PRIOR_MANIFEST_SHA256
        )
        self.prior_terminal_stop_attempt_id = bindings.REQUIRED_PRIOR_ATTEMPT_ID

    def __getattr__(self, name: str):
        return getattr(self._base, name)


class TerminalStopResultBindingTests(unittest.TestCase):
    def test_manifest_binds_all_twenty_eight_members_before_semantics(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_terminal_stop_root,
                bindings.REQUIRED_ENTRY_NAMES,
            )
            request.prior_terminal_stop_manifest_sha256 = manifest_sha256

            with patched_validation(manifest_sha256):
                result = bindings.validate_terminal_stop_result_bindings(
                    request
                )

            self.assertEqual(
                result.evidence["prior_terminal_stop_entries_verified"], 28
            )
            self.assertEqual(result.evidence["outcome"], "stopped-verified")
            self.assertEqual(result.backend_pid, bindings.REQUIRED_BACKEND_PID)

    def test_manifest_member_order_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_terminal_stop_root,
                tuple(reversed(bindings.REQUIRED_ENTRY_NAMES)),
            )
            request.prior_terminal_stop_manifest_sha256 = manifest_sha256

            with patched_validation(manifest_sha256), self.assertRaisesRegex(
                ValueError, "prior-terminal-stop-entry-set-invalid"
            ):
                bindings.validate_terminal_stop_result_bindings(request)

    def test_recorded_execution_head_is_separate_from_successor_head(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_terminal_stop_root
            root.mkdir(mode=0o700)
            upstream = make_upstream()
            recorded = bindings.control.terminal_stop_request_json(request)
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
                ValueError, "prior-terminal-stop-head-invalid"
            ):
                bindings._validate_request_and_binding(request, upstream, root)

    def test_inventory_binds_started_to_stopped_transition(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_terminal_stop_root
            root.mkdir(mode=0o700)
            upstream = make_upstream()
            preflight = guest_agent_test.observation(
                ("utmctl", "list"),
                stdout=runtime_test.inventory_stdout(
                    request, target_status="started", other_active=False
                ),
            )
            terminal = guest_agent_test.observation(
                ("utmctl", "list"),
                stdout=runtime_test.inventory_stdout(
                    request, target_status="stopped", other_active=False
                ),
            )
            write_json(root / "utmctl-list-preflight.json", preflight.as_json())
            write_json(root / "utmctl-list-terminal.json", terminal.as_json())
            preflight_inventory = bindings.start_control.parse_utmctl_list(
                preflight
            )
            terminal_inventory = bindings.start_control.parse_utmctl_list(
                terminal
            )
            write_json(
                root / "inventory-preflight-classification.json",
                bindings.control._inventory_evidence(
                    request, preflight_inventory, "started"
                ),
            )
            write_json(
                root / "inventory-terminal-classification.json",
                bindings.control._inventory_evidence(
                    request, terminal_inventory, "stopped"
                ),
            )

            observed = bindings._validate_inventory_transition(
                request, upstream, root
            )

            self.assertEqual(len(observed[0]), 21)
            self.assertEqual(len(observed[1]), 21)
            terminal_bad = guest_agent_test.observation(
                ("utmctl", "list"),
                stdout=runtime_test.inventory_stdout(
                    request, target_status="started", other_active=False
                ),
            )
            write_json(
                root / "utmctl-list-terminal.json", terminal_bad.as_json()
            )
            with self.assertRaisesRegex(
                ValueError,
                "prior-terminal-stop-inventory-transition-invalid",
            ):
                bindings._validate_inventory_transition(request, upstream, root)

    def test_handle_chain_binds_prior_pid_then_proves_absence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_terminal_stop_root
            root.mkdir(mode=0o700)
            untargeted = bindings.network_ready._lsof_argv(request)
            targeted = bindings.runtime_control.targeted_lsof_argv(
                request, bindings.REQUIRED_BACKEND_PID
            )
            write_json(
                root / "target-handles-preflight.json",
                present_handle(untargeted, bindings.control.EVIDENCE_FORMAT),
            )
            write_json(
                root / "target-handle-pid-discovery.json",
                present_handle(
                    untargeted,
                    bindings.runtime_control.EVIDENCE_FORMAT,
                    include_pid=True,
                ),
            )
            for index in range(1, 4):
                write_json(
                    root / f"target-handle-pid-confirmation-{index:03d}.json",
                    present_handle(
                        targeted,
                        bindings.runtime_control.EVIDENCE_FORMAT,
                        include_pid=True,
                    ),
                )
            write_json(
                root / "target-handles-quiescence-001.json",
                present_handle(untargeted, bindings.control.EVIDENCE_FORMAT),
            )
            for name in (
                "target-handles-quiescence-002.json",
                "target-handles-terminal.json",
            ):
                write_json(root / name, absent_handle(untargeted))

            bindings._validate_handle_evidence(request, root)

            drift = absent_handle(untargeted)
            drift["observation"]["exit_code"] = 0
            write_json(root / "target-handles-terminal.json", drift)
            with self.assertRaisesRegex(
                ValueError, "prior-terminal-stop-handle-absent-invalid"
            ):
                bindings._validate_handle_evidence(request, root)

    def test_command_inventory_refuses_any_extra_utmctl_command(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_terminal_stop_root
            root.mkdir(mode=0o700)
            expected = command_inventory(request)
            for name in bindings.REQUIRED_ENTRY_NAMES:
                value: dict[str, object] = {}
                if name in expected:
                    value = {"observation": {"argv": expected[name]}}
                write_json(root / name, value)

            bindings._validate_command_inventory(request, root)

            write_json(
                root / "terminal.json",
                {"observation": {"argv": ["utmctl", "status"]}},
            )
            with self.assertRaisesRegex(
                ValueError, "prior-terminal-stop-command-inventory-invalid"
            ):
                bindings._validate_command_inventory(request, root)


def make_request(root: Path) -> RequestProxy:
    return RequestProxy(terminal_stop_test.make_request(root), root)


def make_upstream() -> bindings.control.TerminalStopBinding:
    return bindings.control.TerminalStopBinding(
        evidence={
            "format": bindings.control.EVIDENCE_FORMAT,
            "repository_clean": True,
            "repository_head": "b" * 40,
        },
        upstream=SimpleNamespace(),
        baseline_inventory=runtime_test.baseline_inventory(),
        prior_backend_pid=bindings.REQUIRED_BACKEND_PID,
        prior_handle_sha256=bindings.REQUIRED_HANDLE_SHA256,
        prior_handle_size=bindings.REQUIRED_HANDLE_SIZE,
    )


@contextlib.contextmanager
def patched_validation(manifest_sha256: str):
    upstream = make_upstream()
    semantic_names = (
        "_validate_request_and_binding",
        "_validate_source_target",
        "_validate_process_evidence",
        "_validate_handle_evidence",
        "_validate_inventory_transition",
        "_validate_stop_observation",
        "_validate_terminal",
        "_validate_command_inventory",
    )
    returns = {
        "_validate_request_and_binding": "a" * 40,
        "_validate_process_evidence": bindings.REQUIRED_EXCLUDED_GENERIC_QEMU,
        "_validate_inventory_transition": (
            runtime_test.baseline_inventory(),
            runtime_test.baseline_inventory(),
        ),
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
                "validate_terminal_stop_bindings",
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


def present_handle(
    argv: tuple[str, ...], evidence_format: str, *, include_pid: bool = False
) -> dict[str, object]:
    value: dict[str, object] = {
        "backend_command": "QEMULauncher",
        "efi_handle_count": 1,
        "format": evidence_format,
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
    if include_pid:
        value["backend_pid"] = bindings.REQUIRED_BACKEND_PID
    return value


def absent_handle(argv: tuple[str, ...]) -> dict[str, object]:
    return {
        "backend_command": None,
        "efi_handle_count": 0,
        "format": bindings.control.EVIDENCE_FORMAT,
        "observation": {
            "argv": list(argv),
            "exit_code": 1,
            "stderr": empty_stream(),
            "stdout": empty_stream(),
            "timed_out": False,
        },
        "process_record_count": 0,
        "qcow2_handle_count": 0,
        "state": "absent",
    }


def command_inventory(request: RequestProxy) -> dict[str, list[str]]:
    process = list(bindings.launch_transport.PROCESS_COMMAND)
    untargeted = list(bindings.network_ready._lsof_argv(request))
    targeted = list(
        bindings.runtime_control.targeted_lsof_argv(
            request, bindings.REQUIRED_BACKEND_PID
        )
    )
    expected = {
        "host-process-preflight.json": process,
        "target-handles-preflight.json": untargeted,
        "utmctl-list-preflight.json": ["utmctl", "list"],
        "target-handle-pid-discovery.json": untargeted,
        "utmctl-stop-request-once.json": [
            "utmctl",
            "stop",
            request.target_uuid,
            "--request",
        ],
        "host-process-quiescence-001.json": process,
        "target-handles-quiescence-001.json": untargeted,
        "host-process-quiescence-002.json": process,
        "target-handles-quiescence-002.json": untargeted,
        "utmctl-list-terminal.json": ["utmctl", "list"],
        "host-process-terminal.json": process,
        "target-handles-terminal.json": untargeted,
    }
    for index in range(1, 4):
        expected[f"target-handle-pid-confirmation-{index:03d}.json"] = targeted
        expected[f"host-process-confirmation-{index:03d}.json"] = process
    return expected


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


def empty_stream() -> dict[str, object]:
    return {
        "sha256": hashlib.sha256(b"").hexdigest(),
        "total_bytes": 0,
        "truncated": False,
    }


def write_json(path: Path, value: dict[str, object]) -> None:
    path.write_text(json.dumps(value, sort_keys=True) + "\n", encoding="utf-8")
    path.chmod(0o600)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


if __name__ == "__main__":
    unittest.main()

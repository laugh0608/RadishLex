#!/usr/bin/env python3
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_terminal_stop as control
import test_l6_utm_install_artifacts_staged_guest_agent_resolution as guest_agent_test
import test_l6_utm_install_artifacts_staged_runtime_resolution as runtime_test
import test_l6_utm_install_artifacts_staged_transaction_state_result_resolution as result_resolution_test


class RequestProxy:
    def __init__(self, base, root: Path) -> None:
        self._base = base
        self.output_root = root / "terminal-stop-output"
        self.prior_transaction_state_result_resolution_root = (
            root / "prior-completed-result"
        )
        self.prior_transaction_state_result_resolution_manifest_sha256 = (
            control.completed_bindings.REQUIRED_PRIOR_MANIFEST_SHA256
        )
        self.prior_transaction_state_result_resolution_attempt_id = (
            control.completed_bindings.REQUIRED_PRIOR_ATTEMPT_ID
        )
        self.terminal_stop_attempt_id = control.REQUIRED_ATTEMPT_ID
        self.stop_poll_attempts = 60
        self.stop_poll_interval_seconds = 1
        self.stop_timeout_seconds = 120
        self.authorized_terminal_stop_control = True
        self.authorized_bound_completed_transaction_result = True
        self.authorized_one_preflight_and_one_terminal_inventory = True
        self.authorized_at_most_one_graceful_stop_request = True
        self.authorized_bounded_host_only_quiescence_observation = True
        self.authorized_no_status_start_guest_file_resume_dpkg_retry_repair_cleanup_force_kill_or_quit = True

    def __getattr__(self, name: str):
        return getattr(self._base, name)


class StopRunner:
    def __init__(
        self,
        request: RequestProxy,
        *,
        preflight_status: str = "started",
        other_active: bool = False,
        stop_exit_code: int = 0,
        stop_stderr: bytes = b"",
        handles_remain: bool = False,
        backend_process_remains: bool = False,
        drift_confirmation: bool = False,
    ) -> None:
        self.request = request
        self.registered_status = preflight_status
        self.other_active = other_active
        self.stop_exit_code = stop_exit_code
        self.stop_stderr = stop_stderr
        self.handles_remain = handles_remain
        self.backend_process_remains = backend_process_remains
        self.drift_confirmation = drift_confirmation
        self.live = preflight_status == "started"
        self.stop_called = False
        self.calls: list[tuple[str, ...]] = []
        self.list_calls = 0
        self.targeted_handle_calls = 0

    def run(self, argv, timeout_seconds, *, stdin_file=None):
        del timeout_seconds, stdin_file
        self.calls.append(argv)
        if argv == control.launch_transport.PROCESS_COMMAND:
            if self.stop_called and self.backend_process_remains:
                return guest_agent_test.observation(
                    argv,
                    stdout=b"1 0 0 launchd\n99 1 501 QEMULauncher\n",
                )
            return guest_agent_test.observation(
                argv, stdout=runtime_test.quiet()
            )
        if argv[0:1] == ("/usr/sbin/lsof",):
            targeted = "-p" in argv
            if targeted:
                self.targeted_handle_calls += 1
            if not self.live:
                return guest_agent_test.observation(argv, exit_code=1)
            pid = 42
            if self.drift_confirmation and self.targeted_handle_calls == 2:
                pid = 43
            return guest_agent_test.observation(
                argv,
                stdout=runtime_test.handle_payload(
                    self.request, pid, "QEMULauncher"
                ),
            )
        if argv == ("utmctl", "list"):
            self.list_calls += 1
            return guest_agent_test.observation(
                argv,
                stdout=runtime_test.inventory_stdout(
                    self.request,
                    target_status=self.registered_status,
                    other_active=self.other_active and self.list_calls == 1,
                ),
            )
        if argv == (
            "utmctl",
            "stop",
            self.request.target_uuid,
            "--request",
        ):
            self.stop_called = True
            self.registered_status = "stopped"
            if not self.handles_remain:
                self.live = False
            return guest_agent_test.observation(
                argv,
                exit_code=self.stop_exit_code,
                stderr=self.stop_stderr,
            )
        raise AssertionError(f"unexpected command: {argv}")


class TerminalStopTests(unittest.TestCase):
    def test_request_requires_exact_scope_and_separate_authorizations(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            control.validate_terminal_stop_request(request)

            request.authorized_at_most_one_graceful_stop_request = False
            with self.assertRaisesRegex(
                control.TerminalStopError,
                "at-most-one-graceful-stop-authorization-required",
            ):
                control.validate_terminal_stop_request(request)

            request.authorized_at_most_one_graceful_stop_request = True
            request.stop_poll_attempts = 59
            with self.assertRaisesRegex(
                control.TerminalStopError,
                "stop-poll-attempts-must-be-exactly-60",
            ):
                control.validate_terminal_stop_request(request)

    def test_binding_requires_completed_result_and_exact_twenty_vm_baseline(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            completed = make_completed_binding(request)
            with mock.patch.object(
                control.network_ready,
                "_require_committed_regular",
            ), mock.patch.object(
                control.completed_bindings,
                "validate_transaction_state_result_resolution_result_bindings",
                return_value=completed,
            ):
                binding = control.validate_terminal_stop_bindings(request)

            self.assertEqual(len(binding.baseline_inventory), 20)
            self.assertEqual(
                binding.evidence["completed_result_transaction"], "completed"
            )
            self.assertEqual(binding.prior_backend_pid, 36343)

            completed.evidence["transaction"] = "state-indeterminate"
            with mock.patch.object(
                control.network_ready,
                "_require_committed_regular",
            ), mock.patch.object(
                control.completed_bindings,
                "validate_transaction_state_result_resolution_result_bindings",
                return_value=completed,
            ), self.assertRaisesRegex(
                control.TerminalStopError,
                "completed-transaction-result-required",
            ):
                control.validate_terminal_stop_bindings(request)

    def test_started_target_uses_one_graceful_request_and_cross_checks_stopped(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = StopRunner(request)

            result, source = run_case(request, runner)

            self.assertEqual(result.outcome, "stopped-verified")
            self.assertEqual(result.exit_code, control.EXIT_STOPPED_VERIFIED)
            self.assertEqual(result.graceful_stop_invocations, 1)
            self.assertEqual(result.preflight_inventory_invocations, 1)
            self.assertEqual(result.terminal_inventory_invocations, 1)
            self.assertEqual(result.quiescence_poll_count, 1)
            self.assertTrue(source.file_object.closed)
            self.assertEqual(
                runner.calls.count(
                    (
                        "utmctl",
                        "stop",
                        request.target_uuid,
                        "--request",
                    )
                ),
                1,
            )
            self.assert_forbidden_commands_absent(runner.calls)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["transaction"], "completed-frozen-before-stop")
            self.assertEqual(terminal["plain_utmctl_list_invocations"], 2)
            self.assertEqual(terminal["automatic_force"], "not-performed")
            self.assertEqual(terminal["automatic_kill"], "not-performed")
            runtime_test.assert_manifest_valid(self, request.output_root)

    def test_already_stopped_is_verified_without_issuing_stop(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = StopRunner(request, preflight_status="stopped")

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "already-stopped-verified")
            self.assertEqual(result.graceful_stop_invocations, 0)
            self.assertEqual(result.terminal_inventory_invocations, 1)
            self.assertFalse(
                any(call[:2] == ("utmctl", "stop") for call in runner.calls)
            )
            self.assert_forbidden_commands_absent(runner.calls)

    def test_other_started_vm_fails_before_stop_and_never_guesses(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = StopRunner(request, other_active=True)

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, control.EXIT_STATE_INDETERMINATE)
            self.assertEqual(result.graceful_stop_invocations, 0)
            self.assertFalse(
                any(call[:2] == ("utmctl", "stop") for call in runner.calls)
            )

    def test_live_handle_drift_fails_before_stop(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = StopRunner(request, drift_confirmation=True)

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.graceful_stop_invocations, 0)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertIn("live-target-handle-identity-drift", terminal["reason"])

    def test_untrusted_stop_exit_is_not_promoted_by_stopped_postcondition(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = StopRunner(
                request,
                stop_exit_code=1,
                stop_stderr=b"synthetic stop failure",
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.graceful_stop_invocations, 1)
            self.assertEqual(
                runner.calls.count(
                    (
                        "utmctl",
                        "stop",
                        request.target_uuid,
                        "--request",
                    )
                ),
                1,
            )
            terminal = read_json(request.output_root / "terminal.json")
            self.assertIn("utmctl-stop-request-exit-1", terminal["stop_command_error"])
            self.assert_forbidden_commands_absent(runner.calls)

    def test_handles_remaining_exhausts_bound_without_retry_or_force(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = StopRunner(request, handles_remain=True)

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.graceful_stop_invocations, 1)
            self.assertEqual(result.quiescence_poll_count, 60)
            self.assertEqual(result.terminal_inventory_invocations, 0)
            self.assertEqual(
                sum(call[:2] == ("utmctl", "stop") for call in runner.calls),
                1,
            )
            self.assert_forbidden_commands_absent(runner.calls)

    def test_backend_process_remaining_never_counts_as_quiescent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = StopRunner(request, backend_process_remains=True)

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.graceful_stop_invocations, 1)
            self.assertEqual(result.quiescence_poll_count, 60)
            self.assertEqual(result.terminal_inventory_invocations, 0)
            self.assert_forbidden_commands_absent(runner.calls)

    def assert_forbidden_commands_absent(
        self, calls: list[tuple[str, ...]]
    ) -> None:
        flattened = [" ".join(call) for call in calls]
        for forbidden in (
            "utmctl status",
            "utmctl start",
            "utmctl exec",
            "utmctl file",
            " resume ",
            " retry ",
            " repair ",
            " cleanup ",
            " force ",
            " kill ",
            " quit ",
            " dpkg ",
        ):
            self.assertFalse(
                any(forbidden in value for value in flattened), forbidden
            )


def make_request(root: Path) -> RequestProxy:
    base = result_resolution_test.make_request(root)
    return RequestProxy(base, root)


def make_binding(request: RequestProxy) -> control.TerminalStopBinding:
    upstream = SimpleNamespace(
        boot_id_sha256="1" * 64,
        receipt_sha256="2" * 64,
    )
    return control.TerminalStopBinding(
        evidence={"format": control.EVIDENCE_FORMAT},
        upstream=upstream,
        baseline_inventory=runtime_test.baseline_inventory(),
        prior_backend_pid=36343,
        prior_handle_sha256="3" * 64,
        prior_handle_size=378,
    )


def make_completed_binding(request: RequestProxy):
    state_binding = SimpleNamespace(
        baseline_inventory=runtime_test.baseline_inventory()
    )
    prior_result_binding = SimpleNamespace(upstream=state_binding)
    result_control_binding = SimpleNamespace(upstream=prior_result_binding)
    return SimpleNamespace(
        boot_id_sha256="1" * 64,
        evidence={
            "package_profile": "installed-verified",
            "prior_transaction_state_result_resolution_entries_verified": 21,
            "startup_profile": "allowed",
            "transaction": "completed",
        },
        prior_backend_pid=36343,
        prior_handle_sha256="3" * 64,
        prior_handle_size=378,
        receipt_sha256="2" * 64,
        upstream=result_control_binding,
    )


def run_case(request: RequestProxy, runner: StopRunner):
    binding = make_binding(request)
    source = runtime_test.SyntheticSource(runtime_test.CloseTracker())
    result = control.run_terminal_stop(
        request,
        runner=runner,
        binding_validator=lambda _: binding,
        source_opener=lambda _: source,
        source_revalidator=lambda _request, _source: {
            "descriptor_unchanged": True,
            "format": "synthetic-source-v1",
            "inventory_unchanged": True,
            "sha256": request.source_bundle_sha256,
            "size": request.source_bundle_size,
        },
        target_validator=lambda _: runtime_test.target_identity(request),
        sleeper=lambda _: None,
    )
    return result, source


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


if __name__ == "__main__":
    unittest.main()

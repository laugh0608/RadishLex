#!/usr/bin/env python3
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_transaction_state_result_resolution as control
import test_l6_utm_install_artifacts_staged_guest_agent_resolution as guest_agent_test
import test_l6_utm_install_artifacts_staged_runtime_resolution as runtime_test
import test_l6_utm_install_artifacts_staged_transaction_state_resolution as prior_test


class ResolutionRunner:
    def __init__(
        self,
        request: control.TransactionStateResultResolutionRequest,
        binding: SimpleNamespace,
        *,
        result_payloads: list[bytes] | None = None,
        first_result_missing: bool = False,
        backend_pid: int = 42,
    ) -> None:
        self.request = request
        self.binding = binding
        self.result_payloads = list(
            result_payloads
            or [
                prior_test.result_payload("completed", binding.probe_sha256),
                prior_test.result_payload("completed", binding.probe_sha256),
            ]
        )
        self.first_result_missing = first_result_missing
        self.backend_pid = backend_pid
        self.calls: list[tuple[str, ...]] = []

    def run(self, argv, timeout_seconds, *, stdin_file=None):
        del timeout_seconds, stdin_file
        self.calls.append(argv)
        if argv == control.runtime_control.launch_transport.PROCESS_COMMAND:
            return guest_agent_test.observation(argv, stdout=runtime_test.quiet())
        if argv[0:1] == ("/usr/sbin/lsof",):
            return guest_agent_test.observation(
                argv,
                stdout=runtime_test.handle_payload(
                    self.request, self.backend_pid, "QEMULauncher"
                ),
            )
        if argv[0:3] == ("utmctl", "file", "pull"):
            path = argv[4]
            if path == self.request.guest_result_path:
                if self.first_result_missing:
                    self.first_result_missing = False
                    return guest_agent_test.observation(
                        argv, stderr=b"synthetic missing result"
                    )
                return guest_agent_test.observation(
                    argv, stdout=self.result_payloads.pop(0)
                )
            if path == self.request.guest_phase_path:
                transaction = json.loads(self.result_payloads_used[0])[
                    "transaction"
                ]
                return guest_agent_test.observation(
                    argv,
                    stdout=control.prior_control.guest_probe.canonical_json(
                        {
                            "format": control.prior_control.guest_probe.PHASE_FORMAT,
                            "phase": transaction,
                        }
                    ),
                )
        raise AssertionError(f"unexpected command: {argv}")

    @property
    def result_payloads_used(self) -> list[bytes]:
        result_pulls = [
            call
            for call in self.calls
            if call[0:3] == ("utmctl", "file", "pull")
            and call[4] == self.request.guest_result_path
        ]
        used = len(result_pulls)
        original = getattr(self, "_original_payloads", None)
        if original is None:
            raise AssertionError("original payloads missing")
        return original[:used]


class TransactionStateResultResolutionTests(unittest.TestCase):
    def test_completed_uses_only_two_result_and_one_phase_pull(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding(request)
            runner = make_runner(request, binding, "completed")

            result, source = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "transaction-completed")
            self.assertEqual(result.exit_code, control.EXIT_TRANSACTION_COMPLETED)
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertEqual(result.phase_readback_invocations, 1)
            self.assertTrue(source.file_object.closed)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["transaction"], "completed")
            self.assertEqual(terminal["file_pull_invocations"], 3)
            self.assertEqual(terminal["file_push_invocations"], 0)
            self.assertEqual(terminal["guest_exec_invocations"], 0)
            self.assertEqual(terminal["guest_probe_invocations"], 0)
            self.assert_forbidden_actions_absent(runner.calls)
            runtime_test.assert_manifest_valid(self, request.output_root)

    def test_artifacts_staged_is_authoritative_without_resume(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding(request)
            runner = make_runner(request, binding, "artifacts-staged")

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "transaction-artifacts-staged")
            self.assertEqual(result.exit_code, control.EXIT_ARTIFACTS_STAGED)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["transaction"], "artifacts-staged")
            self.assertEqual(terminal["maintenance_resume_invocations"], 0)
            self.assert_forbidden_actions_absent(runner.calls)

    def test_guest_indeterminate_remains_authoritative_and_read_only(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding(request)
            runner = make_runner(request, binding, "state-indeterminate")

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, control.EXIT_STATE_INDETERMINATE)
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertEqual(result.phase_readback_invocations, 1)
            self.assert_forbidden_actions_absent(runner.calls)

    def test_missing_first_result_stays_indeterminate_without_retry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding(request)
            runner = make_runner(request, binding, "completed")
            runner.first_result_missing = True

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.result_readback_invocations, 1)
            self.assertEqual(result.phase_readback_invocations, 0)
            pulls = [
                call
                for call in runner.calls
                if call[0:3] == ("utmctl", "file", "pull")
            ]
            self.assertEqual(len(pulls), 1)
            self.assert_forbidden_actions_absent(runner.calls)

    def test_double_readback_drift_stays_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding(request)
            first = prior_test.result_payload("completed", binding.probe_sha256)
            second_value = json.loads(first)
            second_value["reason"] = "synthetic-drift"
            second = control.prior_control.guest_probe.canonical_json(second_value)
            runner = ResolutionRunner(
                request, binding, result_payloads=[first, second]
            )
            runner._original_payloads = [first, second]

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertEqual(result.phase_readback_invocations, 0)
            self.assertFalse(
                (request.output_root / "existing-transaction-state-result.json").exists()
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_backend_pid_or_handle_drift_rejects_before_guest_pull(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding(request)
            runner = make_runner(request, binding, "completed", backend_pid=99)

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.result_readback_invocations, 0)
            self.assertFalse(
                any(
                    call[0:3] == ("utmctl", "file", "pull")
                    for call in runner.calls
                )
            )

    def test_binding_and_authorization_contract_are_fixed(self) -> None:
        request = make_request(Path("/tmp/radishlex-transaction-result-binding"))
        upstream = make_upstream_result_binding(request)
        with mock.patch.object(
            control.result_bindings,
            "validate_transaction_state_resolution_result_bindings",
            return_value=upstream,
        ), mock.patch.object(control.network_ready, "_require_committed_regular"):
            binding = control.validate_transaction_state_result_resolution_bindings(
                request
            )

        self.assertEqual(binding.prior_backend_pid, 42)
        self.assertEqual(
            binding.evidence[
                "prior_transaction_state_resolution_entries_verified"
            ],
            67,
        )
        unauthorized = replace_request(
            request,
            authorized_two_existing_transaction_state_result_readbacks=False,
        )
        with self.assertRaisesRegex(
            control.TransactionStateResultResolutionError,
            "two-existing-transaction-state-result-readbacks-authorization-required",
        ):
            unauthorized.validate()
        overlap = replace_request(
            request,
            output_root=request.prior_transaction_state_resolution_root / "new",
        )
        with self.assertRaisesRegex(
            control.TransactionStateResultResolutionError,
            "output-root-must-not-overlap-prior-transaction-state-resolution-root",
        ):
            overlap.validate()

    def assert_forbidden_actions_absent(
        self, calls: list[tuple[str, ...]]
    ) -> None:
        flattened = [" ".join(call) for call in calls]
        for forbidden in (
            "utmctl list",
            "utmctl status",
            "utmctl start",
            "utmctl exec",
            "utmctl file push",
            " resume ",
            " retry ",
            " repair ",
            " cleanup ",
            " stop ",
            " quit ",
            " dpkg ",
        ):
            self.assertFalse(any(forbidden in value for value in flattened))


def make_request(root: Path) -> control.TransactionStateResultResolutionRequest:
    base = prior_test.make_request(root)
    values = dict(base.__dict__)
    values.update(
        output_root=root / "transaction-state-result-resolution-output",
        prior_transaction_state_resolution_root=(
            root / "prior-transaction-state-resolution"
        ),
        prior_transaction_state_resolution_manifest_sha256=(
            control.REQUIRED_PRIOR_MANIFEST_SHA256
        ),
        prior_transaction_state_resolution_attempt_id=(
            control.REQUIRED_PRIOR_ATTEMPT_ID
        ),
        transaction_state_result_resolution_attempt_id=(
            control.REQUIRED_ATTEMPT_ID
        ),
        authorized_transaction_state_result_resolution=True,
        authorized_bound_prior_transaction_state_resolution_result=True,
        authorized_two_existing_transaction_state_result_readbacks=True,
        authorized_one_existing_transaction_state_phase_readback=True,
        authorized_no_inventory_list_status_start_push_exec_probe_resume_dpkg_retry_repair_cleanup_or_automatic_stop=True,
    )
    return control.TransactionStateResultResolutionRequest(**values)


def replace_request(
    request: control.TransactionStateResultResolutionRequest,
    **changes: object,
) -> control.TransactionStateResultResolutionRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return control.TransactionStateResultResolutionRequest(**values)


def make_binding(request: control.TransactionStateResultResolutionRequest):
    payload = runtime_test.handle_payload(request, 42, "QEMULauncher")
    return SimpleNamespace(
        evidence={"format": control.EVIDENCE_FORMAT},
        prior_backend_pid=42,
        prior_handle_sha256=__import__("hashlib").sha256(payload).hexdigest(),
        prior_handle_size=len(payload),
        probe_sha256="e" * 64,
    )


def make_upstream_result_binding(
    request: control.TransactionStateResultResolutionRequest,
):
    binding = make_binding(request)
    binding.evidence = {
        "prior_transaction_state_resolution_entries_verified": 67
    }
    return binding


def make_runner(
    request: control.TransactionStateResultResolutionRequest,
    binding: SimpleNamespace,
    transaction: str,
    *,
    backend_pid: int = 42,
) -> ResolutionRunner:
    payload = prior_test.result_payload(transaction, binding.probe_sha256)
    runner = ResolutionRunner(
        request,
        binding,
        result_payloads=[payload, payload],
        backend_pid=backend_pid,
    )
    runner._original_payloads = [payload, payload]
    return runner


def run_case(
    request: control.TransactionStateResultResolutionRequest,
    binding: SimpleNamespace,
    runner: ResolutionRunner,
):
    source = prior_test.boot_test.SyntheticSource(prior_test.boot_test.CloseTracker())
    result = control.run_transaction_state_result_resolution(
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
    )
    return result, source


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


if __name__ == "__main__":
    unittest.main()

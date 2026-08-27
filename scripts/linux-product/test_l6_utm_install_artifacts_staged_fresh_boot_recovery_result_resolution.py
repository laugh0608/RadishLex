#!/usr/bin/env python3
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_fresh_boot_recovery_result_resolution as control
import l6_v4_install_artifacts_staged_new_boot_recovery_preflight as guest_probe
import test_l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight as prior_test
import test_l6_utm_install_artifacts_staged_guest_agent_resolution as guest_agent_test
import test_l6_utm_install_artifacts_staged_runtime_resolution as runtime_test


class ResolutionRunner:
    def __init__(
        self,
        request: control.FreshBootRecoveryResultResolutionRequest,
        binding: SimpleNamespace,
        *,
        result_payloads: list[bytes] | None = None,
        phase: str = "complete",
        first_result_missing: bool = False,
        backend_pid: int = 42,
    ) -> None:
        self.request = request
        self.binding = binding
        self.result_payloads = list(
            result_payloads or [qualified_payload(request, binding)] * 2
        )
        self.phase = phase
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
            if path == self.request.guest_terminal_path:
                if self.first_result_missing:
                    self.first_result_missing = False
                    return guest_agent_test.observation(
                        argv, stderr=b"synthetic missing result"
                    )
                return guest_agent_test.observation(
                    argv, stdout=self.result_payloads.pop(0)
                )
            if path == self.request.guest_phase_path:
                return guest_agent_test.observation(
                    argv,
                    stdout=guest_probe.canonical_json(
                        {"format": guest_probe.PHASE_FORMAT, "phase": self.phase}
                    ),
                )
        raise AssertionError(f"unexpected command: {argv}")


class FreshBootRecoveryResultResolutionTests(unittest.TestCase):
    def test_qualified_result_uses_only_two_result_and_one_phase_pull(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = ResolutionRunner(request, binding)

            result, source = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "recovery-qualified")
            self.assertEqual(result.exit_code, control.EXIT_RECOVERY_QUALIFIED)
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertEqual(result.phase_readback_invocations, 1)
            self.assertTrue(source.file_object.closed)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["file_pull_invocations"], 3)
            self.assertEqual(terminal["file_push_invocations"], 0)
            self.assertEqual(terminal["guest_exec_invocations"], 0)
            self.assertEqual(terminal["guest_probe_invocations"], 0)
            self.assert_forbidden_actions_absent(runner.calls)
            runtime_test.assert_manifest_valid(self, request.output_root)

    def test_rejected_result_is_authoritative_without_resume(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            rejected = rejected_payload(request, binding)
            runner = ResolutionRunner(
                request,
                binding,
                result_payloads=[rejected, rejected],
                phase="rejected",
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "recovery-rejected")
            self.assertEqual(result.exit_code, control.EXIT_RECOVERY_REJECTED)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(
                terminal["guest_recovery_outcome"], "recovery-rejected"
            )
            self.assertEqual(terminal["maintenance_resume_invocations"], 0)
            self.assert_forbidden_actions_absent(runner.calls)

    def test_missing_first_result_stays_indeterminate_without_retry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = ResolutionRunner(
                request, binding, first_result_missing=True
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, control.EXIT_STATE_INDETERMINATE)
            self.assertEqual(result.result_readback_invocations, 1)
            self.assertEqual(result.phase_readback_invocations, 0)
            pulls = [call for call in runner.calls if call[0:3] == ("utmctl", "file", "pull")]
            self.assertEqual(len(pulls), 1)
            self.assert_forbidden_actions_absent(runner.calls)

    def test_double_readback_drift_stays_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            first = qualified_payload(request, binding)
            second_value = json.loads(first)
            second_value["reason"] = "synthetic-drift"
            second = guest_probe.canonical_json(second_value)
            runner = ResolutionRunner(
                request, binding, result_payloads=[first, second]
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertEqual(result.phase_readback_invocations, 0)
            self.assertFalse(
                (request.output_root / "existing-recovery-result.json").exists()
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_backend_pid_drift_rejects_before_any_guest_pull(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = ResolutionRunner(request, binding, backend_pid=99)

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.result_readback_invocations, 0)
            self.assertFalse(
                any(
                    call[0:3] == ("utmctl", "file", "pull")
                    for call in runner.calls
                )
            )

    def test_binding_carries_frozen_result_and_current_code_identity(self) -> None:
        request = make_request(Path("/tmp/radishlex-result-resolution-binding"))
        upstream = SimpleNamespace(
            current_boot_id_sha256="c" * 64,
            prior_boot_id_sha256="b" * 64,
            prior_backend_pid=42,
            evidence={
                "prior_fresh_boot_recovery_preflight_entries_verified": 35
            },
        )
        with mock.patch.object(
            control.result_bindings,
            "validate_fresh_boot_recovery_preflight_result_bindings",
            return_value=upstream,
        ), mock.patch.object(
            control.network_ready, "_require_committed_regular"
        ):
            binding = control.validate_result_resolution_bindings(request)

        self.assertIs(binding.upstream, upstream)
        self.assertEqual(binding.prior_backend_pid, 42)
        self.assertEqual(
            binding.evidence[
                "prior_fresh_boot_recovery_preflight_entries_verified"
            ],
            35,
        )
        self.assertIn(
            "fresh_boot_recovery_result_resolution_control_sha256",
            binding.evidence,
        )

    def test_authorization_and_prior_root_overlap_reject(self) -> None:
        request = make_request(Path("/tmp/radishlex-result-resolution-request"))
        unauthorized = replace_request(
            request,
            authorized_two_existing_recovery_result_readbacks=False,
        )
        with self.assertRaisesRegex(
            control.FreshBootRecoveryResultResolutionError,
            "two-existing-recovery-result-readbacks-authorization-required",
        ):
            unauthorized.validate()

        overlap = replace_request(
            request,
            output_root=request.prior_fresh_boot_recovery_preflight_root / "new",
        )
        with self.assertRaisesRegex(
            control.FreshBootRecoveryResultResolutionError,
            "output-root-must-not-overlap-prior-fresh-boot-recovery-preflight-root",
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
            " stop ",
            " quit ",
            " dpkg ",
        ):
            self.assertFalse(any(forbidden in value for value in flattened))


def make_request(
    root: Path,
) -> control.FreshBootRecoveryResultResolutionRequest:
    prior = prior_test.make_request(root)
    values = dict(prior.__dict__)
    values.update(
        output_root=root / "fresh-boot-recovery-result-resolution-output",
        prior_fresh_boot_recovery_preflight_root=(
            root / "prior-fresh-boot-recovery-preflight"
        ),
        prior_fresh_boot_recovery_preflight_manifest_sha256=(
            control.REQUIRED_PRIOR_MANIFEST_SHA256
        ),
        prior_fresh_boot_recovery_preflight_attempt_id=(
            control.REQUIRED_PRIOR_ATTEMPT_ID
        ),
        result_resolution_attempt_id=control.REQUIRED_ATTEMPT_ID,
        authorized_install_artifacts_staged_fresh_boot_recovery_result_resolution=True,
        authorized_bound_prior_fresh_boot_recovery_preflight_result=True,
        authorized_two_existing_recovery_result_readbacks=True,
        authorized_one_existing_recovery_phase_readback=True,
        authorized_no_inventory_start_status_probe_push_exec_resume_dpkg_retry_stop_or_quit=True,
    )
    return control.FreshBootRecoveryResultResolutionRequest(**values)


def replace_request(
    request: control.FreshBootRecoveryResultResolutionRequest,
    **changes: object,
) -> control.FreshBootRecoveryResultResolutionRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return control.FreshBootRecoveryResultResolutionRequest(**values)


def make_binding() -> SimpleNamespace:
    return SimpleNamespace(
        evidence={"format": control.EVIDENCE_FORMAT},
        current_boot_id_sha256="c" * 64,
        prior_boot_id_sha256="b" * 64,
        prior_backend_pid=42,
    )


def qualified_payload(
    request: control.FreshBootRecoveryResultResolutionRequest,
    binding: SimpleNamespace,
) -> bytes:
    return guest_probe.canonical_json(
        {
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "current_boot_id_sha256": binding.current_boot_id_sha256,
            "dpkg_mutation_executed": False,
            "format": guest_probe.EVIDENCE_FORMAT,
            "guard_profile": "absent-after-reboot",
            "maintenance_resume_invocations": 0,
            "operation_id": "existing-receipt-hash-only",
            "operation_id_sha256": guest_probe.EXPECTED_OPERATION_ID_SHA256,
            "outcome": "recovery-qualified",
            "phase": "complete",
            "prior_boot_id_sha256": binding.prior_boot_id_sha256,
            "reason": "persistent-artifacts-staged-new-boot-readonly-qualified",
            "receipt_state": "artifacts_staged",
            "startup_gate": guest_probe.EXPECTED_OPERATION_IN_PROGRESS_STARTUP,
            "target_uuid": request.target_uuid,
            "transaction": "artifacts-staged-preserved-no-resume",
        }
    )


def rejected_payload(
    request: control.FreshBootRecoveryResultResolutionRequest,
    binding: SimpleNamespace,
) -> bytes:
    value = json.loads(qualified_payload(request, binding))
    value.pop("receipt_state")
    value.pop("startup_gate")
    value.update(
        guard_profile="not-observed",
        outcome="recovery-rejected",
        phase="persistent-transaction",
        reason="RecoveryPreflightError:synthetic-rejection",
    )
    return guest_probe.canonical_json(value)


def run_case(
    request: control.FreshBootRecoveryResultResolutionRequest,
    binding: SimpleNamespace,
    runner: ResolutionRunner,
):
    source = prior_test.recovery_test.classification_test.boot_test.SyntheticSource(
        prior_test.recovery_test.classification_test.boot_test.CloseTracker()
    )
    result = control.run_fresh_boot_recovery_result_resolution(
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

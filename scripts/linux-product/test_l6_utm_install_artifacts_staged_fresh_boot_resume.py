#!/usr/bin/env python3
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_fresh_boot_resume as control
import test_l6_utm_install_artifacts_staged_fresh_boot_recovery_result_resolution as resolution_test
import test_l6_utm_install_artifacts_staged_guest_agent_resolution as guest_agent_test
import test_l6_utm_install_artifacts_staged_runtime_resolution as runtime_test


class FreshBootResumeRunner:
    def __init__(
        self,
        request: control.FreshBootResumeRequest,
        binding: control.FreshBootResumeBinding,
        *,
        terminal_payload: bytes | None = None,
        driver_exit_code: int = 0,
        missing_terminal: bool = False,
    ) -> None:
        self.request = request
        self.binding = binding
        self.terminal_payload = terminal_payload or completed_payload(request, binding)
        self.driver_exit_code = driver_exit_code
        self.missing_terminal = missing_terminal
        self.calls: list[tuple[str, ...]] = []

    def run(self, argv, timeout_seconds, *, stdin_file=None):
        del timeout_seconds, stdin_file
        self.calls.append(argv)
        if argv == control.runtime_control.launch_transport.PROCESS_COMMAND:
            return guest_agent_test.observation(argv, stdout=runtime_test.quiet())
        if argv[0:1] == ("/usr/sbin/lsof",):
            return guest_agent_test.observation(
                argv,
                stdout=runtime_test.handle_payload(self.request, 42, "QEMULauncher"),
            )
        if argv == control.readiness_argv(self.request):
            return guest_agent_test.observation(argv)
        if argv == control.driver_argv(self.request, self.binding):
            return guest_agent_test.observation(argv, exit_code=self.driver_exit_code)
        if argv[0:3] == ("utmctl", "file", "push"):
            return guest_agent_test.observation(argv)
        if argv[0:3] == ("utmctl", "file", "pull"):
            path = argv[4]
            payloads = {
                self.request.guest_frozen_resume_driver_path: (
                    self.binding.frozen_resume_driver_bytes
                ),
                self.request.guest_frozen_recovery_probe_path: (
                    self.binding.frozen_recovery_probe_bytes
                ),
                self.request.guest_fresh_boot_resume_driver_path: (
                    self.binding.fresh_boot_resume_driver_bytes
                ),
                self.request.guest_fresh_boot_resume_marker_path: control.marker_bytes(
                    self.request, self.binding
                ),
                self.request.guest_fresh_boot_resume_terminal_path: (
                    self.terminal_payload
                ),
                self.request.guest_fresh_boot_resume_phase_path: control.guest_driver.canonical_json(
                    {
                        "format": control.guest_driver.PHASE_FORMAT,
                        "phase": phase_for(self.terminal_payload),
                    }
                ),
            }
            if path == self.request.guest_fresh_boot_resume_terminal_path and self.missing_terminal:
                return guest_agent_test.observation(argv, stderr=b"synthetic missing")
            return guest_agent_test.observation(argv, stdout=payloads[path])
        if argv[0:2] == ("utmctl", "exec"):
            return guest_agent_test.observation(argv)
        raise AssertionError(f"unexpected command: {argv}")


class FreshBootResumeTests(unittest.TestCase):
    def test_completed_result_runs_one_driver_and_never_retries_or_stops(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding(request)
            runner = FreshBootResumeRunner(request, binding)

            result, source = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "resume-completed")
            self.assertEqual(result.resume_invocations, 1)
            self.assertEqual(result.postflight_invocations, 1)
            self.assertTrue(source.file_object.closed)
            driver_calls = [
                call for call in runner.calls if call == control.driver_argv(request, binding)
            ]
            self.assertEqual(len(driver_calls), 1)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["file_push_invocations"], 3)
            self.assertEqual(terminal["file_pull_invocations"], 7)
            self.assertFalse(terminal["transport_exit_disambiguated_by_terminal"])
            self.assert_forbidden_actions_absent(runner.calls)
            runtime_test.assert_manifest_valid(self, request.output_root)

    def test_stable_rejected_terminal_disambiguates_transport_exit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding(request)
            payload = rejected_payload(request, binding)
            runner = FreshBootResumeRunner(
                request,
                binding,
                terminal_payload=payload,
                driver_exit_code=1,
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "resume-rejected")
            self.assertEqual(result.resume_invocations, 0)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertTrue(terminal["transport_exit_disambiguated_by_terminal"])
            self.assertEqual(
                terminal["transaction"], "artifacts-staged-preserved-no-resume"
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_missing_terminal_after_driver_is_indeterminate_without_retry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding(request)
            runner = FreshBootResumeRunner(request, binding, missing_terminal=True)

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            driver_calls = [
                call for call in runner.calls if call == control.driver_argv(request, binding)
            ]
            self.assertEqual(len(driver_calls), 1)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["transaction"], "state-indeterminate")
            self.assertEqual(
                terminal["maintenance_resume_invocations"],
                "unknown-after-driver-invocation",
            )
            self.assertEqual(
                terminal["postflight_invocations"],
                "unknown-after-driver-invocation",
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_backend_pid_drift_rejects_before_guest_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding(request)
            binding = control.FreshBootResumeBinding(
                **{**binding.__dict__, "prior_backend_pid": 99}
            )
            runner = FreshBootResumeRunner(request, binding)

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertFalse(
                any(call[0:2] == ("utmctl", "exec") for call in runner.calls)
            )

    def test_binding_carries_qualified_result_and_three_code_identities(self) -> None:
        request = make_request(Path("/tmp/radishlex-fresh-resume-binding"))
        upstream = SimpleNamespace(
            evidence={"prior_recovery_result_resolution_entries_verified": 21},
            current_boot_id_sha256=control.guest_driver.EXPECTED_CURRENT_BOOT_ID_SHA256,
            prior_boot_id_sha256=control.guest_driver.EXPECTED_PRIOR_BOOT_ID_SHA256,
            prior_backend_pid=42,
        )
        with mock.patch.object(
            control.network_ready, "_require_committed_regular"
        ), mock.patch.object(
            control.result_bindings,
            "validate_fresh_boot_recovery_result_resolution_result_bindings",
            return_value=upstream,
        ):
            binding = control.validate_fresh_boot_resume_bindings(request)

        self.assertEqual(binding.prior_backend_pid, 42)
        self.assertEqual(
            binding.evidence["prior_recovery_result_resolution_outcome"],
            "recovery-qualified",
        )
        self.assertIn("fresh_boot_resume_driver_sha256", binding.evidence)
        self.assertIn("frozen_recovery_probe_sha256", binding.evidence)

    def test_authorization_and_prior_root_overlap_reject(self) -> None:
        request = make_request(Path("/tmp/radishlex-fresh-resume-request"))
        values = dict(request.__dict__)
        values["authorized_one_maintenance_resume_and_postflight"] = False
        unauthorized = control.FreshBootResumeRequest(**values)
        with self.assertRaisesRegex(
            control.FreshBootResumeError,
            "one-maintenance-resume-and-postflight-authorization-required",
        ):
            unauthorized.validate()

        values = dict(request.__dict__)
        values["output_root"] = (
            request.prior_fresh_boot_recovery_result_resolution_root / "new"
        )
        overlap = control.FreshBootResumeRequest(**values)
        with self.assertRaisesRegex(
            control.FreshBootResumeError,
            "output-root-must-not-overlap-prior-recovery-result-resolution-root",
        ):
            overlap.validate()

    def assert_forbidden_actions_absent(self, calls: list[tuple[str, ...]]) -> None:
        flattened = [" ".join(call) for call in calls]
        for forbidden in (
            "utmctl list",
            "utmctl status",
            "utmctl start",
            " retry ",
            " cleanup ",
            " stop ",
            " quit ",
        ):
            self.assertFalse(any(forbidden in value for value in flattened))


def make_request(root: Path) -> control.FreshBootResumeRequest:
    base = resolution_test.make_request(root)
    values = dict(base.__dict__)
    values.update(
        output_root=root / "fresh-boot-resume-output",
        prior_fresh_boot_recovery_result_resolution_root=(
            root / "prior-recovery-result-resolution"
        ),
        prior_fresh_boot_recovery_result_resolution_manifest_sha256=(
            control.REQUIRED_PRIOR_MANIFEST_SHA256
        ),
        prior_fresh_boot_recovery_result_resolution_attempt_id=(
            control.REQUIRED_PRIOR_ATTEMPT_ID
        ),
        fresh_boot_resume_attempt_id=control.REQUIRED_ATTEMPT_ID,
        resume_timeout_seconds=1500,
        evidence_settle_seconds=1,
        authorized_install_artifacts_staged_fresh_boot_resume=True,
        authorized_bound_qualified_fresh_boot_recovery_result=True,
        authorized_one_private_fresh_boot_resume_delivery_and_execution=True,
        authorized_one_maintenance_resume_and_postflight=True,
        authorized_no_inventory_start_status_probe_retry_cleanup_stop_or_quit=True,
    )
    return control.FreshBootResumeRequest(**values)


def make_binding(request: control.FreshBootResumeRequest) -> control.FreshBootResumeBinding:
    repository = request.repository_root
    upstream = SimpleNamespace(
        evidence={"prior_recovery_result_resolution_entries_verified": 21},
        current_boot_id_sha256=control.guest_driver.EXPECTED_CURRENT_BOOT_ID_SHA256,
        prior_boot_id_sha256=control.guest_driver.EXPECTED_PRIOR_BOOT_ID_SHA256,
        prior_backend_pid=42,
    )
    return control.FreshBootResumeBinding(
        evidence={"format": control.EVIDENCE_FORMAT},
        upstream=upstream,
        frozen_resume_driver_bytes=(repository / control.FROZEN_RESUME_DRIVER_RELATIVE_PATH).read_bytes(),
        frozen_recovery_probe_bytes=(repository / control.FROZEN_RECOVERY_PROBE_RELATIVE_PATH).read_bytes(),
        fresh_boot_resume_driver_bytes=(repository / control.FRESH_BOOT_RESUME_DRIVER_RELATIVE_PATH).read_bytes(),
        current_boot_id_sha256=upstream.current_boot_id_sha256,
        prior_boot_id_sha256=upstream.prior_boot_id_sha256,
        prior_backend_pid=42,
    )


def completed_payload(
    request: control.FreshBootResumeRequest, binding: control.FreshBootResumeBinding
) -> bytes:
    return control.guest_driver.canonical_json(
        {
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "checkpoint_attempt_id": control.guest_driver.EXPECTED_CHECKPOINT_ATTEMPT_ID,
            "checkpoint_sha256": control.guest_driver.EXPECTED_CHECKPOINT_SHA256,
            "current_boot_id_sha256": binding.current_boot_id_sha256,
            "dpkg_delta_sha256": "c" * 64,
            "dpkg_delta_size": 1512,
            "dpkg_log_sha256": "d" * 64,
            "dpkg_log_size": 881152,
            "dpkg_mutation_executed": True,
            "format": control.guest_driver.EVIDENCE_FORMAT,
            "guard_profile_before_resume": "absent-after-reboot",
            "maintenance_resume_invocations": 1,
            "operation_id": "reconstructed-secret-hash-only",
            "operation_id_sha256": control.guest_driver.EXPECTED_OPERATION_ID_SHA256,
            "outcome": "resume-completed",
            "phase": "complete",
            "postflight_invocations": 1,
            "prior_boot_id_sha256": binding.prior_boot_id_sha256,
            "reason": "one-shot-fresh-boot-install-artifacts-staged-resume-passed",
            "receipt_sha256": "e" * 64,
            "receipt_size": 4000,
            "resume_attempt_id": request.fresh_boot_resume_attempt_id,
            "resume_result_sha256": "f" * 64,
            "terminal_case_sha256": "b" * 64,
            "terminal_postflight_sha256": "1" * 64,
            "transaction": "completed",
            "transient_secret_reconstructed": True,
        }
    )


def rejected_payload(
    request: control.FreshBootResumeRequest, binding: control.FreshBootResumeBinding
) -> bytes:
    value = json.loads(completed_payload(request, binding))
    for key in (
        "checkpoint_sha256",
        "dpkg_delta_sha256",
        "dpkg_delta_size",
        "dpkg_log_sha256",
        "dpkg_log_size",
        "guard_profile_before_resume",
        "receipt_sha256",
        "receipt_size",
        "resume_result_sha256",
        "terminal_postflight_sha256",
    ):
        value.pop(key)
    value.update(
        dpkg_mutation_executed=False,
        maintenance_resume_invocations=0,
        outcome="resume-rejected",
        phase="fresh-boot-readiness",
        postflight_invocations=0,
        reason="FreshBootResumeDriverError:synthetic-drift",
        terminal_case_sha256=None,
        transaction="artifacts-staged-preserved-no-resume",
        transient_secret_reconstructed=False,
    )
    return control.guest_driver.canonical_json(value)


def phase_for(payload: bytes) -> str:
    outcome = json.loads(payload)["outcome"]
    return {
        "resume-completed": "complete",
        "resume-rejected": "rejected",
        "state-indeterminate": "indeterminate",
    }[outcome]


def run_case(
    request: control.FreshBootResumeRequest,
    binding: control.FreshBootResumeBinding,
    runner: FreshBootResumeRunner,
):
    boot_test = resolution_test.prior_test.recovery_test.classification_test.boot_test
    source = boot_test.SyntheticSource(boot_test.CloseTracker())
    result = control.run_fresh_boot_resume(
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

#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_new_boot_recovery_preflight as control
import l6_v4_install_artifacts_staged_new_boot_recovery_preflight as guest_probe
import test_l6_utm_install_artifacts_staged_boot_classification_resolution as classification_test
import test_l6_utm_install_artifacts_staged_guest_agent_resolution as guest_agent_test
import test_l6_utm_install_artifacts_staged_runtime_resolution as runtime_test


class RecoveryRunner:
    def __init__(
        self,
        request: control.RecoveryPreflightRequest,
        binding: SimpleNamespace,
        *,
        result_payloads: list[bytes] | None = None,
        handles_present: bool = True,
    ) -> None:
        self.request = request
        self.binding = binding
        self.result_payloads = list(result_payloads or [])
        self.handles_present = handles_present
        self.calls: list[tuple[str, ...]] = []
        self.incoming: dict[str, bytes] = {}
        self.published: dict[str, bytes] = {}

    def run(self, argv, timeout_seconds, *, stdin_file=None):
        del timeout_seconds
        self.calls.append(argv)
        if argv == control.launch_transport.PROCESS_COMMAND:
            return guest_agent_test.observation(argv, stdout=runtime_test.quiet())
        if argv[0:1] == ("/usr/sbin/lsof",):
            if not self.handles_present:
                return guest_agent_test.observation(argv, exit_code=1)
            return guest_agent_test.observation(
                argv,
                stdout=runtime_test.handle_payload(
                    self.request, 42, "QEMULauncher"
                ),
            )
        if argv == control.readiness_argv(self.request):
            return guest_agent_test.observation(argv)
        if argv[0:3] == ("utmctl", "file", "push"):
            if stdin_file is None:
                raise AssertionError("file push missing stdin")
            self.incoming[argv[4]] = stdin_file.read()
            return guest_agent_test.observation(argv)
        if argv[0:3] == ("utmctl", "file", "pull"):
            path = argv[4]
            if path in self.published:
                payload = self.published[path]
            elif path == self.request.guest_marker_path:
                payload = control.marker_bytes(self.request, self.binding)
            elif path == self.request.guest_terminal_path:
                payload = (
                    self.result_payloads.pop(0)
                    if self.result_payloads
                    else qualified_payload(self.request)
                )
            elif path == self.request.guest_phase_path:
                payload = guest_probe.canonical_json(
                    {"format": guest_probe.PHASE_FORMAT, "phase": "complete"}
                )
            else:
                raise AssertionError(f"unexpected pull path: {path}")
            return guest_agent_test.observation(argv, stdout=payload)
        if argv[0:3] == ("utmctl", "exec", self.request.target_uuid):
            if "/bin/mv" in argv:
                incoming, final = argv[-2:]
                self.published[final] = self.incoming[incoming]
            return guest_agent_test.observation(argv)
        raise AssertionError(f"unexpected command: {argv}")


class NewBootRecoveryPreflightTests(unittest.TestCase):
    def test_binding_preserves_frozen_new_boot_result_and_driver_identity(self) -> None:
        request = make_request(Path("/tmp/radishlex-recovery-binding-test"))
        upstream = SimpleNamespace(
            evidence={"prior_boot_classification_entries_verified": 72},
            expected_boot_id_sha256=guest_probe.EXPECTED_PRIOR_BOOT_ID_SHA256,
            observed_boot_id_sha256=guest_probe.EXPECTED_CURRENT_BOOT_ID_SHA256,
            prior_backend_pid=92422,
        )
        with mock.patch.object(
            control.bindings.result_bindings,
            "validate_boot_classification_result_bindings",
            return_value=upstream,
        ), mock.patch.object(
            control.bindings.network_ready, "_require_committed_regular"
        ):
            binding = control.bindings.validate_recovery_preflight_bindings(
                request
            )

        self.assertIs(binding.upstream, upstream)
        self.assertEqual(binding.prior_backend_pid, 92422)
        self.assertEqual(
            hashlib.sha256(binding.resume_driver_bytes).hexdigest(),
            guest_probe.EXPECTED_RESUME_DRIVER_SHA256,
        )
        self.assertEqual(
            binding.evidence["prior_boot_classification_entries_verified"],
            72,
        )

    def test_qualified_result_preserves_transaction_without_forbidden_actions(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = RecoveryRunner(request, binding)

            result, source = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "recovery-qualified")
            self.assertEqual(result.exit_code, control.EXIT_RECOVERY_QUALIFIED)
            self.assertEqual(result.guest_probe_invocations, 1)
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertTrue(source.file_object.closed)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["maintenance_resume_invocations"], 0)
            self.assertEqual(
                terminal["transaction"],
                "artifacts-staged-preserved-no-resume",
            )
            self.assertEqual(terminal["file_push_invocations"], 2)
            self.assert_forbidden_actions_absent(runner.calls)
            runtime_test.assert_manifest_valid(self, request.output_root)

    def test_double_readback_drift_is_state_indeterminate_after_probe(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            first = qualified_payload(request)
            second_value = json.loads(first)
            second_value["reason"] = "synthetic-drift"
            second = guest_probe.canonical_json(second_value)
            runner = RecoveryRunner(
                request, binding, result_payloads=[first, second]
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, control.EXIT_STATE_INDETERMINATE)
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertFalse(
                (request.output_root / "guest-recovery-result.json").exists()
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_missing_target_handles_rejects_before_guest_write(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = RecoveryRunner(
                request, binding, handles_present=False
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.guest_probe_invocations, 0)
            self.assertFalse(
                any(call[0:3] == ("utmctl", "file", "push") for call in runner.calls)
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_authorization_and_prior_result_overlap_reject(self) -> None:
        request = make_request(Path("/tmp/radishlex-recovery-preflight-test"))
        unauthorized = replace_request(
            request,
            authorized_bounded_read_only_recovery_probe=False,
        )
        with self.assertRaisesRegex(
            control.RecoveryPreflightError,
            "bounded-readonly-recovery-probe-authorization-required",
        ):
            unauthorized.validate()

        overlapping = replace_request(
            request,
            output_root=request.prior_boot_classification_root / "new",
        )
        with self.assertRaisesRegex(
            control.RecoveryPreflightError,
            "output-root-must-not-overlap-prior-boot-classification-root",
        ):
            overlapping.validate()

    def test_startup_profile_must_match_guard_profile(self) -> None:
        request = make_request(Path("/tmp/radishlex-recovery-profile-test"))
        binding = make_binding()
        value = json.loads(qualified_payload(request))
        value["startup_gate"] = guest_probe.EXPECTED_ACTIVE_GUARD_STARTUP

        with self.assertRaisesRegex(
            control.RecoveryPreflightError,
            "guest-qualified-semantics-invalid",
        ):
            control.parse_guest_terminal(
                guest_probe.canonical_json(value), request, binding
            )

    def assert_forbidden_actions_absent(
        self, calls: list[tuple[str, ...]]
    ) -> None:
        flattened = [" ".join(call) for call in calls]
        for forbidden in (
            "utmctl list",
            "utmctl status",
            "utmctl start",
            " resume ",
            " retry ",
            " stop ",
            " quit ",
            " dpkg ",
        ):
            self.assertFalse(any(forbidden in value for value in flattened))


def make_request(root: Path) -> control.RecoveryPreflightRequest:
    prior = classification_test.make_request(root)
    values = dict(prior.__dict__)
    values.update(
        output_root=root / "new-boot-recovery-preflight-output",
        prior_boot_classification_root=root / "prior-boot-classification",
        prior_boot_classification_manifest_sha256=(
            control.bindings.REQUIRED_PRIOR_MANIFEST_SHA256
        ),
        prior_boot_classification_attempt_id=(
            control.bindings.REQUIRED_PRIOR_ATTEMPT_ID
        ),
        recovery_preflight_attempt_id=control.bindings.REQUIRED_ATTEMPT_ID,
        authorized_install_artifacts_staged_new_boot_recovery_preflight=True,
        authorized_bound_prior_boot_classification_result=True,
        authorized_bounded_read_only_recovery_probe=True,
        authorized_one_private_recovery_probe_delivery_and_execution=True,
        authorized_no_inventory_start_status_resume_dpkg_mutation_retry_stop_or_quit=(
            True
        ),
    )
    return control.RecoveryPreflightRequest(**values)


def replace_request(
    request: control.RecoveryPreflightRequest,
    **changes: object,
) -> control.RecoveryPreflightRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return control.RecoveryPreflightRequest(**values)


def make_binding() -> SimpleNamespace:
    repository_root = Path(control.__file__).resolve().parents[2]
    return SimpleNamespace(
        evidence={"format": control.bindings.EVIDENCE_FORMAT},
        probe_bytes=(repository_root / control.PROBE_RELATIVE_PATH).read_bytes(),
        resume_driver_bytes=(
            repository_root / control.RESUME_DRIVER_RELATIVE_PATH
        ).read_bytes(),
        prior_boot_id_sha256=guest_probe.EXPECTED_PRIOR_BOOT_ID_SHA256,
        current_boot_id_sha256=guest_probe.EXPECTED_CURRENT_BOOT_ID_SHA256,
        prior_backend_pid=42,
    )


def qualified_payload(request: control.RecoveryPreflightRequest) -> bytes:
    return guest_probe.canonical_json(
        {
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "current_boot_id_sha256": guest_probe.EXPECTED_CURRENT_BOOT_ID_SHA256,
            "dpkg_mutation_executed": False,
            "format": guest_probe.EVIDENCE_FORMAT,
            "guard_profile": "absent-after-reboot",
            "maintenance_resume_invocations": 0,
            "operation_id": "existing-receipt-hash-only",
            "operation_id_sha256": guest_probe.EXPECTED_OPERATION_ID_SHA256,
            "outcome": "recovery-qualified",
            "phase": "complete",
            "prior_boot_id_sha256": guest_probe.EXPECTED_PRIOR_BOOT_ID_SHA256,
            "reason": "persistent-artifacts-staged-new-boot-readonly-qualified",
            "receipt_state": "artifacts_staged",
            "startup_gate": guest_probe.EXPECTED_OPERATION_IN_PROGRESS_STARTUP,
            "target_uuid": request.target_uuid,
            "transaction": "artifacts-staged-preserved-no-resume",
        }
    )


def run_case(
    request: control.RecoveryPreflightRequest,
    binding: SimpleNamespace,
    runner: RecoveryRunner,
):
    source = classification_test.boot_test.SyntheticSource(
        classification_test.boot_test.CloseTracker()
    )
    result = control.run_recovery_preflight(
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

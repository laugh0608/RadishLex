#!/usr/bin/env python3
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_fresh_boot_classification_result_bindings as classification_result_bindings
import l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight as control
import l6_utm_install_artifacts_staged_fresh_boot_recovery_preflight_bindings as bindings
import l6_v4_install_artifacts_staged_new_boot_recovery_preflight as guest_probe
import test_l6_utm_install_artifacts_staged_fresh_boot_classification as classification_test
import test_l6_utm_install_artifacts_staged_new_boot_recovery_preflight as recovery_test
import test_l6_utm_install_artifacts_staged_runtime_resolution as runtime_test


FRESH_BOOT_HASH = "c" * 64
ENDED_BOOT_HASH = "b" * 64


class FreshBootRecoveryPreflightTests(unittest.TestCase):
    def test_dynamic_fresh_boot_manifest_is_fully_validated(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            classification_request = classification_test.make_request(root)
            classification_binding = (
                classification_test.classification_test.boot_test.make_binding()
            )
            classification_binding.expected_boot_id_sha256 = ENDED_BOOT_HASH
            runner = classification_test.classification_test.ClassificationRunner(
                classification_request,
                classification_binding.probe_bytes,
                boot_hash=FRESH_BOOT_HASH,
            )
            classification_result = classification_test.run_case(
                classification_request, classification_binding, runner
            )
            request = make_request(
                root,
                prior_root=classification_result.evidence_root,
                prior_manifest=classification_result.manifest_sha256,
            )

            with mock.patch.object(
                classification_result_bindings.bindings,
                "validate_fresh_boot_classification_bindings",
                return_value=classification_binding,
            ):
                result = classification_result_bindings.validate_fresh_boot_classification_result_bindings(
                    request
                )

            self.assertEqual(result.observed_boot_id_sha256, FRESH_BOOT_HASH)
            self.assertEqual(result.expected_boot_id_sha256, ENDED_BOOT_HASH)
            self.assertEqual(result.prior_backend_pid, 42)
            self.assertEqual(
                result.evidence[
                    "prior_fresh_boot_classification_outcome"
                ],
                "new-boot-started",
            )

            terminal_path = classification_result.evidence_root / "terminal.json"
            terminal_path.write_text("{}\n", encoding="utf-8")
            with mock.patch.object(
                classification_result_bindings.bindings,
                "validate_fresh_boot_classification_bindings",
                return_value=classification_binding,
            ), self.assertRaisesRegex(ValueError, "entry-drift"):
                classification_result_bindings.validate_fresh_boot_classification_result_bindings(
                    request
                )

    def test_qualification_binding_carries_fresh_boot_and_same_backend(self) -> None:
        request = make_request(Path("/tmp/radishlex-fresh-recovery-binding"))
        upstream = SimpleNamespace(
            evidence={
                "prior_fresh_boot_classification_entries_verified": 48
            },
            expected_boot_id_sha256=bindings.REQUIRED_ENDED_BOOT_ID_SHA256,
            observed_boot_id_sha256=FRESH_BOOT_HASH,
            prior_backend_pid=42,
        )
        with mock.patch.object(
            bindings.classification_result_bindings,
            "validate_fresh_boot_classification_result_bindings",
            return_value=upstream,
        ):
            binding = bindings.validate_fresh_boot_recovery_preflight_bindings(
                request
            )

        self.assertEqual(binding.current_boot_id_sha256, FRESH_BOOT_HASH)
        self.assertEqual(
            binding.prior_boot_id_sha256,
            guest_probe.EXPECTED_PRIOR_BOOT_ID_SHA256,
        )
        self.assertEqual(binding.prior_backend_pid, 42)

        upstream.observed_boot_id_sha256 = bindings.REQUIRED_PRIOR_BOOT_ID_SHA256
        with mock.patch.object(
            bindings.classification_result_bindings,
            "validate_fresh_boot_classification_result_bindings",
            return_value=upstream,
        ), self.assertRaisesRegex(
            ValueError, "fresh-boot-classification-identity-invalid"
        ):
            bindings.validate_fresh_boot_recovery_preflight_bindings(request)

    def test_read_only_qualification_accepts_dynamic_fresh_boot(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding(request)
            runner = recovery_test.RecoveryRunner(
                request,
                binding,
                result_payloads=[qualified_payload(request, binding)] * 2,
            )

            result = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "recovery-qualified")
            self.assertEqual(result.exit_code, control.EXIT_RECOVERY_QUALIFIED)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["inventory_probe_invocations"], 0)
            self.assertEqual(terminal["maintenance_resume_invocations"], 0)
            self.assert_forbidden_actions_absent(runner.calls)

    def test_backend_pid_drift_fails_before_guest_delivery(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding(request)
            binding.prior_backend_pid = 99
            runner = recovery_test.RecoveryRunner(request, binding)

            result = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.guest_probe_invocations, 0)
            self.assertFalse(
                any(call[0:3] == ("utmctl", "file", "push") for call in runner.calls)
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_old_or_overlapping_classification_input_is_rejected(self) -> None:
        request = make_request(Path("/tmp/radishlex-fresh-recovery-request"))
        old_attempt = replace_request(
            request,
            prior_fresh_boot_classification_attempt_id="old-attempt",
        )
        with self.assertRaisesRegex(
            control.recovery_control.RecoveryPreflightError,
            "fixed-fresh-boot-recovery-input-mismatch",
        ):
            old_attempt.validate()

        overlap = replace_request(
            request,
            output_root=request.prior_fresh_boot_classification_root / "new",
        )
        with self.assertRaisesRegex(
            control.recovery_control.RecoveryPreflightError,
            "output-root-must-not-overlap-prior-fresh-boot-classification-root",
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
            " resume ",
            " retry ",
            " stop ",
            " quit ",
            " dpkg ",
        ):
            self.assertFalse(any(forbidden in value for value in flattened))


def make_request(
    root: Path,
    *,
    prior_root: Path | None = None,
    prior_manifest: str = "d" * 64,
) -> control.FreshBootRecoveryPreflightRequest:
    prior = classification_test.make_request(root)
    values = dict(prior.__dict__)
    values.update(
        output_root=root / "fresh-boot-recovery-preflight-output",
        recovery_preflight_attempt_id=bindings.REQUIRED_ATTEMPT_ID,
        prior_fresh_boot_classification_root=(
            prior_root or root / "prior-fresh-boot-classification"
        ),
        prior_fresh_boot_classification_manifest_sha256=prior_manifest,
        prior_fresh_boot_classification_attempt_id=(
            bindings.REQUIRED_PRIOR_CLASSIFICATION_ATTEMPT_ID
        ),
        authorized_install_artifacts_staged_fresh_boot_recovery_preflight=True,
        authorized_bound_prior_fresh_boot_classification_result=True,
        authorized_bounded_read_only_fresh_boot_recovery_probe=True,
        authorized_one_private_fresh_boot_recovery_probe_delivery_and_execution=True,
        authorized_no_inventory_start_status_resume_dpkg_mutation_retry_stop_or_quit=True,
    )
    return control.FreshBootRecoveryPreflightRequest(**values)


def replace_request(
    request: control.FreshBootRecoveryPreflightRequest,
    **changes: object,
) -> control.FreshBootRecoveryPreflightRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return control.FreshBootRecoveryPreflightRequest(**values)


def make_binding(
    request: control.FreshBootRecoveryPreflightRequest,
) -> SimpleNamespace:
    return SimpleNamespace(
        evidence={"format": bindings.EVIDENCE_FORMAT},
        probe_bytes=(request.repository_root / control.PROBE_RELATIVE_PATH).read_bytes(),
        resume_driver_bytes=(
            request.repository_root / bindings.RESUME_DRIVER_RELATIVE_PATH
        ).read_bytes(),
        prior_boot_id_sha256=guest_probe.EXPECTED_PRIOR_BOOT_ID_SHA256,
        current_boot_id_sha256=FRESH_BOOT_HASH,
        prior_backend_pid=42,
    )


def qualified_payload(
    request: control.FreshBootRecoveryPreflightRequest,
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


def run_case(
    request: control.FreshBootRecoveryPreflightRequest,
    binding: SimpleNamespace,
    runner: recovery_test.RecoveryRunner,
):
    source = recovery_test.classification_test.boot_test.SyntheticSource(
        recovery_test.classification_test.boot_test.CloseTracker()
    )
    return control.run_fresh_boot_recovery_preflight(
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


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


if __name__ == "__main__":
    unittest.main()

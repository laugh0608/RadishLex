#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_transaction_state_resolution as resolution
import l6_utm_install_artifacts_staged_transaction_state_resolution_bindings as bindings
import test_l6_utm_install_artifacts_staged_boot_start_resolution as boot_test
import test_l6_utm_install_artifacts_staged_fresh_boot_resume as resume_test
import test_l6_utm_install_artifacts_staged_runtime_resolution as runtime_test


BOOT_HASH = hashlib.sha256(b"synthetic-transaction-resolution-boot").hexdigest()


class TransactionRunner(boot_test.FakeRunner):
    def __init__(
        self,
        request: resolution.TransactionStateResolutionRequest,
        binding: bindings.TransactionStateResolutionBinding,
        transaction: str,
        *,
        result_payloads: list[bytes] | None = None,
    ) -> None:
        super().__init__(
            request,
            binding.probe_bytes,
            handle_identities=[None] * 5 + [(42, "QEMULauncher")] * 6,
        )
        self.binding = binding
        self.transaction = transaction
        self.transaction_result_payloads = list(result_payloads or [])

    def run(self, argv, timeout_seconds, *, stdin_file=None):
        if (
            argv[0:3] == ("utmctl", "exec", self.request.target_uuid)
            and "/usr/bin/test" in argv
            and "/proc/sys/kernel/random/boot_id" in argv
        ):
            self.calls.append(argv)
            return boot_test.observation(argv)
        if argv[0:3] == ("utmctl", "file", "pull"):
            path = argv[4]
            if path == self.request.guest_marker_path:
                self.calls.append(argv)
                return boot_test.observation(
                    argv,
                    stdout=resolution.guest_probe.marker_bytes(
                        self.request.transaction_state_resolution_attempt_id,
                        self.probe_sha256,
                    ),
                )
            if path == self.request.guest_result_path:
                self.calls.append(argv)
                payload = (
                    self.transaction_result_payloads.pop(0)
                    if self.transaction_result_payloads
                    else result_payload(self.transaction, self.probe_sha256)
                )
                return boot_test.observation(argv, stdout=payload)
        return super().run(
            argv,
            timeout_seconds,
            stdin_file=stdin_file,
        )


class TransactionStateResolutionTests(unittest.TestCase):
    def test_completed_is_observed_once_without_stop_or_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = TransactionRunner(request, binding, "completed")

            result = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "transaction-completed")
            self.assertEqual(result.exit_code, resolution.EXIT_TRANSACTION_COMPLETED)
            self.assertEqual(result.inventory_probe_invocations, 1)
            self.assertEqual(result.foreground_start_invocations, 1)
            self.assertEqual(result.guest_probe_invocations, 1)
            self.assertEqual(result.result_readback_invocations, 2)
            terminal = boot_test.read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["transaction"], "completed")
            self.assertEqual(
                terminal["business_guest_action"],
                "one-read-only-transaction-observation",
            )
            self.assertEqual(terminal["operation_id"], "hash-only")
            self.assertEqual(terminal["automatic_stop"], "not-performed")
            self.assert_forbidden_actions_absent(runner.calls)
            runtime_test.assert_manifest_valid(self, request.output_root)

    def test_artifacts_staged_is_distinct_terminal(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = TransactionRunner(request, binding, "artifacts-staged")

            result = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "transaction-artifacts-staged")
            self.assertEqual(result.exit_code, resolution.EXIT_ARTIFACTS_STAGED)
            terminal = boot_test.read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["transaction"], "artifacts-staged")
            classification = boot_test.read_json(
                request.output_root / "transaction-state-classification.json"
            )
            self.assertEqual(classification["transaction"], "artifacts-staged")
            self.assert_forbidden_actions_absent(runner.calls)

    def test_guest_indeterminate_stays_indeterminate_without_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = TransactionRunner(request, binding, "state-indeterminate")

            result = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, resolution.EXIT_STATE_INDETERMINATE)
            terminal = boot_test.read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["transaction"], "state-indeterminate")
            self.assertEqual(terminal["automatic_retry"], "not-performed")
            self.assert_forbidden_actions_absent(runner.calls)

    def test_double_readback_drift_discards_transaction_classification(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = TransactionRunner(
                request,
                binding,
                "completed",
                result_payloads=[
                    result_payload("completed", runner_probe_sha256(binding)),
                    result_payload("artifacts-staged", runner_probe_sha256(binding)),
                ],
            )

            result = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertFalse(
                (
                    request.output_root
                    / "transaction-state-classification.json"
                ).exists()
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_authorization_and_prior_root_overlap_reject_before_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            unauthorized = replace_request(
                request,
                authorized_one_read_only_transaction_observation=False,
            )
            with self.assertRaisesRegex(
                resolution.boot_control.BootStartResolutionError,
                "one-read-only-transaction-observation-authorization-required",
            ):
                unauthorized.validate()

            overlapping = replace_request(
                request,
                output_root=request.prior_fresh_boot_resume_root / "new",
            )
            with self.assertRaisesRegex(
                resolution.boot_control.BootStartResolutionError,
                "output-root-must-not-overlap-prior-fresh-boot-resume-root",
            ):
                overlapping.validate()

    def test_request_exposes_current_attempt_and_separate_stop_boundary(self) -> None:
        request = make_request(Path("/tmp/radishlex-transaction-request"))

        value = request.as_json()

        self.assertEqual(
            value["boot_start_attempt_id"],
            resolution.REQUIRED_ATTEMPT_ID,
        )
        self.assertEqual(
            value["prior_boot_start_attempt_id"],
            request.boot_start_attempt_id,
        )
        authorization = value["authorization"]
        self.assertEqual(
            authorization["terminal_stop_separate_and_not_performed"],
            True,
        )
        self.assertNotIn(
            "one_maintenance_resume_and_postflight",
            authorization,
        )

    def test_prior_binding_view_restores_frozen_guest_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            view = bindings._PriorFreshBootResumeRequestView(request)
            root = (
                "/var/tmp/radishlex-l6-v4-boot-start-"
                + bindings.classification_bindings.REQUIRED_ATTEMPT_ID
            )

            self.assertEqual(view.guest_control_root, root)
            self.assertEqual(
                view.guest_probe_incoming,
                f"{view.guest_recovery_root}/recovery-preflight.incoming.py",
            )
            self.assertEqual(
                view.guest_probe_path,
                f"{view.guest_recovery_root}/recovery-preflight.py",
            )
            self.assertEqual(
                view.guest_marker_path,
                f"{view.guest_recovery_root}/attempt.marker.json",
            )
            self.assertEqual(
                view.guest_result_path,
                f"{root}/boot-identity.evidence.json",
            )

    def test_binding_failure_keeps_prior_state_indeterminate_without_system_calls(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = TransactionRunner(request, binding, "completed")
            source = boot_test.SyntheticSource(boot_test.CloseTracker())

            result = resolution.run_transaction_state_resolution(
                request,
                runner=runner,
                binding_validator=lambda _: (_ for _ in ()).throw(
                    resolution.boot_control.BootStartResolutionError(
                        "synthetic-prior-binding-drift"
                    )
                ),
                source_opener=lambda _: source,
                sleeper=lambda _: None,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(runner.calls, [])
            terminal = boot_test.read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["transaction"], "state-indeterminate")
            self.assertEqual(terminal["automatic_stop"], "not-performed")

    def test_prior_forty_seven_member_result_is_recursive_authority(self) -> None:
        request = make_request(Path("/tmp/radishlex-transaction-binding"))
        prior = synthetic_prior_result()
        with mock.patch.object(
            bindings.prior_bindings,
            "validate_fresh_boot_resume_result_bindings",
            return_value=prior,
        ) as validate_prior, mock.patch.object(
            bindings.network_ready, "_require_committed_regular"
        ), mock.patch.object(
            bindings, "_baseline_inventory", return_value=runtime_test.baseline_inventory()
        ):
            result = bindings.validate_transaction_state_resolution_bindings(
                request
            )

        validate_prior.assert_called_once()
        self.assertEqual(
            result.evidence["prior_fresh_boot_resume_entries_verified"], 47
        )
        self.assertEqual(
            result.evidence["prior_transaction"], "state-indeterminate"
        )
        self.assertEqual(len(result.baseline_inventory), 20)
        self.assertTrue(result.probe_bytes)

    def assert_forbidden_actions_absent(self, calls) -> None:
        commands = [" ".join(call) for call in calls]
        for forbidden in (
            "utmctl status",
            "utmctl stop",
            " resume ",
            " retry ",
            " repair ",
            " cleanup ",
            "dpkg --install",
            "dpkg -i",
        ):
            self.assertFalse(any(forbidden in value for value in commands))


def make_request(root: Path) -> resolution.TransactionStateResolutionRequest:
    base = resume_test.make_request(root)
    values = dict(base.__dict__)
    values.update(
        output_root=root / "transaction-state-resolution",
        prior_fresh_boot_resume_root=root / "prior-fresh-boot-resume",
        prior_fresh_boot_resume_manifest_sha256=(
            bindings.REQUIRED_PRIOR_MANIFEST_SHA256
        ),
        prior_fresh_boot_resume_attempt_id=bindings.REQUIRED_PRIOR_ATTEMPT_ID,
        transaction_state_resolution_attempt_id=resolution.REQUIRED_ATTEMPT_ID,
        authorized_transaction_state_resolution=True,
        authorized_bound_prior_indeterminate_resume_result=True,
        authorized_one_all_stopped_inventory=True,
        authorized_one_foreground_start_for_read_only_observation=True,
        authorized_one_read_only_transaction_observation=True,
        authorized_terminal_stop_separate_and_not_performed=True,
        authorized_no_resume_dpkg_mutation_retry_repair_cleanup_or_automatic_stop=True,
    )
    return resolution.TransactionStateResolutionRequest(**values)


def replace_request(
    request: resolution.TransactionStateResolutionRequest,
    **changes: object,
) -> resolution.TransactionStateResolutionRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return resolution.TransactionStateResolutionRequest(**values)


def make_binding() -> bindings.TransactionStateResolutionBinding:
    probe_bytes = (
        Path(resolution.__file__).resolve().parents[2]
        / resolution.PROBE_RELATIVE_PATH
    ).read_bytes()
    return bindings.TransactionStateResolutionBinding(
        evidence={"format": bindings.EVIDENCE_FORMAT},
        upstream=synthetic_prior_result(),
        baseline_inventory=runtime_test.baseline_inventory(),
        probe_bytes=probe_bytes,
    )


def synthetic_prior_result():
    return SimpleNamespace(
        evidence={
            "dpkg_mutation_executed": "unknown",
            "prior_fresh_boot_resume_entries_verified": 47,
            "transaction": "state-indeterminate",
        },
        guest_resume_outcome="state-indeterminate",
        maintenance_resume_invocations=1,
        postflight_invocations=1,
    )


def result_payload(transaction: str, probe_sha256: str) -> bytes:
    if transaction == "completed":
        observation = {
            "boot_id_sha256": BOOT_HASH,
            "dpkg_log_sha256": "c" * 64,
            "dpkg_log_size": 880_000,
            "dpkg_status_sha256": (
                resolution.guest_probe.EXPECTED_INSTALLED_DPKG_STATUS_SHA256
            ),
            "guard_profile": "absent-after-reboot",
            "package_profile": "installed-verified",
            "receipt_sha256": "d" * 64,
            "receipt_size": 4096,
            "startup_profile": "allowed",
            "transaction": transaction,
        }
    elif transaction == "artifacts-staged":
        observation = {
            "boot_id_sha256": BOOT_HASH,
            "dpkg_log_sha256": (
                resolution.guest_probe.EXPECTED_INITIAL_DPKG_LOG_SHA256
            ),
            "dpkg_log_size": (
                resolution.guest_probe.EXPECTED_INITIAL_DPKG_LOG_SIZE
            ),
            "dpkg_status_sha256": (
                resolution.guest_probe.EXPECTED_INITIAL_DPKG_STATUS_SHA256
            ),
            "guard_profile": "absent-after-reboot",
            "package_profile": "not-installed-staging-preserved",
            "receipt_sha256": resolution.guest_probe.EXPECTED_RECEIPT_SHA256,
            "receipt_size": resolution.guest_probe.EXPECTED_RECEIPT_SIZE,
            "startup_profile": "operation-in-progress",
            "transaction": transaction,
        }
    else:
        observation = None
    value = resolution.guest_probe.result_value(
        attempt_id=resolution.REQUIRED_ATTEMPT_ID,
        target_uuid=resolution.guest_probe.EXPECTED_TARGET_UUID,
        probe_sha256=probe_sha256,
        observation=observation,
        reason=(
            "read-only-transaction-state-observed"
            if observation is not None
            else "TransactionStateProbeError:synthetic-indeterminate"
        ),
    )
    return resolution.guest_probe.canonical_json(value)


def runner_probe_sha256(
    binding: bindings.TransactionStateResolutionBinding,
) -> str:
    return hashlib.sha256(binding.probe_bytes).hexdigest()


def run_case(
    request: resolution.TransactionStateResolutionRequest,
    binding: bindings.TransactionStateResolutionBinding,
    runner: TransactionRunner,
):
    source = boot_test.SyntheticSource(boot_test.CloseTracker())
    return resolution.run_transaction_state_resolution(
        request,
        runner=runner,
        binding_validator=lambda _: binding,
        source_opener=lambda _: source,
        source_revalidator=lambda _request, _source: {
            "descriptor_unchanged": True,
            "format": "synthetic-source-v1",
            "inventory_unchanged": True,
            "sha256": resolution.boot_control.reactivation_control.REQUIRED_SOURCE_BUNDLE_SHA256,
            "size": resolution.boot_control.reactivation_control.REQUIRED_SOURCE_BUNDLE_SIZE,
        },
        target_validator=lambda _: runtime_test.target_identity(request),
        sleeper=lambda _: None,
    )


if __name__ == "__main__":
    unittest.main()

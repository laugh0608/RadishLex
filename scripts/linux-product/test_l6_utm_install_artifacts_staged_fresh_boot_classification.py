#!/usr/bin/env python3
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_fresh_boot_classification as control
import l6_utm_install_artifacts_staged_fresh_boot_classification_bindings as bindings
import test_l6_utm_install_artifacts_staged_boot_classification_resolution as classification_test
import test_l6_utm_install_artifacts_staged_new_boot_recovery_preflight as recovery_test
import test_l6_utm_install_artifacts_staged_runtime_resolution as runtime_test


class FreshBootClassificationTests(unittest.TestCase):
    def test_latest_rejected_preflight_maps_to_ended_boot_transport(self) -> None:
        request = make_request(Path("/tmp/radishlex-fresh-boot-binding-test"))
        transport = SimpleNamespace(label="boot-transport")
        prior_binding = SimpleNamespace(upstream=transport)
        prior_classification = SimpleNamespace(
            observed_boot_id_sha256=(
                bindings.recovery_result_bindings.recovery_bindings.REQUIRED_CURRENT_BOOT_ID_SHA256
            ),
            probe_bytes=b"fixed boot probe",
            upstream=prior_binding,
        )
        result = SimpleNamespace(
            evidence={
                "host_recorded_outcome": "state-indeterminate",
                "prior_recovery_preflight_entries_verified": 38,
                "result_authority": "guest-marker-double-result-and-phase",
            },
            guest_probe_transport_exit_code=0,
            guest_recovery_outcome="recovery-rejected",
            prior_backend_pid=92422,
            upstream=prior_classification,
        )
        with mock.patch.object(
            bindings.recovery_result_bindings,
            "validate_recovery_preflight_result_bindings",
            return_value=result,
        ):
            binding = bindings.validate_fresh_boot_classification_bindings(
                request
            )

        self.assertIs(binding.upstream, transport)
        self.assertEqual(
            binding.expected_boot_id_sha256,
            prior_classification.observed_boot_id_sha256,
        )
        self.assertEqual(
            binding.evidence["prior_recovery_preflight_entries_verified"], 38
        )

    def test_fully_stopped_inventory_starts_once_and_classifies_fresh_boot(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = classification_test.boot_test.make_binding()
            binding.expected_boot_id_sha256 = "b" * 64
            runner = classification_test.ClassificationRunner(
                request,
                binding.probe_bytes,
                boot_hash="c" * 64,
            )

            result = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "new-boot-started")
            self.assertEqual(result.foreground_start_invocations, 1)
            self.assertEqual(result.inventory_probe_invocations, 1)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["format"], control.EVIDENCE_FORMAT)
            self.assertEqual(
                terminal["boot_start_attempt_id"], bindings.REQUIRED_ATTEMPT_ID
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_authorization_and_frozen_root_overlap_fail_before_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            unauthorized = replace_request(
                request,
                authorized_bound_prior_recovery_preflight_result=False,
            )
            with self.assertRaisesRegex(
                control.boot_control.BootStartResolutionError,
                "authorized-bound-prior-recovery-preflight-result-required",
            ):
                unauthorized.validate()

            overlapping = replace_request(
                request,
                output_root=request.prior_recovery_preflight_root / "new",
            )
            with self.assertRaisesRegex(
                control.boot_control.BootStartResolutionError,
                "output-root-must-not-overlap-prior-recovery-preflight-root",
            ):
                overlapping.validate()

    def assert_forbidden_actions_absent(
        self, calls: list[tuple[str, ...]]
    ) -> None:
        flattened = [" ".join(call) for call in calls]
        for forbidden in (
            "utmctl status",
            "utmctl start",
            " resume ",
            " retry ",
            " stop ",
            " quit ",
            " dpkg ",
        ):
            self.assertFalse(any(forbidden in value for value in flattened))


def make_request(root: Path) -> control.FreshBootClassificationRequest:
    prior = recovery_test.make_request(root)
    values = dict(prior.__dict__)
    values.update(
        output_root=root / "fresh-boot-classification-output",
        boot_start_attempt_id=bindings.REQUIRED_ATTEMPT_ID,
        prior_recovery_preflight_root=root / "prior-recovery-preflight",
        prior_recovery_preflight_manifest_sha256=(
            bindings.REQUIRED_PRIOR_RECOVERY_MANIFEST_SHA256
        ),
        prior_recovery_preflight_attempt_id=(
            bindings.REQUIRED_PRIOR_RECOVERY_ATTEMPT_ID
        ),
        authorized_install_artifacts_staged_fresh_boot_classification=True,
        authorized_bound_prior_recovery_preflight_result=True,
        authorized_bounded_fresh_boot_agent_readiness=True,
    )
    return control.FreshBootClassificationRequest(**values)


def replace_request(
    request: control.FreshBootClassificationRequest,
    **changes: object,
) -> control.FreshBootClassificationRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return control.FreshBootClassificationRequest(**values)


def run_case(
    request: control.FreshBootClassificationRequest,
    binding: SimpleNamespace,
    runner: classification_test.ClassificationRunner,
):
    source = SimpleNamespace(
        file_object=classification_test.boot_test.CloseTracker(),
        as_json=lambda: {
            "descriptor": {
                "sha256": request.source_bundle_sha256,
                "size": request.source_bundle_size,
            },
            "format": "radishlex-linux-l6-utm-canonical-input-transfer-v1",
            "inventory_count": 12,
        },
    )
    return control.run_fresh_boot_classification(
        request,
        runner=runner,
        binding_validator=lambda _: binding,
        source_opener=lambda _: source,
        source_revalidator=lambda _request, _source: {
            "descriptor_unchanged": True,
            "format": "radishlex-linux-l6-utm-canonical-input-transfer-v1",
            "inventory_unchanged": True,
            "sha256": request.source_bundle_sha256,
            "size": request.source_bundle_size,
        },
        target_validator=lambda _: {
            **runtime_test.target_identity(request),
            "format": control.boot_control.network_ready.EVIDENCE_FORMAT,
            "target_config_sha256": (
                control.boot_control.network_ready.REQUIRED_TARGET_CONFIG_SHA256
            ),
        },
        sleeper=lambda _: None,
    )


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


if __name__ == "__main__":
    unittest.main()

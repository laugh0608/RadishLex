#!/usr/bin/env python3
from __future__ import annotations

import unittest

import l6_guest_case_contract


def crash_matrix_cases() -> list[dict[str, object]]:
    return [
        {
            "id": "install_artifacts_staged",
            "operation": "install_source",
            "checkpoint": "artifacts_staged",
            "fault": "process_group_terminated",
            "expected_terminal": "completed",
            "restore_snapshot_after": True,
        },
        {
            "id": "upgrade_quiesced",
            "operation": "upgrade_target",
            "checkpoint": "quiesced",
            "fault": "process_group_terminated",
            "expected_terminal": "completed",
            "restore_snapshot_after": True,
        },
    ]


class LinuxL6GuestCaseContractTests(unittest.TestCase):
    def test_input_inventory_is_canonical_and_uses_one_environment_name(self) -> None:
        inventory = l6_guest_case_contract.canonical_input_inventory(
            "radishlex_26.7.1+38-1_arm64.deb",
            "radishlex_26.7.1+38-2_arm64.deb",
        )

        self.assertEqual(
            inventory,
            tuple(sorted(inventory, key=lambda item: item.encode("utf-8"))),
        )
        self.assertIn("build-environment.json", inventory)
        self.assertNotIn("build-environment.evidence.json", inventory)
        self.assertLess(
            inventory.index(
                "target-startup/usr/lib/aarch64-linux-gnu/radishlex/manager/lib/"
                "libradishlex_ime_ffi.so"
            ),
            inventory.index(
                "target/artifacts/radishlex_26.7.1+38-2_arm64.deb"
            ),
        )

    def test_input_inventory_rejects_unsafe_or_wrong_architecture_names(self) -> None:
        invalid_names = (
            "../radishlex_26.7.1+38-1_arm64.deb",
            "radishlex_26.7.1+38-1_amd64.deb",
            "radishlex_26.7.1+38-1_arm64.deb/evidence",
            "radishlex 26.7.1+38-1_arm64.deb",
        )
        for name in invalid_names:
            with self.subTest(name=name):
                with self.assertRaises(
                    l6_guest_case_contract.LinuxL6GuestCaseContractError
                ):
                    l6_guest_case_contract.canonical_input_inventory(
                        name, "radishlex_26.7.1+38-2_arm64.deb"
                    )

    def test_guest_agent_observation_is_not_a_success_truth_source(self) -> None:
        observation = l6_guest_case_contract.GuestAgentObservation(
            observer_exit_code=0,
            observer_stdout=b"",
            observer_stderr=b"",
            result_file=None,
            evidence_file=None,
        )
        with self.assertRaises(
            l6_guest_case_contract.LinuxL6GuestCaseContractError
        ):
            observation.require_file_readback()

    def test_result_and_evidence_readback_remain_authoritative(self) -> None:
        observation = l6_guest_case_contract.GuestAgentObservation(
            observer_exit_code=255,
            observer_stdout=b"untrusted observer output\n",
            observer_stderr=b"untrusted observer error\n",
            result_file=b"phase=preflight\nexit_code=0\n",
            evidence_file=b"format=synthetic-l6-evidence-v1\noutcome=passed\n",
        )

        readback = observation.require_file_readback()

        self.assertEqual(readback.result, "phase=preflight\nexit_code=0\n")
        self.assertEqual(
            readback.evidence,
            "format=synthetic-l6-evidence-v1\noutcome=passed\n",
        )

    def test_empty_or_noncanonical_readback_is_rejected(self) -> None:
        invalid_values = (
            (b"", b"format=synthetic\n"),
            (b"exit_code=0", b"format=synthetic\n"),
            (b"exit_code=0\n", b"format=synthetic\x00\n"),
        )
        for result_file, evidence_file in invalid_values:
            with self.subTest():
                observation = l6_guest_case_contract.GuestAgentObservation(
                    observer_exit_code=0,
                    observer_stdout=b"",
                    observer_stderr=b"",
                    result_file=result_file,
                    evidence_file=evidence_file,
                )
                with self.assertRaises(
                    l6_guest_case_contract.LinuxL6GuestCaseContractError
                ):
                    observation.require_file_readback()

    def test_fresh_absent_and_removed_terminal_are_distinct(self) -> None:
        fresh = l6_guest_case_contract.startup_expectation("fresh-absent")
        removed = l6_guest_case_contract.startup_expectation("removed-terminal")

        self.assertEqual(fresh.decision, "FailedClosed")
        self.assertEqual(fresh.reason, "ReceiptMissing")
        self.assertIsNone(fresh.receipt_terminal)
        self.assertEqual(fresh.wire_output, "0:1:4:15:0|error-absent")
        self.assertEqual(removed.decision, "FailedClosed")
        self.assertEqual(removed.reason, "RemovedProgram")
        self.assertEqual(removed.receipt_terminal, "completed")
        self.assertEqual(removed.wire_output, "0:1:4:24:6|error-absent")
        self.assertNotEqual(fresh, removed)

    def test_install_artifacts_staged_checkpoint_is_exact(self) -> None:
        expectation = l6_guest_case_contract.crash_checkpoint_expectation(
            "install_artifacts_staged"
        )

        self.assertEqual(expectation.matrix_operation, "install_source")
        self.assertEqual(expectation.checkpoint, "artifacts_staged")
        self.assertEqual(expectation.receipt.operation_kind, "install")
        self.assertEqual(expectation.receipt.version_relation, "not_applicable")
        self.assertEqual(expectation.receipt.state, "artifacts_staged")
        self.assertEqual(expectation.receipt.operation_chain_length, 1)
        self.assertEqual(expectation.receipt.required_staged_slots, ("target",))
        self.assertFalse(expectation.receipt.source_artifact_present)
        self.assertTrue(expectation.receipt.target_artifact_present)
        self.assertEqual(expectation.receipt.staged_slots, ("target",))
        self.assertFalse(expectation.receipt.target_proof_present)
        self.assertFalse(expectation.receipt.source_proof_present)
        self.assertIsNone(expectation.receipt.failure_code)
        self.assertFalse(expectation.receipt.manual_recovery_required)
        self.assertEqual(expectation.package_state, "not_installed")
        self.assertEqual(expectation.dpkg_status, "unchanged_from_preflight")
        self.assertEqual(expectation.dpkg_log, "unchanged_from_preflight")
        self.assertEqual(expectation.guard.owner, "root:root")
        self.assertEqual(expectation.guard.mode, "0600")
        self.assertEqual(expectation.guard.size_bytes, 0)
        self.assertEqual(expectation.guard.link_count, 1)
        self.assertEqual(
            expectation.guard.advisory_lock, "unlocked_after_worker_exit"
        )
        self.assertEqual(expectation.startup_case, "active-guard")
        self.assertEqual(expectation.process_group, "terminated")
        self.assertEqual(expectation.process_group_member_count, 0)
        self.assertEqual(expectation.dpkg_child, "absent")
        self.assertEqual(expectation.xdg, "unchanged_from_preflight")
        self.assertEqual(
            expectation.product_processes, "unchanged_from_preflight"
        )
        self.assertEqual(expectation.network, "unchanged_from_preflight")
        self.assertEqual(
            expectation.resume_steps,
            (
                "validate_staged_relationship",
                "prove_target_quiescence",
                "apply_target_once",
                "verify_target",
                "complete",
            ),
        )
        self.assertEqual(expectation.expected_terminal, "completed")
        l6_guest_case_contract.validate_crash_checkpoint_matrix(
            crash_matrix_cases()
        )

    def test_install_artifacts_staged_rejects_matrix_drift(self) -> None:
        with self.assertRaises(
            l6_guest_case_contract.LinuxL6GuestCaseContractError
        ):
            scenarios = crash_matrix_cases()
            scenarios[0]["checkpoint"] = "prepared"
            l6_guest_case_contract.validate_crash_checkpoint_matrix(scenarios)

    def test_upgrade_quiesced_checkpoint_is_exact(self) -> None:
        expectation = l6_guest_case_contract.crash_checkpoint_expectation(
            "upgrade_quiesced"
        )

        self.assertEqual(expectation.matrix_operation, "upgrade_target")
        self.assertEqual(expectation.checkpoint, "quiesced")
        self.assertEqual(expectation.receipt.operation_kind, "upgrade")
        self.assertEqual(expectation.receipt.version_relation, "target_newer")
        self.assertEqual(expectation.receipt.state, "quiesced")
        self.assertEqual(expectation.receipt.operation_chain_length, 2)
        self.assertEqual(
            expectation.receipt.required_staged_slots, ("source", "target")
        )
        self.assertTrue(expectation.receipt.source_artifact_present)
        self.assertTrue(expectation.receipt.target_artifact_present)
        self.assertEqual(
            expectation.receipt.staged_slots, ("source", "target")
        )
        self.assertFalse(expectation.receipt.target_proof_present)
        self.assertFalse(expectation.receipt.source_proof_present)
        self.assertIsNone(expectation.receipt.failure_code)
        self.assertFalse(expectation.receipt.manual_recovery_required)
        self.assertEqual(expectation.package_state, "source_installed")
        self.assertEqual(expectation.dpkg_status, "unchanged_from_preflight")
        self.assertEqual(expectation.dpkg_log, "unchanged_from_preflight")
        self.assertEqual(expectation.startup_case, "active-guard")
        self.assertEqual(expectation.process_group, "terminated")
        self.assertEqual(expectation.process_group_member_count, 0)
        self.assertEqual(expectation.dpkg_child, "absent")
        self.assertEqual(expectation.xdg, "unchanged_from_s2")
        self.assertEqual(
            expectation.resume_steps,
            (
                "validate_staged_relationship",
                "prove_target_quiescence",
                "apply_target_once",
                "verify_target",
                "complete",
            ),
        )
        self.assertEqual(expectation.expected_terminal, "completed")
        l6_guest_case_contract.validate_crash_checkpoint_matrix(
            crash_matrix_cases()
        )

    def test_upgrade_quiesced_rejects_matrix_drift(self) -> None:
        with self.assertRaises(
            l6_guest_case_contract.LinuxL6GuestCaseContractError
        ):
            scenarios = crash_matrix_cases()
            scenarios[1]["checkpoint"] = "package_mutating_before_dpkg"
            l6_guest_case_contract.validate_crash_checkpoint_matrix(scenarios)

    def test_unknown_startup_case_is_rejected(self) -> None:
        with self.assertRaises(
            l6_guest_case_contract.LinuxL6GuestCaseContractError
        ):
            l6_guest_case_contract.startup_expectation("unknown")


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
from __future__ import annotations

import copy
import unittest

import l6_upgrade_quiesced_case_contract as case_contract


class LinuxL6UpgradeQuiescedCaseContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.contract, self.matrix, self.release_pair = (
            case_contract.load_repository_contract()
        )

    def validate(self) -> None:
        case_contract.validate_upgrade_quiesced_case_contract(
            self.contract, self.matrix, self.release_pair
        )

    def test_repository_contract_is_complete(self) -> None:
        self.validate()

        self.assertEqual(
            self.contract["start"]["identity"],
            "S2-source-data-512e8ab-0c2cefd6",
        )
        self.assertFalse(self.contract["start"]["registered_with_utm"])
        self.assertEqual(
            self.contract["release_pair"]["source"]["package_sha256"],
            case_contract.EXPECTED_START_PACKAGE_SHA256,
        )
        self.assertEqual(
            self.contract["checkpoint"]["receipt"]["operation_chain_length"],
            2,
        )
        self.assertEqual(
            [item["id"] for item in self.contract["authorization_sequence"]],
            [
                "clone",
                "start",
                "input-preflight",
                "crash",
                "resume",
                "terminal-stop",
            ],
        )

    def test_snapshot_or_installed_source_drift_is_rejected(self) -> None:
        for field, value in (
            ("qcow2_sha256", "0" * 64),
            ("source_dpkg_status_sha256", "1" * 64),
        ):
            with self.subTest(field=field):
                contract = copy.deepcopy(self.contract)
                contract["start"][field] = value
                with self.assertRaises(
                    case_contract.UpgradeQuiescedCaseContractError
                ):
                    case_contract.validate_upgrade_quiesced_case_contract(
                        contract, self.matrix, self.release_pair
                    )

    def test_source_target_relationship_drift_is_rejected(self) -> None:
        contract = copy.deepcopy(self.contract)
        contract["release_pair"]["target"]["package_sha256"] = "2" * 64

        with self.assertRaises(
            case_contract.UpgradeQuiescedCaseContractError
        ):
            case_contract.validate_upgrade_quiesced_case_contract(
                contract, self.matrix, self.release_pair
            )

    def test_quiesced_checkpoint_drift_is_rejected(self) -> None:
        contract = copy.deepcopy(self.contract)
        contract["checkpoint"]["receipt"]["state"] = (
            "package_mutating"
        )

        with self.assertRaises(
            case_contract.UpgradeQuiescedCaseContractError
        ):
            case_contract.validate_upgrade_quiesced_case_contract(
                contract, self.matrix, self.release_pair
            )

    def test_matrix_drift_is_rejected(self) -> None:
        matrix = copy.deepcopy(self.matrix)
        scenario = next(
            item
            for item in matrix["crash_scenarios"]
            if item["id"] == "upgrade_quiesced"
        )
        scenario["checkpoint"] = "package_mutating_before_dpkg"

        with self.assertRaises(
            case_contract.UpgradeQuiescedCaseContractError
        ):
            case_contract.validate_upgrade_quiesced_case_contract(
                self.contract, matrix, self.release_pair
            )

    def test_authorization_phases_cannot_be_collapsed(self) -> None:
        contract = copy.deepcopy(self.contract)
        contract["authorization_sequence"].pop(2)

        with self.assertRaises(
            case_contract.UpgradeQuiescedCaseContractError
        ):
            case_contract.validate_upgrade_quiesced_case_contract(
                contract, self.matrix, self.release_pair
            )


if __name__ == "__main__":
    unittest.main()

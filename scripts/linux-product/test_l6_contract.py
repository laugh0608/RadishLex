#!/usr/bin/env python3
from __future__ import annotations

import copy
import json
import tempfile
import unittest
from pathlib import Path

import l6_contract


class LinuxL6ContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.matrix = l6_contract.validate_matrix()

    def write_matrix(self, value: dict[str, object], canonical: bool = True) -> Path:
        temporary = tempfile.NamedTemporaryFile(delete=False)
        temporary.close()
        path = Path(temporary.name)
        if canonical:
            path.write_bytes(l6_contract.canonical_json_bytes(value))
        else:
            path.write_text(json.dumps(value), encoding="utf-8")
        self.addCleanup(path.unlink)
        return path

    def assert_rejected(self, value: dict[str, object]) -> None:
        with self.assertRaises(l6_contract.LinuxL6ContractError):
            l6_contract.validate_matrix(self.write_matrix(value))

    def test_canonical_matrix_covers_primary_sequence_and_crash_recovery(self) -> None:
        self.assertEqual(
            [item["kind"] for item in self.matrix["operations"]],
            ["install", "upgrade", "repair", "rollback", "remove", "install"],
        )
        self.assertEqual(len(self.matrix["crash_scenarios"]), 8)
        self.assertEqual(
            {item["expected_terminal"] for item in self.matrix["crash_scenarios"]},
            {"completed", "rolled_back"},
        )

    def test_guest_identity_and_p04_isolation_are_fixed(self) -> None:
        for key, value in (
            ("version_id", "12"),
            ("architecture", "amd64"),
            ("acceptance_user", "existing-user"),
            ("p04_guest_reuse", True),
        ):
            changed = copy.deepcopy(self.matrix)
            changed["guest"][key] = value
            self.assert_rejected(changed)

    def test_release_pair_cannot_be_same_artifact_or_same_commit(self) -> None:
        for key in ("package_hash_distinct", "data_contract_equal"):
            changed = copy.deepcopy(self.matrix)
            changed["release_pair"][key] = not changed["release_pair"][key]
            self.assert_rejected(changed)
        changed = copy.deepcopy(self.matrix)
        changed["release_pair"]["strategy"] = "same-build-renamed-v1"
        self.assert_rejected(changed)

    def test_primary_operation_order_and_artifact_roles_are_fixed(self) -> None:
        changed = copy.deepcopy(self.matrix)
        changed["operations"][1], changed["operations"][2] = (
            changed["operations"][2],
            changed["operations"][1],
        )
        self.assert_rejected(changed)
        changed = copy.deepcopy(self.matrix)
        changed["operations"][3]["source"] = "source"
        self.assert_rejected(changed)

    def test_crash_checkpoints_and_snapshot_restoration_are_fixed(self) -> None:
        changed = copy.deepcopy(self.matrix)
        changed["crash_scenarios"][3]["checkpoint"] = "arbitrary-signal"
        self.assert_rejected(changed)
        changed = copy.deepcopy(self.matrix)
        changed["crash_scenarios"][0]["restore_snapshot_after"] = False
        self.assert_rejected(changed)

    def test_font_startup_and_xdg_probes_cannot_be_weakened(self) -> None:
        for section in ("font_roles", "startup_cases", "xdg_paths"):
            changed = copy.deepcopy(self.matrix)
            changed["probes"][section].pop()
            self.assert_rejected(changed)
        changed = copy.deepcopy(self.matrix)
        changed["probes"]["xdg_paths"][0] = "/home/p04/.local/share/radishlex"
        self.assert_rejected(changed)

    def test_authorization_offline_and_preservation_policies_are_fixed(self) -> None:
        for key, value in (
            ("one_mutation_per_authorization", False),
            ("network_policy", "online"),
            ("cleanup_policy", "clean-on-success"),
        ):
            changed = copy.deepcopy(self.matrix)
            changed["execution"][key] = value
            self.assert_rejected(changed)

    def test_noncanonical_unknown_or_symlink_matrix_is_rejected(self) -> None:
        with self.assertRaises(l6_contract.LinuxL6ContractError):
            l6_contract.validate_matrix(self.write_matrix(self.matrix, canonical=False))
        changed = copy.deepcopy(self.matrix)
        changed["unexpected"] = True
        self.assert_rejected(changed)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / "target.json"
            target.write_bytes(l6_contract.canonical_json_bytes(self.matrix))
            link = root / "link.json"
            link.symlink_to(target)
            with self.assertRaises(l6_contract.LinuxL6ContractError):
                l6_contract.validate_matrix(link)


if __name__ == "__main__":
    unittest.main()

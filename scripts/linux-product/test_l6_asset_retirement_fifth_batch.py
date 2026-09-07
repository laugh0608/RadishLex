#!/usr/bin/env python3
from __future__ import annotations

import json
import unittest
from pathlib import Path

import l6_asset_retirement_fifth_batch as fifth_batch
import l6_utm_asset_retirement_prepare as retirement


class LinuxL6AssetRetirementFifthBatchTests(unittest.TestCase):
    def setUp(self) -> None:
        self.repository_root = Path(__file__).resolve().parents[2]
        allowlist = retirement.load_allowlist(
            self.repository_root
            / "packaging/linux/l6-asset-retirement-allowlist.json"
        )
        self.batch = allowlist.batches["fifth-batch-v1"]
        projection_path = (
            self.repository_root
            / "packaging/linux/l6-asset-retirement-fifth-batch-projections.json"
        )
        self.projection = json.loads(projection_path.read_bytes())

    def test_projection_keeps_legacy_owner_and_s2_fail_closed(self) -> None:
        for asset in self.batch.assets:
            fifth_batch.validate_terminal_projection(self.projection, asset)

        changed_owner = json.loads(json.dumps(self.projection))
        changed_owner["policy"]["historical_evidence_owner"] = "501:0-accepted"
        with self.assertRaises(fifth_batch.ProjectionValidationError):
            fifth_batch.validate_terminal_projection(
                changed_owner, self.batch.assets[0]
            )

        false_copy = json.loads(json.dumps(self.projection))
        source_terminal = self.batch.assets[2]
        false_copy["assets"][2]["retained_predecessor"]["efi_sha256"] = (
            source_terminal.efi_sha256
        )
        with self.assertRaises(fifth_batch.ProjectionValidationError):
            fifth_batch.validate_terminal_projection(false_copy, source_terminal)

    def test_retained_s2_is_parsed_as_predecessor_not_current_copy(self) -> None:
        source_terminal = self.batch.assets[2]
        predecessor = {
            "format": "radishlex-linux-l6-local-snapshot-evidence-v1",
            "source_vm": {
                "relative_path": source_terminal.relative_path,
                "name": source_terminal.name,
                "uuid": source_terminal.uuid,
                "source_vm_stopped_after_verification": True,
                "all_registered_vms_stopped_after_verification": True,
                "registered_running_vm_count_after_verification": 0,
                "qcow_open_handles_after_verification": 0,
            },
            "snapshot": {
                "id": fifth_batch.S2_PREDECESSOR["identity"],
                "registered_with_utm": False,
                "restorable_files": [
                    {
                        "path": "config.plist",
                        "sha256": fifth_batch.S2_PREDECESSOR["config_sha256"],
                    },
                    {
                        "path": "Data/efi_vars.fd",
                        "sha256": fifth_batch.S2_PREDECESSOR["efi_sha256"],
                    },
                    {
                        "path": f"Data/{source_terminal.qcow2_name}",
                        "sha256": fifth_batch.S2_PREDECESSOR["qcow2_sha256"],
                    },
                ],
            },
        }
        fifth_batch.validate_retained_predecessor_snapshot(
            predecessor, source_terminal
        )

        predecessor["snapshot"]["restorable_files"][1]["sha256"] = (
            source_terminal.efi_sha256
        )
        with self.assertRaises(fifth_batch.ProjectionValidationError):
            fifth_batch.validate_retained_predecessor_snapshot(
                predecessor, source_terminal
            )


if __name__ == "__main__":
    unittest.main()

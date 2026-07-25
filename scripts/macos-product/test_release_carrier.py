#!/usr/bin/env python3
"""Contract tests for the macOS notarized release carrier evidence."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import release_carrier


SUBMISSION_ID = "12345678-1234-4234-9234-123456789abc"


class ReleaseCarrierTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        metadata = release_carrier.load_metadata()
        self.carrier = (
            self.root
            / f"RadishLex-{metadata.product_version}-{metadata.build_number}.dmg"
        )
        self.carrier.write_bytes(b"signed-dmg")
        self.installer = self.root / "RadishLex Installer.app"
        executable = self.installer / "Contents/MacOS/RadishLex Installer"
        executable.parent.mkdir(parents=True)
        executable.write_bytes(b"installer")
        resource = self.installer / "Contents/Resources/value.txt"
        resource.parent.mkdir(parents=True)
        resource.write_text("frozen\n", encoding="utf-8")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def submission(self, status: str = "Accepted") -> bytes:
        return json.dumps(
            {
                "id": SUBMISSION_ID,
                "message": "Successfully uploaded file",
                "status": status,
            }
        ).encode()

    def receipt(self) -> dict[str, object]:
        return release_carrier.build_submission_receipt(
            self.submission(), self.carrier, self.installer
        )

    def notary_log(
        self,
        receipt: dict[str, object],
        *,
        status: str = "Accepted",
        status_code: int = 0,
        issues: object = None,
    ) -> bytes:
        return json.dumps(
            {
                "logFormatVersion": 1,
                "jobId": receipt["submission_id"],
                "status": status,
                "statusSummary": "Ready for distribution",
                "statusCode": status_code,
                "archiveFilename": receipt["carrier_file_name"],
                "uploadDate": "2026-07-25T00:00:00.000Z",
                "sha256": str(receipt["submitted_sha256"]).upper(),
                "ticketContents": [],
                "issues": issues,
            }
        ).encode()

    def test_binds_accepted_submission_log_and_final_carrier(self) -> None:
        receipt = self.receipt()
        self.assertEqual(receipt["notary_status"], "accepted")
        release_carrier.validate_notary_log(self.notary_log(receipt), receipt)
        release_carrier.verify_submission_artifacts(
            receipt, self.carrier, self.installer
        )

        self.carrier.write_bytes(b"signed-dmg-with-stapled-ticket")
        qualification = release_carrier.build_qualification(
            receipt, self.carrier, self.installer
        )
        release_carrier.verify_qualification(
            qualification, receipt, self.carrier, self.installer
        )
        self.assertNotEqual(
            qualification["submitted_sha256"],
            qualification["distribution_sha256"],
        )

    def test_rejects_unknown_or_nonterminal_submission(self) -> None:
        with self.assertRaises(release_carrier.ReleaseCarrierError):
            release_carrier.build_submission_receipt(
                self.submission("In Progress"), self.carrier, self.installer
            )
        value = json.loads(self.submission())
        value["unknown"] = True
        with self.assertRaises(release_carrier.ReleaseCarrierError):
            release_carrier.build_submission_receipt(
                json.dumps(value).encode(), self.carrier, self.installer
            )
        value = json.loads(self.submission())
        value["id"] = SUBMISSION_ID.upper()
        with self.assertRaises(release_carrier.ReleaseCarrierError):
            release_carrier.build_submission_receipt(
                json.dumps(value).encode(), self.carrier, self.installer
            )

    def test_parses_one_canonical_dmg_team(self) -> None:
        self.assertEqual(
            release_carrier.parse_codesign_team(
                b"Executable=carrier\nTeamIdentifier=ABCDEFGHIJ\n"
            ),
            "ABCDEFGHIJ",
        )
        for output in (
            b"TeamIdentifier=not set\n",
            b"TeamIdentifier=ABCDEFGHIJ\nTeamIdentifier=ABCDEFGHIJ\n",
            b"TeamIdentifier=ABCDEFGHIJ\r\n",
        ):
            with self.assertRaises(release_carrier.ReleaseCarrierError):
                release_carrier.parse_codesign_team(output)

        identity = self.root / "ReleaseIdentity.json"
        team = release_carrier.load_metadata().developer_team_id
        identity.write_text(
            json.dumps(
                {
                    "format_version": 1,
                    "team_identifier": team,
                    "installer_designated_requirement": "installer",
                    "manager_designated_requirement": "manager",
                    "input_method_designated_requirement": "input",
                }
            ),
            encoding="utf-8",
        )
        self.assertEqual(release_carrier.release_identity_team(identity), team)
        value = json.loads(identity.read_text(encoding="utf-8"))
        value["team_identifier"] = "ABCDEFGHIJ"
        identity.write_text(json.dumps(value), encoding="utf-8")
        with self.assertRaises(release_carrier.ReleaseCarrierError):
            release_carrier.release_identity_team(identity)

    def test_rejects_log_identity_hash_status_and_issues_drift(self) -> None:
        receipt = self.receipt()
        for changed in (
            {**receipt, "submission_id": "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"},
            {**receipt, "submitted_sha256": "0" * 64},
        ):
            with self.assertRaises(release_carrier.ReleaseCarrierError):
                release_carrier.validate_notary_log(self.notary_log(changed), receipt)
        with self.assertRaises(release_carrier.ReleaseCarrierError):
            release_carrier.validate_notary_log(
                self.notary_log(receipt, status="Invalid", status_code=4000),
                receipt,
            )
        with self.assertRaises(release_carrier.ReleaseCarrierError):
            release_carrier.validate_notary_log(
                self.notary_log(receipt, issues=[{"severity": "error"}]),
                receipt,
            )

    def test_rejects_installer_and_distribution_drift(self) -> None:
        receipt = self.receipt()
        qualification = release_carrier.build_qualification(
            receipt, self.carrier, self.installer
        )
        self.carrier.write_bytes(b"another-valid-looking-carrier")
        with self.assertRaises(release_carrier.ReleaseCarrierError):
            release_carrier.verify_submission_artifacts(
                receipt, self.carrier, self.installer
            )
        self.carrier.write_bytes(b"signed-dmg")
        (self.installer / "Contents/Resources/value.txt").write_text(
            "changed\n", encoding="utf-8"
        )
        with self.assertRaises(release_carrier.ReleaseCarrierError):
            release_carrier.verify_qualification(
                qualification, receipt, self.carrier, self.installer
            )


if __name__ == "__main__":
    unittest.main()

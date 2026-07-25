#!/usr/bin/env python3
"""Contract tests for Developer ID release identity parsing and encoding."""

from __future__ import annotations

import unittest

import release_identity


def details(bundle_id: str, team: str) -> bytes:
    requirement = (
        f'identifier "{bundle_id}" and anchor apple generic and '
        "certificate 1[field.1.2.840.113635.100.6.2.6] /* exists */ and "
        "certificate leaf[field.1.2.840.113635.100.6.1.13] /* exists */ and "
        f'certificate leaf[subject.OU] = "{team}"'
    )
    return (
        f"Identifier={bundle_id}\n"
        f"TeamIdentifier={team}\n"
        "Signature size=9000\n"
        f"# designated => {requirement}\n"
    ).encode()


class ReleaseIdentityTests(unittest.TestCase):
    def test_builds_deterministic_identity_for_one_team(self) -> None:
        team = "ABCDEFGHIJ"
        identities = [
            release_identity.parse_codesign_output(
                details(bundle_id, team), bundle_id
            )
            for bundle_id in release_identity.EXPECTED_BUNDLE_IDS.values()
        ]
        identity = release_identity.build_identity(*identities)
        encoded = release_identity.encoded_identity(identity)
        self.assertTrue(encoded.endswith(b"\n"))
        self.assertIn(b'"installer_designated_requirement"', encoded)
        self.assertEqual(encoded, release_identity.encoded_identity(identity))

    def test_rejects_ad_hoc_mixed_team_and_identifier_drift(self) -> None:
        installer_id = release_identity.EXPECTED_BUNDLE_IDS["installer"]
        with self.assertRaises(release_identity.ReleaseIdentityError):
            release_identity.parse_codesign_output(
                details(installer_id, "not set"), installer_id
            )
        with self.assertRaises(release_identity.ReleaseIdentityError):
            release_identity.parse_codesign_output(
                details("org.example.other", "ABCDEFGHIJ"), installer_id
            )

        identities = [
            release_identity.parse_codesign_output(
                details(bundle_id, "ABCDEFGHIJ"), bundle_id
            )
            for bundle_id in release_identity.EXPECTED_BUNDLE_IDS.values()
        ]
        changed_manager = release_identity.parse_codesign_output(
            details(
                release_identity.EXPECTED_BUNDLE_IDS["manager"], "KLMNOPQRST"
            ),
            release_identity.EXPECTED_BUNDLE_IDS["manager"],
        )
        with self.assertRaises(release_identity.ReleaseIdentityError):
            release_identity.build_identity(
                identities[0], changed_manager, identities[2]
            )

    def test_upgrade_source_requires_exact_component_identities(self) -> None:
        manager_id = release_identity.EXPECTED_BUNDLE_IDS["manager"]
        input_method_id = release_identity.EXPECTED_BUNDLE_IDS["input_method"]
        manager = release_identity.parse_codesign_output(
            details(manager_id, "ABCDEFGHIJ"), manager_id
        )
        input_method = release_identity.parse_codesign_output(
            details(input_method_id, "ABCDEFGHIJ"), input_method_id
        )
        release_identity.verify_upgrade_source_identity(
            manager,
            input_method,
            manager,
            input_method,
        )
        drifted_manager = release_identity.parse_codesign_output(
            details(manager_id, "KLMNOPQRST"), manager_id
        )
        with self.assertRaises(release_identity.ReleaseIdentityError):
            release_identity.verify_upgrade_source_identity(
                manager,
                input_method,
                drifted_manager,
                input_method,
            )


if __name__ == "__main__":
    unittest.main()

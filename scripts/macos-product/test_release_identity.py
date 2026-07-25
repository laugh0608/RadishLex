#!/usr/bin/env python3
"""Contract tests for community ad-hoc release identity parsing and encoding."""

from __future__ import annotations

import unittest

import release_identity


def details(bundle_id: str, cdhash: str, *, ad_hoc: bool = True) -> bytes:
    signature = "adhoc" if ad_hoc else "size=9000"
    flags = "0x2(adhoc)" if ad_hoc else "0x10000(runtime)"
    return (
        f"Identifier={bundle_id}\n"
        f"CodeDirectory v=20400 size=663 flags={flags} hashes=10+7 location=embedded\n"
        f"CDHash={cdhash}\n"
        f"Signature={signature}\n"
        "TeamIdentifier=not set\n"
        f'# designated => cdhash H"{cdhash}"\n'
    ).encode()


class ReleaseIdentityTests(unittest.TestCase):
    def identity(self, component: str, digit: str) -> release_identity.SignedBundleIdentity:
        bundle_id = release_identity.EXPECTED_BUNDLE_IDS[component]
        return release_identity.parse_codesign_output(
            details(bundle_id, digit * 40), bundle_id
        )

    def test_builds_deterministic_sorted_identity_sets(self) -> None:
        installer = self.identity("installer", "0")
        manager = self.identity("manager", "2")
        older_manager = self.identity("manager", "1")
        input_method = self.identity("input_method", "3")
        identity = release_identity.build_identity(
            installer, [manager, older_manager, manager], [input_method]
        )
        encoded = release_identity.encoded_identity(identity)
        self.assertTrue(encoded.endswith(b"\n"))
        self.assertIn(b'"distribution_identity": "community-adhoc-v1"', encoded)
        self.assertNotIn(b"installer_designated_requirement", encoded)
        self.assertEqual(
            identity["manager_designated_requirements"],
            [
                older_manager.designated_requirement,
                manager.designated_requirement,
            ],
        )

    def test_rejects_non_ad_hoc_identifier_drift_and_missing_primary_hash(self) -> None:
        installer_id = release_identity.EXPECTED_BUNDLE_IDS["installer"]
        with self.assertRaises(release_identity.ReleaseIdentityError):
            release_identity.parse_codesign_output(
                details(installer_id, "0" * 40, ad_hoc=False), installer_id
            )
        with self.assertRaises(release_identity.ReleaseIdentityError):
            release_identity.parse_codesign_output(
                details("org.example.other", "0" * 40), installer_id
            )
        value = details(installer_id, "0" * 40).replace(
            b'cdhash H"0000000000000000000000000000000000000000"',
            b'cdhash H"1111111111111111111111111111111111111111"',
        )
        with self.assertRaises(release_identity.ReleaseIdentityError):
            release_identity.parse_codesign_output(value, installer_id)

    def test_rejects_requirement_overlap_between_components(self) -> None:
        installer = self.identity("installer", "0")
        manager = self.identity("manager", "1")
        input_method = release_identity.SignedBundleIdentity(
            release_identity.EXPECTED_BUNDLE_IDS["input_method"],
            manager.designated_requirement,
        )
        with self.assertRaises(release_identity.ReleaseIdentityError):
            release_identity.build_identity(installer, [manager], [input_method])


if __name__ == "__main__":
    unittest.main()

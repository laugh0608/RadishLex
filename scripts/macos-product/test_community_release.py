#!/usr/bin/env python3
"""Contract tests for the unnotarized community release evidence."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import community_release
import product_manifest


class CommunityReleaseTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        metadata = product_manifest.ProductMetadata.load()
        self.carrier = (
            self.root
            / f"RadishLex-{metadata.product_version}-{metadata.build_number}.dmg"
        )
        self.carrier.write_bytes(b"synthetic-community-dmg")
        self.identity = self.root / "ReleaseIdentity.json"
        self.identity.write_text("synthetic sealed identity\n", encoding="utf-8")
        self.evidence = self.root / community_release.EVIDENCE_NAME

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_evidence_binds_version_carrier_and_identity(self) -> None:
        value = community_release.expected(self.carrier, self.identity)
        self.assertEqual(value["distribution_identity"], "community-adhoc-v1")
        self.assertFalse(value["apple_notarized"])
        self.evidence.write_bytes(community_release.encoded(value))
        community_release.regular_file(self.evidence, "community evidence")
        self.assertEqual(
            self.evidence.read_bytes(),
            community_release.encoded(
                community_release.expected(self.carrier, self.identity)
            ),
        )

    def test_rejects_carrier_or_identity_drift(self) -> None:
        value = community_release.expected(self.carrier, self.identity)
        self.carrier.write_bytes(b"changed")
        self.assertNotEqual(
            value["carrier_sha256"],
            community_release.expected(self.carrier, self.identity)["carrier_sha256"],
        )
        self.carrier.write_bytes(b"synthetic-community-dmg")
        self.identity.write_text("changed identity\n", encoding="utf-8")
        self.assertNotEqual(
            value["release_identity_sha256"],
            community_release.expected(self.carrier, self.identity)[
                "release_identity_sha256"
            ],
        )


if __name__ == "__main__":
    unittest.main()

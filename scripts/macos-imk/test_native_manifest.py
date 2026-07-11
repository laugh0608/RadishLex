#!/usr/bin/env python3
from __future__ import annotations

import argparse
import tempfile
import unittest
from pathlib import Path

import native_manifest


class NativeManifestTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="radishlex-rime-manifest-")
        self.root = Path(self.temporary.name)
        self.data_dir = self.root / "RimeData"
        self.data_dir.mkdir()
        (self.data_dir / "default.yaml").write_text("schema_list: []\n", encoding="utf-8")
        (self.data_dir / "contract.schema.yaml").write_text(
            "schema:\n  schema_id: contract\n", encoding="utf-8"
        )
        (self.data_dir / "dictionary.txt").write_text("synthetic\n", encoding="utf-8")
        self.license = self.root / "LICENSE"
        self.license.write_text("synthetic fixture license\n", encoding="utf-8")
        self.manifest = self.root / "manifest.plist"

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def arguments(self) -> argparse.Namespace:
        return argparse.Namespace(
            data_dir=self.data_dir,
            license=self.license,
            schema="contract",
            deploy_on_start=True,
            output=self.manifest,
            manifest=self.manifest,
        )

    def test_create_and_verify_cover_every_data_file(self) -> None:
        args = self.arguments()
        self.assertEqual(native_manifest.create_manifest(args), 0)
        self.assertEqual(native_manifest.verify_manifest(args), 0)

        (self.data_dir / "unexpected.txt").write_text("tampered\n", encoding="utf-8")
        with self.assertRaisesRegex(SystemExit, "does not match"):
            native_manifest.verify_manifest(args)

    def test_symlink_is_rejected(self) -> None:
        (self.data_dir / "linked-license").symlink_to(self.license)
        with self.assertRaisesRegex(SystemExit, "must not contain symlinks"):
            native_manifest.create_manifest(self.arguments())

    def test_missing_required_schema_is_rejected(self) -> None:
        (self.data_dir / "contract.schema.yaml").unlink()
        with self.assertRaisesRegex(SystemExit, "missing required file"):
            native_manifest.create_manifest(self.arguments())


if __name__ == "__main__":
    unittest.main()

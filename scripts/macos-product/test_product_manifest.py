#!/usr/bin/env python3
from __future__ import annotations

import json
import plistlib
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

import product_manifest


class ProductManifestTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.metadata = product_manifest.ProductMetadata.load()
        self.manager = self.make_bundle(
            "radishlex_manager.app", self.metadata.manager_bundle_id
        )
        self.input_method = self.make_bundle(
            "RadishLexInputMethod.app", self.metadata.input_method_bundle_id
        )
        resources = self.input_method / "Contents/Resources"
        self.write_plist(
            resources / "RimeData.manifest.plist",
            {
                "format_version": self.metadata.rime_data_manifest_version,
                "schema_id": "radishlex_pinyin",
            },
        )
        self.write_plist(
            resources / "NativeLibraries.manifest.plist",
            {"format_version": self.metadata.native_libraries_manifest_version},
        )
        self.license = self.root / "LICENSE"
        self.license.write_text("synthetic product license\n", encoding="utf-8")
        self.manifest = self.root / "ProductManifest.json"

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write_plist(self, path: Path, value: dict[str, object]) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("wb") as stream:
            plistlib.dump(value, stream, sort_keys=True)

    def make_bundle(self, name: str, bundle_id: str) -> Path:
        bundle = self.root / name
        contents = bundle / "Contents"
        self.write_plist(
            contents / "Info.plist",
            {
                "CFBundleIdentifier": bundle_id,
                "CFBundleShortVersionString": self.metadata.product_version,
                "CFBundleVersion": self.metadata.build_number,
                "LSMinimumSystemVersion": self.metadata.minimum_macos,
            },
        )
        executable = contents / "MacOS/component"
        executable.parent.mkdir(parents=True, exist_ok=True)
        executable.write_bytes(b"synthetic executable")
        return bundle

    def create_manifest(self) -> dict[str, object]:
        value = product_manifest.expected_manifest(
            self.metadata, self.manager, self.input_method, self.license
        )
        product_manifest.write_manifest(self.manifest, value)
        return value

    def test_repository_source_contract_matches_product_metadata(self) -> None:
        product_manifest.validate_source_contract(self.metadata)

    def test_manifest_is_deterministic_and_contains_no_absolute_paths(self) -> None:
        first = self.create_manifest()
        first_bytes = self.manifest.read_bytes()
        product_manifest.write_manifest(
            self.manifest,
            product_manifest.expected_manifest(
                self.metadata, self.manager, self.input_method, self.license
            ),
        )
        self.assertEqual(self.manifest.read_bytes(), first_bytes)
        self.assertNotIn(str(self.root), self.manifest.read_text(encoding="utf-8"))
        self.assertEqual(first["product_version"], self.metadata.product_version)

    def test_verification_rejects_artifact_mutation(self) -> None:
        self.create_manifest()
        (self.manager / "Contents/MacOS/component").write_bytes(b"mutated")
        with self.assertRaisesRegex(
            product_manifest.ProductManifestError, "does not match"
        ):
            product_manifest.verify_manifest(
                self.metadata,
                self.manager,
                self.input_method,
                self.license,
                self.manifest,
            )

    def test_manifest_rejects_symlinked_bundle_content(self) -> None:
        target = self.root / "outside.txt"
        target.write_text("outside\n", encoding="utf-8")
        (self.manager / "Contents/Resources").mkdir(parents=True)
        (self.manager / "Contents/Resources/link").symlink_to(target)
        with self.assertRaisesRegex(
            product_manifest.ProductManifestError, "absolute|escaping or broken symlink"
        ):
            product_manifest.expected_manifest(
                self.metadata, self.manager, self.input_method, self.license
            )

    def test_manifest_records_internal_framework_symlink(self) -> None:
        framework = self.manager / "Contents/Frameworks/Synthetic.framework"
        version = framework / "Versions/A"
        version.mkdir(parents=True)
        (version / "Synthetic").write_bytes(b"framework binary")
        (framework / "Versions/Current").symlink_to("A", target_is_directory=True)
        (framework / "Synthetic").symlink_to("Versions/Current/Synthetic")
        value = product_manifest.expected_manifest(
            self.metadata, self.manager, self.input_method, self.license
        )
        manager = next(
            item for item in value["components"] if item["component"] == "manager"
        )
        links = [item for item in manager["files"] if item["type"] == "symlink"]
        self.assertEqual(len(links), 2)

    def test_manifest_rejects_bundle_version_mismatch(self) -> None:
        info_path = self.manager / "Contents/Info.plist"
        info = product_manifest.load_plist(info_path, "Manager Info.plist")
        info["CFBundleVersion"] = "999"
        self.write_plist(info_path, info)
        with self.assertRaisesRegex(
            product_manifest.ProductManifestError, "CFBundleVersion"
        ):
            product_manifest.expected_manifest(
                self.metadata, self.manager, self.input_method, self.license
            )

    def test_metadata_rejects_unknown_fields(self) -> None:
        value = json.loads(product_manifest.METADATA_PATH.read_text(encoding="utf-8"))
        value["unexpected"] = True
        path = self.root / "product.json"
        path.write_text(json.dumps(value), encoding="utf-8")
        with self.assertRaisesRegex(
            product_manifest.ProductManifestError, "fields do not match"
        ):
            product_manifest.ProductMetadata.load(path)

    def test_metadata_rejects_unknown_distribution_identity(self) -> None:
        value = json.loads(product_manifest.METADATA_PATH.read_text(encoding="utf-8"))
        for identity in ("", "developer-id-v1", "community-adhoc-v2"):
            value["distribution_identity"] = identity
            path = self.root / f"product-{len(identity)}.json"
            path.write_text(json.dumps(value), encoding="utf-8")
            with self.assertRaisesRegex(
                product_manifest.ProductManifestError, "distribution_identity"
            ):
                product_manifest.ProductMetadata.load(path)

    def test_source_contract_rejects_distribution_identity_drift(self) -> None:
        with self.assertRaisesRegex(
            product_manifest.ProductManifestError, "Rust distribution identity"
        ):
            product_manifest.validate_source_contract(
                replace(self.metadata, distribution_identity="community-adhoc-v2")
            )


if __name__ == "__main__":
    unittest.main()

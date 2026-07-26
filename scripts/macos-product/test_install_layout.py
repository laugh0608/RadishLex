#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import plistlib
import shutil
import tempfile
import unittest
from pathlib import Path

import install_layout
import product_manifest


class InstallLayoutTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.layout = install_layout.InstallLayout.load()
        self.metadata = product_manifest.ProductMetadata.load()
        self.product = self.root / "product"
        self.components = self.product / "Components"
        self.manager = self.make_bundle(
            self.layout.manager_component_path, self.metadata.manager_bundle_id
        )
        self.input_method = self.make_bundle(
            self.layout.input_method_component_path,
            self.metadata.input_method_bundle_id,
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
        self.license = self.product / "LICENSE"
        self.license.write_text("synthetic product license\n", encoding="utf-8")
        self.manifest = self.product / "ProductManifest.json"
        product_manifest.write_manifest(
            self.manifest,
            product_manifest.expected_manifest(
                self.metadata,
                self.manager,
                self.input_method,
                self.license,
            ),
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write_plist(self, path: Path, value: dict[str, object]) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("wb") as stream:
            plistlib.dump(value, stream, sort_keys=True)

    def make_bundle(self, component_path: str, bundle_id: str) -> Path:
        bundle = self.product / component_path
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

    def write_layout(self, value: dict[str, object]) -> Path:
        path = self.root / "install-layout.json"
        path.write_text(
            json.dumps(value, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        return path

    def write_metadata(self, **overrides: object) -> Path:
        value = json.loads(
            product_manifest.METADATA_PATH.read_text(encoding="utf-8")
        )
        value.update(overrides)
        path = self.root / "product-metadata.json"
        path.write_text(
            json.dumps(value, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        return path

    def test_repository_layout_matches_the_accepted_user_domain_decision(self) -> None:
        self.assertEqual(self.layout.distribution_container, "dmg")
        self.assertEqual(self.layout.installation_scope, "current-user")
        self.assertEqual(
            self.layout.manager_target_path,
            "Applications/RadishLex Manager.app",
        )
        self.assertEqual(
            self.layout.input_method_target_path,
            "Library/Input Methods/RadishLexInputMethod.app",
        )

    def test_layout_rejects_unknown_fields_and_unsafe_paths(self) -> None:
        value = json.loads(install_layout.LAYOUT_PATH.read_text(encoding="utf-8"))
        value["unexpected"] = True
        with self.assertRaisesRegex(
            install_layout.InstallLayoutError, "fields do not match"
        ):
            install_layout.InstallLayout.load(self.write_layout(value))

        value.pop("unexpected")
        value["manager_target_path"] = "/Applications/RadishLex Manager.app"
        with self.assertRaisesRegex(
            install_layout.InstallLayoutError, "accepted M4-P03 layout"
        ):
            install_layout.InstallLayout.load(self.write_layout(value))

    def test_payload_assembly_is_deterministic_and_verifiable(self) -> None:
        first = self.root / "payload-first"
        second = self.root / "payload-second"
        install_layout.assemble_payload(self.product, first)
        install_layout.assemble_payload(self.product, second)
        install_layout.verify_payload(first)
        install_layout.verify_payload(second)
        self.assertEqual(
            (first / install_layout.PAYLOAD_MANIFEST_NAME).read_bytes(),
            (second / install_layout.PAYLOAD_MANIFEST_NAME).read_bytes(),
        )
        manifest_text = (first / install_layout.PAYLOAD_MANIFEST_NAME).read_text(
            encoding="utf-8"
        )
        self.assertNotIn(str(self.root), manifest_text)
        manifest = json.loads(manifest_text)
        self.assertEqual(manifest["format_version"], 2)
        self.assertEqual(manifest["upgrade_sources"], [])
        self.assertTrue((first / install_layout.UPGRADE_SOURCES_DIRECTORY).is_dir())

    def test_payload_binds_historical_product_assemblies_by_exact_release(self) -> None:
        source = self.make_historical_product("26.6.1", "34")
        payload = self.root / "payload-with-source"
        install_layout.assemble_payload(
            self.product,
            payload,
            upgrade_source_product_roots=[source],
        )
        install_layout.verify_payload(payload)
        manifest = json.loads(
            (payload / install_layout.PAYLOAD_MANIFEST_NAME).read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(
            manifest["upgrade_sources"],
            [
                {
                    "build_number": "34",
                    "product_manifest": install_layout.regular_file_record(
                        payload
                        / install_layout.UPGRADE_SOURCES_DIRECTORY
                        / "26.6.1-34"
                        / "ProductManifest.json",
                        "UpgradeSources/26.6.1-34/ProductManifest.json",
                    ),
                    "product_path": "UpgradeSources/26.6.1-34",
                    "product_version": "26.6.1",
                }
            ],
        )
        source_program = (
            payload
            / install_layout.UPGRADE_SOURCES_DIRECTORY
            / "26.6.1-34"
            / self.layout.manager_component_path
            / "Contents/MacOS/component"
        )
        source_program.write_bytes(b"mutated historical executable")
        with self.assertRaisesRegex(
            install_layout.InstallLayoutError,
            "component does not match its manifest",
        ):
            install_layout.verify_payload(payload)

    def test_payload_rejects_duplicate_or_nonhistorical_source_builds(self) -> None:
        source = self.make_historical_product("26.6.1", "34")
        with self.assertRaisesRegex(
            install_layout.InstallLayoutError,
            "must be distinct",
        ):
            install_layout.assemble_payload(
                self.product,
                self.root / "duplicate-source-payload",
                upgrade_source_product_roots=[source, source],
            )
        target_release = self.make_historical_product(
            self.metadata.product_version, self.metadata.build_number
        )
        with self.assertRaisesRegex(
            install_layout.InstallLayoutError,
            "must be older",
        ):
            install_layout.assemble_payload(
                self.product,
                self.root / "nonhistorical-source-payload",
                upgrade_source_product_roots=[target_release],
            )

    def test_multiple_historical_sources_are_sorted_and_keep_distinct_paths(self) -> None:
        older = self.make_historical_product("26.5.1", "33")
        newer = self.make_historical_product("26.6.1", "34")
        payload = self.root / "multiple-source-payload"
        install_layout.assemble_payload(
            self.product,
            payload,
            upgrade_source_product_roots=[newer, older],
        )
        manifest = json.loads(
            (payload / install_layout.PAYLOAD_MANIFEST_NAME).read_text(
                encoding="utf-8"
            )
        )
        self.assertEqual(
            [
                (source["build_number"], source["product_path"])
                for source in manifest["upgrade_sources"]
            ],
            [
                ("33", "UpgradeSources/26.5.1-33"),
                ("34", "UpgradeSources/26.6.1-34"),
            ],
        )
        install_layout.verify_payload(payload)

    def test_payload_verification_rejects_product_mutation(self) -> None:
        payload = self.root / "payload"
        install_layout.assemble_payload(self.product, payload)
        (
            payload
            / "Product"
            / self.layout.manager_component_path
            / "Contents/MacOS/component"
        ).write_bytes(b"mutated")
        with self.assertRaisesRegex(
            install_layout.InstallLayoutError, "product manifest verification failed"
        ):
            install_layout.verify_payload(payload)

    def test_payload_can_bind_an_explicit_source_release_metadata(self) -> None:
        metadata_path = self.write_metadata(
            product_version="26.6.1",
            build_number="34",
        )
        source_metadata = product_manifest.ProductMetadata.load(metadata_path)
        for bundle in (self.manager, self.input_method):
            info_path = bundle / "Contents/Info.plist"
            with info_path.open("rb") as stream:
                info = plistlib.load(stream)
            info["CFBundleShortVersionString"] = source_metadata.product_version
            info["CFBundleVersion"] = source_metadata.build_number
            self.write_plist(info_path, info)
        product_manifest.write_manifest(
            self.manifest,
            product_manifest.expected_manifest(
                source_metadata,
                self.manager,
                self.input_method,
                self.license,
            ),
        )

        payload = self.root / "source-payload"
        install_layout.assemble_payload(self.product, payload, source_metadata)
        install_layout.verify_payload(payload, source_metadata)
        with self.assertRaisesRegex(
            install_layout.InstallLayoutError,
            "product manifest verification failed",
        ):
            install_layout.verify_payload(payload)

    def test_product_root_rejects_unbound_entries(self) -> None:
        (self.product / "unbound.txt").write_text("unbound\n", encoding="utf-8")
        with self.assertRaisesRegex(
            install_layout.InstallLayoutError, "entries do not match"
        ):
            install_layout.assemble_payload(
                self.product, self.root / "rejected-payload"
            )

    def test_product_root_rejects_symlinked_component_bundle(self) -> None:
        manager_target = self.root / "manager-target.app"
        self.manager.rename(manager_target)
        os.symlink(manager_target, self.manager)
        with self.assertRaisesRegex(
            install_layout.InstallLayoutError, "non-symlink directory"
        ):
            install_layout.assemble_payload(
                self.product, self.root / "rejected-payload"
            )

    def test_payload_rejects_layout_substitution_and_existing_output(self) -> None:
        payload = self.root / "payload"
        install_layout.assemble_payload(self.product, payload)
        with self.assertRaisesRegex(
            install_layout.InstallLayoutError, "already exists"
        ):
            install_layout.assemble_payload(self.product, payload)
        layout_path = payload / "InstallLayout.json"
        layout_path.write_text("{}\n", encoding="utf-8")
        with self.assertRaisesRegex(
            install_layout.InstallLayoutError, "differs from committed layout"
        ):
            install_layout.verify_payload(payload)

    def make_historical_product(self, version: str, build: str) -> Path:
        source = self.root / f"historical-{version}-{build}"
        shutil.copytree(self.product, source, symlinks=True)
        metadata_path = self.write_metadata(
            product_version=version,
            build_number=build,
        )
        metadata = product_manifest.ProductMetadata.load(metadata_path)
        manager = source / self.layout.manager_component_path
        input_method = source / self.layout.input_method_component_path
        for bundle in (manager, input_method):
            info_path = bundle / "Contents/Info.plist"
            with info_path.open("rb") as stream:
                info = plistlib.load(stream)
            info["CFBundleShortVersionString"] = version
            info["CFBundleVersion"] = build
            self.write_plist(info_path, info)
        product_manifest.write_manifest(
            source / "ProductManifest.json",
            product_manifest.expected_manifest(
                metadata,
                manager,
                input_method,
                source / "LICENSE",
            ),
        )
        return source


if __name__ == "__main__":
    unittest.main()

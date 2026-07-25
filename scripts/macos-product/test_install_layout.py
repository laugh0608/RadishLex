#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import plistlib
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


if __name__ == "__main__":
    unittest.main()

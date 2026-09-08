#!/usr/bin/env python3
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import product_metadata


class LinuxProductMetadataTest(unittest.TestCase):
    def test_repository_source_contract_is_valid(self) -> None:
        metadata, layout = product_metadata.validate_source_contract()

        self.assertEqual(metadata.product_version, "26.7.1")
        self.assertEqual(metadata.build_number, "39")
        self.assertEqual(metadata.package_version, "26.7.1+39-2")
        self.assertEqual(layout["layout_id"], "debian-system-v1")
        self.assertEqual(layout["installation_scope"], "system")

    def test_control_rendering_is_deterministic_and_strict(self) -> None:
        metadata = product_metadata.LinuxProductMetadata.load()

        first = product_metadata.render_control(metadata)
        second = product_metadata.render_control(metadata)

        self.assertEqual(first, second)
        self.assertIn("Version: 26.7.1+39-2\n", first)
        self.assertIn("Architecture: arm64\n", first)
        self.assertIn("fonts-dejavu-core, fonts-noto-cjk\n", first)
        self.assertNotIn("Recommends:", first)
        self.assertNotRegex(first, r"@[A-Z_]+@")

    def test_shlibdeps_control_is_a_fixed_ephemeral_source_view(self) -> None:
        metadata = product_metadata.LinuxProductMetadata.load()
        rendered = product_metadata.render_shlibdeps_control(metadata)

        self.assertIn("Source: radishlex\n", rendered)
        self.assertIn("Package: radishlex\n", rendered)
        self.assertIn("Architecture: arm64\n", rendered)
        self.assertIn("Rules-Requires-Root: no\n", rendered)
        self.assertIn("used only by dpkg-shlibdeps", rendered)
        self.assertNotIn("Depends:", rendered)

    def test_metadata_rejects_unknown_field(self) -> None:
        value = json.loads(product_metadata.METADATA_PATH.read_text(encoding="utf-8"))
        value["unexpected"] = True
        with self.temporary_file("product.json", value) as path:
            with self.assertRaisesRegex(
                product_metadata.LinuxProductMetadataError,
                "fields do not match",
            ):
                product_metadata.LinuxProductMetadata.load(path)

    def test_metadata_rejects_derived_version_drift(self) -> None:
        value = json.loads(product_metadata.METADATA_PATH.read_text(encoding="utf-8"))
        value["package_version"] = "26.7.1+38-1"
        with self.temporary_file("product.json", value) as path:
            with self.assertRaisesRegex(
                product_metadata.LinuxProductMetadataError,
                "package_version differs",
            ):
                product_metadata.LinuxProductMetadata.load(path)

    def test_metadata_rejects_softened_font_dependency(self) -> None:
        value = json.loads(product_metadata.METADATA_PATH.read_text(encoding="utf-8"))
        value["hard_dependencies"].remove("fonts-noto-cjk")
        with self.temporary_file("product.json", value) as path:
            with self.assertRaisesRegex(
                product_metadata.LinuxProductMetadataError,
                "hard_dependencies differ",
            ):
                product_metadata.LinuxProductMetadata.load(path)

    def test_metadata_rejects_wrong_multiarch(self) -> None:
        value = json.loads(product_metadata.METADATA_PATH.read_text(encoding="utf-8"))
        value["multiarch_tuple"] = "x86_64-linux-gnu"
        with self.temporary_file("product.json", value) as path:
            with self.assertRaisesRegex(
                product_metadata.LinuxProductMetadataError,
                "multiarch_tuple must remain",
            ):
                product_metadata.LinuxProductMetadata.load(path)

    def test_layout_rejects_path_drift(self) -> None:
        metadata = product_metadata.LinuxProductMetadata.load()
        value = json.loads(product_metadata.LAYOUT_PATH.read_text(encoding="utf-8"))
        value["paths"]["rime_data_root"] = "/opt/radishlex/rime"
        with self.temporary_file("install-layout.json", value) as path:
            with self.assertRaisesRegex(
                product_metadata.LinuxProductMetadataError,
                "differs from debian-system-v1",
            ):
                product_metadata.load_layout(metadata, path)

    def test_desktop_entry_rejects_shell_indirection(self) -> None:
        metadata = product_metadata.LinuxProductMetadata.load()
        layout = product_metadata.load_layout(metadata)
        value = product_metadata.expected_desktop_entry(metadata, layout).replace(
            "Exec=/usr/", "Exec=sh -c /usr/"
        )
        with self.temporary_text_file("manager.desktop", value) as path:
            with self.assertRaises(
                product_metadata.LinuxProductMetadataError
            ):
                product_metadata.validate_desktop_entry(metadata, layout, path)

    def test_icon_rejects_external_resource(self) -> None:
        value = (
            '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">'
            '<path fill="#000" d="M0 0" href="https://example.invalid/a"/>'
            '<path fill="#000" d="M0 0"/>'
            '<path fill="#000" d="M0 0"/>'
            "</svg>\n"
        )
        with self.temporary_text_file("icon.svg", value) as path:
            with self.assertRaises(
                product_metadata.LinuxProductMetadataError
            ):
                product_metadata.validate_icon(path)

    def temporary_file(self, name: str, value: object):
        return self.temporary_text_file(
            name, json.dumps(value, ensure_ascii=False, indent=2) + "\n"
        )

    def temporary_text_file(self, name: str, value: str):
        class TemporaryTextFile:
            def __init__(self, filename: str, text: str) -> None:
                self.directory = tempfile.TemporaryDirectory()
                self.path = Path(self.directory.name) / filename
                self.path.write_text(text, encoding="utf-8")

            def __enter__(self) -> Path:
                return self.path

            def __exit__(self, *_: object) -> None:
                self.directory.cleanup()

        return TemporaryTextFile(name, value)


if __name__ == "__main__":
    unittest.main()

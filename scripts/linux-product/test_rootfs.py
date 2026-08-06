#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import stat
import tempfile
import unittest
from pathlib import Path

import product_metadata
import rootfs


class LinuxRootfsTest(unittest.TestCase):
    def setUp(self) -> None:
        temporary_root = Path(tempfile.gettempdir()).resolve()
        self.temporary = tempfile.TemporaryDirectory(
            prefix="radishlex-linux-rootfs-test.", dir=temporary_root
        )
        self.work = Path(self.temporary.name)
        self.manager = self.work / "manager-bundle"
        self.addon = self.work / "addon-stage"
        self.metadata, self.layout = product_metadata.validate_source_contract()
        self.ffi_bytes = b"synthetic-radishlex-ffi-v9\n"
        self.create_manager_bundle()
        self.create_addon_stage()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def write_file(
        self, root: Path, relative: Path | str, value: bytes, mode: int = 0o644
    ) -> Path:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(value)
        path.chmod(mode)
        return path

    def create_manager_bundle(self) -> None:
        values = {
            Path("radishlex_manager"): b"synthetic-manager-executable\n",
            Path("lib/libapp.so"): b"synthetic-flutter-app\n",
            Path("lib/libflutter_linux_gtk.so"): b"synthetic-flutter-engine\n",
            Path("lib/libradishlex_ime_ffi.so"): self.ffi_bytes,
            Path("data/icudtl.dat"): b"synthetic-icu-data\n",
            Path("data/flutter_assets/AssetManifest.bin"): b"synthetic-assets\n",
            rootfs.FONT_MANIFEST_RELATIVE_PATH: (
                b'[{"family":"MaterialIcons","fonts":'
                b'[{"asset":"fonts/MaterialIcons-Regular.otf"}]}]\n'
            ),
            rootfs.NOTICES_RELATIVE_PATH: b"synthetic-generated-notices\n",
            rootfs.MATERIAL_FONT_RELATIVE_PATH: b"synthetic-material-icons\n",
        }
        for relative, value in values.items():
            mode = 0o755 if relative == Path("radishlex_manager") else 0o644
            self.write_file(self.manager, relative, value, mode)

    def create_addon_stage(self) -> None:
        for component in self.layout["components"]:
            if component["source_id"] != "addon_stage":
                continue
            relative = Path(component["source_path"])
            if component["component_id"] == "fcitx-addon":
                value = b"synthetic-fcitx-addon\n"
            elif component["component_id"] == "fcitx-ffi":
                value = self.ffi_bytes
            elif component["component_id"] == "fcitx-addon-metadata":
                value = product_metadata.expected_addon_metadata(
                    self.metadata
                ).encode("utf-8")
            else:
                value = product_metadata.expected_input_method_metadata().encode(
                    "utf-8"
                )
            self.write_file(self.addon, relative, value)

    def assemble(self, name: str = "rootfs") -> Path:
        output = self.work / name
        rootfs.assemble(self.manager, self.addon, output)
        return output

    def test_assembly_is_deterministic_and_declares_owner_model(self) -> None:
        first = self.assemble("first")
        second = self.assemble("second")

        first_manifest_path = rootfs.target_in_rootfs(
            first, self.layout["paths"]["product_manifest"]
        )
        second_manifest_path = rootfs.target_in_rootfs(
            second, self.layout["paths"]["product_manifest"]
        )
        self.assertEqual(
            first_manifest_path.read_bytes(), second_manifest_path.read_bytes()
        )
        manifest = rootfs.verify(first)
        self.assertEqual(manifest["owner_model"], "package-install-root-v1")
        self.assertTrue(manifest["directories"])
        self.assertTrue(manifest["files"])
        for record in [*manifest["directories"], *manifest["files"]]:
            self.assertEqual(record["uid"], 0)
            self.assertEqual(record["gid"], 0)
            self.assertNotIn(str(self.work), json.dumps(record))

        manager_ffi = rootfs.target_in_rootfs(
            first, self.layout["paths"]["manager_ffi"]
        )
        addon_ffi = rootfs.target_in_rootfs(
            first, self.layout["paths"]["addon_ffi"]
        )
        self.assertEqual(rootfs.sha256(manager_ffi), rootfs.sha256(addon_ffi))
        self.assertNotEqual(manager_ffi.stat().st_ino, addon_ffi.stat().st_ino)

    def test_manager_rejects_unapproved_text_font(self) -> None:
        self.write_file(
            self.manager,
            "data/flutter_assets/fonts/NotoSansCJK-Regular.otf",
            b"forbidden-font\n",
        )

        with self.assertRaisesRegex(
            rootfs.LinuxRootfsError, "unapproved embedded font"
        ):
            rootfs.validate_manager_bundle(self.manager)

    def test_manager_rejects_missing_runtime_file(self) -> None:
        (self.manager / "lib/libapp.so").unlink()

        with self.assertRaisesRegex(rootfs.LinuxRootfsError, "missing required"):
            rootfs.validate_manager_bundle(self.manager)

    def test_manager_rejects_symlink(self) -> None:
        os.symlink(
            "AssetManifest.bin",
            self.manager / "data/flutter_assets/linked-asset",
        )

        with self.assertRaisesRegex(rootfs.LinuxRootfsError, "symlink"):
            rootfs.validate_manager_bundle(self.manager)

    def test_manager_rejects_hardlink(self) -> None:
        source = self.manager / "data/icudtl.dat"
        os.link(source, self.manager / "data/icudtl-copy.dat")

        with self.assertRaisesRegex(rootfs.LinuxRootfsError, "hardlinked"):
            rootfs.validate_manager_bundle(self.manager)

    def test_manager_rejects_group_writable_file(self) -> None:
        path = self.manager / "lib/libapp.so"
        path.chmod(0o664)

        with self.assertRaisesRegex(
            rootfs.LinuxRootfsError, "writable by group or other"
        ):
            rootfs.validate_manager_bundle(self.manager)

    def test_addon_stage_rejects_extra_file(self) -> None:
        self.write_file(self.addon, "usr/share/radishlex/unexpected", b"extra\n")

        with self.assertRaisesRegex(
            rootfs.LinuxRootfsError, "inventory differs from layout"
        ):
            rootfs.validate_addon_stage(
                self.addon, self.metadata, self.layout
            )

    def test_addon_stage_rejects_development_version(self) -> None:
        metadata_path = next(
            Path(component["source_path"])
            for component in self.layout["components"]
            if component["component_id"] == "fcitx-addon-metadata"
        )
        path = self.addon / metadata_path
        path.write_text(
            path.read_text(encoding="utf-8").replace("26.7.1", "0.1.0"),
            encoding="utf-8",
        )

        with self.assertRaisesRegex(
            rootfs.LinuxRootfsError, "wrong version"
        ):
            rootfs.validate_addon_stage(
                self.addon, self.metadata, self.layout
            )

    def test_assembly_rejects_ffi_content_drift(self) -> None:
        manager_ffi = self.manager / "lib/libradishlex_ime_ffi.so"
        manager_ffi.write_bytes(b"different-ffi\n")

        with self.assertRaisesRegex(rootfs.LinuxRootfsError, "identical content"):
            rootfs.assemble(self.manager, self.addon, self.work / "rootfs")
        self.assertFalse((self.work / "rootfs").exists())

    def test_verifier_rejects_forbidden_system_path(self) -> None:
        assembled = self.assemble()
        forbidden = assembled / "etc/radishlex.conf"
        forbidden.parent.mkdir()
        forbidden.write_text("forbidden\n", encoding="utf-8")

        with self.assertRaisesRegex(rootfs.LinuxRootfsError, "forbidden path"):
            rootfs.verify(assembled)

    def test_verifier_rejects_payload_mutation(self) -> None:
        assembled = self.assemble()
        desktop = rootfs.target_in_rootfs(
            assembled, self.layout["paths"]["desktop_entry"]
        )
        desktop.write_text(
            desktop.read_text(encoding="utf-8") + "X-RadishLex-Drift=true\n",
            encoding="utf-8",
        )

        with self.assertRaises(rootfs.LinuxRootfsError):
            rootfs.verify(assembled)

    def test_verifier_rejects_extended_attribute(self) -> None:
        assembled = self.assemble()
        desktop = rootfs.target_in_rootfs(
            assembled, self.layout["paths"]["desktop_entry"]
        )
        try:
            os.setxattr(desktop, "user.radishlex-test", b"forbidden")
        except (AttributeError, OSError):
            self.skipTest("test filesystem does not support user xattrs")

        with self.assertRaisesRegex(
            rootfs.LinuxRootfsError, "extended attribute"
        ):
            rootfs.verify(assembled)

    def test_only_manager_executable_is_executable(self) -> None:
        assembled = self.assemble()
        manager_executable = self.layout["paths"]["manager_executable"]

        for path in assembled.rglob("*"):
            if not path.is_file():
                continue
            actual = stat.S_IMODE(path.stat().st_mode)
            expected = 0o755 if rootfs.system_path(assembled, path) == manager_executable else 0o644
            self.assertEqual(actual, expected)


if __name__ == "__main__":
    unittest.main()

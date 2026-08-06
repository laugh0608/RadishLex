#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path

import deb_artifact
import product_metadata
import rootfs


class DebianArtifactTest(unittest.TestCase):
    def setUp(self) -> None:
        self.previous_umask = os.umask(0o022)
        temporary_root = Path(tempfile.gettempdir()).resolve()
        self.temporary = tempfile.TemporaryDirectory(
            prefix="radishlex-deb-artifact-test.", dir=temporary_root
        )
        self.work = Path(self.temporary.name)
        self.manager = self.work / "manager-bundle"
        self.addon = self.work / "addon-stage"
        self.metadata, self.layout = product_metadata.validate_source_contract()
        self.ffi_bytes = b"synthetic-radishlex-ffi-v9\n"
        self.create_manager_bundle()
        self.create_addon_stage()
        self.product_rootfs = self.work / "rootfs"
        rootfs.assemble(self.manager, self.addon, self.product_rootfs)
        self.shlibs = self.write_file(
            self.work,
            "shlibs-depends.txt",
            b"shlibs:Depends=libc6 (>= 2.34), libstdc++6 (>= 11)\n",
        )

    def tearDown(self) -> None:
        try:
            self.temporary.cleanup()
        finally:
            os.umask(self.previous_umask)

    def write_file(
        self,
        base: Path,
        relative: Path | str,
        value: bytes,
        mode: int = 0o644,
    ) -> Path:
        path = base / relative
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

    def output_directory(self, name: str) -> Path:
        path = self.work / name
        path.mkdir(mode=0o755)
        return path

    def build(self, name: str = "artifact") -> tuple[Path, Path]:
        return deb_artifact.build(
            self.product_rootfs,
            self.shlibs,
            self.output_directory(name),
        )

    def test_repeated_builds_are_byte_identical_and_standalone_verifiable(self) -> None:
        first_package, first_evidence = self.build("first")
        second_package, second_evidence = self.build("second")
        self.assertEqual(first_package.read_bytes(), second_package.read_bytes())
        self.assertEqual(first_evidence.read_bytes(), second_evidence.read_bytes())

        evidence = deb_artifact.verify(first_package, first_evidence)
        self.assertEqual(
            [item["name"] for item in evidence["archive"]["members"]],
            ["debian-binary", "control.tar", "data.tar"],
        )
        self.assertEqual(evidence["archive"]["compression"], "none")
        self.assertEqual(evidence["archive"]["source_date_epoch"], 0)
        self.assertEqual(
            evidence["dependency_analysis"]["profile"],
            "dpkg-shlibdeps-debian13-arm64-v1",
        )

    def test_binary_control_resolves_substvars_and_keeps_fixed_dependencies(self) -> None:
        package, evidence_path = self.build()
        evidence = deb_artifact.verify(package, evidence_path)
        dependencies = evidence["control"]["depends"]
        self.assertEqual(dependencies[:2], ["libc6 (>= 2.34)", "libstdc++6 (>= 11)"])
        self.assertEqual(
            dependencies[-4:],
            [
                "fcitx5 (>= 5.1.9)",
                "librime1t64 (>= 1.13.1)",
                "fonts-dejavu-core",
                "fonts-noto-cjk",
            ],
        )
        control_member = deb_artifact.parse_ar(package.read_bytes())[1]
        control, _ = deb_artifact.control_tar_values(control_member.data)
        self.assertNotIn(b"${", control)
        self.assertIn(b"Installed-Size: ", control)

    def test_artifact_contract_rejects_filename_drift(self) -> None:
        value = json.loads(
            deb_artifact.ARTIFACT_CONTRACT_PATH.read_text(encoding="utf-8")
        )
        value["package_filename"] = "radishlex_latest_arm64.deb"
        path = self.write_file(
            self.work,
            "artifact-drift.json",
            (json.dumps(value, indent=2) + "\n").encode("utf-8"),
        )
        with self.assertRaisesRegex(
            deb_artifact.DebianArtifactError, "package_filename differs"
        ):
            deb_artifact.DebianArtifactContract.load(self.metadata, path)

    def test_builder_rejects_unresolved_or_overlapping_shlibs(self) -> None:
        unresolved = self.write_file(
            self.work,
            "unresolved.txt",
            b"shlibs:Depends=${misc:Depends}\n",
        )
        with self.assertRaisesRegex(
            deb_artifact.DebianArtifactError, "resolved dependency list"
        ):
            deb_artifact.build(
                self.product_rootfs,
                unresolved,
                self.output_directory("unresolved-output"),
            )

        overlap = self.write_file(
            self.work,
            "overlap.txt",
            b"shlibs:Depends=librime1t64 (>= 1.13.1)\n",
        )
        with self.assertRaisesRegex(
            deb_artifact.DebianArtifactError, "overlaps fixed"
        ):
            deb_artifact.build(
                self.product_rootfs,
                overlap,
                self.output_directory("overlap-output"),
            )

    def test_builder_rejects_nonempty_output_without_overwrite(self) -> None:
        output = self.output_directory("nonempty")
        sentinel = self.write_file(output, "keep.txt", b"keep\n")
        with self.assertRaisesRegex(
            deb_artifact.DebianArtifactError, "must be empty"
        ):
            deb_artifact.build(self.product_rootfs, self.shlibs, output)
        self.assertEqual(sentinel.read_bytes(), b"keep\n")

    def test_verifier_rejects_package_tampering(self) -> None:
        package, evidence = self.build()
        value = bytearray(package.read_bytes())
        value[-1024] ^= 0x01
        package.write_bytes(value)
        with self.assertRaises(deb_artifact.DebianArtifactError):
            deb_artifact.verify(package, evidence)

    def test_verifier_rejects_ar_identity_drift(self) -> None:
        package, evidence = self.build()
        value = bytearray(package.read_bytes())
        value[8 + 16] = ord("1")
        package.write_bytes(value)
        with self.assertRaisesRegex(
            deb_artifact.DebianArtifactError, "ar identity"
        ):
            deb_artifact.verify(package, evidence)

    def test_verifier_rejects_evidence_rewrite(self) -> None:
        package, evidence = self.build()
        value = json.loads(evidence.read_text(encoding="utf-8"))
        value["package"]["size"] += 1
        evidence.write_text(
            json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            deb_artifact.DebianArtifactError, "does not match package"
        ):
            deb_artifact.verify(package, evidence)

    def test_builder_revalidates_rootfs_before_packaging(self) -> None:
        desktop = rootfs.target_in_rootfs(
            self.product_rootfs, self.layout["paths"]["desktop_entry"]
        )
        desktop.write_text(
            desktop.read_text(encoding="utf-8") + "X-Drift=true\n",
            encoding="utf-8",
        )
        output = self.output_directory("drift-output")
        with self.assertRaises(deb_artifact.DebianArtifactError):
            deb_artifact.build(self.product_rootfs, self.shlibs, output)
        self.assertEqual(list(output.iterdir()), [])


if __name__ == "__main__":
    unittest.main()

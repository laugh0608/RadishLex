#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import plistlib
import shutil
import tempfile
from dataclasses import asdict, dataclass
from pathlib import Path, PurePosixPath
from typing import Any

import product_manifest


REPO_ROOT = Path(__file__).resolve().parents[2]
LAYOUT_PATH = REPO_ROOT / "packaging/macos/install-layout.json"
PAYLOAD_MANIFEST_NAME = "InstallPayloadManifest.json"
UPGRADE_SOURCES_DIRECTORY = "UpgradeSources"
MAX_UPGRADE_SOURCES = 64
EXPECTED_KEYS = {
    "format_version",
    "product_id",
    "distribution_container",
    "installer_kind",
    "installation_scope",
    "installer_bundle_id",
    "manager_component_path",
    "manager_target_path",
    "input_method_component_path",
    "input_method_target_path",
    "data_root_path",
    "install_state_path",
    "default_removal",
    "data_removal",
}
FIXED_VALUES = {
    "format_version": 1,
    "distribution_container": "dmg",
    "installer_kind": "dedicated-user-domain-app",
    "installation_scope": "current-user",
    "installer_bundle_id": "org.radishlex.installer.macos",
    "manager_component_path": "Components/radishlex_manager.app",
    "manager_target_path": "Applications/RadishLex Manager.app",
    "input_method_component_path": "Components/RadishLexInputMethod.app",
    "input_method_target_path": "Library/Input Methods/RadishLexInputMethod.app",
    "data_root_path": "Library/Application Support/RadishLex",
    "install_state_path": (
        "Library/Application Support/RadishLex/.radishlex-install-v1"
    ),
    "default_removal": "programs-only",
    "data_removal": "separate-authorized-flow",
}


class InstallLayoutError(RuntimeError):
    pass


@dataclass(frozen=True)
class InstallLayout:
    format_version: int
    product_id: str
    distribution_container: str
    installer_kind: str
    installation_scope: str
    installer_bundle_id: str
    manager_component_path: str
    manager_target_path: str
    input_method_component_path: str
    input_method_target_path: str
    data_root_path: str
    install_state_path: str
    default_removal: str
    data_removal: str

    @classmethod
    def load(
        cls,
        path: Path = LAYOUT_PATH,
        product_metadata: product_manifest.ProductMetadata | None = None,
    ) -> "InstallLayout":
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except Exception as exc:
            raise InstallLayoutError(f"invalid install layout: {exc}") from exc
        if not isinstance(value, dict) or set(value) != EXPECTED_KEYS:
            raise InstallLayoutError("install layout fields do not match format v1")
        if isinstance(value["format_version"], bool) or not isinstance(
            value["format_version"], int
        ):
            raise InstallLayoutError("format_version must be an integer")
        for key in EXPECTED_KEYS - {"format_version"}:
            if not isinstance(value[key], str) or not value[key]:
                raise InstallLayoutError(f"{key} must be a non-empty string")
            if value[key] != value[key].strip():
                raise InstallLayoutError(f"{key} must not contain outer whitespace")

        metadata = product_metadata or product_manifest.ProductMetadata.load()
        if value["product_id"] != metadata.product_id:
            raise InstallLayoutError("install layout product_id differs from product metadata")
        for key, expected in FIXED_VALUES.items():
            if value[key] != expected:
                raise InstallLayoutError(f"{key} differs from the accepted M4-P03 layout")
        for key in (
            "manager_component_path",
            "manager_target_path",
            "input_method_component_path",
            "input_method_target_path",
            "data_root_path",
            "install_state_path",
        ):
            validate_relative_path(value[key], key)
        if value["manager_component_path"] == value["input_method_component_path"]:
            raise InstallLayoutError("product component paths must be distinct")
        if value["manager_target_path"] == value["input_method_target_path"]:
            raise InstallLayoutError("installed component paths must be distinct")
        data_root = PurePosixPath(value["data_root_path"])
        install_state = PurePosixPath(value["install_state_path"])
        if data_root not in install_state.parents:
            raise InstallLayoutError("install_state_path must remain inside data_root_path")
        return cls(**value)


def validate_relative_path(value: str, label: str) -> None:
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or "\\" in value
        or "//" in value
        or value.endswith("/")
        or any(part in ("", ".", "..") for part in path.parts)
        or path.as_posix() != value
    ):
        raise InstallLayoutError(f"{label} must be a normalized relative POSIX path")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def regular_file_record(path: Path, payload_path: str) -> dict[str, Any]:
    try:
        metadata = path.lstat()
    except OSError as exc:
        raise InstallLayoutError(f"missing payload metadata file: {payload_path}") from exc
    if path.is_symlink() or not path.is_file() or metadata.st_nlink != 1:
        raise InstallLayoutError(f"unsafe payload metadata file: {payload_path}")
    return {
        "path": payload_path,
        "size": metadata.st_size,
        "sha256": sha256(path),
    }


def verify_product_root(
    layout: InstallLayout,
    product_root: Path,
    product_metadata: product_manifest.ProductMetadata | None = None,
) -> product_manifest.ProductMetadata:
    try:
        metadata = product_root.lstat()
    except OSError as exc:
        raise InstallLayoutError("product root is unavailable") from exc
    if product_root.is_symlink() or not product_root.is_dir():
        raise InstallLayoutError("product root must be a non-symlink directory")
    expected_root_entries = {"Components", "LICENSE", "ProductManifest.json"}
    if {entry.name for entry in product_root.iterdir()} != expected_root_entries:
        raise InstallLayoutError("product root entries do not match the install payload contract")
    components = product_root / "Components"
    if components.is_symlink() or not components.is_dir():
        raise InstallLayoutError("product Components directory is invalid")
    expected_components = {
        PurePosixPath(layout.manager_component_path).name,
        PurePosixPath(layout.input_method_component_path).name,
    }
    if {entry.name for entry in components.iterdir()} != expected_components:
        raise InstallLayoutError("product component entries do not match install layout")
    for component_path in (
        layout.manager_component_path,
        layout.input_method_component_path,
    ):
        component = product_root / component_path
        if component.is_symlink() or not component.is_dir():
            raise InstallLayoutError(
                f"product component must be a non-symlink directory: {component_path}"
            )

    product_metadata = product_metadata or product_manifest.ProductMetadata.load()
    try:
        product_manifest.verify_manifest(
            product_metadata,
            product_root / layout.manager_component_path,
            product_root / layout.input_method_component_path,
            product_root / "LICENSE",
            product_root / "ProductManifest.json",
        )
    except product_manifest.ProductManifestError as exc:
        raise InstallLayoutError("product manifest verification failed") from exc
    return product_metadata


def verify_historical_product_root(
    layout: InstallLayout,
    product_root: Path,
) -> tuple[str, str]:
    try:
        product_root.lstat()
    except OSError as exc:
        raise InstallLayoutError("historical product root is unavailable") from exc
    if product_root.is_symlink() or not product_root.is_dir():
        raise InstallLayoutError("historical product root must be a non-symlink directory")
    if {entry.name for entry in product_root.iterdir()} != {
        "Components",
        "LICENSE",
        "ProductManifest.json",
    }:
        raise InstallLayoutError("historical product root entries do not match the contract")
    components_root = product_root / "Components"
    if components_root.is_symlink() or not components_root.is_dir():
        raise InstallLayoutError("historical product Components directory is invalid")
    expected_components = {
        PurePosixPath(layout.manager_component_path).name,
        PurePosixPath(layout.input_method_component_path).name,
    }
    if {entry.name for entry in components_root.iterdir()} != expected_components:
        raise InstallLayoutError("historical product components do not match the contract")
    try:
        manifest = json.loads(
            (product_root / "ProductManifest.json").read_text(encoding="utf-8")
        )
    except Exception as exc:
        raise InstallLayoutError(f"invalid historical product manifest: {exc}") from exc
    expected_manifest_keys = {
        "format_version",
        "product_id",
        "product_version",
        "build_number",
        "minimum_macos",
        "ffi_abi_version",
        "userdb_schema_version",
        "rime_data_manifest_version",
        "native_libraries_manifest_version",
        "data_layout",
        "distribution_identity",
        "rime_schema_id",
        "components",
        "licenses",
    }
    if not isinstance(manifest, dict) or set(manifest) != expected_manifest_keys:
        raise InstallLayoutError("historical product manifest fields do not match format v3")
    current_metadata = product_manifest.ProductMetadata.load()
    product_version = manifest["product_version"]
    build_number = manifest["build_number"]
    if (
        manifest["format_version"] != 3
        or manifest["product_id"] != current_metadata.product_id
        or not isinstance(product_version, str)
        or product_manifest.SEMVER_PATTERN.fullmatch(product_version) is None
        or not isinstance(build_number, str)
        or not build_number.isdigit()
        or int(build_number) <= 0
        or not isinstance(manifest["minimum_macos"], str)
        or product_manifest.MACOS_PATTERN.fullmatch(manifest["minimum_macos"]) is None
        or manifest["data_layout"] != "application-support-v1"
        or manifest["distribution_identity"]
        != current_metadata.distribution_identity
        or not isinstance(manifest["rime_schema_id"], str)
        or not manifest["rime_schema_id"]
    ):
        raise InstallLayoutError("historical product release metadata is invalid")
    for key in (
        "ffi_abi_version",
        "userdb_schema_version",
        "rime_data_manifest_version",
        "native_libraries_manifest_version",
    ):
        if (
            isinstance(manifest[key], bool)
            or not isinstance(manifest[key], int)
            or manifest[key] <= 0
        ):
            raise InstallLayoutError(f"historical product {key} is invalid")
    component_values = manifest["components"]
    if not isinstance(component_values, list) or len(component_values) != 2:
        raise InstallLayoutError("historical product components are invalid")
    by_name: dict[str, dict[str, Any]] = {}
    expected_component_keys = {
        "component",
        "bundle_id",
        "product_version",
        "build_number",
        "minimum_macos",
        "files",
    }
    for component in component_values:
        if (
            not isinstance(component, dict)
            or set(component) != expected_component_keys
            or not isinstance(component.get("component"), str)
            or component["component"] in by_name
        ):
            raise InstallLayoutError("historical product component manifest is invalid")
        by_name[component["component"]] = component
    expected = {
        "manager": (
            layout.manager_component_path,
            current_metadata.manager_bundle_id,
        ),
        "input_method": (
            layout.input_method_component_path,
            current_metadata.input_method_bundle_id,
        ),
    }
    if set(by_name) != set(expected):
        raise InstallLayoutError("historical product component names are invalid")
    for name, (relative_path, bundle_id) in expected.items():
        component = by_name[name]
        bundle = product_root / relative_path
        if bundle.is_symlink() or not bundle.is_dir():
            raise InstallLayoutError("historical product bundle is invalid")
        if (
            component["bundle_id"] != bundle_id
            or component["product_version"] != product_version
            or component["build_number"] != build_number
            or component["minimum_macos"] != manifest["minimum_macos"]
            or component["files"]
            != product_manifest.file_records(bundle, f"historical {name} bundle")
        ):
            raise InstallLayoutError("historical product component does not match its manifest")
        try:
            with (bundle / "Contents/Info.plist").open("rb") as stream:
                info = plistlib.load(stream)
        except Exception as exc:
            raise InstallLayoutError("historical product Info.plist is invalid") from exc
        expected_info = {
            "CFBundleIdentifier": bundle_id,
            "CFBundleShortVersionString": product_version,
            "CFBundleVersion": build_number,
            "LSMinimumSystemVersion": manifest["minimum_macos"],
        }
        if any(str(info.get(key, "")) != value for key, value in expected_info.items()):
            raise InstallLayoutError("historical product Info.plist identity drifted")
    license_path = product_root / "LICENSE"
    if manifest["licenses"] != [product_manifest.license_record(license_path)]:
        raise InstallLayoutError("historical product license does not match its manifest")
    input_resources = (
        product_root / layout.input_method_component_path / "Contents/Resources"
    )
    try:
        with (input_resources / "RimeData.manifest.plist").open("rb") as stream:
            rime_manifest = plistlib.load(stream)
        with (input_resources / "NativeLibraries.manifest.plist").open("rb") as stream:
            native_manifest = plistlib.load(stream)
    except Exception as exc:
        raise InstallLayoutError("historical product runtime manifest is invalid") from exc
    if (
        rime_manifest.get("format_version") != manifest["rime_data_manifest_version"]
        or rime_manifest.get("schema_id") != manifest["rime_schema_id"]
        or native_manifest.get("format_version")
        != manifest["native_libraries_manifest_version"]
    ):
        raise InstallLayoutError("historical product runtime manifest drifted")
    return product_version, build_number


def upgrade_source_manifest(
    layout: InstallLayout,
    upgrade_sources_root: Path,
    target_build_number: str,
) -> list[dict[str, Any]]:
    if upgrade_sources_root.is_symlink() or not upgrade_sources_root.is_dir():
        raise InstallLayoutError("UpgradeSources must be a non-symlink directory")
    if sum(1 for _ in upgrade_sources_root.iterdir()) > MAX_UPGRADE_SOURCES:
        raise InstallLayoutError("too many historical product assemblies")
    inspected: list[tuple[int, str, str, Path]] = []
    for source_root in upgrade_sources_root.iterdir():
        product_version, build_number = verify_historical_product_root(layout, source_root)
        expected_name = f"{product_version}-{build_number}"
        if source_root.name != expected_name:
            raise InstallLayoutError("historical product directory name does not match release")
        inspected.append((int(build_number), product_version, build_number, source_root))
    inspected.sort()
    sources: list[dict[str, Any]] = []
    previous_build = 0
    for build, product_version, build_number, source_root in inspected:
        if build <= previous_build or build >= int(target_build_number):
            raise InstallLayoutError(
                "historical product builds must be unique, ordered, and older than target"
            )
        previous_build = build
        expected_name = f"{product_version}-{build_number}"
        product_path = f"{UPGRADE_SOURCES_DIRECTORY}/{expected_name}"
        sources.append(
            {
                "product_version": product_version,
                "build_number": build_number,
                "product_path": product_path,
                "product_manifest": regular_file_record(
                    source_root / "ProductManifest.json",
                    f"{product_path}/ProductManifest.json",
                ),
            }
        )
    return sources


def expected_payload_manifest(
    layout: InstallLayout,
    product_root: Path,
    layout_path: Path,
    product_metadata: product_manifest.ProductMetadata | None = None,
) -> dict[str, Any]:
    metadata = verify_product_root(layout, product_root, product_metadata)
    return {
        "format_version": 2,
        "product_id": metadata.product_id,
        "product_version": metadata.product_version,
        "build_number": metadata.build_number,
        "distribution_container": layout.distribution_container,
        "installer_kind": layout.installer_kind,
        "installation_scope": layout.installation_scope,
        "installer_bundle_id": layout.installer_bundle_id,
        "components": {
            "manager": {
                "product_path": layout.manager_component_path,
                "target_path": layout.manager_target_path,
            },
            "input_method": {
                "product_path": layout.input_method_component_path,
                "target_path": layout.input_method_target_path,
            },
        },
        "data": {
            "root_path": layout.data_root_path,
            "install_state_path": layout.install_state_path,
            "default_removal": layout.default_removal,
            "data_removal": layout.data_removal,
        },
        "install_layout": regular_file_record(layout_path, "InstallLayout.json"),
        "product_manifest": regular_file_record(
            product_root / "ProductManifest.json",
            "Product/ProductManifest.json",
        ),
        "upgrade_sources": upgrade_source_manifest(
            layout,
            product_root.parent / UPGRADE_SOURCES_DIRECTORY,
            metadata.build_number,
        ),
    }


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def verify_payload(
    payload_root: Path,
    product_metadata: product_manifest.ProductMetadata | None = None,
) -> None:
    if payload_root.is_symlink() or not payload_root.is_dir():
        raise InstallLayoutError("install payload root must be a non-symlink directory")
    expected_entries = {
        "InstallLayout.json",
        PAYLOAD_MANIFEST_NAME,
        "Product",
        UPGRADE_SOURCES_DIRECTORY,
    }
    if {entry.name for entry in payload_root.iterdir()} != expected_entries:
        raise InstallLayoutError("install payload entries do not match format v2")
    layout_path = payload_root / "InstallLayout.json"
    if layout_path.read_bytes() != LAYOUT_PATH.read_bytes():
        raise InstallLayoutError("payload InstallLayout.json differs from committed layout")
    layout = InstallLayout.load(layout_path, product_metadata)
    expected = expected_payload_manifest(
        layout,
        payload_root / "Product",
        layout_path,
        product_metadata,
    )
    try:
        actual = json.loads(
            (payload_root / PAYLOAD_MANIFEST_NAME).read_text(encoding="utf-8")
        )
    except Exception as exc:
        raise InstallLayoutError(f"invalid install payload manifest: {exc}") from exc
    if actual != expected:
        raise InstallLayoutError("install payload manifest does not match payload contents")


def assemble_payload(
    product_root: Path,
    output: Path,
    product_metadata: product_manifest.ProductMetadata | None = None,
    upgrade_source_product_roots: list[Path] | None = None,
) -> None:
    layout = InstallLayout.load(product_metadata=product_metadata)
    target_metadata = verify_product_root(layout, product_root, product_metadata)
    upgrade_source_product_roots = upgrade_source_product_roots or []
    if len(upgrade_source_product_roots) > MAX_UPGRADE_SOURCES:
        raise InstallLayoutError("too many historical product assemblies")
    verified_sources: list[tuple[int, str, str, Path]] = []
    seen_roots: set[Path] = set()
    for source_root in upgrade_source_product_roots:
        canonical_source = source_root.resolve(strict=True)
        if canonical_source in seen_roots or canonical_source == product_root.resolve(strict=True):
            raise InstallLayoutError("historical product roots must be distinct")
        seen_roots.add(canonical_source)
        product_version, build_number = verify_historical_product_root(
            layout, canonical_source
        )
        build = int(build_number)
        if build >= int(target_metadata.build_number):
            raise InstallLayoutError("historical product must be older than target product")
        verified_sources.append(
            (build, product_version, build_number, canonical_source)
        )
    verified_sources.sort()
    if any(
        left[0] == right[0]
        for left, right in zip(verified_sources, verified_sources[1:])
    ):
        raise InstallLayoutError("historical product build numbers must be unique")
    if output.exists() or output.is_symlink():
        raise InstallLayoutError("install payload output already exists")
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.parent.is_symlink() or not output.parent.is_dir():
        raise InstallLayoutError("install payload parent is unsafe")
    staging = Path(
        tempfile.mkdtemp(prefix=f".{output.name}.", dir=str(output.parent))
    )
    try:
        shutil.copytree(product_root, staging / "Product", symlinks=True)
        (staging / UPGRADE_SOURCES_DIRECTORY).mkdir()
        for _, product_version, build_number, source_root in verified_sources:
            shutil.copytree(
                source_root,
                staging
                / UPGRADE_SOURCES_DIRECTORY
                / f"{product_version}-{build_number}",
                symlinks=True,
            )
        shutil.copy2(LAYOUT_PATH, staging / "InstallLayout.json", follow_symlinks=False)
        copied_layout = InstallLayout.load(
            staging / "InstallLayout.json",
            product_metadata,
        )
        write_json(
            staging / PAYLOAD_MANIFEST_NAME,
            expected_payload_manifest(
                copied_layout,
                staging / "Product",
                staging / "InstallLayout.json",
                product_metadata,
            ),
        )
        verify_payload(staging, product_metadata)
        os.rename(staging, output)
    except Exception:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate and assemble the RadishLex macOS install payload layout."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("validate-source")
    field_parser = subparsers.add_parser("field")
    field_parser.add_argument("name", choices=sorted(EXPECTED_KEYS))
    assemble_parser = subparsers.add_parser("assemble")
    assemble_parser.add_argument("--product-root", required=True, type=Path)
    assemble_parser.add_argument("--output", required=True, type=Path)
    assemble_parser.add_argument("--metadata", type=Path)
    assemble_parser.add_argument(
        "--upgrade-source-product-root",
        action="append",
        default=[],
        type=Path,
    )
    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("--payload-root", required=True, type=Path)
    verify_parser.add_argument("--metadata", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        layout = InstallLayout.load()
        if args.command == "validate-source":
            return 0
        if args.command == "field":
            value = asdict(layout)[args.name]
            print(value)
            return 0
        if args.command == "assemble":
            metadata = (
                product_manifest.ProductMetadata.load(args.metadata)
                if args.metadata
                else None
            )
            assemble_payload(
                args.product_root,
                args.output,
                metadata,
                args.upgrade_source_product_root,
            )
            return 0
        if args.command == "verify":
            metadata = (
                product_manifest.ProductMetadata.load(args.metadata)
                if args.metadata
                else None
            )
            verify_payload(args.payload_root, metadata)
            return 0
    except InstallLayoutError as exc:
        raise SystemExit(str(exc)) from exc
    raise SystemExit("unsupported install layout command")


if __name__ == "__main__":
    raise SystemExit(main())

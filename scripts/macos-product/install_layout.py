#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import tempfile
from dataclasses import asdict, dataclass
from pathlib import Path, PurePosixPath
from typing import Any

import product_manifest


REPO_ROOT = Path(__file__).resolve().parents[2]
LAYOUT_PATH = REPO_ROOT / "packaging/macos/install-layout.json"
PAYLOAD_MANIFEST_NAME = "InstallPayloadManifest.json"
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
    def load(cls, path: Path = LAYOUT_PATH) -> "InstallLayout":
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

        metadata = product_manifest.ProductMetadata.load()
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
    layout: InstallLayout, product_root: Path
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

    product_metadata = product_manifest.ProductMetadata.load()
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


def expected_payload_manifest(
    layout: InstallLayout, product_root: Path, layout_path: Path
) -> dict[str, Any]:
    metadata = verify_product_root(layout, product_root)
    return {
        "format_version": 1,
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
    }


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def verify_payload(payload_root: Path) -> None:
    if payload_root.is_symlink() or not payload_root.is_dir():
        raise InstallLayoutError("install payload root must be a non-symlink directory")
    expected_entries = {"InstallLayout.json", PAYLOAD_MANIFEST_NAME, "Product"}
    if {entry.name for entry in payload_root.iterdir()} != expected_entries:
        raise InstallLayoutError("install payload entries do not match format v1")
    layout_path = payload_root / "InstallLayout.json"
    if layout_path.read_bytes() != LAYOUT_PATH.read_bytes():
        raise InstallLayoutError("payload InstallLayout.json differs from committed layout")
    layout = InstallLayout.load(layout_path)
    expected = expected_payload_manifest(layout, payload_root / "Product", layout_path)
    try:
        actual = json.loads(
            (payload_root / PAYLOAD_MANIFEST_NAME).read_text(encoding="utf-8")
        )
    except Exception as exc:
        raise InstallLayoutError(f"invalid install payload manifest: {exc}") from exc
    if actual != expected:
        raise InstallLayoutError("install payload manifest does not match payload contents")


def assemble_payload(product_root: Path, output: Path) -> None:
    layout = InstallLayout.load()
    verify_product_root(layout, product_root)
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
        shutil.copy2(LAYOUT_PATH, staging / "InstallLayout.json", follow_symlinks=False)
        copied_layout = InstallLayout.load(staging / "InstallLayout.json")
        write_json(
            staging / PAYLOAD_MANIFEST_NAME,
            expected_payload_manifest(
                copied_layout,
                staging / "Product",
                staging / "InstallLayout.json",
            ),
        )
        verify_payload(staging)
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
    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("--payload-root", required=True, type=Path)
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
            assemble_payload(args.product_root, args.output)
            return 0
        if args.command == "verify":
            verify_payload(args.payload_root)
            return 0
    except InstallLayoutError as exc:
        raise SystemExit(str(exc)) from exc
    raise SystemExit("unsupported install layout command")


if __name__ == "__main__":
    raise SystemExit(main())

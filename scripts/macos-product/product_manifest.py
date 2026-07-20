#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import plistlib
import re
from dataclasses import dataclass
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
METADATA_PATH = REPO_ROOT / "packaging/macos/product.json"
EXPECTED_KEYS = {
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
    "manager_bundle_id",
    "input_method_bundle_id",
}
SEMVER_PATTERN = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+")
MACOS_PATTERN = re.compile(r"[0-9]+\.[0-9]+")
IDENTIFIER_PATTERN = re.compile(r"[A-Za-z0-9][A-Za-z0-9.-]*")


class ProductManifestError(RuntimeError):
    pass


@dataclass(frozen=True)
class ProductMetadata:
    format_version: int
    product_id: str
    product_version: str
    build_number: str
    minimum_macos: str
    ffi_abi_version: int
    userdb_schema_version: int
    rime_data_manifest_version: int
    native_libraries_manifest_version: int
    data_layout: str
    manager_bundle_id: str
    input_method_bundle_id: str

    @classmethod
    def load(cls, path: Path = METADATA_PATH) -> "ProductMetadata":
        try:
            value = json.loads(path.read_text(encoding="utf-8"))
        except Exception as exc:
            raise ProductManifestError(f"invalid product metadata: {exc}") from exc
        if not isinstance(value, dict) or set(value) != EXPECTED_KEYS:
            raise ProductManifestError("product metadata fields do not match format v1")

        for key in (
            "format_version",
            "ffi_abi_version",
            "userdb_schema_version",
            "rime_data_manifest_version",
            "native_libraries_manifest_version",
        ):
            if isinstance(value[key], bool) or not isinstance(value[key], int):
                raise ProductManifestError(f"{key} must be an integer")
            if value[key] <= 0:
                raise ProductManifestError(f"{key} must be positive")
        if value["format_version"] != 1:
            raise ProductManifestError("unsupported product metadata format")

        for key in (
            "product_id",
            "product_version",
            "build_number",
            "minimum_macos",
            "data_layout",
            "manager_bundle_id",
            "input_method_bundle_id",
        ):
            if not isinstance(value[key], str) or not value[key]:
                raise ProductManifestError(f"{key} must be a non-empty string")
            if value[key] != value[key].strip():
                raise ProductManifestError(f"{key} must not contain outer whitespace")

        if not SEMVER_PATTERN.fullmatch(value["product_version"]):
            raise ProductManifestError("product_version must use numeric major.minor.patch")
        if not value["build_number"].isdigit() or int(value["build_number"]) <= 0:
            raise ProductManifestError("build_number must be a positive decimal string")
        if not MACOS_PATTERN.fullmatch(value["minimum_macos"]):
            raise ProductManifestError("minimum_macos must use numeric major.minor")
        for key in ("product_id", "data_layout"):
            if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", value[key]):
                raise ProductManifestError(f"{key} contains unsupported characters")
        for key in ("manager_bundle_id", "input_method_bundle_id"):
            if not IDENTIFIER_PATTERN.fullmatch(value[key]) or "." not in value[key]:
                raise ProductManifestError(f"{key} is not a stable bundle identifier")
        return cls(**value)

    def manifest_fields(self) -> dict[str, Any]:
        return {
            "format_version": self.format_version,
            "product_id": self.product_id,
            "product_version": self.product_version,
            "build_number": self.build_number,
            "minimum_macos": self.minimum_macos,
            "ffi_abi_version": self.ffi_abi_version,
            "userdb_schema_version": self.userdb_schema_version,
            "rime_data_manifest_version": self.rime_data_manifest_version,
            "native_libraries_manifest_version": self.native_libraries_manifest_version,
            "data_layout": self.data_layout,
        }


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except Exception as exc:
        raise ProductManifestError(f"cannot read {path.name}: {exc}") from exc


def required_match(pattern: str, text: str, label: str) -> str:
    match = re.search(pattern, text, re.MULTILINE)
    if match is None:
        raise ProductManifestError(f"cannot resolve {label}")
    return match.group(1)


def validate_source_contract(
    metadata: ProductMetadata, repo_root: Path = REPO_ROOT
) -> None:
    pubspec = read_text(repo_root / "apps/radishlex-manager/pubspec.yaml")
    manager_version_match = re.search(
        r"^version:\s*([^\s+]+)\+([^\s]+)\s*$", pubspec, re.MULTILINE
    )
    if manager_version_match is None:
        raise ProductManifestError("cannot resolve Manager version")
    manager_version, manager_build = manager_version_match.groups()
    if (
        manager_version != metadata.product_version
        or manager_build != metadata.build_number
    ):
        raise ProductManifestError("Manager pubspec version differs from product metadata")

    app_info = read_text(
        repo_root / "apps/radishlex-manager/macos/Runner/Configs/AppInfo.xcconfig"
    )
    manager_bundle_id = required_match(
        r"^PRODUCT_BUNDLE_IDENTIFIER\s*=\s*([^\s]+)\s*$",
        app_info,
        "Manager bundle ID",
    )
    if manager_bundle_id != metadata.manager_bundle_id:
        raise ProductManifestError("Manager bundle ID differs from product metadata")

    project = read_text(
        repo_root / "apps/radishlex-manager/macos/Runner.xcodeproj/project.pbxproj"
    )
    deployment_targets = set(
        re.findall(r"MACOSX_DEPLOYMENT_TARGET\s*=\s*([^;]+);", project)
    )
    if deployment_targets != {metadata.minimum_macos}:
        raise ProductManifestError("Manager deployment targets differ from product metadata")
    marketing_versions = set(re.findall(r"MARKETING_VERSION\s*=\s*([^;]+);", project))
    current_project_versions = set(
        re.findall(r"CURRENT_PROJECT_VERSION\s*=\s*([^;]+);", project)
    )
    if marketing_versions != {metadata.product_version}:
        raise ProductManifestError("Manager marketing versions differ from product metadata")
    if current_project_versions != {metadata.build_number}:
        raise ProductManifestError("Manager project versions differ from product metadata")

    imk_template_path = repo_root / "platforms/macos-imk/Resources/Info.plist.in"
    try:
        with imk_template_path.open("rb") as stream:
            imk_template = plistlib.load(stream)
    except Exception as exc:
        raise ProductManifestError(f"invalid InputMethod Info.plist template: {exc}") from exc
    expected_imk_fields = {
        "CFBundleIdentifier": metadata.input_method_bundle_id,
        "CFBundleShortVersionString": "__RADISHLEX_PRODUCT_VERSION__",
        "CFBundleVersion": "__RADISHLEX_PRODUCT_BUILD__",
        "LSMinimumSystemVersion": metadata.minimum_macos,
    }
    for key, expected in expected_imk_fields.items():
        if imk_template.get(key) != expected:
            raise ProductManifestError(f"InputMethod {key} differs from product contract")

    contract = read_text(repo_root / "crates/ime-ffi/src/contract.rs")
    rust_abi = int(
        required_match(
            r"RADISHLEX_ABI_CONTRACT_VERSION:\s*u32\s*=\s*([0-9]+)",
            contract,
            "Rust FFI ABI",
        )
    )
    header = read_text(repo_root / "crates/ime-ffi/include/radishlex_input.h")
    header_abi = int(
        required_match(
            r"^#define RADISHLEX_ABI_CONTRACT_VERSION\s+([0-9]+)u$",
            header,
            "C header FFI ABI",
        )
    )
    dart = read_text(
        repo_root
        / "apps/radishlex-manager/lib/src/bridge/ffi_dynamic_native_api.dart"
    )
    dart_abi = int(
        required_match(
            r"_expectedContractVersion\s*=\s*([0-9]+);",
            dart,
            "Dart FFI ABI",
        )
    )
    if {rust_abi, header_abi, dart_abi} != {metadata.ffi_abi_version}:
        raise ProductManifestError("FFI ABI declarations differ from product metadata")

    userdb = read_text(repo_root / "crates/ime-userdb/src/store/connection.rs")
    userdb_schema = int(
        required_match(
            r"SCHEMA_VERSION:\s*i64\s*=\s*([0-9]+)",
            userdb,
            "userdb schema",
        )
    )
    if userdb_schema != metadata.userdb_schema_version:
        raise ProductManifestError("userdb schema differs from product metadata")

    rime_manifest = read_text(repo_root / "scripts/macos-imk/native_manifest.py")
    rime_manifest_version = int(
        required_match(
            r"^FORMAT_VERSION\s*=\s*([0-9]+)$",
            rime_manifest,
            "RimeData manifest version",
        )
    )
    native_manifest = read_text(repo_root / "scripts/macos-imk/bundle_dylibs.py")
    native_manifest_version = int(
        required_match(
            r'"format_version":\s*([0-9]+)',
            native_manifest,
            "native libraries manifest version",
        )
    )
    if rime_manifest_version != metadata.rime_data_manifest_version:
        raise ProductManifestError("RimeData manifest version differs from product metadata")
    if native_manifest_version != metadata.native_libraries_manifest_version:
        raise ProductManifestError(
            "native libraries manifest version differs from product metadata"
        )


def load_plist(path: Path, label: str) -> dict[str, Any]:
    try:
        with path.open("rb") as stream:
            value = plistlib.load(stream)
    except Exception as exc:
        raise ProductManifestError(f"invalid {label}: {exc}") from exc
    if not isinstance(value, dict):
        raise ProductManifestError(f"{label} must contain a dictionary")
    return value


def validate_bundle_info(
    bundle: Path, bundle_id: str, metadata: ProductMetadata, label: str
) -> None:
    value = load_plist(bundle / "Contents/Info.plist", f"{label} Info.plist")
    expected = {
        "CFBundleIdentifier": bundle_id,
        "CFBundleShortVersionString": metadata.product_version,
        "CFBundleVersion": metadata.build_number,
        "LSMinimumSystemVersion": metadata.minimum_macos,
    }
    for key, expected_value in expected.items():
        if str(value.get(key, "")) != expected_value:
            raise ProductManifestError(f"{label} {key} differs from product metadata")


def validate_input_method_manifests(
    bundle: Path, metadata: ProductMetadata
) -> dict[str, Any]:
    resources = bundle / "Contents/Resources"
    rime_manifest = load_plist(
        resources / "RimeData.manifest.plist", "RimeData manifest"
    )
    native_manifest = load_plist(
        resources / "NativeLibraries.manifest.plist", "native libraries manifest"
    )
    if rime_manifest.get("format_version") != metadata.rime_data_manifest_version:
        raise ProductManifestError("RimeData manifest format differs from product metadata")
    try:
        rime_lock = json.loads(
            (REPO_ROOT / "packaging/rime/product-rime-data.json").read_text(
                encoding="utf-8"
            )
        )
    except Exception as exc:
        raise ProductManifestError(f"invalid product RimeData lock: {exc}") from exc
    if rime_manifest.get("schema_id") != rime_lock.get("schema_id"):
        raise ProductManifestError("RimeData schema differs from the product source lock")
    if (
        native_manifest.get("format_version")
        != metadata.native_libraries_manifest_version
    ):
        raise ProductManifestError(
            "native libraries manifest format differs from product metadata"
        )
    return rime_manifest


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def symlink_record(path: Path, root: Path, label: str) -> dict[str, Any]:
    target = os.readlink(path)
    if Path(target).is_absolute():
        raise ProductManifestError(f"{label} contains an absolute symlink")
    try:
        resolved = path.resolve(strict=True)
        resolved.relative_to(root.resolve(strict=True))
    except (OSError, ValueError) as exc:
        raise ProductManifestError(f"{label} contains an escaping or broken symlink") from exc
    return {
        "path": path.relative_to(root).as_posix(),
        "type": "symlink",
        "target": target,
    }


def file_records(root: Path, label: str) -> list[dict[str, Any]]:
    if not root.is_dir() or root.is_symlink():
        raise ProductManifestError(f"{label} must be a real directory")
    records: list[dict[str, Any]] = []
    for current, directory_names, file_names in os.walk(root, followlinks=False):
        current_path = Path(current)
        for name in sorted(directory_names):
            path = current_path / name
            if path.is_symlink():
                records.append(symlink_record(path, root, label))
        directory_names[:] = [
            name for name in directory_names if not (current_path / name).is_symlink()
        ]
        for name in sorted(file_names):
            path = current_path / name
            if path.is_symlink():
                records.append(symlink_record(path, root, label))
                continue
            if not path.is_file():
                raise ProductManifestError(f"{label} contains a non-regular file")
            records.append(
                {
                    "path": path.relative_to(root).as_posix(),
                    "type": "file",
                    "size": path.stat().st_size,
                    "sha256": sha256(path),
                }
            )
    records.sort(key=lambda item: item["path"])
    if not records:
        raise ProductManifestError(f"{label} contains no files")
    return records


def license_record(path: Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
        raise ProductManifestError("product license must be a non-empty regular file")
    return {"path": "LICENSE", "size": path.stat().st_size, "sha256": sha256(path)}


def expected_manifest(
    metadata: ProductMetadata,
    manager_bundle: Path,
    input_method_bundle: Path,
    license_path: Path,
) -> dict[str, Any]:
    validate_bundle_info(
        manager_bundle, metadata.manager_bundle_id, metadata, "Manager"
    )
    validate_bundle_info(
        input_method_bundle,
        metadata.input_method_bundle_id,
        metadata,
        "InputMethod",
    )
    rime_manifest = validate_input_method_manifests(input_method_bundle, metadata)
    return {
        **metadata.manifest_fields(),
        "rime_schema_id": rime_manifest["schema_id"],
        "components": [
            {
                "component": "input_method",
                "bundle_id": metadata.input_method_bundle_id,
                "product_version": metadata.product_version,
                "build_number": metadata.build_number,
                "minimum_macos": metadata.minimum_macos,
                "files": file_records(input_method_bundle, "InputMethod bundle"),
            },
            {
                "component": "manager",
                "bundle_id": metadata.manager_bundle_id,
                "product_version": metadata.product_version,
                "build_number": metadata.build_number,
                "minimum_macos": metadata.minimum_macos,
                "files": file_records(manager_bundle, "Manager bundle"),
            },
        ],
        "licenses": [license_record(license_path)],
    }


def write_manifest(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def verify_manifest(
    metadata: ProductMetadata,
    manager_bundle: Path,
    input_method_bundle: Path,
    license_path: Path,
    manifest_path: Path,
) -> None:
    expected = expected_manifest(
        metadata, manager_bundle, input_method_bundle, license_path
    )
    try:
        actual = json.loads(manifest_path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise ProductManifestError(f"invalid product manifest: {exc}") from exc
    if actual != expected:
        raise ProductManifestError("product manifest does not match assembled artifacts")


def command_validate_source(args: argparse.Namespace) -> int:
    metadata = ProductMetadata.load(args.metadata)
    validate_source_contract(metadata, args.repo_root)
    return 0


def command_field(args: argparse.Namespace) -> int:
    metadata = ProductMetadata.load(args.metadata)
    fields = metadata.manifest_fields() | {
        "manager_bundle_id": metadata.manager_bundle_id,
        "input_method_bundle_id": metadata.input_method_bundle_id,
    }
    if args.name not in fields:
        raise ProductManifestError(f"unsupported product metadata field: {args.name}")
    print(fields[args.name])
    return 0


def command_create(args: argparse.Namespace) -> int:
    metadata = ProductMetadata.load(args.metadata)
    value = expected_manifest(
        metadata, args.manager_bundle, args.input_method_bundle, args.license
    )
    write_manifest(args.manifest, value)
    return 0


def command_verify(args: argparse.Namespace) -> int:
    metadata = ProductMetadata.load(args.metadata)
    verify_manifest(
        metadata,
        args.manager_bundle,
        args.input_method_bundle,
        args.license,
        args.manifest,
    )
    return 0


def add_artifact_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--manager-bundle", type=Path, required=True)
    parser.add_argument("--input-method-bundle", type=Path, required=True)
    parser.add_argument("--license", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--metadata", type=Path, default=METADATA_PATH)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate and manifest the RadishLex macOS product assembly."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    validate_source = subparsers.add_parser("validate-source")
    validate_source.add_argument("--metadata", type=Path, default=METADATA_PATH)
    validate_source.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    validate_source.set_defaults(handler=command_validate_source)

    field = subparsers.add_parser("field")
    field.add_argument("name")
    field.add_argument("--metadata", type=Path, default=METADATA_PATH)
    field.set_defaults(handler=command_field)

    create = subparsers.add_parser("create")
    add_artifact_arguments(create)
    create.set_defaults(handler=command_create)

    verify = subparsers.add_parser("verify")
    add_artifact_arguments(verify)
    verify.set_defaults(handler=command_verify)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        return int(args.handler(args))
    except ProductManifestError as exc:
        raise SystemExit(str(exc)) from exc


if __name__ == "__main__":
    raise SystemExit(main())

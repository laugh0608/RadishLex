#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
PACKAGE_ROOT = REPO_ROOT / "packaging/rime"
LOCK_PATH = PACKAGE_ROOT / "product-rime-data.json"
FORMAT_VERSION = 1
TOP_LEVEL_KEYS = {"format_version", "schema_id", "assets", "license_files"}
ASSET_KEYS = {
    "asset_id",
    "kind",
    "source_path",
    "runtime_path",
    "sha256",
    "license_id",
    "provenance",
}
LICENSE_KEYS = {"component", "source_path", "runtime_path", "sha256"}
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")
IDENTIFIER_PATTERN = re.compile(r"[a-z0-9][a-z0-9-]*")
SCHEMA_PATTERN = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")


class ProductDataError(RuntimeError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def relative_path(value: Any, field: str) -> PurePosixPath:
    if not isinstance(value, str) or not value or value != value.strip():
        raise ProductDataError(f"{field} must be a non-empty normalized string")
    path = PurePosixPath(value)
    if path.is_absolute() or path.as_posix() != value:
        raise ProductDataError(f"{field} must be a normalized relative path")
    if any(part in ("", ".", "..") for part in path.parts):
        raise ProductDataError(f"{field} must not traverse directories")
    return path


def regular_source(package_root: Path, value: Any, field: str) -> Path:
    relative = relative_path(value, field)
    path = package_root.joinpath(*relative.parts)
    if path.is_symlink() or not path.is_file():
        raise ProductDataError(f"{field} must identify a regular repository file")
    try:
        path.resolve(strict=True).relative_to(package_root.resolve(strict=True))
    except (OSError, ValueError) as exc:
        raise ProductDataError(f"{field} escapes the Rime package root") from exc
    return path


def expected_hash(value: Any, field: str) -> str:
    if not isinstance(value, str) or SHA256_PATTERN.fullmatch(value) is None:
        raise ProductDataError(f"{field} must be a lowercase SHA-256")
    return value


def validate_provenance(value: Any, label: str) -> None:
    if not isinstance(value, dict) or not isinstance(value.get("type"), str):
        raise ProductDataError(f"{label} provenance must be an object with a type")
    provenance_type = value["type"]
    if provenance_type == "radishlex-authored":
        expected_keys = {"type", "description"}
        text_fields = ("description",)
    elif provenance_type == "radishlex-maintained":
        expected_keys = {
            "type",
            "upstream_repository",
            "upstream_commit",
            "upstream_path",
            "modifications",
        }
        text_fields = (
            "upstream_repository",
            "upstream_commit",
            "upstream_path",
            "modifications",
        )
    elif provenance_type == "upstream-verbatim":
        expected_keys = {"type", "repository", "commit", "path"}
        text_fields = ("repository", "commit", "path")
    else:
        raise ProductDataError(f"{label} has unsupported provenance type")
    if set(value) != expected_keys:
        raise ProductDataError(f"{label} provenance fields do not match its type")
    for field in text_fields:
        if not isinstance(value[field], str) or not value[field].strip():
            raise ProductDataError(f"{label} provenance {field} must be non-empty")
    commit = value.get("commit", value.get("upstream_commit"))
    if commit is not None and re.fullmatch(r"[0-9a-f]{40}", commit) is None:
        raise ProductDataError(f"{label} provenance commit must be a full Git SHA")
    repository = value.get("repository", value.get("upstream_repository"))
    if repository is not None and not repository.startswith("https://github.com/"):
        raise ProductDataError(f"{label} provenance repository must use GitHub HTTPS")


def load_lock(lock_path: Path) -> dict[str, Any]:
    try:
        value = json.loads(lock_path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise ProductDataError(f"invalid RimeData lock: {exc}") from exc
    if not isinstance(value, dict) or set(value) != TOP_LEVEL_KEYS:
        raise ProductDataError("RimeData lock fields do not match format v1")
    if value["format_version"] != FORMAT_VERSION:
        raise ProductDataError("unsupported RimeData lock format")
    if not isinstance(value["schema_id"], str) or SCHEMA_PATTERN.fullmatch(
        value["schema_id"]
    ) is None:
        raise ProductDataError("schema_id is not a stable Rime identifier")
    if not isinstance(value["assets"], list) or not value["assets"]:
        raise ProductDataError("RimeData lock must contain assets")
    if not isinstance(value["license_files"], list) or not value["license_files"]:
        raise ProductDataError("RimeData lock must contain license files")
    return value


def validate_source(
    lock_path: Path = LOCK_PATH, package_root: Path = PACKAGE_ROOT
) -> dict[str, Any]:
    value = load_lock(lock_path)
    asset_ids: set[str] = set()
    runtime_paths: set[str] = set()
    source_paths: set[str] = set()
    licensed_components: set[str] = set()

    for index, asset in enumerate(value["assets"]):
        label = f"assets[{index}]"
        if not isinstance(asset, dict) or set(asset) != ASSET_KEYS:
            raise ProductDataError(f"{label} fields do not match format v1")
        asset_id = asset["asset_id"]
        if not isinstance(asset_id, str) or IDENTIFIER_PATTERN.fullmatch(asset_id) is None:
            raise ProductDataError(f"{label} asset_id is invalid")
        if asset_id in asset_ids:
            raise ProductDataError(f"duplicate RimeData asset_id: {asset_id}")
        asset_ids.add(asset_id)
        if asset["kind"] not in ("configuration", "schema", "dictionary"):
            raise ProductDataError(f"{label} kind is unsupported")
        if asset["license_id"] != "Apache-2.0":
            raise ProductDataError(f"{label} must use the audited Apache-2.0 license")
        source = regular_source(package_root, asset["source_path"], f"{label}.source_path")
        source_relative = relative_path(asset["source_path"], f"{label}.source_path").as_posix()
        runtime = relative_path(asset["runtime_path"], f"{label}.runtime_path").as_posix()
        if source_relative in source_paths:
            raise ProductDataError(f"duplicate RimeData source path: {source_relative}")
        if runtime in runtime_paths:
            raise ProductDataError(f"duplicate RimeData runtime path: {runtime}")
        source_paths.add(source_relative)
        runtime_paths.add(runtime)
        expected = expected_hash(asset["sha256"], f"{label}.sha256")
        if sha256(source) != expected:
            raise ProductDataError(f"RimeData source hash mismatch: {source_relative}")
        validate_provenance(asset["provenance"], label)

    for index, license_file in enumerate(value["license_files"]):
        label = f"license_files[{index}]"
        if not isinstance(license_file, dict) or set(license_file) != LICENSE_KEYS:
            raise ProductDataError(f"{label} fields do not match format v1")
        component = license_file["component"]
        if not isinstance(component, str) or IDENTIFIER_PATTERN.fullmatch(component) is None:
            raise ProductDataError(f"{label} component is invalid")
        licensed_components.add(component)
        source = regular_source(package_root, license_file["source_path"], f"{label}.source_path")
        source_relative = relative_path(
            license_file["source_path"], f"{label}.source_path"
        ).as_posix()
        runtime = relative_path(
            license_file["runtime_path"], f"{label}.runtime_path"
        ).as_posix()
        if not runtime.startswith(f"Licenses/{component}/"):
            raise ProductDataError(f"{label} runtime path must stay in its component directory")
        if source_relative in source_paths:
            raise ProductDataError(f"duplicate RimeData source path: {source_relative}")
        if runtime in runtime_paths:
            raise ProductDataError(f"duplicate RimeData runtime path: {runtime}")
        source_paths.add(source_relative)
        runtime_paths.add(runtime)
        expected = expected_hash(license_file["sha256"], f"{label}.sha256")
        if sha256(source) != expected:
            raise ProductDataError(f"RimeData source hash mismatch: {source_relative}")

    schema_id = value["schema_id"]
    required_runtime = {"default.yaml", f"{schema_id}.schema.yaml"}
    if not required_runtime.issubset(runtime_paths):
        raise ProductDataError("RimeData lock is missing default or product schema")
    if not any(path.endswith(".dict.yaml") for path in runtime_paths):
        raise ProductDataError("RimeData lock is missing a dictionary")
    if "rime-pinyin-simp" not in licensed_components:
        raise ProductDataError("RimeData lock is missing rime-pinyin-simp license files")
    required_licenses = {
        "Licenses/rime-pinyin-simp/LICENSE",
        "Licenses/rime-pinyin-simp/AUTHORS",
    }
    if not required_licenses.issubset(runtime_paths):
        raise ProductDataError("RimeData lock is missing LICENSE or AUTHORS attribution")

    committed_files = {
        path.relative_to(package_root).as_posix()
        for root in (package_root / "data", package_root / "licenses")
        for path in root.rglob("*")
        if path.is_file() or path.is_symlink()
    }
    if committed_files != source_paths:
        extras = sorted(committed_files - source_paths)
        missing = sorted(source_paths - committed_files)
        raise ProductDataError(
            f"RimeData lock inventory differs from package files; extras={extras}, missing={missing}"
        )

    schema_asset = next(
        asset for asset in value["assets"] if asset["runtime_path"] == f"{schema_id}.schema.yaml"
    )
    schema_text = regular_source(
        package_root, schema_asset["source_path"], "product schema"
    ).read_text(encoding="utf-8")
    for forbidden in ("dependencies:", "import_preset:", "reverse_lookup"):
        if forbidden in schema_text:
            raise ProductDataError(f"product schema contains deferred feature: {forbidden}")
    if f"schema_id: {schema_id}" not in schema_text or "dictionary: pinyin_simp" not in schema_text:
        raise ProductDataError("product schema identity or dictionary binding is invalid")

    default_asset = next(
        asset for asset in value["assets"] if asset["runtime_path"] == "default.yaml"
    )
    default_text = regular_source(
        package_root, default_asset["source_path"], "product default"
    ).read_text(encoding="utf-8")
    if f"- schema: {schema_id}" not in default_text or "  page_size: 5" not in default_text:
        raise ProductDataError("product default does not bind schema and five-candidate page")
    return value


def source_manifest_bytes(value: dict[str, Any]) -> bytes:
    return (json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode(
        "utf-8"
    )


def expected_runtime_sources(
    value: dict[str, Any], package_root: Path
) -> dict[str, Path]:
    result: dict[str, Path] = {}
    for item in [*value["assets"], *value["license_files"]]:
        result[item["runtime_path"]] = regular_source(
            package_root, item["source_path"], item["source_path"]
        )
    return result


def assemble(
    output: Path, lock_path: Path = LOCK_PATH, package_root: Path = PACKAGE_ROOT
) -> None:
    value = validate_source(lock_path, package_root)
    if output.exists() or output.is_symlink():
        raise ProductDataError(f"RimeData assembly output already exists: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=".rime-data.", dir=output.parent))
    try:
        for runtime, source in expected_runtime_sources(value, package_root).items():
            destination = staging.joinpath(*PurePosixPath(runtime).parts)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
            destination.chmod(0o644)
        (staging / "SourceManifest.json").write_bytes(source_manifest_bytes(value))
        (staging / "SourceManifest.json").chmod(0o644)
        staging.rename(output)
    except Exception:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def verify_assembly(
    data_dir: Path, lock_path: Path = LOCK_PATH, package_root: Path = PACKAGE_ROOT
) -> None:
    value = validate_source(lock_path, package_root)
    if data_dir.is_symlink() or not data_dir.is_dir():
        raise ProductDataError("assembled RimeData must be a real directory")
    expected_sources = expected_runtime_sources(value, package_root)
    expected_paths = {*expected_sources, "SourceManifest.json"}
    actual_paths: set[str] = set()
    for path in data_dir.rglob("*"):
        if path.is_symlink():
            raise ProductDataError("assembled RimeData must not contain symlinks")
        if path.is_file():
            actual_paths.add(path.relative_to(data_dir).as_posix())
    if actual_paths != expected_paths:
        raise ProductDataError("assembled RimeData file inventory differs from the source lock")
    for runtime, source in expected_sources.items():
        if sha256(data_dir / runtime) != sha256(source):
            raise ProductDataError(f"assembled RimeData file hash mismatch: {runtime}")
    if (data_dir / "SourceManifest.json").read_bytes() != source_manifest_bytes(value):
        raise ProductDataError("assembled RimeData source manifest differs from the source lock")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate or assemble pinned RadishLex RimeData.")
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("validate")
    field_parser = subparsers.add_parser("field")
    field_parser.add_argument("name", choices=("schema_id",))
    assemble_parser = subparsers.add_parser("assemble")
    assemble_parser.add_argument("--output", type=Path, required=True)
    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("--data-dir", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "validate":
            validate_source()
        elif args.command == "field":
            print(validate_source()[args.name])
        elif args.command == "assemble":
            assemble(args.output)
        else:
            verify_assembly(args.data_dir)
    except ProductDataError as exc:
        raise SystemExit(str(exc)) from exc
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

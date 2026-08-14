#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import stat
import tempfile
from pathlib import Path, PurePosixPath
from typing import Any

import product_metadata


REPO_ROOT = Path(__file__).resolve().parents[2]
MATERIAL_FONT_RELATIVE_PATH = Path(
    "data/flutter_assets/fonts/MaterialIcons-Regular.otf"
)
FONT_MANIFEST_RELATIVE_PATH = Path("data/flutter_assets/FontManifest.json")
NOTICES_RELATIVE_PATH = Path("data/flutter_assets/NOTICES.Z")
MANAGER_REQUIRED_PATHS = {
    Path("radishlex_manager"),
    Path("lib/libapp.so"),
    Path("lib/libflutter_linux_gtk.so"),
    Path("lib/libradishlex_ime_ffi.so"),
    Path("data/icudtl.dat"),
    Path("data/flutter_assets/AssetManifest.bin"),
    FONT_MANIFEST_RELATIVE_PATH,
    NOTICES_RELATIVE_PATH,
    MATERIAL_FONT_RELATIVE_PATH,
}
MANIFEST_KEYS = {
    "format_version",
    "inventory_scope",
    "layout_id",
    "manifest_path",
    "owner_model",
    "product",
    "directories",
    "files",
    "ffi_equivalence",
}


class LinuxRootfsError(RuntimeError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_json_bytes(value: dict[str, Any]) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def file_mode(path: Path) -> int:
    return stat.S_IMODE(path.lstat().st_mode)


def require_no_extended_attributes(path: Path, label: str) -> None:
    if not hasattr(os, "listxattr"):
        return
    try:
        attributes = os.listxattr(path, follow_symlinks=False)
    except (NotImplementedError, OSError) as exc:
        raise LinuxRootfsError(
            f"cannot inspect rootfs extended attributes: {label}"
        ) from exc
    if attributes:
        raise LinuxRootfsError(
            f"rootfs node has an extended attribute: {label}"
        )


def mode_string(value: int) -> str:
    return f"{value:04o}"


def target_in_rootfs(rootfs: Path, system_path: str) -> Path:
    normalized = product_metadata.absolute_system_path(system_path, "system path")
    relative = PurePosixPath(normalized).relative_to("/")
    return rootfs.joinpath(*relative.parts)


def system_path(rootfs: Path, path: Path) -> str:
    relative = path.relative_to(rootfs).as_posix()
    return f"/{relative}"


def require_canonical_input(path: Path, label: str, directory: bool) -> Path:
    if not path.is_absolute() or path != Path(os.path.normpath(path)):
        raise LinuxRootfsError(f"{label} must be an absolute normalized path")
    if path.is_symlink():
        raise LinuxRootfsError(f"{label} must not be a symlink")
    try:
        resolved = path.resolve(strict=True)
    except (OSError, RuntimeError) as exc:
        raise LinuxRootfsError(f"{label} is unavailable: {exc}") from exc
    if resolved != path:
        raise LinuxRootfsError(f"{label} must not traverse a symlinked ancestor")
    if directory and not path.is_dir():
        raise LinuxRootfsError(f"{label} must be a real directory")
    if not directory and not path.is_file():
        raise LinuxRootfsError(f"{label} must be a regular file")
    return path


def require_safe_source_node(path: Path, label: str, directory: bool) -> None:
    status = path.lstat()
    if stat.S_ISLNK(status.st_mode):
        raise LinuxRootfsError(f"{label} contains a symlink")
    if directory:
        if not stat.S_ISDIR(status.st_mode):
            raise LinuxRootfsError(f"{label} contains a non-directory node")
    else:
        if not stat.S_ISREG(status.st_mode):
            raise LinuxRootfsError(f"{label} contains a non-regular file")
        if status.st_nlink != 1:
            raise LinuxRootfsError(f"{label} contains a hardlinked file")
    if status.st_mode & (stat.S_IWGRP | stat.S_IWOTH):
        raise LinuxRootfsError(f"{label} is writable by group or other")


def source_tree_files(root: Path, label: str) -> dict[Path, Path]:
    require_canonical_input(root, label, directory=True)
    require_safe_source_node(root, label, directory=True)
    result: dict[Path, Path] = {}
    for current, directory_names, file_names in os.walk(root, followlinks=False):
        current_path = Path(current)
        for name in directory_names:
            require_safe_source_node(
                current_path / name, f"{label}/{name}", directory=True
            )
        for name in file_names:
            path = current_path / name
            require_safe_source_node(path, f"{label}/{name}", directory=False)
            relative = path.relative_to(root)
            result[relative] = path
    if not result:
        raise LinuxRootfsError(f"{label} contains no files")
    return result


def validate_material_font_manifest(manager_bundle: Path) -> None:
    manifest_path = manager_bundle / FONT_MANIFEST_RELATIVE_PATH
    try:
        value = json.loads(manifest_path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise LinuxRootfsError(f"invalid Manager FontManifest.json: {exc}") from exc
    expected = [
        {
            "family": "MaterialIcons",
            "fonts": [{"asset": "fonts/MaterialIcons-Regular.otf"}],
        }
    ]
    if value != expected:
        raise LinuxRootfsError(
            "Manager font manifest must contain only Material Icons"
        )
    notices = manager_bundle / NOTICES_RELATIVE_PATH
    if notices.stat().st_size == 0:
        raise LinuxRootfsError("Manager NOTICES.Z must bind generated asset notices")


def validate_manager_bundle(manager_bundle: Path) -> dict[Path, Path]:
    files = source_tree_files(manager_bundle, "Manager bundle")
    missing = MANAGER_REQUIRED_PATHS - set(files)
    if missing:
        raise LinuxRootfsError(
            "Manager bundle is missing required files: "
            + ", ".join(path.as_posix() for path in sorted(missing))
        )
    if file_mode(files[Path("radishlex_manager")]) & stat.S_IXUSR == 0:
        raise LinuxRootfsError("Manager executable is not executable")
    font_files = {
        path
        for path in files
        if path.suffix.lower() in {".otf", ".ttc", ".ttf"}
    }
    if font_files != {MATERIAL_FONT_RELATIVE_PATH}:
        raise LinuxRootfsError(
            "Manager bundle contains an unapproved embedded font asset"
        )
    validate_material_font_manifest(manager_bundle)
    return files


def addon_stage_expected_files(
    layout: dict[str, Any],
) -> dict[Path, str]:
    result: dict[Path, str] = {}
    for component in layout["components"]:
        if component["source_id"] != "addon_stage":
            continue
        result[Path(component["source_path"])] = component["component_id"]
    return result


def validate_addon_stage(
    addon_stage: Path,
    metadata: product_metadata.LinuxProductMetadata,
    layout: dict[str, Any],
) -> dict[Path, Path]:
    files = source_tree_files(addon_stage, "product-profile addon stage")
    expected = addon_stage_expected_files(layout)
    if set(files) != set(expected):
        extras = sorted(path.as_posix() for path in set(files) - set(expected))
        missing = sorted(path.as_posix() for path in set(expected) - set(files))
        raise LinuxRootfsError(
            "product-profile addon stage inventory differs from layout; "
            f"extras={extras}, missing={missing}"
        )
    paths = layout["paths"]
    addon_metadata = files[
        Path(
            next(
                item["source_path"]
                for item in layout["components"]
                if item["component_id"] == "fcitx-addon-metadata"
            )
        )
    ]
    input_method_metadata = files[
        Path(
            next(
                item["source_path"]
                for item in layout["components"]
                if item["component_id"] == "fcitx-input-method-metadata"
            )
        )
    ]
    if addon_metadata.read_text(encoding="utf-8") != (
        product_metadata.expected_addon_metadata(metadata)
    ):
        raise LinuxRootfsError(
            "product-profile Fcitx addon metadata has the wrong version or fields"
        )
    if input_method_metadata.read_text(encoding="utf-8") != (
        product_metadata.expected_input_method_metadata()
    ):
        raise LinuxRootfsError(
            "product-profile Fcitx input-method metadata differs from contract"
        )
    if Path(paths["addon_library"]).name != "radishlex.so":
        raise LinuxRootfsError("addon layout filename is not fixed")
    return files


def create_parent_directories(path: Path, rootfs: Path) -> None:
    current = path.parent
    pending: list[Path] = []
    while current != rootfs and not current.exists():
        pending.append(current)
        current = current.parent
    if current != rootfs and current.is_symlink():
        raise LinuxRootfsError("rootfs destination traverses a symlink")
    for directory in reversed(pending):
        directory.mkdir()
        directory.chmod(0o755)


def copy_regular(source: Path, destination: Path, rootfs: Path, mode: int) -> None:
    require_safe_source_node(source, source.name, directory=False)
    if destination.exists() or destination.is_symlink():
        raise LinuxRootfsError(
            f"duplicate rootfs destination: {system_path(rootfs, destination)}"
        )
    create_parent_directories(destination, rootfs)
    shutil.copyfile(source, destination)
    destination.chmod(mode)


def copy_manager_bundle(
    manager_bundle: Path,
    manager_files: dict[Path, Path],
    destination: Path,
    rootfs: Path,
) -> None:
    if destination.exists() or destination.is_symlink():
        raise LinuxRootfsError("Manager rootfs destination already exists")
    create_parent_directories(destination, rootfs)
    destination.mkdir()
    destination.chmod(0o755)
    for relative, source in sorted(manager_files.items()):
        target = destination / relative
        mode = 0o755 if relative == Path("radishlex_manager") else 0o644
        copy_regular(source, target, rootfs, mode)
    for path in sorted(destination.rglob("*")):
        if path.is_dir():
            path.chmod(0o755)


def component_source_path(
    component: dict[str, str],
    addon_stage: Path,
) -> Path:
    source_id = component["source_id"]
    relative = Path(component["source_path"])
    if source_id == "addon_stage":
        return addon_stage / relative
    if source_id == "linux_packaging":
        return product_metadata.PACKAGE_ROOT / relative
    if source_id == "repository":
        return REPO_ROOT / relative
    raise LinuxRootfsError(
        f"component {component['component_id']} has no regular-file source"
    )


def assemble(
    manager_bundle: Path,
    addon_stage: Path,
    output: Path,
) -> None:
    metadata, layout = product_metadata.validate_source_contract()
    manager_bundle = require_canonical_input(
        manager_bundle, "Manager bundle", directory=True
    )
    addon_stage = require_canonical_input(
        addon_stage, "product-profile addon stage", directory=True
    )
    manager_files = validate_manager_bundle(manager_bundle)
    addon_files = validate_addon_stage(addon_stage, metadata, layout)

    if not output.is_absolute() or output != Path(os.path.normpath(output)):
        raise LinuxRootfsError("rootfs output must be an absolute normalized path")
    if output.exists() or output.is_symlink():
        raise LinuxRootfsError("rootfs output must not already exist")
    output_parent = output.parent
    output_parent.mkdir(parents=True, exist_ok=True)
    require_canonical_input(output_parent, "rootfs output parent", directory=True)
    staging = Path(tempfile.mkdtemp(prefix=".radishlex-linux-rootfs.", dir=output_parent))
    staging.chmod(0o755)
    try:
        paths = layout["paths"]
        copy_manager_bundle(
            manager_bundle,
            manager_files,
            target_in_rootfs(staging, paths["manager_root"]),
            staging,
        )
        for component in layout["components"]:
            if component["source_id"] in ("manager_bundle", "product_rime_data"):
                continue
            source = component_source_path(component, addon_stage)
            if component["source_id"] == "addon_stage" and (
                Path(component["source_path"]) not in addon_files
            ):
                raise LinuxRootfsError("validated addon stage source disappeared")
            copy_regular(
                source,
                target_in_rootfs(staging, component["target_path"]),
                staging,
                0o644,
            )

        rime_root = target_in_rootfs(staging, paths["rime_data_root"])
        create_parent_directories(rime_root, staging)
        product_metadata.product_data.assemble(
            rime_root,
            REPO_ROOT / "packaging/rime/product-rime-data.json",
            REPO_ROOT / "packaging/rime",
        )
        rime_root.chmod(0o755)
        for path in sorted(rime_root.rglob("*")):
            path.chmod(0o755 if path.is_dir() else 0o644)

        manager_ffi = target_in_rootfs(staging, paths["manager_ffi"])
        addon_ffi = target_in_rootfs(staging, paths["addon_ffi"])
        if sha256(manager_ffi) != sha256(addon_ffi):
            raise LinuxRootfsError(
                "Manager and Fcitx FFI libraries do not have identical content"
            )
        if manager_ffi.stat().st_ino == addon_ffi.stat().st_ino:
            raise LinuxRootfsError(
                "Manager and Fcitx FFI libraries must use distinct inodes"
            )

        manifest_path = target_in_rootfs(staging, paths["product_manifest"])
        create_parent_directories(manifest_path, staging)
        manifest_path.write_bytes(b"{}\n")
        manifest_path.chmod(0o644)
        manifest = expected_manifest(staging, metadata, layout)
        manifest_path.write_bytes(canonical_json_bytes(manifest))
        verify(staging)
        staging.rename(output)
    except Exception:
        shutil.rmtree(staging, ignore_errors=True)
        raise


def path_has_prefix(path: str, prefix: str) -> bool:
    return path == prefix or path.startswith(f"{prefix}/")


def reject_forbidden_path(path: str, layout: dict[str, Any]) -> None:
    for prefix in layout["forbidden_prefixes"]:
        if path_has_prefix(path, prefix):
            raise LinuxRootfsError(f"rootfs contains a forbidden path: {path}")
    suffix = PurePosixPath(path).suffix.lower()
    if (
        suffix in layout["forbidden_font_suffixes"]
        and path not in layout["allowed_embedded_font_paths"]
    ):
        raise LinuxRootfsError(f"rootfs contains an unapproved font asset: {path}")
    if "/fontconfig/" in path.lower() or path.lower().endswith("/fonts.conf"):
        raise LinuxRootfsError(f"rootfs contains fontconfig payload: {path}")


def component_for_path(path: str, layout: dict[str, Any]) -> str:
    paths = layout["paths"]
    if path_has_prefix(path, paths["manager_root"]):
        return "manager-bundle"
    if path_has_prefix(path, paths["rime_data_root"]):
        return "rime-data"
    for component in layout["components"]:
        if component["kind"] == "file" and component["target_path"] == path:
            return component["component_id"]
    raise LinuxRootfsError(f"rootfs contains a file outside the layout: {path}")


def rime_expected_paths(
    metadata: product_metadata.LinuxProductMetadata,
    layout: dict[str, Any],
) -> set[str]:
    lock = product_metadata.product_data.validate_source(
        REPO_ROOT / "packaging/rime/product-rime-data.json",
        REPO_ROOT / "packaging/rime",
    )
    root = layout["paths"]["rime_data_root"]
    runtime_paths = {
        item["runtime_path"]
        for item in [*lock["assets"], *lock["license_files"]]
    }
    runtime_paths.add("SourceManifest.json")
    if lock["format_version"] != metadata.rime_data_manifest_version:
        raise LinuxRootfsError("RimeData format differs from product metadata")
    return {f"{root}/{relative}" for relative in runtime_paths}


def validate_allowed_inventory(
    files: set[str],
    metadata: product_metadata.LinuxProductMetadata,
    layout: dict[str, Any],
) -> None:
    paths = layout["paths"]
    manifest_path = paths["product_manifest"]
    if manifest_path not in files:
        raise LinuxRootfsError("rootfs product manifest is missing")
    manager_files = {
        path for path in files if path_has_prefix(path, paths["manager_root"])
    }
    required_manager = {
        f"{paths['manager_root']}/{relative.as_posix()}"
        for relative in MANAGER_REQUIRED_PATHS
    }
    if not required_manager.issubset(manager_files):
        raise LinuxRootfsError("rootfs Manager bundle is incomplete")
    rime_files = {
        path for path in files if path_has_prefix(path, paths["rime_data_root"])
    }
    if rime_files != rime_expected_paths(metadata, layout):
        raise LinuxRootfsError("rootfs RimeData inventory differs from the source lock")
    fixed_files = {
        component["target_path"]
        for component in layout["components"]
        if component["kind"] == "file"
    }
    expected = manager_files | rime_files | fixed_files | {manifest_path}
    if files != expected:
        extras = sorted(files - expected)
        missing = sorted(expected - files)
        raise LinuxRootfsError(
            f"rootfs inventory differs from layout; extras={extras}, missing={missing}"
        )
    actual_fonts = {
        path
        for path in files
        if PurePosixPath(path).suffix.lower()
        in set(layout["forbidden_font_suffixes"])
    }
    if actual_fonts != set(layout["allowed_embedded_font_paths"]):
        raise LinuxRootfsError(
            "rootfs embedded font inventory differs from the Material Icons exception"
        )


def collect_inventory(
    rootfs: Path,
    metadata: product_metadata.LinuxProductMetadata,
    layout: dict[str, Any],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    owner = layout["owner"]
    manifest_path = layout["paths"]["product_manifest"]
    executable_path = layout["paths"]["manager_executable"]
    directory_records: list[dict[str, Any]] = []
    file_records: list[dict[str, Any]] = []
    actual_files: set[str] = set()
    require_no_extended_attributes(rootfs, "/")
    for path in sorted(rootfs.rglob("*")):
        relative_system_path = system_path(rootfs, path)
        reject_forbidden_path(relative_system_path, layout)
        require_no_extended_attributes(path, relative_system_path)
        status = path.lstat()
        if stat.S_ISLNK(status.st_mode):
            raise LinuxRootfsError(
                f"rootfs contains a symlink: {relative_system_path}"
            )
        if stat.S_ISDIR(status.st_mode):
            if file_mode(path) != 0o755:
                raise LinuxRootfsError(
                    f"rootfs directory mode differs from 0755: {relative_system_path}"
                )
            directory_records.append(
                {
                    "gid": owner["gid"],
                    "mode": "0755",
                    "path": relative_system_path,
                    "uid": owner["uid"],
                }
            )
            continue
        if not stat.S_ISREG(status.st_mode):
            raise LinuxRootfsError(
                f"rootfs contains a non-regular file: {relative_system_path}"
            )
        if status.st_nlink != 1:
            raise LinuxRootfsError(
                f"rootfs contains a hardlinked file: {relative_system_path}"
            )
        expected_mode = 0o755 if relative_system_path == executable_path else 0o644
        if file_mode(path) != expected_mode:
            raise LinuxRootfsError(
                "rootfs file mode differs from layout: "
                f"{relative_system_path} expected={mode_string(expected_mode)}"
            )
        actual_files.add(relative_system_path)
        if relative_system_path == manifest_path:
            continue
        file_records.append(
            {
                "component_id": component_for_path(
                    relative_system_path, layout
                ),
                "gid": owner["gid"],
                "mode": mode_string(expected_mode),
                "path": relative_system_path,
                "sha256": sha256(path),
                "size": status.st_size,
                "uid": owner["uid"],
            }
        )
    validate_allowed_inventory(actual_files, metadata, layout)
    directory_records.sort(key=lambda item: item["path"])
    file_records.sort(key=lambda item: item["path"])
    return directory_records, file_records


def validate_product_payload_details(
    rootfs: Path,
    metadata: product_metadata.LinuxProductMetadata,
    layout: dict[str, Any],
) -> str:
    paths = layout["paths"]
    addon_metadata = target_in_rootfs(rootfs, paths["addon_metadata"])
    input_method_metadata = target_in_rootfs(
        rootfs, paths["input_method_metadata"]
    )
    desktop_entry = target_in_rootfs(rootfs, paths["desktop_entry"])
    if addon_metadata.read_text(encoding="utf-8") != (
        product_metadata.expected_addon_metadata(metadata)
    ):
        raise LinuxRootfsError("rootfs addon metadata differs from product version")
    if input_method_metadata.read_text(encoding="utf-8") != (
        product_metadata.expected_input_method_metadata()
    ):
        raise LinuxRootfsError("rootfs input-method metadata differs from contract")
    if desktop_entry.read_text(encoding="utf-8") != (
        product_metadata.expected_desktop_entry(metadata, layout)
    ):
        raise LinuxRootfsError("rootfs desktop entry differs from contract")
    rime_root = target_in_rootfs(rootfs, paths["rime_data_root"])
    try:
        product_metadata.product_data.verify_assembly(
            rime_root,
            REPO_ROOT / "packaging/rime/product-rime-data.json",
            REPO_ROOT / "packaging/rime",
        )
    except product_metadata.product_data.ProductDataError as exc:
        raise LinuxRootfsError(str(exc)) from exc

    manager_root = target_in_rootfs(rootfs, paths["manager_root"])
    validate_material_font_manifest(manager_root)
    manager_ffi = target_in_rootfs(rootfs, paths["manager_ffi"])
    addon_ffi = target_in_rootfs(rootfs, paths["addon_ffi"])
    manager_hash = sha256(manager_ffi)
    if manager_hash != sha256(addon_ffi):
        raise LinuxRootfsError("rootfs FFI copies differ")
    if (manager_ffi.stat().st_dev, manager_ffi.stat().st_ino) == (
        addon_ffi.stat().st_dev,
        addon_ffi.stat().st_ino,
    ):
        raise LinuxRootfsError("rootfs FFI copies share an inode")
    return manager_hash


def expected_manifest(
    rootfs: Path,
    metadata: product_metadata.LinuxProductMetadata,
    layout: dict[str, Any],
) -> dict[str, Any]:
    directories, files = collect_inventory(rootfs, metadata, layout)
    ffi_hash = validate_product_payload_details(rootfs, metadata, layout)
    return {
        "format_version": metadata.product_manifest_format_version,
        "inventory_scope": "rootfs-payload-excluding-manifest-self-v1",
        "layout_id": layout["layout_id"],
        "manifest_path": layout["paths"]["product_manifest"],
        "owner_model": "package-install-root-v1",
        "product": metadata.manifest_fields(),
        "directories": directories,
        "files": files,
        "ffi_equivalence": {
            "paths": layout["ffi_equivalence"]["paths"],
            "relationship": layout["ffi_equivalence"]["relationship"],
            "sha256": ffi_hash,
        },
    }


def verify(rootfs: Path) -> dict[str, Any]:
    metadata, layout = product_metadata.validate_source_contract()
    rootfs = require_canonical_input(rootfs, "Linux product rootfs", directory=True)
    if file_mode(rootfs) != 0o755:
        raise LinuxRootfsError("Linux product rootfs must use mode 0755")
    manifest_path = target_in_rootfs(rootfs, layout["paths"]["product_manifest"])
    if manifest_path.is_symlink() or not manifest_path.is_file():
        raise LinuxRootfsError("rootfs product manifest must be a regular file")
    if manifest_path.stat().st_nlink != 1 or file_mode(manifest_path) != 0o644:
        raise LinuxRootfsError("rootfs product manifest identity is unsafe")
    try:
        actual = json.loads(manifest_path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise LinuxRootfsError(f"invalid rootfs product manifest: {exc}") from exc
    if not isinstance(actual, dict) or set(actual) != MANIFEST_KEYS:
        raise LinuxRootfsError("rootfs product manifest fields do not match format v1")
    expected = expected_manifest(rootfs, metadata, layout)
    if actual != expected:
        raise LinuxRootfsError("rootfs product manifest does not match payload")
    if manifest_path.read_bytes() != canonical_json_bytes(actual):
        raise LinuxRootfsError("rootfs product manifest is not canonical JSON")
    return actual


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Assemble or verify a RadishLex Linux product rootfs."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    assemble_parser = subparsers.add_parser("assemble")
    assemble_parser.add_argument("--manager-bundle", type=Path, required=True)
    assemble_parser.add_argument("--addon-stage", type=Path, required=True)
    assemble_parser.add_argument("--output", type=Path, required=True)
    addon_parser = subparsers.add_parser("validate-addon-stage")
    addon_parser.add_argument("--addon-stage", type=Path, required=True)
    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("--rootfs", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "assemble":
            assemble(args.manager_bundle, args.addon_stage, args.output)
        elif args.command == "validate-addon-stage":
            metadata, layout = product_metadata.validate_source_contract()
            addon_stage = require_canonical_input(
                args.addon_stage,
                "product-profile addon stage",
                directory=True,
            )
            validate_addon_stage(addon_stage, metadata, layout)
        else:
            verify(args.rootfs)
    except (
        LinuxRootfsError,
        product_metadata.LinuxProductMetadataError,
        product_metadata.product_data.ProductDataError,
    ) as exc:
        raise SystemExit(str(exc)) from exc
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

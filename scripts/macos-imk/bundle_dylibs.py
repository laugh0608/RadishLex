#!/usr/bin/env python3
"""Bundle and verify non-system Mach-O dylib dependencies for macOS IMK."""

from __future__ import annotations

import argparse
import hashlib
import plistlib
import shutil
import subprocess
from pathlib import Path


SYSTEM_PREFIXES = ("/System/", "/usr/lib/")
LICENSE_PATTERNS = ("LICENSE*", "COPYING*", "NOTICE*")


def run(*args: str) -> str:
    return subprocess.run(args, check=True, text=True, capture_output=True).stdout


def install_id(binary: Path) -> str | None:
    lines = [line.strip() for line in run("otool", "-D", str(binary)).splitlines()[1:]]
    return lines[0] if lines else None


def dependencies(binary: Path) -> list[str]:
    own_id = install_id(binary)
    result: list[str] = []
    for line in run("otool", "-L", str(binary)).splitlines()[1:]:
        value = line.strip().split(" (compatibility version", 1)[0]
        if value and value != own_id:
            result.append(value)
    return result


def is_system_dependency(value: str) -> bool:
    return value.startswith(SYSTEM_PREFIXES)


def resolve_dependency(value: str, binary: Path, search_dirs: list[Path]) -> Path:
    if value.startswith("/"):
        candidate = Path(value)
    elif value.startswith("@loader_path/"):
        candidate = binary.parent / value.removeprefix("@loader_path/")
    elif value.startswith("@rpath/"):
        name = value.removeprefix("@rpath/")
        candidates = [binary.parent / name, *(directory / name for directory in search_dirs)]
        candidate = next((path for path in candidates if path.is_file()), Path())
    else:
        raise RuntimeError(f"unsupported dependency path: {value}")
    if not candidate.is_file():
        raise RuntimeError(f"dependency does not exist: {value}")
    return candidate.resolve()


def license_files(source: Path) -> tuple[str, list[Path]]:
    current = source.resolve().parent
    for _ in range(5):
        matches = sorted(
            {
                item
                for pattern in LICENSE_PATTERNS
                for item in current.glob(pattern)
                if item.is_file()
            }
        )
        if matches:
            package = current.parent.name if current.parent.name else current.name
            return package, matches
        if current == current.parent:
            break
        current = current.parent
    raise RuntimeError(f"no adjacent license found for native dependency: {source.name}")


def copy_licenses(source: Path, destination: Path) -> None:
    package, sources = license_files(source)
    destination.mkdir(parents=True, exist_ok=True)
    for item in sources:
        target = destination / f"{package}-{item.name}"
        if target.exists() and target.read_bytes() != item.read_bytes():
            raise RuntimeError(f"license filename collision: {target.name}")
        shutil.copy2(item, target)


def bundle(root_binary: Path, frameworks: Path, licenses: Path, search_dirs: list[Path]) -> None:
    frameworks.mkdir(parents=True, exist_ok=True)
    queue = [root_binary.resolve()]
    source_by_name: dict[str, Path] = {root_binary.name: root_binary.resolve()}
    destination_by_source: dict[Path, Path] = {root_binary.resolve(): root_binary.resolve()}
    processed: set[Path] = set()

    while queue:
        source = queue.pop(0)
        destination = destination_by_source[source]
        if source in processed:
            continue
        processed.add(source)

        for value in dependencies(destination):
            if is_system_dependency(value):
                continue
            dependency_source = resolve_dependency(value, source, search_dirs)
            name = dependency_source.name
            existing = source_by_name.get(name)
            if existing is not None and existing != dependency_source:
                if existing.read_bytes() != dependency_source.read_bytes():
                    raise RuntimeError(f"native dylib basename collision: {name}")
                dependency_source = existing
            else:
                source_by_name[name] = dependency_source

            dependency_destination = frameworks / name
            if dependency_source not in destination_by_source:
                shutil.copy2(dependency_source, dependency_destination)
                destination_by_source[dependency_source] = dependency_destination
                copy_licenses(dependency_source, licenses)
                queue.append(dependency_source)

            subprocess.run(
                ["install_name_tool", "-change", value, f"@rpath/{name}", str(destination)],
                check=True,
            )

        if destination != root_binary.resolve():
            subprocess.run(
                ["install_name_tool", "-id", f"@rpath/{destination.name}", str(destination)],
                check=True,
            )


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def records(directory: Path, pattern: str) -> list[dict[str, str]]:
    return [
        {"file": path.name, "sha256": sha256(path)}
        for path in sorted(directory.glob(pattern))
        if path.is_file()
    ]


def create_manifest(frameworks: Path, licenses: Path, output: Path) -> None:
    payload = {
        "format_version": 1,
        "libraries": records(frameworks, "*.dylib"),
        "licenses": records(licenses, "*"),
    }
    if not payload["libraries"] or not payload["licenses"]:
        raise RuntimeError("native library manifest requires libraries and licenses")
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.open("wb") as stream:
        plistlib.dump(payload, stream, sort_keys=True)


def verify_records(directory: Path, pattern: str, expected: list[dict[str, str]]) -> None:
    actual = records(directory, pattern)
    if actual != expected:
        raise RuntimeError(f"manifest mismatch for {directory}")


def verify(frameworks: Path, licenses: Path, manifest: Path) -> None:
    with manifest.open("rb") as stream:
        payload = plistlib.load(stream)
    if payload.get("format_version") != 1:
        raise RuntimeError("unsupported native library manifest version")
    verify_records(frameworks, "*.dylib", payload.get("libraries", []))
    verify_records(licenses, "*", payload.get("licenses", []))

    names = {path.name for path in frameworks.glob("*.dylib") if path.is_file()}
    for binary in sorted(frameworks.glob("*.dylib")):
        for value in dependencies(binary):
            if is_system_dependency(value):
                continue
            if not value.startswith("@rpath/"):
                raise RuntimeError(f"external native dependency remains in {binary.name}: {value}")
            if value.removeprefix("@rpath/") not in names:
                raise RuntimeError(f"unresolved bundled dependency in {binary.name}: {value}")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    subparsers = result.add_subparsers(dest="command", required=True)

    bundle_parser = subparsers.add_parser("bundle")
    bundle_parser.add_argument("--root-binary", type=Path, required=True)
    bundle_parser.add_argument("--frameworks-dir", type=Path, required=True)
    bundle_parser.add_argument("--licenses-dir", type=Path, required=True)
    bundle_parser.add_argument("--search-dir", action="append", type=Path, default=[])

    manifest_parser = subparsers.add_parser("manifest")
    manifest_parser.add_argument("--frameworks-dir", type=Path, required=True)
    manifest_parser.add_argument("--licenses-dir", type=Path, required=True)
    manifest_parser.add_argument("--output", type=Path, required=True)

    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("--frameworks-dir", type=Path, required=True)
    verify_parser.add_argument("--licenses-dir", type=Path, required=True)
    verify_parser.add_argument("--manifest", type=Path, required=True)
    return result


def main() -> None:
    args = parser().parse_args()
    if args.command == "bundle":
        bundle(args.root_binary, args.frameworks_dir, args.licenses_dir, args.search_dir)
    elif args.command == "manifest":
        create_manifest(args.frameworks_dir, args.licenses_dir, args.output)
    else:
        verify(args.frameworks_dir, args.licenses_dir, args.manifest)


if __name__ == "__main__":
    main()

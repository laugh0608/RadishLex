#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import plistlib
from pathlib import Path
from typing import Any


FORMAT_VERSION = 2


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def data_hashes(data_dir: Path) -> dict[str, str]:
    hashes: dict[str, str] = {}
    for path in sorted(data_dir.rglob("*")):
        if path.is_symlink():
            raise SystemExit(f"Rime data must not contain symlinks: {path.relative_to(data_dir)}")
        if path.is_file():
            relative = path.relative_to(data_dir).as_posix()
            hashes[relative] = sha256(path)
    if not hashes:
        raise SystemExit("Rime data directory contains no files")
    return hashes


def license_hashes(data_dir: Path) -> dict[str, str]:
    licenses_dir = data_dir / "Licenses"
    if licenses_dir.is_symlink() or not licenses_dir.is_dir():
        raise SystemExit("Rime data must contain a real Licenses directory")
    hashes: dict[str, str] = {}
    for path in sorted(licenses_dir.rglob("*")):
        if path.is_file() and not path.is_symlink():
            if path.stat().st_size == 0:
                raise SystemExit(
                    f"Rime data license file must not be empty: {path.relative_to(data_dir)}"
                )
            hashes[path.relative_to(data_dir).as_posix()] = sha256(path)
    if not hashes or not any(path.endswith("/LICENSE") for path in hashes):
        raise SystemExit("Rime data must contain at least one component LICENSE")
    if not any(path.endswith("/AUTHORS") for path in hashes):
        raise SystemExit("Rime data must contain component AUTHORS attribution")
    return hashes


def expected_manifest(
    data_dir: Path, schema: str, deploy_on_start: bool
) -> dict[str, Any]:
    if data_dir.is_symlink() or not data_dir.is_dir():
        raise SystemExit(f"Rime data directory does not exist: {data_dir}")
    for required in ("default.yaml", f"{schema}.schema.yaml", "SourceManifest.json"):
        if not (data_dir / required).is_file() or (data_dir / required).stat().st_size == 0:
            raise SystemExit(f"Rime data is missing required file: {required}")
    hashes = data_hashes(data_dir)
    return {
        "format_version": FORMAT_VERSION,
        "schema_id": schema,
        "deploy_on_start": deploy_on_start,
        "source_manifest_sha256": hashes["SourceManifest.json"],
        "licenses": license_hashes(data_dir),
        "files": hashes,
    }


def create_manifest(args: argparse.Namespace) -> int:
    manifest = expected_manifest(
        args.data_dir, args.schema, args.deploy_on_start
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("wb") as destination:
        plistlib.dump(manifest, destination, fmt=plistlib.FMT_XML, sort_keys=True)
    return 0


def verify_manifest(args: argparse.Namespace) -> int:
    with args.manifest.open("rb") as source:
        actual = plistlib.load(source)
    expected = expected_manifest(
        args.data_dir, args.schema, args.deploy_on_start
    )
    if actual != expected:
        raise SystemExit("Rime data manifest does not match the bundled files")
    return 0


def parse_bool(value: str) -> bool:
    if value == "1":
        return True
    if value == "0":
        return False
    raise argparse.ArgumentTypeError("deploy-on-start must be 0 or 1")


def add_common_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--data-dir", type=Path, required=True)
    parser.add_argument("--schema", required=True)
    parser.add_argument("--deploy-on-start", type=parse_bool, required=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Create or verify a native Rime data manifest.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    create = subparsers.add_parser("create")
    add_common_arguments(create)
    create.add_argument("--output", type=Path, required=True)
    create.set_defaults(handler=create_manifest)

    verify = subparsers.add_parser("verify")
    add_common_arguments(verify)
    verify.add_argument("--manifest", type=Path, required=True)
    verify.set_defaults(handler=verify_manifest)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    return int(args.handler(args))


if __name__ == "__main__":
    raise SystemExit(main())

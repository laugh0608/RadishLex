#!/usr/bin/env python3
"""Create and verify fail-closed evidence for an unnotarized community DMG."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import product_manifest

FORMAT_VERSION = 1
EVIDENCE_NAME = "CommunityReleaseEvidence.json"


class CommunityReleaseError(RuntimeError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def regular_file(path: Path, label: str) -> None:
    if (
        not path.is_absolute()
        or not path.is_file()
        or path.is_symlink()
        or path.stat().st_nlink != 1
        or path.stat().st_size <= 0
    ):
        raise CommunityReleaseError(f"{label} is unsafe")


def expected(carrier: Path, identity: Path) -> dict[str, object]:
    regular_file(carrier, "community DMG")
    regular_file(identity, "release identity")
    metadata = product_manifest.ProductMetadata.load()
    if metadata.distribution_identity != "community-adhoc-v1":
        raise CommunityReleaseError("product is not configured for community ad-hoc")
    expected_name = (
        f"RadishLex-{metadata.product_version}-{metadata.build_number}.dmg"
    )
    if carrier.name != expected_name:
        raise CommunityReleaseError("community DMG filename differs from product version")
    return {
        "format_version": FORMAT_VERSION,
        "distribution_identity": metadata.distribution_identity,
        "product_id": metadata.product_id,
        "product_version": metadata.product_version,
        "build_number": metadata.build_number,
        "carrier_file": carrier.name,
        "carrier_size": carrier.stat().st_size,
        "carrier_sha256": sha256(carrier),
        "release_identity_sha256": sha256(identity),
        "apple_notarized": False,
    }


def encoded(value: dict[str, object]) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def create(arguments: argparse.Namespace) -> None:
    value = expected(arguments.carrier, arguments.identity)
    output = arguments.output
    if (
        not output.is_absolute()
        or output.name != EVIDENCE_NAME
        or output.exists()
        or output.is_symlink()
    ):
        raise CommunityReleaseError("community evidence output is unsafe")
    output.write_bytes(encoded(value))
    output.chmod(0o644)


def verify(arguments: argparse.Namespace) -> None:
    regular_file(arguments.evidence, "community evidence")
    if arguments.evidence.name != EVIDENCE_NAME:
        raise CommunityReleaseError("community evidence filename changed")
    if arguments.evidence.read_bytes() != encoded(
        expected(arguments.carrier, arguments.identity)
    ):
        raise CommunityReleaseError("community release evidence does not match artifacts")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    subparsers = result.add_subparsers(dest="command", required=True)
    for command in ("create", "verify"):
        subparser = subparsers.add_parser(command)
        subparser.add_argument("--carrier", type=Path, required=True)
        subparser.add_argument("--identity", type=Path, required=True)
        if command == "create":
            subparser.add_argument("--output", type=Path, required=True)
        else:
            subparser.add_argument("--evidence", type=Path, required=True)
    return result


def main() -> None:
    arguments = parser().parse_args()
    try:
        if arguments.command == "create":
            create(arguments)
        else:
            verify(arguments)
    except CommunityReleaseError as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()

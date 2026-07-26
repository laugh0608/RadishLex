#!/usr/bin/env python3
"""Create and verify the sealed community ad-hoc product identity resource."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path

import product_manifest

FORMAT_VERSION = 2
DISTRIBUTION_IDENTITY = "community-adhoc-v1"
EXPECTED_BUNDLE_IDS = {
    "installer": "org.radishlex.installer.macos",
    "manager": "dev.radishlex.radishlexManager",
    "input_method": "org.radishlex.inputmethod.macos",
}
MAX_CODESIGN_OUTPUT_BYTES = 64 * 1024
MAX_REQUIREMENT_BYTES = 4096
CDHASH_PATTERN = re.compile(r'[0-9A-Fa-f]{40}|[0-9A-Fa-f]{64}')
TEAM_PATTERN = re.compile(r"^[A-Z0-9]{10}$")


class ReleaseIdentityError(RuntimeError):
    pass


@dataclass(frozen=True)
class SignedBundleIdentity:
    bundle_id: str
    designated_requirement: str


def _unique_prefixed_line(text: str, prefix: str) -> str:
    values = [line[len(prefix) :] for line in text.splitlines() if line.startswith(prefix)]
    if len(values) != 1 or not values[0]:
        raise ReleaseIdentityError(f"codesign output has invalid {prefix.rstrip('=')}")
    return values[0]


def _valid_ad_hoc_requirement(requirement: str, primary_cdhash: str) -> bool:
    clauses = requirement.split(" or ")
    hashes: list[str] = []
    if not clauses or len(clauses) > 16:
        return False
    for clause in clauses:
        if not clause.startswith('cdhash H"') or not clause.endswith('"'):
            return False
        value = clause[len('cdhash H"') : -1]
        if CDHASH_PATTERN.fullmatch(value) is None:
            return False
        hashes.append(value.lower())
    return primary_cdhash.lower() in hashes


def parse_codesign_output(details: bytes, expected_bundle_id: str) -> SignedBundleIdentity:
    if (
        not details
        or len(details) > MAX_CODESIGN_OUTPUT_BYTES
        or b"\0" in details
        or b"\r" in details
    ):
        raise ReleaseIdentityError("codesign output is invalid")
    try:
        text = details.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ReleaseIdentityError("codesign output is not UTF-8") from error
    bundle_id = _unique_prefixed_line(text, "Identifier=")
    team_identifier = _unique_prefixed_line(text, "TeamIdentifier=")
    cdhash = _unique_prefixed_line(text, "CDHash=")
    signature = _unique_prefixed_line(text, "Signature=")
    code_directory = next(
        (line for line in text.splitlines() if line.startswith("CodeDirectory ")),
        "",
    )
    requirement = _unique_prefixed_line(text, "# designated => ")
    if bundle_id != expected_bundle_id:
        raise ReleaseIdentityError("signed bundle identifier changed")
    if (
        team_identifier != "not set"
        or signature != "adhoc"
        or " flags=0x2(adhoc) " not in code_directory
        or len(requirement.encode("utf-8")) > MAX_REQUIREMENT_BYTES
        or not _valid_ad_hoc_requirement(requirement, cdhash)
    ):
        raise ReleaseIdentityError("code identity is not strict community ad-hoc")
    return SignedBundleIdentity(bundle_id, requirement)


def inspect_signed_bundle(path: Path, expected_bundle_id: str) -> SignedBundleIdentity:
    if not path.is_absolute() or not path.is_dir() or path.is_symlink():
        raise ReleaseIdentityError("signed bundle path is unsafe")
    verified = subprocess.run(
        ["/usr/bin/codesign", "--verify", "--deep", "--strict", str(path)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if verified.returncode != 0:
        raise ReleaseIdentityError("strict code signature verification failed")
    inspected = subprocess.run(
        ["/usr/bin/codesign", "-d", "--verbose=4", "-r-", str(path)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if inspected.returncode != 0:
        raise ReleaseIdentityError("code identity inspection failed")
    return parse_codesign_output(
        inspected.stderr + inspected.stdout, expected_bundle_id
    )


def build_identity(
    installer: SignedBundleIdentity,
    managers: list[SignedBundleIdentity],
    input_methods: list[SignedBundleIdentity],
) -> dict[str, object]:
    if installer.bundle_id != EXPECTED_BUNDLE_IDS["installer"]:
        raise ReleaseIdentityError("Installer bundle identity changed")
    manager_requirements = sorted({item.designated_requirement for item in managers})
    input_method_requirements = sorted(
        {item.designated_requirement for item in input_methods}
    )
    if (
        not manager_requirements
        or not input_method_requirements
        or set(manager_requirements) & set(input_method_requirements)
    ):
        raise ReleaseIdentityError("product bundle requirements must differ")
    return {
        "format_version": FORMAT_VERSION,
        "distribution_identity": DISTRIBUTION_IDENTITY,
        "manager_designated_requirements": manager_requirements,
        "input_method_designated_requirements": input_method_requirements,
    }


def inspect_arguments(arguments: argparse.Namespace) -> dict[str, object]:
    metadata = product_manifest.ProductMetadata.load()
    if metadata.distribution_identity != DISTRIBUTION_IDENTITY:
        raise ReleaseIdentityError("product metadata is not community ad-hoc")
    managers = [
        inspect_signed_bundle(arguments.manager_bundle, EXPECTED_BUNDLE_IDS["manager"])
    ]
    input_methods = [
        inspect_signed_bundle(
            arguments.input_method_bundle, EXPECTED_BUNDLE_IDS["input_method"]
        )
    ]
    embedded_product = (
        arguments.installer_bundle
        / "Contents/Resources/InstallPayload/Product/Components"
    )
    embedded_manager = inspect_signed_bundle(
        embedded_product / "radishlex_manager.app",
        EXPECTED_BUNDLE_IDS["manager"],
    )
    embedded_input_method = inspect_signed_bundle(
        embedded_product / "RadishLexInputMethod.app",
        EXPECTED_BUNDLE_IDS["input_method"],
    )
    if embedded_manager != managers[0] or embedded_input_method != input_methods[0]:
        raise ReleaseIdentityError(
            "embedded target code identity differs from frozen product"
        )
    sources = (
        arguments.installer_bundle
        / "Contents/Resources/InstallPayload/UpgradeSources"
    )
    if sources.is_dir() and not sources.is_symlink():
        for source in sorted(sources.iterdir()):
            if not source.is_dir() or source.is_symlink():
                raise ReleaseIdentityError("upgrade source identity root is unsafe")
            managers.append(
                inspect_signed_bundle(
                    source / "Components/radishlex_manager.app",
                    EXPECTED_BUNDLE_IDS["manager"],
                )
            )
            input_methods.append(
                inspect_signed_bundle(
                    source / "Components/RadishLexInputMethod.app",
                    EXPECTED_BUNDLE_IDS["input_method"],
                )
            )
    return build_identity(
        inspect_signed_bundle(
            arguments.installer_bundle, EXPECTED_BUNDLE_IDS["installer"]
        ),
        managers,
        input_methods,
    )


def encoded_identity(identity: dict[str, object]) -> bytes:
    return (
        json.dumps(identity, ensure_ascii=False, indent=2, separators=(",", ": "))
        + "\n"
    ).encode("utf-8")


def create(arguments: argparse.Namespace) -> None:
    identity = inspect_arguments(arguments)
    output = arguments.output
    if not output.is_absolute() or output.exists() or output.is_symlink():
        raise ReleaseIdentityError("release identity output must be a new absolute path")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(encoded_identity(identity))
    output.chmod(0o644)


def verify(arguments: argparse.Namespace) -> None:
    identity = inspect_arguments(arguments)
    resource = arguments.identity
    if (
        not resource.is_absolute()
        or not resource.is_file()
        or resource.is_symlink()
        or resource.stat().st_nlink != 1
    ):
        raise ReleaseIdentityError("release identity resource is unsafe")
    if resource.read_bytes() != encoded_identity(identity):
        raise ReleaseIdentityError("release identity does not match signed bundles")


def verify_upgrade_source(arguments: argparse.Namespace) -> None:
    target_manager = inspect_signed_bundle(
        arguments.target_manager_bundle, EXPECTED_BUNDLE_IDS["manager"]
    )
    target_input_method = inspect_signed_bundle(
        arguments.target_input_method_bundle, EXPECTED_BUNDLE_IDS["input_method"]
    )
    source_manager = inspect_signed_bundle(
        arguments.source_manager_bundle, EXPECTED_BUNDLE_IDS["manager"]
    )
    source_input_method = inspect_signed_bundle(
        arguments.source_input_method_bundle, EXPECTED_BUNDLE_IDS["input_method"]
    )
    if target_manager.bundle_id != source_manager.bundle_id:
        raise ReleaseIdentityError("historical Manager bundle identity changed")
    if target_input_method.bundle_id != source_input_method.bundle_id:
        raise ReleaseIdentityError("historical InputMethod bundle identity changed")


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    subparsers = result.add_subparsers(dest="command", required=True)
    for command in ("create", "verify"):
        subparser = subparsers.add_parser(command)
        subparser.add_argument("--installer-bundle", type=Path, required=True)
        subparser.add_argument("--manager-bundle", type=Path, required=True)
        subparser.add_argument("--input-method-bundle", type=Path, required=True)
        if command == "create":
            subparser.add_argument("--output", type=Path, required=True)
        else:
            subparser.add_argument("--identity", type=Path, required=True)
    source = subparsers.add_parser("verify-upgrade-source")
    source.add_argument("--target-manager-bundle", type=Path, required=True)
    source.add_argument("--target-input-method-bundle", type=Path, required=True)
    source.add_argument("--source-manager-bundle", type=Path, required=True)
    source.add_argument("--source-input-method-bundle", type=Path, required=True)
    return result


def main() -> None:
    arguments = parser().parse_args()
    try:
        if arguments.command == "create":
            create(arguments)
        elif arguments.command == "verify":
            verify(arguments)
        else:
            verify_upgrade_source(arguments)
    except ReleaseIdentityError as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()

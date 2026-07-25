#!/usr/bin/env python3
"""Generate and verify the sealed macOS Developer ID release identity resource."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path

FORMAT_VERSION = 1
EXPECTED_BUNDLE_IDS = {
    "installer": "org.radishlex.installer.macos",
    "manager": "dev.radishlex.radishlexManager",
    "input_method": "org.radishlex.inputmethod.macos",
}
MAX_CODESIGN_OUTPUT_BYTES = 64 * 1024
MAX_REQUIREMENT_BYTES = 4096
TEAM_PATTERN = re.compile(r"^[A-Z0-9]{10}$")


class ReleaseIdentityError(RuntimeError):
    pass


@dataclass(frozen=True)
class SignedBundleIdentity:
    bundle_id: str
    team_identifier: str
    designated_requirement: str


def _unique_prefixed_line(text: str, prefix: str) -> str:
    values = [line[len(prefix) :] for line in text.splitlines() if line.startswith(prefix)]
    if len(values) != 1 or not values[0]:
        raise ReleaseIdentityError(f"codesign output has invalid {prefix.rstrip('=')}")
    return values[0]


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
    requirement = _unique_prefixed_line(text, "# designated => ")
    if bundle_id != expected_bundle_id:
        raise ReleaseIdentityError("signed bundle identifier changed")
    if not TEAM_PATTERN.fullmatch(team_identifier):
        raise ReleaseIdentityError("Developer ID TeamIdentifier is unavailable")
    if (
        len(requirement.encode("utf-8")) > MAX_REQUIREMENT_BYTES
        or not requirement.startswith(
            f'identifier "{expected_bundle_id}" and anchor apple generic and '
        )
        or "certificate 1[field.1.2.840.113635.100.6.2.6]" not in requirement
        or "certificate leaf[field.1.2.840.113635.100.6.1.13]" not in requirement
        or not (
            f"certificate leaf[subject.OU] = {team_identifier}" in requirement
            or f'certificate leaf[subject.OU] = "{team_identifier}"' in requirement
        )
    ):
        raise ReleaseIdentityError("designated requirement is not Developer ID Application")
    return SignedBundleIdentity(bundle_id, team_identifier, requirement)


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
    details = inspected.stderr + inspected.stdout
    if inspected.returncode != 0:
        raise ReleaseIdentityError("code identity inspection failed")
    return parse_codesign_output(details, expected_bundle_id)


def build_identity(
    installer: SignedBundleIdentity,
    manager: SignedBundleIdentity,
    input_method: SignedBundleIdentity,
) -> dict[str, object]:
    teams = {
        installer.team_identifier,
        manager.team_identifier,
        input_method.team_identifier,
    }
    if len(teams) != 1:
        raise ReleaseIdentityError("release bundles use different TeamIdentifier values")
    return {
        "format_version": FORMAT_VERSION,
        "team_identifier": installer.team_identifier,
        "installer_designated_requirement": installer.designated_requirement,
        "manager_designated_requirement": manager.designated_requirement,
        "input_method_designated_requirement": input_method.designated_requirement,
    }


def verify_upgrade_source_identity(
    target_manager: SignedBundleIdentity,
    target_input_method: SignedBundleIdentity,
    source_manager: SignedBundleIdentity,
    source_input_method: SignedBundleIdentity,
) -> None:
    if (
        target_manager != source_manager
        or target_input_method != source_input_method
        or target_manager.team_identifier != target_input_method.team_identifier
    ):
        raise ReleaseIdentityError(
            "historical source code identity differs from target release"
        )


def inspect_arguments(arguments: argparse.Namespace) -> dict[str, object]:
    return build_identity(
        inspect_signed_bundle(
            arguments.installer_bundle, EXPECTED_BUNDLE_IDS["installer"]
        ),
        inspect_signed_bundle(
            arguments.manager_bundle, EXPECTED_BUNDLE_IDS["manager"]
        ),
        inspect_signed_bundle(
            arguments.input_method_bundle,
            EXPECTED_BUNDLE_IDS["input_method"],
        ),
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
    verify_upgrade_source_identity(
        inspect_signed_bundle(
            arguments.target_manager_bundle, EXPECTED_BUNDLE_IDS["manager"]
        ),
        inspect_signed_bundle(
            arguments.target_input_method_bundle,
            EXPECTED_BUNDLE_IDS["input_method"],
        ),
        inspect_signed_bundle(
            arguments.source_manager_bundle, EXPECTED_BUNDLE_IDS["manager"]
        ),
        inspect_signed_bundle(
            arguments.source_input_method_bundle,
            EXPECTED_BUNDLE_IDS["input_method"],
        ),
    )


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

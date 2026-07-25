#!/usr/bin/env python3
"""Bind macOS DMG notarization and Gatekeeper evidence to one frozen carrier."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import uuid
from pathlib import Path
from typing import Any

import product_manifest
import release_identity


FORMAT_VERSION = 1
LEGACY_DEVELOPER_TEAM_ID = "WF9UUN335P"
MAX_JSON_BYTES = 1024 * 1024
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
EXPECTED_SUBMISSION_KEYS = {"id", "message", "status"}
EXPECTED_NOTARY_LOG_KEYS = {
    "logFormatVersion",
    "jobId",
    "status",
    "statusSummary",
    "statusCode",
    "archiveFilename",
    "uploadDate",
    "sha256",
    "ticketContents",
    "issues",
}
EXPECTED_RECEIPT_KEYS = {
    "format_version",
    "product_id",
    "product_version",
    "build_number",
    "carrier_file_name",
    "submitted_sha256",
    "submitted_size",
    "installer_tree_sha256",
    "submission_id",
    "notary_status",
}
EXPECTED_QUALIFICATION_KEYS = {
    "format_version",
    "product_id",
    "product_version",
    "build_number",
    "carrier_file_name",
    "submitted_sha256",
    "distribution_sha256",
    "distribution_size",
    "installer_tree_sha256",
    "submission_id",
    "notary_status",
    "notary_log_status_code",
    "staple",
    "gatekeeper_dmg",
    "gatekeeper_installer",
}
EXPECTED_RELEASE_IDENTITY_KEYS = {
    "format_version",
    "team_identifier",
    "installer_designated_requirement",
    "manager_designated_requirement",
    "input_method_designated_requirement",
}


class ReleaseCarrierError(RuntimeError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_regular_file(path: Path, label: str) -> None:
    if (
        not path.is_absolute()
        or not path.is_file()
        or path.is_symlink()
        or path.stat().st_nlink != 1
    ):
        raise ReleaseCarrierError(f"{label} is unsafe")


def safe_bundle(path: Path, label: str) -> None:
    if not path.is_absolute() or not path.is_dir() or path.is_symlink():
        raise ReleaseCarrierError(f"{label} is unsafe")


def read_json_bytes(raw: bytes, label: str) -> dict[str, Any]:
    if not raw or len(raw) > MAX_JSON_BYTES or b"\0" in raw or b"\r" in raw:
        raise ReleaseCarrierError(f"{label} is invalid")
    try:
        value = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReleaseCarrierError(f"{label} is invalid") from error
    if not isinstance(value, dict):
        raise ReleaseCarrierError(f"{label} must contain an object")
    return value


def read_json_file(path: Path, label: str) -> dict[str, Any]:
    safe_regular_file(path, label)
    return read_json_bytes(path.read_bytes(), label)


def normalized_uuid(value: object, label: str) -> str:
    if not isinstance(value, str):
        raise ReleaseCarrierError(f"{label} is invalid")
    try:
        parsed = uuid.UUID(value)
    except ValueError as error:
        raise ReleaseCarrierError(f"{label} is invalid") from error
    normalized = str(parsed)
    if value != normalized:
        raise ReleaseCarrierError(f"{label} is not canonical")
    return normalized


def installer_tree_sha256(bundle: Path) -> str:
    safe_bundle(bundle, "Installer bundle")
    try:
        records = product_manifest.file_records(bundle, "Installer bundle")
    except product_manifest.ProductManifestError as error:
        raise ReleaseCarrierError(str(error)) from error
    encoded = json.dumps(
        records, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def parse_codesign_team(raw: bytes) -> str:
    if (
        not raw
        or len(raw) > release_identity.MAX_CODESIGN_OUTPUT_BYTES
        or b"\0" in raw
        or b"\r" in raw
    ):
        raise ReleaseCarrierError("DMG code identity output is invalid")
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ReleaseCarrierError("DMG code identity output is not UTF-8") from error
    values = [
        line[len("TeamIdentifier=") :]
        for line in text.splitlines()
        if line.startswith("TeamIdentifier=")
    ]
    if len(values) != 1 or not release_identity.TEAM_PATTERN.fullmatch(values[0]):
        raise ReleaseCarrierError("DMG Developer ID TeamIdentifier is unavailable")
    return values[0]


def release_identity_team(path: Path) -> str:
    value = read_json_file(path, "release identity")
    if set(value) != EXPECTED_RELEASE_IDENTITY_KEYS or value["format_version"] != 1:
        raise ReleaseCarrierError("release identity fields changed")
    team = value["team_identifier"]
    expected_team = LEGACY_DEVELOPER_TEAM_ID
    if (
        not isinstance(team, str)
        or not release_identity.TEAM_PATTERN.fullmatch(team)
        or team != expected_team
    ):
        raise ReleaseCarrierError("release identity TeamIdentifier is invalid")
    return team


def verify_carrier_team(carrier: Path, identity: Path) -> None:
    safe_regular_file(carrier, "release carrier")
    inspected = subprocess.run(
        ["/usr/bin/codesign", "-d", "--verbose=4", str(carrier)],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if inspected.returncode != 0:
        raise ReleaseCarrierError("DMG code identity inspection failed")
    if parse_codesign_team(inspected.stderr + inspected.stdout) != release_identity_team(
        identity
    ):
        raise ReleaseCarrierError("DMG and Installer use different TeamIdentifier values")


def load_metadata() -> product_manifest.ProductMetadata:
    try:
        return product_manifest.ProductMetadata.load()
    except product_manifest.ProductManifestError as error:
        raise ReleaseCarrierError(str(error)) from error


def expected_carrier_name(metadata: product_manifest.ProductMetadata) -> str:
    return f"RadishLex-{metadata.product_version}-{metadata.build_number}.dmg"


def validate_carrier(path: Path, metadata: product_manifest.ProductMetadata) -> None:
    safe_regular_file(path, "release carrier")
    if path.name != expected_carrier_name(metadata):
        raise ReleaseCarrierError("release carrier file name changed")
    if path.stat().st_size <= 0:
        raise ReleaseCarrierError("release carrier is empty")


def build_submission_receipt(
    raw: bytes, carrier: Path, installer: Path
) -> dict[str, Any]:
    metadata = load_metadata()
    validate_carrier(carrier, metadata)
    response = read_json_bytes(raw, "notary submission result")
    if set(response) != EXPECTED_SUBMISSION_KEYS:
        raise ReleaseCarrierError("notary submission result fields changed")
    if response["status"] != "Accepted":
        raise ReleaseCarrierError("notary submission was not accepted")
    if not isinstance(response["message"], str) or not response["message"]:
        raise ReleaseCarrierError("notary submission message is invalid")
    submission_id = normalized_uuid(response["id"], "notary submission ID")
    return {
        "format_version": FORMAT_VERSION,
        "product_id": metadata.product_id,
        "product_version": metadata.product_version,
        "build_number": metadata.build_number,
        "carrier_file_name": carrier.name,
        "submitted_sha256": sha256(carrier),
        "submitted_size": carrier.stat().st_size,
        "installer_tree_sha256": installer_tree_sha256(installer),
        "submission_id": submission_id,
        "notary_status": "accepted",
    }


def encoded_json(value: dict[str, Any]) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def write_new(path: Path, value: dict[str, Any]) -> None:
    if not path.is_absolute() or path.exists() or path.is_symlink():
        raise ReleaseCarrierError("evidence output must be a new absolute path")
    if not path.parent.is_dir() or path.parent.is_symlink():
        raise ReleaseCarrierError("evidence output parent is unsafe")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    descriptor = os.open(path, flags, 0o644)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(encoded_json(value))
            stream.flush()
            os.fsync(stream.fileno())
    except Exception:
        path.unlink(missing_ok=True)
        raise


def validate_receipt(value: dict[str, Any]) -> dict[str, Any]:
    metadata = load_metadata()
    if set(value) != EXPECTED_RECEIPT_KEYS:
        raise ReleaseCarrierError("notary submission receipt fields changed")
    expected = {
        "format_version": FORMAT_VERSION,
        "product_id": metadata.product_id,
        "product_version": metadata.product_version,
        "build_number": metadata.build_number,
        "carrier_file_name": expected_carrier_name(metadata),
        "notary_status": "accepted",
    }
    if any(value.get(key) != expected_value for key, expected_value in expected.items()):
        raise ReleaseCarrierError("notary submission receipt metadata changed")
    normalized_uuid(value["submission_id"], "notary submission ID")
    for key in ("submitted_sha256", "installer_tree_sha256"):
        if not isinstance(value[key], str) or not SHA256_PATTERN.fullmatch(value[key]):
            raise ReleaseCarrierError(f"{key} is invalid")
    if (
        isinstance(value["submitted_size"], bool)
        or not isinstance(value["submitted_size"], int)
        or value["submitted_size"] <= 0
    ):
        raise ReleaseCarrierError("submitted_size is invalid")
    return value


def validate_notary_log(raw: bytes, receipt: dict[str, Any]) -> int:
    value = read_json_bytes(raw, "notary log")
    if set(value) != EXPECTED_NOTARY_LOG_KEYS or value["logFormatVersion"] != 1:
        raise ReleaseCarrierError("notary log fields changed")
    if normalized_uuid(value["jobId"], "notary log job ID") != receipt["submission_id"]:
        raise ReleaseCarrierError("notary log job ID changed")
    if (
        value["status"] != "Accepted"
        or isinstance(value["statusCode"], bool)
        or value["statusCode"] != 0
    ):
        raise ReleaseCarrierError("notary log did not accept the carrier")
    issues = value["issues"]
    if issues not in (None, []):
        raise ReleaseCarrierError("notary log contains issues")
    log_hash = value["sha256"]
    if not isinstance(log_hash, str) or log_hash.lower() != receipt["submitted_sha256"]:
        raise ReleaseCarrierError("notary log carrier hash changed")
    if value["archiveFilename"] != receipt["carrier_file_name"]:
        raise ReleaseCarrierError("notary log carrier file name changed")
    if not isinstance(value["statusSummary"], str) or not value["statusSummary"]:
        raise ReleaseCarrierError("notary log status summary is invalid")
    if not isinstance(value["uploadDate"], str) or not value["uploadDate"]:
        raise ReleaseCarrierError("notary log upload date is invalid")
    if not isinstance(value["ticketContents"], list):
        raise ReleaseCarrierError("notary log ticket contents are invalid")
    return 0


def build_qualification(
    receipt: dict[str, Any], carrier: Path, installer: Path
) -> dict[str, Any]:
    metadata = load_metadata()
    validate_carrier(carrier, metadata)
    receipt = validate_receipt(receipt)
    tree_hash = installer_tree_sha256(installer)
    if tree_hash != receipt["installer_tree_sha256"]:
        raise ReleaseCarrierError("Installer bundle changed after notarization submission")
    return {
        "format_version": FORMAT_VERSION,
        "product_id": metadata.product_id,
        "product_version": metadata.product_version,
        "build_number": metadata.build_number,
        "carrier_file_name": carrier.name,
        "submitted_sha256": receipt["submitted_sha256"],
        "distribution_sha256": sha256(carrier),
        "distribution_size": carrier.stat().st_size,
        "installer_tree_sha256": tree_hash,
        "submission_id": receipt["submission_id"],
        "notary_status": "accepted",
        "notary_log_status_code": 0,
        "staple": "validated",
        "gatekeeper_dmg": "accepted",
        "gatekeeper_installer": "accepted",
    }


def verify_submission_artifacts(
    receipt: dict[str, Any], carrier: Path, installer: Path
) -> None:
    metadata = load_metadata()
    validate_carrier(carrier, metadata)
    receipt = validate_receipt(receipt)
    if (
        sha256(carrier) != receipt["submitted_sha256"]
        or carrier.stat().st_size != receipt["submitted_size"]
    ):
        raise ReleaseCarrierError("submitted release carrier changed before stapling")
    if installer_tree_sha256(installer) != receipt["installer_tree_sha256"]:
        raise ReleaseCarrierError("Installer bundle changed after notarization submission")


def verify_qualification(
    qualification: dict[str, Any],
    receipt: dict[str, Any],
    carrier: Path,
    installer: Path,
) -> None:
    if set(qualification) != EXPECTED_QUALIFICATION_KEYS:
        raise ReleaseCarrierError("release qualification fields changed")
    expected = build_qualification(receipt, carrier, installer)
    if qualification != expected:
        raise ReleaseCarrierError("release qualification does not match frozen artifacts")


def command_parse_submission(arguments: argparse.Namespace) -> None:
    safe_regular_file(arguments.input, "notary submission result")
    receipt = build_submission_receipt(
        arguments.input.read_bytes(), arguments.carrier, arguments.installer_bundle
    )
    write_new(arguments.output, receipt)


def command_print_submission_id(arguments: argparse.Namespace) -> None:
    receipt = validate_receipt(
        read_json_file(arguments.receipt, "notary submission receipt")
    )
    print(receipt["submission_id"])


def command_verify_log(arguments: argparse.Namespace) -> None:
    receipt = validate_receipt(
        read_json_file(arguments.receipt, "notary submission receipt")
    )
    safe_regular_file(arguments.log, "notary log")
    validate_notary_log(arguments.log.read_bytes(), receipt)


def command_verify_installer(arguments: argparse.Namespace) -> None:
    expected = installer_tree_sha256(arguments.expected_bundle)
    actual = installer_tree_sha256(arguments.actual_bundle)
    if actual != expected:
        raise ReleaseCarrierError("mounted Installer differs from the frozen bundle")


def command_verify_submission_artifacts(arguments: argparse.Namespace) -> None:
    receipt = validate_receipt(
        read_json_file(arguments.receipt, "notary submission receipt")
    )
    verify_submission_artifacts(
        receipt, arguments.carrier, arguments.installer_bundle
    )


def command_verify_carrier_team(arguments: argparse.Namespace) -> None:
    verify_carrier_team(arguments.carrier, arguments.identity)


def command_create_qualification(arguments: argparse.Namespace) -> None:
    receipt = validate_receipt(
        read_json_file(arguments.receipt, "notary submission receipt")
    )
    qualification = build_qualification(
        receipt, arguments.carrier, arguments.installer_bundle
    )
    write_new(arguments.output, qualification)


def command_verify_qualification(arguments: argparse.Namespace) -> None:
    receipt = validate_receipt(
        read_json_file(arguments.receipt, "notary submission receipt")
    )
    qualification = read_json_file(arguments.qualification, "release qualification")
    verify_qualification(
        qualification, receipt, arguments.carrier, arguments.installer_bundle
    )


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    subparsers = result.add_subparsers(dest="command", required=True)

    parse_submission = subparsers.add_parser("parse-submission")
    parse_submission.add_argument("--input", type=Path, required=True)
    parse_submission.add_argument("--carrier", type=Path, required=True)
    parse_submission.add_argument("--installer-bundle", type=Path, required=True)
    parse_submission.add_argument("--output", type=Path, required=True)
    parse_submission.set_defaults(handler=command_parse_submission)

    print_submission = subparsers.add_parser("print-submission-id")
    print_submission.add_argument("--receipt", type=Path, required=True)
    print_submission.set_defaults(handler=command_print_submission_id)

    verify_log = subparsers.add_parser("verify-log")
    verify_log.add_argument("--receipt", type=Path, required=True)
    verify_log.add_argument("--log", type=Path, required=True)
    verify_log.set_defaults(handler=command_verify_log)

    verify_installer = subparsers.add_parser("verify-installer")
    verify_installer.add_argument("--expected-bundle", type=Path, required=True)
    verify_installer.add_argument("--actual-bundle", type=Path, required=True)
    verify_installer.set_defaults(handler=command_verify_installer)

    verify_submission = subparsers.add_parser("verify-submission-artifacts")
    verify_submission.add_argument("--receipt", type=Path, required=True)
    verify_submission.add_argument("--carrier", type=Path, required=True)
    verify_submission.add_argument("--installer-bundle", type=Path, required=True)
    verify_submission.set_defaults(handler=command_verify_submission_artifacts)

    verify_team = subparsers.add_parser("verify-carrier-team")
    verify_team.add_argument("--carrier", type=Path, required=True)
    verify_team.add_argument("--identity", type=Path, required=True)
    verify_team.set_defaults(handler=command_verify_carrier_team)

    for command, handler in (
        ("create-qualification", command_create_qualification),
        ("verify-qualification", command_verify_qualification),
    ):
        qualification = subparsers.add_parser(command)
        qualification.add_argument("--receipt", type=Path, required=True)
        qualification.add_argument("--carrier", type=Path, required=True)
        qualification.add_argument("--installer-bundle", type=Path, required=True)
        if command == "create-qualification":
            qualification.add_argument("--output", type=Path, required=True)
        else:
            qualification.add_argument("--qualification", type=Path, required=True)
        qualification.set_defaults(handler=handler)
    return result


def main() -> None:
    arguments = parser().parse_args()
    try:
        arguments.handler(arguments)
    except ReleaseCarrierError as error:
        raise SystemExit(str(error)) from error


if __name__ == "__main__":
    main()

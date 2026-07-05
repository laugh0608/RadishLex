#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


REPO_ROOT = Path(__file__).resolve().parents[1]
VALID_FIXTURE = REPO_ROOT / "tests" / "fixtures" / "sync-deployment-evidence-valid.txt"

HEADER = "deployment_evidence.v1"
SUMMARY_HEADER = "deployment_evidence_summary.v1"
REQUIRED_FIELDS = [
    "reviewed_at",
    "target_alias",
    "git_commit",
    "image_tag",
    "compose_file",
    "public_url_status",
    "access_token_status",
    "access_control",
    "external_tls",
    "backup_restore",
    "upgrade_rollback",
    "log_redaction",
    "notes",
]
ALLOWED_VALUES = {
    "public_url_status": {"configured", "not_configured"},
    "access_token_status": {"configured_and_401_verified", "missing", "failed"},
    "access_control": {"passed", "failed", "not_run"},
    "external_tls": {"passed", "failed", "not_run"},
    "backup_restore": {"passed", "failed", "not_run"},
    "upgrade_rollback": {"passed", "failed", "not_run"},
    "log_redaction": {"passed", "failed", "not_run"},
}
ALLOWED_COMPOSE_FILE = "deploy/sync-server/docker-compose.yaml"
SUMMARY_STATUS_FIELDS = [
    "public_url_status",
    "access_token_status",
    "access_control",
    "external_tls",
    "backup_restore",
    "upgrade_rollback",
    "log_redaction",
]
DEPLOYMENT_EVIDENCE_SOURCE_FIELDS = [
    "external_tls",
    "backup_restore",
    "upgrade_rollback",
]
ABSOLUTE_PATH_PATTERN = re.compile(r"(^|\s)(/Users/|/home/|/private/|/var/|/etc/|[A-Za-z]:\\)")
GIT_COMMIT_PATTERN = re.compile(r"^[0-9a-f]{7,40}$")
SAFE_ALIAS_PATTERN = re.compile(r"^[A-Za-z0-9._-]{3,64}$")
SAFE_IMAGE_TAG_PATTERN = re.compile(r"^[A-Za-z0-9._:/-]{3,128}$")
REVIEWED_AT_PATTERN = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\+08:00|Z)$")
FORBIDDEN_MARKERS = [
    "Authorization:",
    "Bearer ",
    "RADISHLEX_SYNC_ACCESS_TOKEN=",
    "access_token=",
    "token=",
    "password=",
    "secret=",
    "BEGIN CERTIFICATE",
    "BEGIN PRIVATE KEY",
    "BEGIN RSA PRIVATE KEY",
    "BEGIN EC PRIVATE KEY",
    "request body",
    "response body",
    "HTTP/1.",
    "curl -i",
    "encrypted_payload",
    "payload bytes",
    "signature bytes",
    "wrapped material",
    "recovery material",
    "recovery code",
    "SyncMasterKey",
    "private key",
]


@dataclass(frozen=True)
class ValidationResult:
    path: str
    errors: list[str]

    @property
    def ok(self) -> bool:
        return not self.errors


class EvidenceValidationError(RuntimeError):
    pass


def parse_evidence(content: str) -> tuple[str, dict[str, str], list[str]]:
    errors: list[str] = []
    lines = content.splitlines()
    if not lines:
        return "", {}, ["evidence file is empty"]

    header = lines[0].strip()
    if header != HEADER:
        errors.append(f"first line must be {HEADER!r}")

    fields: dict[str, str] = {}
    for line_number, raw_line in enumerate(lines[1:], start=2):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if ":" not in line:
            errors.append(f"line {line_number}: expected key: value")
            continue
        key, value = line.split(":", 1)
        key = key.strip()
        value = value.strip()
        if key in fields:
            errors.append(f"line {line_number}: duplicate field {key!r}")
            continue
        fields[key] = value
    return header, fields, errors


def validate_fields(fields: dict[str, str]) -> list[str]:
    errors: list[str] = []
    unknown = sorted(set(fields) - set(REQUIRED_FIELDS))
    missing = [field for field in REQUIRED_FIELDS if field not in fields]
    if unknown:
        errors.append(f"unknown fields: {', '.join(unknown)}")
    if missing:
        errors.append(f"missing required fields: {', '.join(missing)}")

    for field, allowed in ALLOWED_VALUES.items():
        value = fields.get(field)
        if value is not None and value not in allowed:
            errors.append(f"{field}: expected one of {sorted(allowed)}, got {value!r}")

    compose_file = fields.get("compose_file")
    if compose_file is not None and compose_file != ALLOWED_COMPOSE_FILE:
        errors.append(f"compose_file: expected {ALLOWED_COMPOSE_FILE!r}, got {compose_file!r}")

    reviewed_at = fields.get("reviewed_at")
    if reviewed_at is not None and not REVIEWED_AT_PATTERN.match(reviewed_at):
        errors.append("reviewed_at: expected ISO timestamp with +08:00 or Z timezone")

    target_alias = fields.get("target_alias")
    if target_alias is not None and not SAFE_ALIAS_PATTERN.match(target_alias):
        errors.append("target_alias: use 3-64 characters from A-Z, a-z, 0-9, dot, underscore, hyphen")

    git_commit = fields.get("git_commit")
    if git_commit is not None and not GIT_COMMIT_PATTERN.match(git_commit):
        errors.append("git_commit: expected 7-40 lowercase hex characters")

    image_tag = fields.get("image_tag")
    if image_tag is not None and not SAFE_IMAGE_TAG_PATTERN.match(image_tag):
        errors.append("image_tag: contains unsupported characters")

    return errors


def validate_redaction(content: str, fields: dict[str, str]) -> list[str]:
    errors: list[str] = []
    lowered_content = content.lower()
    for marker in FORBIDDEN_MARKERS:
        if marker.lower() in lowered_content:
            errors.append(f"contains forbidden sensitive marker: {marker!r}")

    for field, value in fields.items():
        if "<" in value or ">" in value:
            errors.append(f"{field}: placeholder values must be replaced")
        if ABSOLUTE_PATH_PATTERN.search(value):
            errors.append(f"{field}: absolute local paths are not allowed")
        if "://" in value and field != "image_tag":
            errors.append(f"{field}: full URLs are not allowed in evidence package values")
        if field != "git_commit" and re.search(r"\b[A-Za-z0-9_-]{48,}\b", value):
            errors.append(f"{field}: contains a long token-like value")
    return errors


def validate_evidence_text(content: str, path_label: str = "<memory>") -> ValidationResult:
    _header, fields, errors = parse_evidence(content)
    errors.extend(validate_fields(fields))
    errors.extend(validate_redaction(content, fields))
    return ValidationResult(path_label, errors)


def load_evidence_file(path: Path) -> tuple[Optional[str], ValidationResult]:
    try:
        content = path.read_text(encoding="utf-8")
    except OSError as exc:
        return None, ValidationResult(path.as_posix(), [f"failed to read file: {exc}"])
    return content, validate_evidence_text(content, path.as_posix())


def validate_file(path: Path) -> ValidationResult:
    _content, result = load_evidence_file(path)
    return result


def target_evidence_complete(fields: dict[str, str]) -> bool:
    return (
        fields["public_url_status"] == "configured"
        and fields["access_token_status"] == "configured_and_401_verified"
        and fields["access_control"] == "passed"
        and fields["external_tls"] == "passed"
        and fields["backup_restore"] == "passed"
        and fields["upgrade_rollback"] == "passed"
        and fields["log_redaction"] == "passed"
    )


def deployment_evidence_sources(fields: dict[str, str]) -> list[str]:
    return [field for field in DEPLOYMENT_EVIDENCE_SOURCE_FIELDS if fields[field] == "passed"]


def build_summary(fields: dict[str, str]) -> dict[str, object]:
    return {
        "schema": SUMMARY_HEADER,
        "target_alias": fields["target_alias"],
        "reviewed_at": fields["reviewed_at"],
        "git_commit": fields["git_commit"],
        "image_tag": fields["image_tag"],
        "compose_file": fields["compose_file"],
        "target_evidence_complete": target_evidence_complete(fields),
        "deployment_evidence_sources": deployment_evidence_sources(fields),
        "statuses": {field: fields[field] for field in SUMMARY_STATUS_FIELDS},
    }


def summary_from_text(content: str, path_label: str = "<memory>") -> dict[str, object]:
    _header, fields, errors = parse_evidence(content)
    errors.extend(validate_fields(fields))
    errors.extend(validate_redaction(content, fields))
    if errors:
        raise EvidenceValidationError(f"{path_label} is not valid: {errors}")
    return build_summary(fields)


def format_summary_text(summary: dict[str, object]) -> str:
    statuses = summary["statuses"]
    if not isinstance(statuses, dict):
        raise EvidenceValidationError("summary statuses must be a dictionary")
    source_labels = summary["deployment_evidence_sources"]
    if not isinstance(source_labels, list):
        raise EvidenceValidationError("summary deployment evidence sources must be a list")

    source_value = ", ".join(str(label) for label in source_labels) if source_labels else "none"
    lines = [
        SUMMARY_HEADER,
        f"target_alias: {summary['target_alias']}",
        f"reviewed_at: {summary['reviewed_at']}",
        f"git_commit: {summary['git_commit']}",
        f"image_tag: {summary['image_tag']}",
        f"compose_file: {summary['compose_file']}",
        f"target_evidence_complete: {str(summary['target_evidence_complete']).lower()}",
        f"deployment_evidence_sources: {source_value}",
    ]
    for field in SUMMARY_STATUS_FIELDS:
        lines.append(f"{field}: {statuses[field]}")
    return "\n".join(lines)


def format_summary_json(summary: dict[str, object]) -> str:
    return json.dumps(summary, indent=2, sort_keys=True)


def assert_valid(content: str, label: str) -> None:
    result = validate_evidence_text(content, label)
    if not result.ok:
        raise EvidenceValidationError(f"{label} should be valid: {result.errors}")


def assert_invalid(content: str, label: str, expected: str) -> None:
    result = validate_evidence_text(content, label)
    if result.ok:
        raise EvidenceValidationError(f"{label} should be invalid")
    if not any(expected in error for error in result.errors):
        raise EvidenceValidationError(f"{label} expected error containing {expected!r}, got {result.errors}")


def valid_fixture_text() -> str:
    return VALID_FIXTURE.read_text(encoding="utf-8")


def run_self_test() -> None:
    valid = valid_fixture_text()
    assert_valid(valid, "valid fixture")
    summary = summary_from_text(valid, "valid summary")
    if summary["schema"] != SUMMARY_HEADER:
        raise EvidenceValidationError(f"summary schema mismatch: {summary['schema']!r}")
    if summary["target_evidence_complete"] is not True:
        raise EvidenceValidationError("valid fixture should produce a complete target evidence summary")
    if summary["deployment_evidence_sources"] != ["external_tls", "backup_restore", "upgrade_rollback"]:
        raise EvidenceValidationError(f"unexpected evidence source labels: {summary['deployment_evidence_sources']!r}")
    summary_json = format_summary_json(summary)
    summary_text = format_summary_text(summary)
    for rendered in (summary_json, summary_text):
        if "notes" in rendered or "synthetic deployment evidence only" in rendered:
            raise EvidenceValidationError("summary output must not include free-form notes")
        if "Authorization" in rendered or "Bearer" in rendered or "payload" in rendered:
            raise EvidenceValidationError("summary output contains a forbidden sensitive marker")

    incomplete_summary = summary_from_text(valid.replace("external_tls: passed", "external_tls: not_run"), "incomplete summary")
    if incomplete_summary["target_evidence_complete"] is not False:
        raise EvidenceValidationError("incomplete evidence should not produce a complete target evidence summary")
    if "external_tls" in incomplete_summary["deployment_evidence_sources"]:
        raise EvidenceValidationError("incomplete evidence should not expose external_tls as a source label")

    assert_invalid(valid.replace("access_control: passed\n", ""), "missing field", "missing required fields")
    assert_invalid(valid.replace("external_tls: passed", "external_tls: maybe"), "invalid status", "external_tls")
    assert_invalid(valid.replace("notes: synthetic deployment evidence only", "notes: Authorization: Bearer abc"), "bearer", "Authorization")
    assert_invalid(valid.replace("notes: synthetic deployment evidence only", "notes: /Users/example/sync"), "path", "absolute local paths")
    assert_invalid(valid.replace("notes: synthetic deployment evidence only", "notes: encrypted_payload bytes leaked"), "payload", "encrypted_payload")
    assert_invalid(
        valid.replace("notes: synthetic deployment evidence only", "notes: abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKL"),
        "token-like",
        "long token-like",
    )
    try:
        summary_from_text(valid.replace("notes: synthetic deployment evidence only", "notes: Authorization: Bearer abc"), "invalid summary")
    except EvidenceValidationError as exc:
        if "Authorization" not in str(exc):
            raise EvidenceValidationError(f"invalid summary did not report the sensitive marker: {exc}") from exc
    else:
        raise EvidenceValidationError("invalid evidence should not produce summary output")


def print_result(result: ValidationResult) -> None:
    if result.ok:
        print(f"{result.path}: deployment evidence passed.")
        return
    print(f"{result.path}: deployment evidence failed.", file=sys.stderr)
    for error in result.errors:
        print(f"- {error}", file=sys.stderr)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate RadishLex sync deployment evidence packages.")
    parser.add_argument("paths", nargs="*", type=Path, help="Evidence package files to validate.")
    parser.add_argument("--self-test", action="store_true", help="Run built-in validation tests and the repository fixture.")
    summary_group = parser.add_mutually_exclusive_group()
    summary_group.add_argument("--summary-json", action="store_true", help="Print a non-sensitive JSON summary for one valid evidence package.")
    summary_group.add_argument("--summary-text", action="store_true", help="Print a non-sensitive text summary for one valid evidence package.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    summary_mode = args.summary_json or args.summary_text
    if args.self_test and summary_mode:
        print("--self-test cannot be combined with summary output.", file=sys.stderr)
        return 2

    if args.self_test:
        try:
            run_self_test()
        except EvidenceValidationError as exc:
            print(f"deployment evidence self-test failed: {exc}", file=sys.stderr)
            return 1
        print("Deployment evidence self-test passed.")

    if summary_mode:
        if len(args.paths) != 1:
            print("summary output requires exactly one evidence package path.", file=sys.stderr)
            return 2
        content, result = load_evidence_file(args.paths[0])
        if not result.ok or content is None:
            print_result(result)
            return 1
        try:
            summary = summary_from_text(content, result.path)
        except EvidenceValidationError as exc:
            print(f"{result.path}: deployment evidence summary failed.", file=sys.stderr)
            print(f"- {exc}", file=sys.stderr)
            return 1
        if args.summary_json:
            print(format_summary_json(summary))
        else:
            print(format_summary_text(summary))
        return 0

    if not args.paths:
        return 0 if args.self_test else 2

    failed = False
    for path in args.paths:
        result = validate_file(path)
        print_result(result)
        failed = failed or not result.ok
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
VALID_FIXTURE = REPO_ROOT / "tests" / "fixtures" / "sync-deployment-evidence-valid.txt"

HEADER = "deployment_evidence.v1"
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


def validate_file(path: Path) -> ValidationResult:
    try:
        content = path.read_text(encoding="utf-8")
    except OSError as exc:
        return ValidationResult(path.as_posix(), [f"failed to read file: {exc}"])
    return validate_evidence_text(content, path.as_posix())


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
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.self_test:
        try:
            run_self_test()
        except EvidenceValidationError as exc:
            print(f"deployment evidence self-test failed: {exc}", file=sys.stderr)
            return 1
        print("Deployment evidence self-test passed.")

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

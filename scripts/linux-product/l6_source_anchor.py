#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import os
import stat
from pathlib import Path
from typing import Any


class SourceAnchorStageError(RuntimeError):
    pass


def canonical_json_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")


def read_exact_file(path: Path, identity: dict[str, Any], label: str) -> bytes:
    if not path.is_absolute() or path.is_symlink() or not path.is_file():
        raise SourceAnchorStageError(f"{label} must be an absolute regular file")
    if path.parent.resolve() / path.name != path:
        raise SourceAnchorStageError(f"{label} must not traverse symlinked components")
    file_stat = path.stat()
    if file_stat.st_nlink != 1 or stat.S_IMODE(file_stat.st_mode) != 0o644:
        raise SourceAnchorStageError(f"{label} identity or mode is unsafe")
    if path.name != identity.get("filename"):
        raise SourceAnchorStageError(f"{label} filename differs from chain anchor")
    data = path.read_bytes()
    if len(data) != identity.get("size"):
        raise SourceAnchorStageError(f"{label} size differs from chain anchor")
    if hashlib.sha256(data).hexdigest() != identity.get("sha256"):
        raise SourceAnchorStageError(f"{label} SHA-256 differs from chain anchor")
    return data


def require_absent_output_directory(path: Path) -> Path:
    if not path.is_absolute() or path.exists() or path.is_symlink():
        raise SourceAnchorStageError(
            "source anchor output must be an absent absolute path"
        )
    parent = path.parent
    if parent.is_symlink() or not parent.is_dir() or parent.resolve() != parent:
        raise SourceAnchorStageError(
            "source anchor output parent must be an existing real directory"
        )
    return path


def write_exclusive_file(path: Path, data: bytes) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags, 0o644)
    except OSError as exc:
        raise SourceAnchorStageError(
            f"cannot create frozen source artifact: {exc}"
        ) from exc
    try:
        os.fchmod(descriptor, 0o644)
        view = memoryview(data)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise SourceAnchorStageError(
                    "cannot complete frozen source artifact copy"
                )
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def stage_exact_source_anchor(
    package: Path,
    artifact_evidence: Path,
    output_directory: Path,
    anchor: dict[str, Any],
) -> tuple[Path, Path]:
    if set(anchor) != {"artifact_evidence", "package", "policy"} or anchor.get(
        "policy"
    ) != "prior-terminal-installed-artifact-v1":
        raise SourceAnchorStageError("prior-terminal source anchor is invalid")
    package_identity = anchor["package"]
    evidence_identity = anchor["artifact_evidence"]
    if not isinstance(package_identity, dict) or not isinstance(evidence_identity, dict):
        raise SourceAnchorStageError("prior-terminal source identities are invalid")
    identity_fields = {"filename", "sha256", "size"}
    if set(package_identity) != identity_fields or set(evidence_identity) != identity_fields:
        raise SourceAnchorStageError("prior-terminal source identity fields are invalid")
    package_bytes = read_exact_file(package, package_identity, "source package")
    evidence_bytes = read_exact_file(
        artifact_evidence, evidence_identity, "source artifact evidence"
    )
    try:
        evidence_value = json.loads(evidence_bytes.decode("utf-8"))
    except Exception as exc:
        raise SourceAnchorStageError("source artifact evidence is invalid") from exc
    if canonical_json_bytes(evidence_value) != evidence_bytes:
        raise SourceAnchorStageError("source artifact evidence is not canonical")
    if not isinstance(evidence_value, dict) or evidence_value.get("package") != (
        package_identity
    ):
        raise SourceAnchorStageError(
            "source package identity differs from artifact evidence"
        )
    output_directory = require_absent_output_directory(output_directory)
    output_directory.mkdir(mode=0o755)
    output_directory.chmod(0o755)
    staged_package = output_directory / package.name
    staged_evidence = output_directory / artifact_evidence.name
    write_exclusive_file(staged_package, package_bytes)
    write_exclusive_file(staged_evidence, evidence_bytes)
    directory_descriptor = os.open(output_directory, os.O_RDONLY | os.O_CLOEXEC)
    try:
        os.fsync(directory_descriptor)
    finally:
        os.close(directory_descriptor)
    return staged_package, staged_evidence

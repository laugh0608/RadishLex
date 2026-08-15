#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import stat
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import l6_release_pair
from l6_maintenance_refresh_io import (
    L6MaintenanceRefreshError,
    fsync_directory,
    rename_directory_no_replace,
    require_absent_output,
    write_exclusive_file,
)
from l6_maintenance_refresh_environment import (
    collect_environment,
    validate_environment,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
CONTRACT_PATH = REPO_ROOT / "packaging/linux/l6-maintenance-refresh.json"
METADATA_RELATIVE = Path("packaging/linux/product.json")
BASE_TARGET_COMMIT = "80e49ced45b316eaa801f913c659142528a32c08"
REPAIR_FIX_COMMIT = "b0197f5a0f0a88e53cf4b6bf862252a4374197de"
BASE_RECORD_SHA256 = "cda70afa89b3f0ee05235b95dcc346eaeea9805c4b87af9d451ce1900f00659b"
BASE_PACKAGE_SHA256 = "b211d9406825515b2ba1c473b5f98069de00fa709505e2ba3ed3cb9b8af7d09c"
BASE_ARTIFACT_EVIDENCE_SHA256 = (
    "2a1132c6fb27d4ca2e5e4bbd76864e3e753749c2bb43cae82287635b1c2d0e1b"
)
BASE_MAINTENANCE_SHA256 = (
    "b060c2403424560e9ac0c11f890838894d19d153a89541575ade9afd87fe7d81"
)
RECORD_KEYS = {
    "base_release_pair",
    "build_environment",
    "evidence_format",
    "executable",
    "format_version",
    "matrix_profile",
    "profile",
    "refresh",
}
EXPECTED_CONTRACT = {
    "base_release_pair": {
        "record": {
            "filename": "release-pair.evidence.json",
            "sha256": BASE_RECORD_SHA256,
        },
        "target": {
            "artifact_evidence": {
                "filename": "radishlex_26.7.1+38-2_arm64.deb.evidence.json",
                "sha256": BASE_ARTIFACT_EVIDENCE_SHA256,
            },
            "debian_revision": "2",
            "package": {
                "filename": "radishlex_26.7.1+38-2_arm64.deb",
                "sha256": BASE_PACKAGE_SHA256,
            },
            "package_version": "26.7.1+38-2",
            "production_maintenance_sha256": BASE_MAINTENANCE_SHA256,
            "repository_commit": BASE_TARGET_COMMIT,
        },
    },
    "build": {
        "base_artifact_policy": "frozen-release-pair-target-v1",
        "environment_profile": "debian13-arm64-maintenance-refresh-build-v1",
        "network_policy": "dependency-frozen-before-build",
        "output_policy": "private-staging-then-atomic-publish-v1",
        "package_rebuild": False,
        "refresh_clean_root": True,
    },
    "executable": {
        "binary": "radishlex-linux-maintenance",
        "build_identity": "radishlex-linux-maintenance-production-v1",
        "cargo_package": "radishlex-linux-product-install",
        "feature_profile": "no-default-features",
        "role": "production-maintenance",
    },
    "format_version": 1,
    "matrix_profile": "debian13-arm64-ephemeral-v1",
    "profile": "debian13-arm64-maintenance-refresh-v1",
    "refresh": {
        "commit_policy": "clean-descendant-head-containing-repair-fix-v1",
        "required_ancestor_commit": REPAIR_FIX_COMMIT,
    },
}


@dataclass(frozen=True)
class BaseInputs:
    record_path: Path
    target_package: Path
    target_artifact_evidence: Path


def canonical_json_bytes(value: Any) -> bytes:
    return l6_release_pair.canonical_json_bytes(value)


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def require_exact_fields(value: dict[str, Any], expected: set[str], label: str) -> None:
    if set(value) != expected:
        raise L6MaintenanceRefreshError(f"{label} fields differ from format v1")


def load_json(path: Path, label: str, *, canonical: bool = False) -> dict[str, Any]:
    try:
        raw = path.read_bytes()
        value = json.loads(raw.decode("utf-8"))
    except Exception as exc:
        raise L6MaintenanceRefreshError(f"invalid {label}: {exc}") from exc
    if not isinstance(value, dict):
        raise L6MaintenanceRefreshError(f"{label} must be a JSON object")
    if canonical and raw != canonical_json_bytes(value):
        raise L6MaintenanceRefreshError(f"{label} must use canonical JSON")
    return value


def load_contract(path: Path = CONTRACT_PATH) -> dict[str, Any]:
    contract = load_json(path, "L6 maintenance refresh contract", canonical=True)
    if contract != EXPECTED_CONTRACT:
        raise L6MaintenanceRefreshError(
            "L6 maintenance refresh contract differs from frozen v1"
        )
    return contract


def load_base_pair_contract(repository_root: Path) -> dict[str, Any]:
    source_metadata = l6_release_pair.git_json_at_commit(
        repository_root,
        l6_release_pair.SOURCE_COMMIT,
        l6_release_pair.METADATA_RELATIVE,
    )
    target_metadata = l6_release_pair.git_json_at_commit(
        repository_root, BASE_TARGET_COMMIT, l6_release_pair.METADATA_RELATIVE
    )
    return l6_release_pair.load_contract(
        repository_root=repository_root,
        source_metadata=source_metadata,
        target_metadata=target_metadata,
    )


def require_git_ancestor(root: Path, ancestor: str, descendant: str) -> None:
    try:
        subprocess.run(
            [
                "/usr/bin/git",
                "-C",
                str(root),
                "merge-base",
                "--is-ancestor",
                ancestor,
                descendant,
            ],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            env={"LC_ALL": "C", "PATH": "/usr/bin:/bin"},
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        raise L6MaintenanceRefreshError(
            "refresh commit does not contain the required repair fix"
        ) from exc


def verify_refresh_root(
    root: Path, *, require_absent_build_outputs: bool = True
) -> str:
    root_verifier = (
        l6_release_pair.require_clean_root
        if require_absent_build_outputs
        else l6_release_pair.require_repository_root
    )
    root, refresh_commit = root_verifier(root, "maintenance refresh repository root")
    if root != REPO_ROOT.resolve():
        raise L6MaintenanceRefreshError(
            "refresh root must be the repository that owns the refresh builder"
        )
    if refresh_commit == BASE_TARGET_COMMIT:
        raise L6MaintenanceRefreshError("refresh commit must differ from base target")
    require_git_ancestor(root, REPAIR_FIX_COMMIT, refresh_commit)
    base_metadata = l6_release_pair.git_json_at_commit(
        root, BASE_TARGET_COMMIT, METADATA_RELATIVE
    )
    refresh_metadata = load_json(
        root / METADATA_RELATIVE, "refresh Linux product metadata"
    )
    if refresh_metadata != base_metadata:
        raise L6MaintenanceRefreshError(
            "maintenance refresh must not change target product metadata"
        )
    return refresh_commit


def read_identity_file(
    path: Path, identity: dict[str, Any], label: str, mode: int
) -> bytes:
    try:
        path = l6_release_pair.require_real_file(path, label, mode)
    except l6_release_pair.L6ReleasePairError as exc:
        raise L6MaintenanceRefreshError(str(exc)) from exc
    if path.name != identity.get("filename"):
        raise L6MaintenanceRefreshError(f"{label} filename differs from identity")
    data = path.read_bytes()
    if len(data) != identity.get("size"):
        raise L6MaintenanceRefreshError(f"{label} size differs from identity")
    if sha256_bytes(data) != identity.get("sha256"):
        raise L6MaintenanceRefreshError(f"{label} SHA-256 differs from identity")
    return data


def validate_target_release(
    target: dict[str, Any], contract: dict[str, Any]
) -> None:
    expected_fields = {
        "artifact_evidence",
        "debian_revision",
        "dependencies",
        "package",
        "package_version",
        "product_manifest_sha256",
        "repository_commit",
    }
    if not isinstance(target, dict):
        raise L6MaintenanceRefreshError("base target release must be an object")
    require_exact_fields(target, expected_fields, "base target release")
    frozen = contract["base_release_pair"]["target"]
    for field in ("debian_revision", "package_version", "repository_commit"):
        if target.get(field) != frozen[field]:
            raise L6MaintenanceRefreshError(
                f"base target {field} differs from refresh contract"
            )
    for field in ("package", "artifact_evidence"):
        identity = target.get(field)
        expected = frozen[field]
        if not isinstance(identity, dict) or set(identity) != {
            "filename",
            "sha256",
            "size",
        }:
            raise L6MaintenanceRefreshError(
                f"base target {field} identity is invalid"
            )
        if identity.get("filename") != expected["filename"] or identity.get(
            "sha256"
        ) != expected["sha256"]:
            raise L6MaintenanceRefreshError(
                f"base target {field} differs from refresh contract"
            )
        if not isinstance(identity.get("size"), int) or identity["size"] <= 0:
            raise L6MaintenanceRefreshError(f"base target {field} size is invalid")
    if target["package"]["size"] > l6_release_pair.MAX_PACKAGE_SIZE:
        raise L6MaintenanceRefreshError("base target package exceeds the size limit")
    if target["artifact_evidence"]["size"] > l6_release_pair.MAX_EVIDENCE_SIZE:
        raise L6MaintenanceRefreshError(
            "base target artifact evidence exceeds the size limit"
        )
    dependencies = target.get("dependencies")
    if not isinstance(dependencies, dict) or set(dependencies) != {"count", "sha256"}:
        raise L6MaintenanceRefreshError("base target dependency identity is invalid")
    if not isinstance(dependencies["count"], int) or dependencies["count"] <= 0:
        raise L6MaintenanceRefreshError("base target dependency count is invalid")
    for digest in (
        dependencies.get("sha256"),
        target.get("product_manifest_sha256"),
    ):
        if not isinstance(digest, str) or l6_release_pair.SHA256_PATTERN.fullmatch(
            digest
        ) is None:
            raise L6MaintenanceRefreshError("base target digest is invalid")


def validate_base_inputs(
    inputs: BaseInputs,
    contract: dict[str, Any],
    *,
    pair_contract: dict[str, Any] | None = None,
    repository_root: Path = REPO_ROOT,
) -> tuple[dict[str, Any], dict[str, Any]]:
    record_identity = contract["base_release_pair"]["record"]
    try:
        record_path = l6_release_pair.require_real_file(
            inputs.record_path, "base release pair record", 0o644
        )
    except l6_release_pair.L6ReleasePairError as exc:
        raise L6MaintenanceRefreshError(str(exc)) from exc
    if record_path.name != record_identity["filename"]:
        raise L6MaintenanceRefreshError(
            "base release pair record filename differs from contract"
        )
    record_bytes = record_path.read_bytes()
    if len(record_bytes) > l6_release_pair.MAX_EVIDENCE_SIZE:
        raise L6MaintenanceRefreshError("base release pair record exceeds the size limit")
    if sha256_bytes(record_bytes) != record_identity["sha256"]:
        raise L6MaintenanceRefreshError(
            "base release pair record SHA-256 differs from contract"
        )
    record = load_json(record_path, "base release pair record", canonical=True)
    pair_contract = pair_contract or load_base_pair_contract(repository_root)
    try:
        l6_release_pair.validate_record(record, pair_contract)
    except l6_release_pair.L6ReleasePairError as exc:
        raise L6MaintenanceRefreshError(
            f"base release pair record is invalid: {exc}"
        ) from exc
    target = record["releases"]["target"]
    validate_target_release(target, contract)
    production = [
        executable
        for executable in record["executables"]
        if executable.get("role") == "production-maintenance"
    ]
    if len(production) != 1 or production[0].get("sha256") != contract[
        "base_release_pair"
    ]["target"]["production_maintenance_sha256"]:
        raise L6MaintenanceRefreshError(
            "base production maintenance identity differs from refresh contract"
        )
    package_bytes = read_identity_file(
        inputs.target_package, target["package"], "base target package", 0o644
    )
    evidence_bytes = read_identity_file(
        inputs.target_artifact_evidence,
        target["artifact_evidence"],
        "base target artifact evidence",
        0o644,
    )
    try:
        evidence = json.loads(evidence_bytes.decode("utf-8"))
    except Exception as exc:
        raise L6MaintenanceRefreshError(
            "base target artifact evidence is invalid"
        ) from exc
    if canonical_json_bytes(evidence) != evidence_bytes:
        raise L6MaintenanceRefreshError(
            "base target artifact evidence must use canonical JSON"
        )
    if not isinstance(evidence, dict) or evidence.get("package") != target["package"]:
        raise L6MaintenanceRefreshError(
            "base target package identity differs from artifact evidence"
        )
    if len(package_bytes) != target["package"]["size"]:
        raise L6MaintenanceRefreshError("base target package size changed during read")
    return record, target


def stage_base_inputs(
    inputs: BaseInputs,
    refresh_root: Path,
    output_directory: Path,
    contract: dict[str, Any],
    *,
    pair_contract: dict[str, Any] | None = None,
) -> tuple[Path, Path, Path]:
    verify_refresh_root(refresh_root)
    record, target = validate_base_inputs(
        inputs,
        contract,
        pair_contract=pair_contract,
        repository_root=refresh_root,
    )
    output_directory = require_absent_output(
        output_directory, "base input output directory"
    )
    output_directory.mkdir(mode=0o755)
    base_directory = output_directory / "base"
    target_directory = output_directory / "target"
    artifact_directory = target_directory / "artifacts"
    base_directory.mkdir(mode=0o755)
    target_directory.mkdir(mode=0o755)
    artifact_directory.mkdir(mode=0o755)
    staged_record = base_directory / inputs.record_path.name
    staged_package = artifact_directory / inputs.target_package.name
    staged_evidence = artifact_directory / inputs.target_artifact_evidence.name
    write_exclusive_file(staged_record, canonical_json_bytes(record), 0o644)
    write_exclusive_file(staged_package, inputs.target_package.read_bytes(), 0o644)
    write_exclusive_file(
        staged_evidence, inputs.target_artifact_evidence.read_bytes(), 0o644
    )
    for directory in (
        base_directory,
        artifact_directory,
        target_directory,
        output_directory,
    ):
        fsync_directory(directory)
    for path, identity in (
        (staged_package, target["package"]),
        (staged_evidence, target["artifact_evidence"]),
    ):
        if sha256_bytes(path.read_bytes()) != identity["sha256"]:
            raise L6MaintenanceRefreshError("staged base input changed after copy")
    return staged_record, staged_package, staged_evidence


def build_record(
    contract: dict[str, Any],
    environment: dict[str, Any],
    inputs: BaseInputs,
    maintenance: Path,
    *,
    refresh_commit: str,
    pair_contract: dict[str, Any] | None = None,
    repository_root: Path = REPO_ROOT,
) -> dict[str, Any]:
    validate_environment(environment)
    base_record, target = validate_base_inputs(
        inputs,
        contract,
        pair_contract=pair_contract,
        repository_root=repository_root,
    )
    executable_identity = contract["executable"]
    if maintenance.name != executable_identity["binary"]:
        raise L6MaintenanceRefreshError(
            "maintenance executable filename differs from contract"
        )
    try:
        elf = l6_release_pair.parse_aarch64_elf(
            maintenance, "production-maintenance"
        )
    except l6_release_pair.L6ReleasePairError as exc:
        raise L6MaintenanceRefreshError(str(exc)) from exc
    executable_bytes = maintenance.read_bytes()
    executable_sha256 = sha256_bytes(executable_bytes)
    if executable_sha256 == contract["base_release_pair"]["target"][
        "production_maintenance_sha256"
    ]:
        raise L6MaintenanceRefreshError(
            "maintenance refresh executable must differ from the frozen base executable"
        )
    record_bytes = inputs.record_path.read_bytes()
    record = {
        "base_release_pair": {
            "record": {
                "filename": inputs.record_path.name,
                "sha256": sha256_bytes(record_bytes),
                "size": len(record_bytes),
            },
            "target": target,
        },
        "build_environment": environment,
        "evidence_format": "radishlex-linux-l6-maintenance-refresh-evidence-v1",
        "executable": {
            **executable_identity,
            "elf": elf,
            "filename": maintenance.name,
            "repository_commit": refresh_commit,
            "sha256": executable_sha256,
            "size": len(executable_bytes),
        },
        "format_version": 1,
        "matrix_profile": contract["matrix_profile"],
        "profile": contract["profile"],
        "refresh": {
            "repository_commit": refresh_commit,
            "required_ancestor_commit": contract["refresh"][
                "required_ancestor_commit"
            ],
        },
    }
    if base_record["releases"]["target"] != target:
        raise L6MaintenanceRefreshError("base target changed during record creation")
    validate_record(record, contract)
    return record


def validate_record(value: dict[str, Any], contract: dict[str, Any]) -> None:
    require_exact_fields(value, RECORD_KEYS, "maintenance refresh evidence")
    if (
        value.get("evidence_format")
        != "radishlex-linux-l6-maintenance-refresh-evidence-v1"
    ):
        raise L6MaintenanceRefreshError(
            "maintenance refresh evidence format is invalid"
        )
    if value.get("format_version") != 1 or value.get("profile") != contract["profile"]:
        raise L6MaintenanceRefreshError(
            "maintenance refresh evidence identity is invalid"
        )
    if value.get("matrix_profile") != contract["matrix_profile"]:
        raise L6MaintenanceRefreshError(
            "maintenance refresh matrix identity is invalid"
        )
    validate_environment(value.get("build_environment", {}))
    serialized = canonical_json_bytes(value).decode("utf-8").lower()
    for forbidden in (
        '"operation_id"',
        '"operation-id"',
        '"pid"',
        '"proc_maps"',
        '"dpkg_stdout"',
        '"dpkg_stderr"',
        "/users/",
        "/home/",
        "/workspace/",
        "/tmp/",
        "/private/",
        "acceptance-controller",
        "radishlex-linux-l6-acceptance",
        "--authorized-l6-crash",
    ):
        if forbidden in serialized:
            raise L6MaintenanceRefreshError(
                "maintenance refresh evidence contains forbidden identity or capability"
            )
    if len(serialized.encode("utf-8")) > l6_release_pair.MAX_EVIDENCE_SIZE:
        raise L6MaintenanceRefreshError(
            "maintenance refresh evidence exceeds the size limit"
        )
    base = value.get("base_release_pair")
    if not isinstance(base, dict):
        raise L6MaintenanceRefreshError("base release pair evidence must be an object")
    require_exact_fields(base, {"record", "target"}, "base release pair evidence")
    record_identity = base.get("record")
    if not isinstance(record_identity, dict) or set(record_identity) != {
        "filename",
        "sha256",
        "size",
    }:
        raise L6MaintenanceRefreshError("base release pair record identity is invalid")
    expected_record = contract["base_release_pair"]["record"]
    if (
        record_identity.get("filename") != expected_record["filename"]
        or record_identity.get("sha256") != expected_record["sha256"]
        or not isinstance(record_identity.get("size"), int)
        or not 0 < record_identity["size"] <= l6_release_pair.MAX_EVIDENCE_SIZE
    ):
        raise L6MaintenanceRefreshError(
            "base release pair record identity differs from contract"
        )
    validate_target_release(base.get("target"), contract)
    refresh = value.get("refresh")
    if not isinstance(refresh, dict) or set(refresh) != {
        "repository_commit",
        "required_ancestor_commit",
    }:
        raise L6MaintenanceRefreshError("maintenance refresh commit identity is invalid")
    if (
        l6_release_pair.COMMIT_PATTERN.fullmatch(
            str(refresh.get("repository_commit"))
        )
        is None
        or refresh.get("repository_commit") == BASE_TARGET_COMMIT
        or refresh.get("required_ancestor_commit")
        != contract["refresh"]["required_ancestor_commit"]
    ):
        raise L6MaintenanceRefreshError("maintenance refresh commit identity is invalid")
    executable = value.get("executable")
    expected_executable_fields = {
        "binary",
        "build_identity",
        "cargo_package",
        "elf",
        "feature_profile",
        "filename",
        "repository_commit",
        "role",
        "sha256",
        "size",
    }
    if not isinstance(executable, dict) or set(executable) != expected_executable_fields:
        raise L6MaintenanceRefreshError(
            "maintenance refresh executable evidence fields are invalid"
        )
    for field, expected in contract["executable"].items():
        if executable.get(field) != expected:
            raise L6MaintenanceRefreshError(
                "maintenance refresh executable identity differs from contract"
            )
    if executable.get("filename") != executable.get("binary"):
        raise L6MaintenanceRefreshError(
            "maintenance refresh executable filename is invalid"
        )
    if executable.get("repository_commit") != refresh.get("repository_commit"):
        raise L6MaintenanceRefreshError(
            "maintenance refresh executable commit differs from evidence"
        )
    digest = executable.get("sha256")
    if (
        not isinstance(digest, str)
        or l6_release_pair.SHA256_PATTERN.fullmatch(digest) is None
        or digest
        == contract["base_release_pair"]["target"][
            "production_maintenance_sha256"
        ]
    ):
        raise L6MaintenanceRefreshError(
            "maintenance refresh executable digest is invalid"
        )
    if not isinstance(executable.get("size"), int) or not (
        0 < executable["size"] <= l6_release_pair.MAX_EXECUTABLE_SIZE
    ):
        raise L6MaintenanceRefreshError("maintenance refresh executable size is invalid")
    if executable.get("elf") != {
        "class": "ELF64",
        "interpreter": l6_release_pair.ELF_INTERPRETER,
        "machine": "AArch64",
        "type": "dynamic-or-executable",
    }:
        raise L6MaintenanceRefreshError(
            "maintenance refresh executable ELF identity is invalid"
        )


def verify_published_record(
    evidence_path: Path,
    record: dict[str, Any],
    contract: dict[str, Any],
    *,
    pair_contract: dict[str, Any] | None = None,
    repository_root: Path = REPO_ROOT,
) -> None:
    validate_record(record, contract)
    try:
        evidence_path = l6_release_pair.require_real_file(
            evidence_path, "maintenance refresh evidence", 0o644
        )
    except l6_release_pair.L6ReleasePairError as exc:
        raise L6MaintenanceRefreshError(str(exc)) from exc
    if evidence_path.name != "maintenance-refresh.evidence.json":
        raise L6MaintenanceRefreshError(
            "maintenance refresh evidence filename is invalid"
        )
    if evidence_path.read_bytes() != canonical_json_bytes(record):
        raise L6MaintenanceRefreshError(
            "maintenance refresh evidence file differs from canonical record"
        )
    root = evidence_path.parent
    target = record["base_release_pair"]["target"]
    expected_inventory = {
        "base",
        "base/release-pair.evidence.json",
        "build-environment.json",
        "maintenance-refresh.evidence.json",
        "radishlex-linux-maintenance",
        "target",
        "target/artifacts",
        f'target/artifacts/{target["package"]["filename"]}',
        f'target/artifacts/{target["artifact_evidence"]["filename"]}',
    }
    actual_inventory: set[str] = set()
    for path in root.rglob("*"):
        if path.is_symlink():
            raise L6MaintenanceRefreshError(
                "maintenance refresh publication contains a symlink"
            )
        relative = path.relative_to(root).as_posix()
        if path.is_dir():
            if stat.S_IMODE(path.stat().st_mode) != 0o755:
                raise L6MaintenanceRefreshError(
                    "maintenance refresh publication directory mode is invalid"
                )
        elif path.is_file():
            if path.stat().st_nlink != 1:
                raise L6MaintenanceRefreshError(
                    "maintenance refresh publication file link count is invalid"
                )
        else:
            raise L6MaintenanceRefreshError(
                "maintenance refresh publication contains an unsupported entry"
            )
        actual_inventory.add(relative)
    if actual_inventory != expected_inventory:
        raise L6MaintenanceRefreshError(
            "maintenance refresh publication inventory differs from evidence"
        )
    environment_path = root / "build-environment.json"
    try:
        l6_release_pair.require_real_file(
            environment_path, "maintenance refresh build environment", 0o644
        )
    except l6_release_pair.L6ReleasePairError as exc:
        raise L6MaintenanceRefreshError(str(exc)) from exc
    environment = load_json(
        environment_path, "maintenance refresh build environment", canonical=True
    )
    if environment != record["build_environment"]:
        raise L6MaintenanceRefreshError(
            "build environment file differs from maintenance refresh evidence"
        )
    inputs = BaseInputs(
        root / "base/release-pair.evidence.json",
        root / "target/artifacts" / target["package"]["filename"],
        root / "target/artifacts" / target["artifact_evidence"]["filename"],
    )
    published_base_record, published_target = validate_base_inputs(
        inputs,
        contract,
        pair_contract=pair_contract,
        repository_root=repository_root,
    )
    if (
        published_base_record["releases"]["target"] != target
        or published_target != target
    ):
        raise L6MaintenanceRefreshError(
            "published base target differs from maintenance refresh evidence"
        )
    executable = record["executable"]
    executable_path = root / executable["filename"]
    executable_bytes = read_identity_file(
        executable_path, executable, "published maintenance refresh executable", 0o755
    )
    try:
        actual_elf = l6_release_pair.parse_aarch64_elf(
            executable_path, "production-maintenance"
        )
    except l6_release_pair.L6ReleasePairError as exc:
        raise L6MaintenanceRefreshError(str(exc)) from exc
    if actual_elf != executable["elf"] or sha256_bytes(executable_bytes) != executable[
        "sha256"
    ]:
        raise L6MaintenanceRefreshError(
            "published maintenance refresh ELF differs from evidence"
        )


def publish_staging(
    staging: Path,
    output: Path,
    contract: dict[str, Any],
    *,
    pair_contract: dict[str, Any] | None = None,
    repository_root: Path = REPO_ROOT,
) -> Path:
    if not staging.is_absolute() or staging.is_symlink() or not staging.is_dir():
        raise L6MaintenanceRefreshError(
            "maintenance refresh staging must be an absolute real directory"
        )
    if staging.resolve() != staging or stat.S_IMODE(staging.stat().st_mode) != 0o755:
        raise L6MaintenanceRefreshError(
            "maintenance refresh staging identity or mode is invalid"
        )
    output = require_absent_output(output, "maintenance refresh publication output")
    if staging.stat().st_dev != output.parent.stat().st_dev:
        raise L6MaintenanceRefreshError(
            "maintenance refresh staging and output must share a filesystem"
        )
    evidence_path = staging / "maintenance-refresh.evidence.json"
    record = load_json(evidence_path, "maintenance refresh evidence", canonical=True)
    verify_published_record(
        evidence_path,
        record,
        contract,
        pair_contract=pair_contract,
        repository_root=repository_root,
    )
    rename_directory_no_replace(staging, output)
    fsync_directory(output.parent)
    published_evidence = output / "maintenance-refresh.evidence.json"
    verify_published_record(
        published_evidence,
        record,
        contract,
        pair_contract=pair_contract,
        repository_root=repository_root,
    )
    return published_evidence


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build and verify a package-preserving L6 maintenance refresh handoff."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("validate-contract")
    validate_refresh = subparsers.add_parser("validate-refresh")
    validate_refresh.add_argument("--refresh-root", type=Path, required=True)
    stage = subparsers.add_parser("stage-base")
    stage.add_argument("--base-record", type=Path, required=True)
    stage.add_argument("--target-package", type=Path, required=True)
    stage.add_argument("--target-artifact-evidence", type=Path, required=True)
    stage.add_argument("--refresh-root", type=Path, required=True)
    stage.add_argument("--output-dir", type=Path, required=True)
    environment = subparsers.add_parser("environment")
    environment.add_argument("--output", type=Path, required=True)
    record = subparsers.add_parser("record")
    record.add_argument("--base-record", type=Path, required=True)
    record.add_argument("--target-package", type=Path, required=True)
    record.add_argument("--target-artifact-evidence", type=Path, required=True)
    record.add_argument("--refresh-root", type=Path, required=True)
    record.add_argument("--environment", type=Path, required=True)
    record.add_argument("--maintenance-executable", type=Path, required=True)
    record.add_argument("--output", type=Path, required=True)
    verify = subparsers.add_parser("verify")
    verify.add_argument("--evidence", type=Path, required=True)
    publish = subparsers.add_parser("publish")
    publish.add_argument("--staging", type=Path, required=True)
    publish.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        contract = load_contract()
        if args.command == "validate-contract":
            return 0
        if args.command == "validate-refresh":
            verify_refresh_root(args.refresh_root)
            return 0
        if args.command == "stage-base":
            stage_base_inputs(
                BaseInputs(
                    args.base_record,
                    args.target_package,
                    args.target_artifact_evidence,
                ),
                args.refresh_root,
                args.output_dir,
                contract,
            )
            return 0
        if args.command == "environment":
            output = require_absent_output(
                args.output, "maintenance refresh environment output"
            )
            write_exclusive_file(output, canonical_json_bytes(collect_environment()), 0o644)
            fsync_directory(output.parent)
            return 0
        if args.command == "record":
            refresh_commit = verify_refresh_root(
                args.refresh_root, require_absent_build_outputs=False
            )
            environment = load_json(
                args.environment,
                "maintenance refresh build environment",
                canonical=True,
            )
            record = build_record(
                contract,
                environment,
                BaseInputs(
                    args.base_record,
                    args.target_package,
                    args.target_artifact_evidence,
                ),
                args.maintenance_executable,
                refresh_commit=refresh_commit,
                repository_root=args.refresh_root,
            )
            output = require_absent_output(
                args.output, "maintenance refresh evidence output"
            )
            write_exclusive_file(output, canonical_json_bytes(record), 0o644)
            fsync_directory(output.parent)
            return 0
        if args.command == "verify":
            record = load_json(
                args.evidence, "maintenance refresh evidence", canonical=True
            )
            verify_published_record(args.evidence, record, contract)
            return 0
        if args.command == "publish":
            publish_staging(args.staging, args.output, contract)
            return 0
    except (L6MaintenanceRefreshError, l6_release_pair.L6ReleasePairError) as exc:
        raise SystemExit(str(exc)) from exc
    raise SystemExit("unsupported maintenance refresh command")


if __name__ == "__main__":
    raise SystemExit(main())

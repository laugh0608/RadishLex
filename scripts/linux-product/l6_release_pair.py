#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import stat
import struct
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable


REPO_ROOT = Path(__file__).resolve().parents[2]
CONTRACT_PATH = REPO_ROOT / "packaging/linux/l6-release-pair.json"
METADATA_RELATIVE = Path("packaging/linux/product.json")
ARTIFACT_TOOL_RELATIVE = Path("scripts/linux-product/deb_artifact.py")
CLEAN_OUTPUT_RELATIVES = (
    Path("target"),
    Path("apps/radishlex-manager/build"),
    Path("apps/radishlex-manager/.dart_tool"),
    Path("apps/radishlex-manager/linux/flutter/ephemeral"),
)
SOURCE_COMMIT = "55351f21536d6dca3f90ab053c2a81e2b9bea354"
SHA256_PATTERN = re.compile(r"[0-9a-f]{64}")
COMMIT_PATTERN = re.compile(r"[0-9a-f]{40}")
VERSION_PATTERN = re.compile(r"[ -~]{1,160}")
MAX_EXECUTABLE_SIZE = 256 * 1024 * 1024
MAX_PACKAGE_SIZE = 512 * 1024 * 1024
MAX_EVIDENCE_SIZE = 1024 * 1024
ELF_MACHINE_AARCH64 = 183
ELF_INTERPRETER = "/lib/ld-linux-aarch64.so.1"
ACCEPTANCE_MARKERS = (
    b"radishlex-linux-l6-acceptance-v1",
    b"--authorized-l6-crash",
)
MUTABLE_METADATA_FIELDS = {"debian_revision", "package_version"}
SHARED_CONTRACT_FIELDS = (
    "build_number",
    "data_layout",
    "data_removal",
    "debian_architecture",
    "default_removal",
    "distribution_identity",
    "ffi_abi_version",
    "multiarch_tuple",
    "privacy_format_version",
    "product_id",
    "product_version",
    "rime_data_lock_sha256",
    "rime_schema_id",
    "runtime_layout",
    "settings_format_version",
    "userdb_schema_version",
)
CONTRACT_KEYS = {
    "build",
    "executables",
    "format_version",
    "matrix_profile",
    "profile",
    "shared_contract",
    "source",
    "target",
}
ENVIRONMENT_KEYS = {"architecture", "format_version", "operating_system", "profile", "tools"}
RECORD_KEYS = {
    "build_environment",
    "evidence_format",
    "executables",
    "format_version",
    "matrix_profile",
    "profile",
    "releases",
    "shared_contract",
}


class L6ReleasePairError(RuntimeError):
    pass


@dataclass(frozen=True)
class ReleaseInput:
    role: str
    root: Path
    package: Path
    artifact_evidence: Path


def canonical_json_bytes(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def load_json(path: Path, label: str, *, canonical: bool = False) -> dict[str, Any]:
    try:
        raw = path.read_bytes()
        value = json.loads(raw.decode("utf-8"))
    except Exception as exc:
        raise L6ReleasePairError(f"invalid {label}: {exc}") from exc
    if not isinstance(value, dict):
        raise L6ReleasePairError(f"{label} must be a JSON object")
    if canonical and raw != canonical_json_bytes(value):
        raise L6ReleasePairError(f"{label} must use canonical JSON")
    return value


def run_git(root: Path, *arguments: str) -> str:
    try:
        result = subprocess.run(
            ["/usr/bin/git", "-C", str(root), *arguments],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            env={"LC_ALL": "C", "PATH": "/usr/bin:/bin"},
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        raise L6ReleasePairError(f"git identity check failed: {exc}") from exc
    return result.stdout.strip()


def git_json_at_commit(root: Path, commit: str, relative: Path) -> dict[str, Any]:
    raw = run_git(root, "show", f"{commit}:{relative.as_posix()}")
    try:
        value = json.loads(raw)
    except Exception as exc:
        raise L6ReleasePairError(f"invalid committed {relative}: {exc}") from exc
    if not isinstance(value, dict):
        raise L6ReleasePairError(f"committed {relative} must be a JSON object")
    return value


def require_exact_fields(value: dict[str, Any], expected: set[str], label: str) -> None:
    if set(value) != expected:
        raise L6ReleasePairError(f"{label} fields differ from format v1")


def validate_metadata_pair(
    source: dict[str, Any], target: dict[str, Any], contract: dict[str, Any]
) -> None:
    if set(source) != set(target):
        raise L6ReleasePairError("source and target product metadata fields differ")
    for field in sorted(set(target) - MUTABLE_METADATA_FIELDS):
        if source[field] != target[field]:
            raise L6ReleasePairError(f"source/target product contract drifted at {field}")
    if source.get("debian_revision") != contract["source"].get("debian_revision"):
        raise L6ReleasePairError("source Debian revision differs from pair contract")
    if source.get("package_version") != contract["source"].get("package_version"):
        raise L6ReleasePairError("source package version differs from pair contract")
    if target.get("debian_revision") != contract["target"].get("debian_revision"):
        raise L6ReleasePairError("target Debian revision differs from pair contract")
    if target.get("package_version") != contract["target"].get("package_version"):
        raise L6ReleasePairError("target package version differs from pair contract")
    try:
        source_revision = int(source["debian_revision"])
        target_revision = int(target["debian_revision"])
    except (KeyError, TypeError, ValueError) as exc:
        raise L6ReleasePairError("Debian revisions must be decimal integers") from exc
    if target_revision != source_revision + 1:
        raise L6ReleasePairError("source and target Debian revisions are not adjacent")
    shared = {field: target.get(field) for field in SHARED_CONTRACT_FIELDS}
    if contract.get("shared_contract") != shared:
        raise L6ReleasePairError("shared product contract differs from target metadata")


def load_contract(
    path: Path = CONTRACT_PATH,
    *,
    repository_root: Path = REPO_ROOT,
    source_metadata: dict[str, Any] | None = None,
    target_metadata: dict[str, Any] | None = None,
) -> dict[str, Any]:
    contract = load_json(path, "L6 release pair contract", canonical=True)
    require_exact_fields(contract, CONTRACT_KEYS, "L6 release pair contract")
    exact = {
        "format_version": 1,
        "profile": "debian13-arm64-release-pair-v1",
        "matrix_profile": "debian13-arm64-ephemeral-v1",
    }
    for field, expected in exact.items():
        if contract.get(field) != expected:
            raise L6ReleasePairError(f"{field} differs from L6 release pair v1")
    if contract.get("source", {}).get("repository_commit") != SOURCE_COMMIT:
        raise L6ReleasePairError("source commit differs from the frozen L6 identity")
    expected_build = {
        "environment_profile": "debian13-arm64-release-pair-build-v1",
        "network_policy": "dependency-frozen-before-build",
        "output_policy": "private-staging-then-atomic-publish-v1",
        "separate_clean_roots": True,
    }
    if contract.get("build") != expected_build:
        raise L6ReleasePairError("release pair build contract has drifted")
    expected_executables = [
        {
            "binary": "radishlex-linux-maintenance",
            "build_identity": "radishlex-linux-maintenance-production-v1",
            "cargo_package": "radishlex-linux-product-install",
            "feature_profile": "no-default-features",
            "role": "production-maintenance",
        },
        {
            "binary": "radishlex-linux-l6-acceptance",
            "build_identity": "radishlex-linux-l6-acceptance-v1",
            "cargo_package": "radishlex-linux-l6-acceptance",
            "feature_profile": "compile-isolated-l6-acceptance-checkpoints",
            "role": "acceptance-controller",
        },
    ]
    if contract.get("executables") != expected_executables:
        raise L6ReleasePairError("release pair executable identities have drifted")
    if contract.get("target", {}).get("commit_policy") != (
        "clean-descendant-head-distinct-from-source-v1"
    ):
        raise L6ReleasePairError("target commit policy has drifted")
    source_metadata = source_metadata or git_json_at_commit(
        repository_root, SOURCE_COMMIT, METADATA_RELATIVE
    )
    target_metadata = target_metadata or load_json(
        repository_root / METADATA_RELATIVE, "target Linux product metadata"
    )
    validate_metadata_pair(source_metadata, target_metadata, contract)
    return contract


def require_real_file(path: Path, label: str, mode: int) -> Path:
    if not path.is_absolute() or path.is_symlink() or not path.is_file():
        raise L6ReleasePairError(f"{label} must be an absolute regular file")
    canonical = path.parent.resolve() / path.name
    if canonical != path:
        raise L6ReleasePairError(f"{label} must not traverse symlinked components")
    file_stat = path.stat()
    if file_stat.st_nlink != 1 or stat.S_IMODE(file_stat.st_mode) != mode:
        raise L6ReleasePairError(f"{label} identity or mode is unsafe")
    return path


def require_clean_root(path: Path, label: str) -> tuple[Path, str]:
    if not path.is_absolute() or path.is_symlink() or not path.is_dir():
        raise L6ReleasePairError(f"{label} must be an absolute real directory")
    canonical = path.resolve()
    if canonical != path:
        raise L6ReleasePairError(f"{label} must be canonical")
    commit = run_git(path, "rev-parse", "HEAD")
    if COMMIT_PATTERN.fullmatch(commit) is None:
        raise L6ReleasePairError(f"{label} HEAD is invalid")
    if run_git(path, "status", "--porcelain=v1", "--untracked-files=all"):
        raise L6ReleasePairError(f"{label} must be clean")
    require_clean_build_outputs(path, label)
    return path, commit


def require_clean_build_outputs(root: Path, label: str) -> None:
    for relative in CLEAN_OUTPUT_RELATIVES:
        candidate = root / relative
        if candidate.exists() or candidate.is_symlink():
            raise L6ReleasePairError(
                f"{label} contains preexisting build output: {relative.as_posix()}"
            )


def verify_repository_roots(source: Path, target: Path) -> tuple[str, str]:
    source, source_commit = require_clean_root(source, "source repository root")
    target, target_commit = require_clean_root(target, "target repository root")
    if source == target:
        raise L6ReleasePairError("source and target require separate clean roots")
    if target != REPO_ROOT.resolve():
        raise L6ReleasePairError(
            "target root must be the repository that owns the pair builder"
        )
    if source_commit != SOURCE_COMMIT:
        raise L6ReleasePairError("source root is not the frozen source commit")
    if target_commit == source_commit:
        raise L6ReleasePairError("target commit must differ from source")
    try:
        subprocess.run(
            ["/usr/bin/git", "-C", str(target), "merge-base", "--is-ancestor", source_commit, target_commit],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            env={"LC_ALL": "C", "PATH": "/usr/bin:/bin"},
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        raise L6ReleasePairError("target commit is not a descendant of source") from exc
    return source_commit, target_commit


def safe_version(value: str, label: str) -> str:
    value = value.strip()
    if VERSION_PATTERN.fullmatch(value) is None or "\n" in value or "\r" in value:
        raise L6ReleasePairError(f"{label} version is not bounded printable text")
    for forbidden in ("/Users/", "/home/", "/workspace/", "/tmp/", "/private/"):
        if forbidden in value:
            raise L6ReleasePairError(f"{label} version contains a local path")
    return value


def tool_version(command: list[str], label: str, *, json_field: str | None = None) -> str:
    try:
        result = subprocess.run(
            command,
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            env={**os.environ, "LC_ALL": "C"},
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        raise L6ReleasePairError(f"cannot read {label} version: {exc}") from exc
    if json_field is not None:
        try:
            value = json.loads(result.stdout)[json_field]
        except Exception as exc:
            raise L6ReleasePairError(f"invalid {label} version output") from exc
    else:
        value = result.stdout.splitlines()[0] if result.stdout.splitlines() else ""
    if not isinstance(value, str):
        raise L6ReleasePairError(f"invalid {label} version value")
    return safe_version(value, label)


def collect_environment() -> dict[str, Any]:
    if platform.system() != "Linux" or platform.machine() not in {"aarch64", "arm64"}:
        raise L6ReleasePairError("release pair build evidence requires ARM64 Linux")
    os_release: dict[str, str] = {}
    try:
        for line in Path("/etc/os-release").read_text(encoding="utf-8").splitlines():
            if "=" in line:
                key, value = line.split("=", 1)
                os_release[key] = value.strip().strip('"')
    except OSError as exc:
        raise L6ReleasePairError(f"cannot read /etc/os-release: {exc}") from exc
    operating_system = {
        "codename": os_release.get("VERSION_CODENAME"),
        "id": os_release.get("ID"),
        "version_id": os_release.get("VERSION_ID"),
    }
    if operating_system != {"codename": "trixie", "id": "debian", "version_id": "13"}:
        raise L6ReleasePairError("release pair build requires Debian 13 trixie")
    environment = {
        "architecture": "arm64",
        "format_version": 1,
        "operating_system": operating_system,
        "profile": "debian13-arm64-release-pair-build-v1",
        "tools": {
            "cargo": tool_version(["cargo", "--version"], "cargo"),
            "cmake": tool_version(["cmake", "--version"], "cmake"),
            "dpkg_shlibdeps": tool_version(["dpkg-shlibdeps", "--version"], "dpkg-shlibdeps"),
            "flutter": tool_version(["flutter", "--version", "--machine"], "flutter", json_field="frameworkVersion"),
            "git": tool_version(["/usr/bin/git", "--version"], "git"),
            "rustc": tool_version(["rustc", "--version"], "rustc"),
        },
    }
    validate_environment(environment)
    return environment


def validate_environment(value: dict[str, Any]) -> None:
    if not isinstance(value, dict):
        raise L6ReleasePairError("release pair build environment must be an object")
    require_exact_fields(value, ENVIRONMENT_KEYS, "release pair build environment")
    if value.get("format_version") != 1 or value.get("architecture") != "arm64":
        raise L6ReleasePairError("release pair build environment identity is invalid")
    if value.get("profile") != "debian13-arm64-release-pair-build-v1":
        raise L6ReleasePairError("release pair build environment profile is invalid")
    if value.get("operating_system") != {
        "codename": "trixie",
        "id": "debian",
        "version_id": "13",
    }:
        raise L6ReleasePairError("release pair operating system identity is invalid")
    tools = value.get("tools")
    if not isinstance(tools, dict) or set(tools) != {
        "cargo", "cmake", "dpkg_shlibdeps", "flutter", "git", "rustc"
    }:
        raise L6ReleasePairError("release pair tool inventory is invalid")
    for label, version in tools.items():
        if not isinstance(version, str):
            raise L6ReleasePairError(f"{label} version must be text")
        safe_version(version, label)


def parse_aarch64_elf(path: Path, role: str) -> dict[str, Any]:
    path = require_real_file(path, f"{role} executable", 0o755)
    if path.stat().st_size > MAX_EXECUTABLE_SIZE:
        raise L6ReleasePairError(f"{role} executable exceeds the size limit")
    value = path.read_bytes()
    if len(value) < 64 or value[:7] != b"\x7fELF\x02\x01\x01":
        raise L6ReleasePairError(f"{role} executable is not ELF64 little-endian")
    machine = struct.unpack_from("<H", value, 18)[0]
    if machine != ELF_MACHINE_AARCH64:
        raise L6ReleasePairError(f"{role} executable is not AArch64")
    elf_type = struct.unpack_from("<H", value, 16)[0]
    if elf_type not in {2, 3}:
        raise L6ReleasePairError(f"{role} executable ELF type is invalid")
    phoff = struct.unpack_from("<Q", value, 32)[0]
    phentsize = struct.unpack_from("<H", value, 54)[0]
    phnum = struct.unpack_from("<H", value, 56)[0]
    if phentsize != 56 or phnum == 0 or phnum > 128 or phoff + phentsize * phnum > len(value):
        raise L6ReleasePairError(f"{role} executable program headers are invalid")
    interpreters: list[str] = []
    for index in range(phnum):
        header = struct.unpack_from("<IIQQQQQQ", value, phoff + index * phentsize)
        if header[0] != 3:
            continue
        offset, size = header[2], header[5]
        if size < 2 or offset + size > len(value):
            raise L6ReleasePairError(f"{role} executable interpreter is invalid")
        try:
            interpreters.append(value[offset : offset + size].rstrip(b"\0").decode("ascii"))
        except UnicodeDecodeError as exc:
            raise L6ReleasePairError(f"{role} executable interpreter is invalid") from exc
    if interpreters != [ELF_INTERPRETER]:
        raise L6ReleasePairError(f"{role} executable interpreter differs from ARM64 identity")
    marker_presence = [marker in value for marker in ACCEPTANCE_MARKERS]
    if role == "production-maintenance" and any(marker_presence):
        raise L6ReleasePairError("production executable contains acceptance capability")
    if role == "acceptance-controller" and not all(marker_presence):
        raise L6ReleasePairError("acceptance executable lacks compile identity markers")
    return {
        "class": "ELF64",
        "interpreter": ELF_INTERPRETER,
        "machine": "AArch64",
        "type": "dynamic-or-executable",
    }


def verify_debian_artifact(release: ReleaseInput) -> dict[str, Any]:
    tool = release.root / ARTIFACT_TOOL_RELATIVE
    if not tool.is_file() or tool.is_symlink():
        raise L6ReleasePairError(f"{release.role} artifact verifier is unavailable")
    try:
        subprocess.run(
            [sys.executable, str(tool), "verify", "--package", str(release.package), "--evidence", str(release.artifact_evidence)],
            cwd=release.root,
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            env={"LC_ALL": "C", "PATH": "/usr/bin:/bin", "PYTHONDONTWRITEBYTECODE": "1"},
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        raise L6ReleasePairError(f"{release.role} Debian artifact verification failed: {exc}") from exc
    return load_json(release.artifact_evidence, f"{release.role} Debian evidence", canonical=True)


def release_record(
    release: ReleaseInput,
    commit: str,
    expected: dict[str, Any],
    verifier: Callable[[ReleaseInput], dict[str, Any]],
) -> dict[str, Any]:
    package = require_real_file(release.package, f"{release.role} Debian package", 0o644)
    evidence_path = require_real_file(
        release.artifact_evidence, f"{release.role} Debian artifact evidence", 0o644
    )
    evidence = verifier(release)
    package_value = evidence.get("package")
    control = evidence.get("control")
    manifest = evidence.get("product_manifest")
    if not all(isinstance(item, dict) for item in (package_value, control, manifest)):
        raise L6ReleasePairError(f"{release.role} Debian evidence is incomplete")
    if package_value.get("filename") != package.name:
        raise L6ReleasePairError(f"{release.role} package filename differs from evidence")
    package_bytes = package.read_bytes()
    if package_value.get("size") != len(package_bytes) or package_value.get("sha256") != sha256_bytes(package_bytes):
        raise L6ReleasePairError(f"{release.role} package identity differs from evidence")
    if control.get("version") != expected["package_version"] or control.get("architecture") != "arm64":
        raise L6ReleasePairError(f"{release.role} package control identity is invalid")
    dependencies = control.get("depends")
    if not isinstance(dependencies, list) or not dependencies or not all(isinstance(item, str) for item in dependencies):
        raise L6ReleasePairError(f"{release.role} package dependencies are invalid")
    evidence_bytes = evidence_path.read_bytes()
    return {
        "artifact_evidence": {
            "filename": evidence_path.name,
            "sha256": sha256_bytes(evidence_bytes),
            "size": len(evidence_bytes),
        },
        "debian_revision": expected["debian_revision"],
        "dependencies": {
            "count": len(dependencies),
            "sha256": sha256_bytes(("\n".join(dependencies) + "\n").encode("utf-8")),
        },
        "package": package_value,
        "package_version": expected["package_version"],
        "product_manifest_sha256": manifest.get("sha256"),
        "repository_commit": commit,
    }


def build_record(
    contract: dict[str, Any],
    environment: dict[str, Any],
    source: ReleaseInput,
    target: ReleaseInput,
    maintenance: Path,
    acceptance: Path,
    *,
    commits: tuple[str, str] | None = None,
    verifier: Callable[[ReleaseInput], dict[str, Any]] = verify_debian_artifact,
) -> dict[str, Any]:
    validate_environment(environment)
    source_commit, target_commit = commits or verify_repository_roots(source.root, target.root)
    if commits is None:
        source_metadata = load_json(
            source.root / METADATA_RELATIVE, "source Linux product metadata"
        )
        target_metadata = load_json(
            target.root / METADATA_RELATIVE, "target Linux product metadata"
        )
        validate_metadata_pair(source_metadata, target_metadata, contract)
    source_release = release_record(source, source_commit, contract["source"], verifier)
    target_release = release_record(target, target_commit, contract["target"], verifier)
    if source_release["package"]["sha256"] == target_release["package"]["sha256"]:
        raise L6ReleasePairError("source and target Debian artifacts must be distinct")
    if source_release["artifact_evidence"]["sha256"] == target_release["artifact_evidence"]["sha256"]:
        raise L6ReleasePairError("source and target artifact evidence must be distinct")
    if source_release["product_manifest_sha256"] == target_release["product_manifest_sha256"]:
        raise L6ReleasePairError("source and target product manifests must be distinct")
    if source_release["dependencies"] != target_release["dependencies"]:
        raise L6ReleasePairError("source and target dependency identities must match")
    executable_paths = {
        "production-maintenance": maintenance,
        "acceptance-controller": acceptance,
    }
    executables = []
    for identity in contract["executables"]:
        role = identity["role"]
        path = executable_paths[role]
        if path.name != identity["binary"]:
            raise L6ReleasePairError(f"{role} executable filename differs from identity")
        elf = parse_aarch64_elf(path, role)
        data = path.read_bytes()
        executables.append(
            {
                **identity,
                "elf": elf,
                "filename": path.name,
                "repository_commit": target_commit,
                "sha256": sha256_bytes(data),
                "size": len(data),
            }
        )
    if executables[0]["sha256"] == executables[1]["sha256"]:
        raise L6ReleasePairError("production and acceptance executables must be distinct")
    record = {
        "build_environment": environment,
        "evidence_format": "radishlex-linux-l6-release-pair-evidence-v1",
        "executables": executables,
        "format_version": 1,
        "matrix_profile": contract["matrix_profile"],
        "profile": contract["profile"],
        "releases": {"source": source_release, "target": target_release},
        "shared_contract": contract["shared_contract"],
    }
    validate_record(record, contract)
    return record


def validate_record(
    value: dict[str, Any], contract: dict[str, Any] | None = None
) -> None:
    require_exact_fields(value, RECORD_KEYS, "L6 release pair evidence")
    if value.get("evidence_format") != "radishlex-linux-l6-release-pair-evidence-v1":
        raise L6ReleasePairError("L6 release pair evidence format is invalid")
    if value.get("format_version") != 1 or value.get("profile") != "debian13-arm64-release-pair-v1":
        raise L6ReleasePairError("L6 release pair evidence identity is invalid")
    if value.get("matrix_profile") != "debian13-arm64-ephemeral-v1":
        raise L6ReleasePairError("L6 release pair matrix identity is invalid")
    validate_environment(value.get("build_environment", {}))
    serialized = canonical_json_bytes(value).decode("utf-8")
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
    ):
        if forbidden in serialized.lower():
            raise L6ReleasePairError("L6 release pair evidence contains forbidden raw identity")
    if len(serialized.encode("utf-8")) > MAX_EVIDENCE_SIZE:
        raise L6ReleasePairError("L6 release pair evidence exceeds the size limit")
    releases = value.get("releases")
    if not isinstance(releases, dict) or set(releases) != {"source", "target"}:
        raise L6ReleasePairError("L6 release pair release inventory is invalid")
    for role in ("source", "target"):
        release = releases[role]
        if not isinstance(release, dict) or set(release) != {
            "artifact_evidence", "debian_revision", "dependencies", "package", "package_version", "product_manifest_sha256", "repository_commit"
        }:
            raise L6ReleasePairError(f"{role} release evidence fields are invalid")
        if COMMIT_PATTERN.fullmatch(str(release["repository_commit"])) is None:
            raise L6ReleasePairError(f"{role} repository commit is invalid")
        artifact_evidence = release["artifact_evidence"]
        dependencies = release["dependencies"]
        package = release["package"]
        if not isinstance(artifact_evidence, dict) or set(artifact_evidence) != {
            "filename", "sha256", "size"
        }:
            raise L6ReleasePairError(f"{role} artifact evidence identity is invalid")
        if not isinstance(dependencies, dict) or set(dependencies) != {"count", "sha256"}:
            raise L6ReleasePairError(f"{role} dependency identity is invalid")
        if not isinstance(package, dict) or set(package) != {"filename", "sha256", "size"}:
            raise L6ReleasePairError(f"{role} package identity is invalid")
        if not isinstance(artifact_evidence["filename"], str) or not artifact_evidence["filename"].endswith(".deb.evidence.json"):
            raise L6ReleasePairError(f"{role} artifact evidence filename is invalid")
        if Path(artifact_evidence["filename"]).name != artifact_evidence["filename"]:
            raise L6ReleasePairError(f"{role} artifact evidence filename is not a basename")
        if artifact_evidence["filename"] != f'{package["filename"]}.evidence.json':
            raise L6ReleasePairError(f"{role} artifact/evidence filenames differ")
        if not isinstance(package["filename"], str) or not package["filename"].endswith("_arm64.deb"):
            raise L6ReleasePairError(f"{role} package filename is invalid")
        if Path(package["filename"]).name != package["filename"]:
            raise L6ReleasePairError(f"{role} package filename is not a basename")
        if not isinstance(release["debian_revision"], str) or not release["debian_revision"].isdigit():
            raise L6ReleasePairError(f"{role} Debian revision is invalid")
        if not isinstance(release["package_version"], str) or release["package_version"] not in package["filename"]:
            raise L6ReleasePairError(f"{role} package version is invalid")
        if not isinstance(dependencies["count"], int) or dependencies["count"] <= 0:
            raise L6ReleasePairError(f"{role} dependency count is invalid")
        for identity in (artifact_evidence, package):
            if not isinstance(identity["size"], int) or identity["size"] <= 0:
                raise L6ReleasePairError(f"{role} artifact size is invalid")
        if package["size"] > MAX_PACKAGE_SIZE or artifact_evidence["size"] > MAX_EVIDENCE_SIZE:
            raise L6ReleasePairError(f"{role} artifact exceeds the size limit")
        for digest in (
            artifact_evidence.get("sha256"),
            dependencies.get("sha256"),
            package.get("sha256"),
            release["product_manifest_sha256"],
        ):
            if not isinstance(digest, str) or SHA256_PATTERN.fullmatch(digest) is None:
                raise L6ReleasePairError(f"{role} release digest is invalid")
    executables = value.get("executables")
    if not isinstance(executables, list) or len(executables) != 2:
        raise L6ReleasePairError("L6 executable inventory is invalid")
    expected_executable_fields = {
        "binary", "build_identity", "cargo_package", "elf", "feature_profile",
        "filename", "repository_commit", "role", "sha256", "size"
    }
    for executable in executables:
        if not isinstance(executable, dict) or set(executable) != expected_executable_fields:
            raise L6ReleasePairError("L6 executable evidence fields are invalid")
        if not isinstance(executable, dict) or SHA256_PATTERN.fullmatch(str(executable.get("sha256"))) is None:
            raise L6ReleasePairError("L6 executable evidence is invalid")
        if executable["filename"] != executable["binary"]:
            raise L6ReleasePairError("L6 executable filename differs from build identity")
        if not isinstance(executable["size"], int) or not 0 < executable["size"] <= MAX_EXECUTABLE_SIZE:
            raise L6ReleasePairError("L6 executable size is invalid")
        if COMMIT_PATTERN.fullmatch(str(executable["repository_commit"])) is None:
            raise L6ReleasePairError("L6 executable repository commit is invalid")
        if executable["elf"] != {
            "class": "ELF64",
            "interpreter": ELF_INTERPRETER,
            "machine": "AArch64",
            "type": "dynamic-or-executable",
        }:
            raise L6ReleasePairError("L6 executable ELF identity is invalid")
    if [item["role"] for item in executables] != [
        "production-maintenance", "acceptance-controller"
    ]:
        raise L6ReleasePairError("L6 executable role order is invalid")
    if executables[0]["sha256"] == executables[1]["sha256"]:
        raise L6ReleasePairError("production and acceptance executable identities overlap")
    if releases["source"]["package"]["sha256"] == releases["target"]["package"]["sha256"]:
        raise L6ReleasePairError("source and target package identities overlap")
    if int(releases["target"]["debian_revision"]) != int(releases["source"]["debian_revision"]) + 1:
        raise L6ReleasePairError("release evidence revisions are not adjacent")
    if contract is not None:
        if value["profile"] != contract["profile"] or value["matrix_profile"] != contract["matrix_profile"]:
            raise L6ReleasePairError("release evidence profile differs from contract")
        if value["shared_contract"] != contract["shared_contract"]:
            raise L6ReleasePairError("release evidence shared contract has drifted")
        for role in ("source", "target"):
            for field in ("debian_revision", "package_version"):
                if releases[role][field] != contract[role][field]:
                    raise L6ReleasePairError(f"{role} release {field} differs from contract")
        if releases["source"]["repository_commit"] != contract["source"]["repository_commit"]:
            raise L6ReleasePairError("source evidence commit differs from contract")
        if releases["target"]["repository_commit"] == releases["source"]["repository_commit"]:
            raise L6ReleasePairError("target evidence commit must differ from source")
        for executable, expected in zip(executables, contract["executables"], strict=True):
            for field, expected_value in expected.items():
                if executable[field] != expected_value:
                    raise L6ReleasePairError(f"{executable['role']} identity differs from contract")
            if executable["repository_commit"] != releases["target"]["repository_commit"]:
                raise L6ReleasePairError("executable commit differs from target release")


def verify_published_record(
    evidence_path: Path, record: dict[str, Any], contract: dict[str, Any]
) -> None:
    validate_record(record, contract)
    root = evidence_path.parent
    if evidence_path.name != "release-pair.evidence.json":
        raise L6ReleasePairError("release pair evidence filename is invalid")
    expected_inventory = {
        "build-environment.json",
        "radishlex-linux-l6-acceptance",
        "radishlex-linux-maintenance",
        "release-pair.evidence.json",
        "source",
        "source/artifacts",
        f'source/artifacts/{record["releases"]["source"]["package"]["filename"]}',
        f'source/artifacts/{record["releases"]["source"]["artifact_evidence"]["filename"]}',
        "target",
        "target/artifacts",
        f'target/artifacts/{record["releases"]["target"]["package"]["filename"]}',
        f'target/artifacts/{record["releases"]["target"]["artifact_evidence"]["filename"]}',
    }
    actual_inventory: set[str] = set()
    for path in root.rglob("*"):
        if path.is_symlink():
            raise L6ReleasePairError("release pair publication contains a symlink")
        if path.is_dir():
            if stat.S_IMODE(path.stat().st_mode) != 0o755:
                raise L6ReleasePairError("release pair publication directory mode is invalid")
        elif not path.is_file():
            raise L6ReleasePairError("release pair publication contains an unsupported entry")
        actual_inventory.add(path.relative_to(root).as_posix())
    if actual_inventory != expected_inventory:
        raise L6ReleasePairError("release pair publication inventory differs from evidence")
    environment_path = require_real_file(
        root / "build-environment.json", "release pair build environment", 0o644
    )
    environment = load_json(
        environment_path, "release pair build environment", canonical=True
    )
    if environment != record["build_environment"]:
        raise L6ReleasePairError("build environment file differs from release pair evidence")
    for role in ("source", "target"):
        release = record["releases"][role]
        artifact_root = root / role / "artifacts"
        for field in ("package", "artifact_evidence"):
            identity = release[field]
            path = require_real_file(
                artifact_root / identity["filename"],
                f"{role} published {field}",
                0o644,
            )
            data = path.read_bytes()
            if len(data) != identity["size"] or sha256_bytes(data) != identity["sha256"]:
                raise L6ReleasePairError(f"{role} published {field} differs from evidence")
    for executable in record["executables"]:
        path = root / executable["filename"]
        data = require_real_file(
            path, f"published {executable['role']} executable", 0o755
        ).read_bytes()
        if len(data) != executable["size"] or sha256_bytes(data) != executable["sha256"]:
            raise L6ReleasePairError(
                f"published {executable['role']} executable differs from evidence"
            )
        if parse_aarch64_elf(path, executable["role"]) != executable["elf"]:
            raise L6ReleasePairError(
                f"published {executable['role']} ELF identity differs from evidence"
            )


def require_output(path: Path) -> Path:
    if not path.is_absolute() or path.exists() or path.is_symlink():
        raise L6ReleasePairError("--output must be an absent absolute path")
    parent = path.parent
    if parent.is_symlink() or not parent.is_dir() or parent.resolve() != parent:
        raise L6ReleasePairError("--output parent must be an existing real directory")
    return path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Freeze or verify an ARM64 L6 release pair identity.")
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("validate-contract")
    roots = subparsers.add_parser("validate-roots")
    roots.add_argument("--source-root", type=Path, required=True)
    roots.add_argument("--target-root", type=Path, required=True)
    environment = subparsers.add_parser("environment")
    environment.add_argument("--output", type=Path, required=True)
    record = subparsers.add_parser("record")
    for role in ("source", "target"):
        record.add_argument(f"--{role}-root", type=Path, required=True)
        record.add_argument(f"--{role}-package", type=Path, required=True)
        record.add_argument(f"--{role}-artifact-evidence", type=Path, required=True)
    record.add_argument("--environment", type=Path, required=True)
    record.add_argument("--maintenance-executable", type=Path, required=True)
    record.add_argument("--acceptance-executable", type=Path, required=True)
    record.add_argument("--output", type=Path, required=True)
    verify = subparsers.add_parser("verify")
    verify.add_argument("--evidence", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        contract = load_contract()
        if args.command == "validate-contract":
            return 0
        if args.command == "validate-roots":
            verify_repository_roots(args.source_root, args.target_root)
            source_metadata = load_json(
                args.source_root / METADATA_RELATIVE,
                "source Linux product metadata",
            )
            target_metadata = load_json(
                args.target_root / METADATA_RELATIVE,
                "target Linux product metadata",
            )
            validate_metadata_pair(source_metadata, target_metadata, contract)
            return 0
        if args.command == "environment":
            output = require_output(args.output)
            output.write_bytes(canonical_json_bytes(collect_environment()))
            output.chmod(0o644)
            return 0
        if args.command == "verify":
            evidence_path = require_real_file(args.evidence, "L6 release pair evidence", 0o644)
            evidence = load_json(evidence_path, "L6 release pair evidence", canonical=True)
            verify_published_record(evidence_path, evidence, contract)
            return 0
        output = require_output(args.output)
        environment = load_json(args.environment, "release pair build environment", canonical=True)
        source = ReleaseInput("source", args.source_root, args.source_package, args.source_artifact_evidence)
        target = ReleaseInput("target", args.target_root, args.target_package, args.target_artifact_evidence)
        evidence = build_record(
            contract,
            environment,
            source,
            target,
            args.maintenance_executable,
            args.acceptance_executable,
        )
        output.write_bytes(canonical_json_bytes(evidence))
        output.chmod(0o644)
    except L6ReleasePairError as exc:
        raise SystemExit(str(exc)) from exc
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

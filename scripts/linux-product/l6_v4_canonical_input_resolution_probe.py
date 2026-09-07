#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable


EVIDENCE_FORMAT = "radishlex-linux-l6-v4-canonical-input-resolution-probe-v1"
FINAL_INPUT_ROOT = Path("/var/tmp/radishlex-l6-inputs")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
MAX_SMALL_FILE_BYTES = 64 * 1024
MAX_CMDLINE_BYTES = 64 * 1024
EXIT_PASSED = 0
EXIT_INDETERMINATE = 10
EXIT_PRECONDITION_REJECTED = 20


class ResolutionProbeError(ValueError):
    pass


@dataclass(frozen=True)
class FinalInputMember:
    path: str
    mode: int
    size: int
    sha256: str


@dataclass(frozen=True)
class ResolutionProbeRequest:
    resolution_attempt_id: str
    transfer_attempt_id: str
    resolution_root: Path
    transfer_root: Path
    expected_installer_size: int
    expected_installer_sha256: str
    expected_bundle_size: int
    expected_bundle_sha256: str
    expected_transfer_evidence_size: int
    expected_transfer_evidence_sha256: str

    @property
    def probe_path(self) -> Path:
        return self.resolution_root / "resolution-probe.py"

    @property
    def marker_path(self) -> Path:
        return self.resolution_root / "attempt.marker"

    @property
    def evidence_path(self) -> Path:
        return self.resolution_root / "resolution.evidence.json"

    @property
    def installer_path(self) -> Path:
        return self.transfer_root / "install-canonical-input.py"

    @property
    def bundle_path(self) -> Path:
        return self.transfer_root / "canonical-input.ustar.incoming"

    @property
    def transfer_marker_path(self) -> Path:
        return self.transfer_root / "attempt.marker"

    @property
    def transfer_evidence_path(self) -> Path:
        return self.transfer_root / "transfer.evidence.json"

    @property
    def staging_root(self) -> Path:
        return Path(
            f"/var/tmp/.radishlex-l6-inputs-{self.transfer_attempt_id}.incoming"
        )

    def validate(self) -> None:
        for value, label in (
            (self.resolution_attempt_id, "resolution-attempt-id"),
            (self.transfer_attempt_id, "transfer-attempt-id"),
        ):
            if not SAFE_ATTEMPT_ID.fullmatch(value):
                raise ResolutionProbeError(f"{label}-invalid")
        if self.resolution_attempt_id == self.transfer_attempt_id:
            raise ResolutionProbeError("attempt-ids-must-be-distinct")
        expected_resolution_root = Path(
            "/var/tmp/radishlex-l6-v4-input-resolution-"
            f"{self.resolution_attempt_id}"
        )
        expected_transfer_root = Path(
            "/var/tmp/radishlex-l6-v4-input-transfer-"
            f"{self.transfer_attempt_id}"
        )
        if self.resolution_root != expected_resolution_root:
            raise ResolutionProbeError("resolution-root-mismatch")
        if self.transfer_root != expected_transfer_root:
            raise ResolutionProbeError("transfer-root-mismatch")
        for value, label, maximum in (
            (self.expected_installer_size, "installer-size", MAX_SMALL_FILE_BYTES),
            (self.expected_bundle_size, "bundle-size", 256 * 1024 * 1024),
            (
                self.expected_transfer_evidence_size,
                "transfer-evidence-size",
                MAX_SMALL_FILE_BYTES,
            ),
        ):
            if not 1 <= value <= maximum:
                raise ResolutionProbeError(f"{label}-invalid")
        for value, label in (
            (self.expected_installer_sha256, "installer-sha256"),
            (self.expected_bundle_sha256, "bundle-sha256"),
            (
                self.expected_transfer_evidence_sha256,
                "transfer-evidence-sha256",
            ),
        ):
            if not HEX_64.fullmatch(value):
                raise ResolutionProbeError(f"{label}-invalid")


ProcessObserver = Callable[[ResolutionProbeRequest], dict[str, int]]


def run_resolution_probe(
    request: ResolutionProbeRequest,
    *,
    identity_uid: int = 0,
    identity_gid: int = 0,
    final_input_root: Path = FINAL_INPUT_ROOT,
    staging_root: Path | None = None,
    process_observer: ProcessObserver | None = None,
) -> tuple[dict[str, object], int]:
    request.validate()
    if os.geteuid() != identity_uid or os.getegid() != identity_gid:
        raise ResolutionProbeError("root-identity-required")
    _require_directory(
        request.resolution_root,
        mode=0o700,
        uid=identity_uid,
        gid=identity_gid,
        label="resolution-root",
    )
    _require_regular(
        request.probe_path,
        mode=0o600,
        uid=identity_uid,
        gid=identity_gid,
        label="resolution-probe",
    )
    for path, label in (
        (request.marker_path, "resolution-attempt-marker"),
        (request.evidence_path, "resolution-evidence"),
    ):
        if path.exists() or path.is_symlink():
            raise ResolutionProbeError(f"{label}-must-be-absent")
    _write_exclusive(
        request.marker_path,
        (request.resolution_attempt_id + "\n").encode("ascii"),
        mode=0o600,
    )

    phase = "process-static"
    reason = "none"
    outcome = "passed"
    transfer_root_state = "unknown"
    staging_root_state = "unknown"
    final_input_root_state = "unknown"
    installer_identity = "not-observed"
    bundle_identity = "not-observed"
    transfer_marker_identity = "not-observed"
    transfer_evidence_identity = "not-observed"
    final_input_inventory_identity = "not-observed"
    observation = {
        "active_installer_count": 0,
        "suspicious_reference_count": 0,
    }
    try:
        observe = process_observer or observe_related_processes
        observation = observe(request)
        if set(observation) != {
            "active_installer_count",
            "suspicious_reference_count",
        } or any(
            not isinstance(value, int) or isinstance(value, bool) or value < 0
            for value in observation.values()
        ):
            raise ResolutionProbeError("process-observation-shape-invalid")
        if any(observation.values()):
            outcome = "indeterminate"
            reason = "installer-or-related-process-active"
        else:
            phase = "path-identity"
            _require_directory(
                request.transfer_root,
                mode=0o700,
                uid=identity_uid,
                gid=identity_gid,
                label="transfer-root",
            )
            transfer_root_state = "private-directory"
            _require_regular_identity(
                request.installer_path,
                size=request.expected_installer_size,
                sha256=request.expected_installer_sha256,
                uid=identity_uid,
                gid=identity_gid,
                label="installer",
            )
            installer_identity = "matched"
            _require_regular_identity(
                request.bundle_path,
                size=request.expected_bundle_size,
                sha256=request.expected_bundle_sha256,
                uid=identity_uid,
                gid=identity_gid,
                label="source-bundle",
            )
            bundle_identity = "matched"
            marker = _require_regular(
                request.transfer_marker_path,
                mode=0o600,
                uid=identity_uid,
                gid=identity_gid,
                label="transfer-attempt-marker",
            )
            if marker.st_size > MAX_SMALL_FILE_BYTES or (
                request.transfer_marker_path.read_bytes()
                != (request.transfer_attempt_id + "\n").encode("ascii")
            ):
                raise ResolutionProbeError(
                    "transfer-attempt-marker-content-invalid"
                )
            transfer_marker_identity = "matched"
            _require_regular_identity(
                request.transfer_evidence_path,
                size=request.expected_transfer_evidence_size,
                sha256=request.expected_transfer_evidence_sha256,
                uid=identity_uid,
                gid=identity_gid,
                label="transfer-evidence",
            )
            transfer_evidence_identity = "matched"
            final_inventory = _load_transfer_inventory(
                request.transfer_evidence_path
            )
            staging_root_state = _directory_state(
                staging_root or request.staging_root,
                uid=identity_uid,
                gid=identity_gid,
                label="staging-root",
            )
            final_input_root_state = _directory_state(
                final_input_root,
                uid=identity_uid,
                gid=identity_gid,
                label="final-input-root",
            )
            if final_input_root_state == "private-directory":
                _verify_final_input_tree(
                    final_input_root,
                    final_inventory,
                    uid=identity_uid,
                    gid=identity_gid,
                )
                final_input_inventory_identity = "matched"
            phase = "complete"
    except (OSError, ResolutionProbeError) as exc:
        outcome = "indeterminate"
        reason = _reason(exc)

    evidence = {
        "active_installer_count": observation["active_installer_count"],
        "automatic_cleanup": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "bundle_identity": bundle_identity,
        "final_input_root": str(final_input_root),
        "final_input_inventory_identity": final_input_inventory_identity,
        "final_input_root_state": final_input_root_state,
        "format": EVIDENCE_FORMAT,
        "installer_identity": installer_identity,
        "operation_id": "not-generated",
        "outcome": outcome,
        "phase": phase,
        "reason": reason,
        "resolution_attempt_id": request.resolution_attempt_id,
        "staging_root_state": staging_root_state,
        "suspicious_reference_count": observation[
            "suspicious_reference_count"
        ],
        "transaction": "not-performed",
        "transfer_attempt_id": request.transfer_attempt_id,
        "transfer_evidence_identity": transfer_evidence_identity,
        "transfer_marker_identity": transfer_marker_identity,
        "transfer_root_state": transfer_root_state,
    }
    write_evidence_exclusive(request.evidence_path, evidence)
    return evidence, EXIT_PASSED if outcome == "passed" else EXIT_INDETERMINATE


def observe_related_processes(
    request: ResolutionProbeRequest,
    *,
    proc_root: Path = Path("/proc"),
) -> dict[str, int]:
    expected = (
        "/usr/bin/python3",
        str(request.installer_path),
        "--attempt-id",
        request.transfer_attempt_id,
        "--transfer-root",
        str(request.transfer_root),
        "--bundle-path",
        str(request.bundle_path),
        "--expected-bundle-size",
        str(request.expected_bundle_size),
        "--expected-bundle-sha256",
        request.expected_bundle_sha256,
    )
    installer_bytes = str(request.installer_path).encode("utf-8")
    transfer_root_bytes = str(request.transfer_root).encode("utf-8")
    active = 0
    suspicious = 0
    own_pid = os.getpid()
    try:
        entries = tuple(proc_root.iterdir())
    except OSError as exc:
        raise ResolutionProbeError("proc-unavailable") from exc
    for entry in entries:
        if not entry.name.isdigit() or int(entry.name) == own_pid:
            continue
        try:
            with (entry / "cmdline").open("rb") as source:
                raw = source.read(MAX_CMDLINE_BYTES + 1)
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
        except OSError as exc:
            raise ResolutionProbeError("proc-cmdline-read-failed") from exc
        if len(raw) > MAX_CMDLINE_BYTES:
            if installer_bytes in raw or transfer_root_bytes in raw:
                suspicious += 1
            continue
        if installer_bytes not in raw and transfer_root_bytes not in raw:
            continue
        try:
            fields = tuple(
                part.decode("utf-8") for part in raw.rstrip(b"\0").split(b"\0")
            )
        except UnicodeDecodeError:
            suspicious += 1
            continue
        if fields == expected:
            active += 1
        else:
            suspicious += 1
    return {
        "active_installer_count": active,
        "suspicious_reference_count": suspicious,
    }


def write_evidence_exclusive(path: Path, evidence: dict[str, object]) -> None:
    payload = (
        json.dumps(evidence, ensure_ascii=True, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    _write_exclusive(path, payload, mode=0o600)
    _fsync_directory(path.parent)


def _directory_state(path: Path, *, uid: int, gid: int, label: str) -> str:
    try:
        info = path.lstat()
    except FileNotFoundError:
        return "absent"
    except OSError as exc:
        raise ResolutionProbeError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISDIR(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or stat.S_IMODE(info.st_mode) != 0o700
        or info.st_uid != uid
        or info.st_gid != gid
    ):
        raise ResolutionProbeError(f"{label}-identity-invalid")
    return "private-directory"


def _load_transfer_inventory(path: Path) -> tuple[FinalInputMember, ...]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ResolutionProbeError("transfer-evidence-json-invalid") from exc
    if not isinstance(value, dict) or not isinstance(
        value.get("inventory"), list
    ):
        raise ResolutionProbeError("transfer-evidence-inventory-invalid")
    raw_inventory = value["inventory"]
    if value.get("inventory_count") != len(raw_inventory):
        raise ResolutionProbeError("transfer-evidence-inventory-count-invalid")
    members: list[FinalInputMember] = []
    observed_paths: set[str] = set()
    for item in raw_inventory:
        if not isinstance(item, dict) or set(item) != {
            "mode",
            "path",
            "sha256",
            "size",
        }:
            raise ResolutionProbeError("transfer-evidence-member-invalid")
        name = item.get("path")
        mode_text = item.get("mode")
        size = item.get("size")
        sha256 = item.get("sha256")
        if not isinstance(name, str):
            raise ResolutionProbeError("transfer-evidence-member-path-invalid")
        member_path = Path(name)
        if (
            not name
            or member_path.is_absolute()
            or ".." in member_path.parts
            or member_path.as_posix() != name
            or name in observed_paths
        ):
            raise ResolutionProbeError("transfer-evidence-member-path-invalid")
        if (
            not isinstance(mode_text, str)
            or not re.fullmatch(r"0[0-7]{3}", mode_text)
            or int(mode_text, 8) not in {0o600, 0o644, 0o700, 0o755}
        ):
            raise ResolutionProbeError("transfer-evidence-member-mode-invalid")
        if (
            not isinstance(size, int)
            or isinstance(size, bool)
            or size < 0
            or size > 256 * 1024 * 1024
        ):
            raise ResolutionProbeError("transfer-evidence-member-size-invalid")
        if not isinstance(sha256, str) or not HEX_64.fullmatch(sha256):
            raise ResolutionProbeError("transfer-evidence-member-sha256-invalid")
        observed_paths.add(name)
        members.append(
            FinalInputMember(name, int(mode_text, 8), size, sha256)
        )
    return tuple(members)


def _verify_final_input_tree(
    root: Path,
    members: tuple[FinalInputMember, ...],
    *,
    uid: int,
    gid: int,
) -> None:
    expected_files = {member.path: member for member in members}
    expected_directories = {
        str(parent)
        for member in members
        for parent in Path(member.path).parents
        if str(parent) != "."
    }
    observed_files: set[str] = set()
    observed_directories: set[str] = set()
    for directory, child_directories, filenames in os.walk(
        root, topdown=True, followlinks=False
    ):
        directory_path = Path(directory)
        relative_directory = directory_path.relative_to(root)
        if str(relative_directory) != ".":
            observed_directories.add(relative_directory.as_posix())
            _require_directory(
                directory_path,
                mode=0o700,
                uid=uid,
                gid=gid,
                label="final-input-directory",
            )
        for name in child_directories:
            if (directory_path / name).is_symlink():
                raise ResolutionProbeError("final-input-directory-symlink")
        for name in filenames:
            path = directory_path / name
            relative = path.relative_to(root).as_posix()
            expected = expected_files.get(relative)
            if expected is None:
                raise ResolutionProbeError("final-input-file-unexpected")
            info = _require_regular(
                path,
                mode=expected.mode,
                uid=uid,
                gid=gid,
                label="final-input-file",
            )
            if info.st_size != expected.size:
                raise ResolutionProbeError("final-input-file-size-mismatch")
            if _sha256_file(path) != expected.sha256:
                raise ResolutionProbeError("final-input-file-sha256-mismatch")
            observed_files.add(relative)
    if observed_files != set(expected_files):
        raise ResolutionProbeError("final-input-file-inventory-drift")
    if observed_directories != expected_directories:
        raise ResolutionProbeError("final-input-directory-inventory-drift")


def _require_regular_identity(
    path: Path,
    *,
    size: int,
    sha256: str,
    uid: int,
    gid: int,
    label: str,
) -> os.stat_result:
    info = _require_regular(
        path, mode=0o600, uid=uid, gid=gid, label=label
    )
    if info.st_size != size:
        raise ResolutionProbeError(f"{label}-size-mismatch")
    if _sha256_file(path) != sha256:
        raise ResolutionProbeError(f"{label}-sha256-mismatch")
    return info


def _require_directory(
    path: Path, *, mode: int, uid: int, gid: int, label: str
) -> os.stat_result:
    try:
        info = path.lstat()
    except OSError as exc:
        raise ResolutionProbeError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISDIR(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or stat.S_IMODE(info.st_mode) != mode
        or info.st_uid != uid
        or info.st_gid != gid
    ):
        raise ResolutionProbeError(f"{label}-identity-invalid")
    return info


def _require_regular(
    path: Path, *, mode: int, uid: int, gid: int, label: str
) -> os.stat_result:
    try:
        info = path.lstat()
    except OSError as exc:
        raise ResolutionProbeError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISREG(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or stat.S_IMODE(info.st_mode) != mode
        or info.st_uid != uid
        or info.st_gid != gid
        or info.st_nlink != 1
    ):
        raise ResolutionProbeError(f"{label}-identity-invalid")
    return info


def _write_exclusive(path: Path, payload: bytes, *, mode: int) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags, mode)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
        os.fchmod(descriptor, mode)
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _reason(exc: BaseException) -> str:
    if isinstance(exc, ResolutionProbeError):
        return str(exc)
    if isinstance(exc, OSError):
        return f"os-error-{exc.errno or 'unknown'}"
    return exc.__class__.__name__.lower()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Observe a frozen L6 canonical input transfer without modifying "
            "its transfer, staging, or final input paths."
        )
    )
    parser.add_argument("--resolution-attempt-id", required=True)
    parser.add_argument("--transfer-attempt-id", required=True)
    parser.add_argument("--resolution-root", type=Path, required=True)
    parser.add_argument("--transfer-root", type=Path, required=True)
    parser.add_argument("--expected-installer-size", type=int, required=True)
    parser.add_argument("--expected-installer-sha256", required=True)
    parser.add_argument("--expected-bundle-size", type=int, required=True)
    parser.add_argument("--expected-bundle-sha256", required=True)
    parser.add_argument(
        "--expected-transfer-evidence-size", type=int, required=True
    )
    parser.add_argument("--expected-transfer-evidence-sha256", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = ResolutionProbeRequest(
        resolution_attempt_id=args.resolution_attempt_id,
        transfer_attempt_id=args.transfer_attempt_id,
        resolution_root=args.resolution_root,
        transfer_root=args.transfer_root,
        expected_installer_size=args.expected_installer_size,
        expected_installer_sha256=args.expected_installer_sha256,
        expected_bundle_size=args.expected_bundle_size,
        expected_bundle_sha256=args.expected_bundle_sha256,
        expected_transfer_evidence_size=args.expected_transfer_evidence_size,
        expected_transfer_evidence_sha256=(
            args.expected_transfer_evidence_sha256
        ),
    )
    try:
        _, exit_code = run_resolution_probe(request)
    except (OSError, ResolutionProbeError) as exc:
        print(f"l6_v4_canonical_input_resolution_probe_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())

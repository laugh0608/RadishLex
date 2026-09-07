#!/usr/bin/env python3
from __future__ import annotations

import argparse
import ctypes
import errno
import hashlib
import json
import os
import re
import stat
import sys
import tarfile
from dataclasses import dataclass
from pathlib import Path
from typing import BinaryIO, Callable


EVIDENCE_FORMAT = "radishlex-linux-l6-v4-canonical-input-install-v1"
FINAL_INPUT_ROOT = Path("/var/tmp/radishlex-l6-inputs")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
MAX_BUNDLE_BYTES = 256 * 1024 * 1024
AT_FDCWD = -100
RENAME_NOREPLACE = 1
CANONICAL_INVENTORY = (
    "build-environment.json",
    "case.sh",
    "radishlex-linux-l6-acceptance",
    "radishlex-linux-maintenance",
    "release-pair.evidence.json",
    "snapshot-identity.evidence.txt",
    "source/artifacts/radishlex_26.7.1+38-1_arm64.deb",
    "source/artifacts/radishlex_26.7.1+38-1_arm64.deb.evidence.json",
    "startup.py",
    (
        "target-startup/usr/lib/aarch64-linux-gnu/radishlex/manager/"
        "lib/libradishlex_ime_ffi.so"
    ),
    "target/artifacts/radishlex_26.7.1+38-2_arm64.deb",
    "target/artifacts/radishlex_26.7.1+38-2_arm64.deb.evidence.json",
)
ALLOWED_FILE_MODES = frozenset((0o600, 0o644, 0o700, 0o755))


class GuestInputInstallError(ValueError):
    pass


@dataclass(frozen=True)
class GuestInstallRequest:
    attempt_id: str
    transfer_root: Path
    bundle_path: Path
    expected_bundle_size: int
    expected_bundle_sha256: str

    @property
    def expected_transfer_root(self) -> Path:
        return Path(
            f"/var/tmp/radishlex-l6-v4-input-transfer-{self.attempt_id}"
        )

    @property
    def expected_bundle_path(self) -> Path:
        return self.expected_transfer_root / "canonical-input.ustar.incoming"

    @property
    def installer_path(self) -> Path:
        return self.expected_transfer_root / "install-canonical-input.py"

    @property
    def attempt_marker_path(self) -> Path:
        return self.expected_transfer_root / "attempt.marker"

    @property
    def evidence_path(self) -> Path:
        return self.expected_transfer_root / "transfer.evidence.json"

    @property
    def staging_root(self) -> Path:
        return Path(
            f"/var/tmp/.radishlex-l6-inputs-{self.attempt_id}.incoming"
        )

    def validate(self) -> None:
        if not SAFE_ATTEMPT_ID.fullmatch(self.attempt_id):
            raise GuestInputInstallError("attempt-id-invalid")
        if self.transfer_root != self.expected_transfer_root:
            raise GuestInputInstallError("transfer-root-invalid")
        if self.bundle_path != self.expected_bundle_path:
            raise GuestInputInstallError("bundle-path-invalid")
        if not 1 <= self.expected_bundle_size <= MAX_BUNDLE_BYTES:
            raise GuestInputInstallError("expected-bundle-size-invalid")
        if not HEX_64.fullmatch(self.expected_bundle_sha256):
            raise GuestInputInstallError("expected-bundle-sha256-invalid")


@dataclass(frozen=True)
class ArchiveMember:
    path: str
    mode: int
    size: int
    sha256: str

    def as_json(self) -> dict[str, object]:
        return {
            "mode": f"{self.mode:04o}",
            "path": self.path,
            "sha256": self.sha256,
            "size": self.size,
        }


RenameNoReplace = Callable[[Path, Path], None]


def run_guest_install(
    request: GuestInstallRequest,
    *,
    final_root: Path = FINAL_INPUT_ROOT,
    identity_uid: int = 0,
    identity_gid: int = 0,
    rename_noreplace: RenameNoReplace | None = None,
) -> tuple[dict[str, object], int]:
    request.validate()
    phase = "root-preflight"
    final_switch = "not-performed"
    members: tuple[ArchiveMember, ...] = ()
    try:
        if os.geteuid() != identity_uid or os.getegid() != identity_gid:
            raise GuestInputInstallError("root-identity-required")
        _require_directory(
            request.transfer_root,
            mode=0o700,
            uid=identity_uid,
            gid=identity_gid,
            label="transfer-root",
        )
        _require_regular(
            request.installer_path,
            modes=frozenset((0o600,)),
            uid=identity_uid,
            gid=identity_gid,
            label="installer",
        )
        for path, label in (
            (request.attempt_marker_path, "attempt-marker"),
            (request.evidence_path, "transfer-evidence"),
            (request.staging_root, "staging-root"),
            (final_root, "final-input-root"),
        ):
            if path.exists() or path.is_symlink():
                raise GuestInputInstallError(f"{label}-must-be-absent")

        phase = "attempt-marker"
        _write_exclusive(
            request.attempt_marker_path,
            (request.attempt_id + "\n").encode("ascii"),
            mode=0o600,
        )

        phase = "bundle-normalize"
        _require_regular(
            request.bundle_path,
            modes=frozenset((0o600, 0o666)),
            uid=identity_uid,
            gid=identity_gid,
            label="source-bundle",
        )
        os.chown(request.bundle_path, identity_uid, identity_gid)
        os.chmod(request.bundle_path, 0o600)
        _fsync_file(request.bundle_path)
        _require_bundle_identity(
            request.bundle_path,
            request,
            identity_uid=identity_uid,
            identity_gid=identity_gid,
        )

        phase = "archive-validate"
        with request.bundle_path.open("rb") as archive_file:
            members = inspect_canonical_archive(
                archive_file,
                identity_uid=identity_uid,
                identity_gid=identity_gid,
            )

        phase = "private-staging"
        os.mkdir(request.staging_root, 0o700)
        os.chown(request.staging_root, identity_uid, identity_gid)
        os.chmod(request.staging_root, 0o700)
        with request.bundle_path.open("rb") as archive_file:
            extracted = extract_canonical_archive(
                archive_file,
                request.staging_root,
                identity_uid=identity_uid,
                identity_gid=identity_gid,
            )
        if extracted != members:
            raise GuestInputInstallError("archive-member-identity-drift")
        _verify_tree(
            request.staging_root,
            members,
            identity_uid=identity_uid,
            identity_gid=identity_gid,
        )
        _fsync_tree_directories(request.staging_root)

        phase = "atomic-switch"
        publish = rename_noreplace or _rename_noreplace_linux
        publish(request.staging_root, final_root)
        final_switch = "performed"
        _fsync_directory(final_root.parent)

        phase = "final-readback"
        _verify_tree(
            final_root,
            members,
            identity_uid=identity_uid,
            identity_gid=identity_gid,
        )
        _require_bundle_identity(
            request.bundle_path,
            request,
            identity_uid=identity_uid,
            identity_gid=identity_gid,
        )
        outcome = "passed"
        reason = "none"
        exit_code = 0
    except (GuestInputInstallError, OSError, tarfile.TarError) as exc:
        outcome = "failed"
        reason = _failure_reason(exc)
        exit_code = 10

    evidence = {
        "attempt_id": request.attempt_id,
        "automatic_cleanup": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "bundle_sha256": request.expected_bundle_sha256,
        "bundle_size": request.expected_bundle_size,
        "final_input_root": str(final_root),
        "final_switch": final_switch,
        "format": EVIDENCE_FORMAT,
        "inventory": [member.as_json() for member in members],
        "inventory_count": len(members),
        "operation_id": "not-generated",
        "outcome": outcome,
        "phase": phase,
        "reason": reason,
        "transaction": "not-performed",
    }
    return evidence, exit_code


def inspect_canonical_archive(
    archive_file: BinaryIO,
    *,
    identity_uid: int = 0,
    identity_gid: int = 0,
) -> tuple[ArchiveMember, ...]:
    archive_file.seek(0)
    observed: list[ArchiveMember] = []
    with tarfile.open(fileobj=archive_file, mode="r:") as archive:
        tar_members = archive.getmembers()
        names = tuple(member.name for member in tar_members)
        if names != CANONICAL_INVENTORY:
            raise GuestInputInstallError("archive-inventory-invalid")
        for member in tar_members:
            _require_canonical_member(
                archive_file,
                member,
                identity_uid=identity_uid,
                identity_gid=identity_gid,
            )
            source = archive.extractfile(member)
            if source is None:
                raise GuestInputInstallError("archive-member-unreadable")
            digest = hashlib.sha256()
            size = 0
            while chunk := source.read(1024 * 1024):
                digest.update(chunk)
                size += len(chunk)
            if size != member.size:
                raise GuestInputInstallError("archive-member-size-drift")
            observed.append(
                ArchiveMember(member.name, member.mode, size, digest.hexdigest())
            )
        _require_canonical_trailer(archive_file, tar_members)
    archive_file.seek(0)
    return tuple(observed)


def extract_canonical_archive(
    archive_file: BinaryIO,
    staging_root: Path,
    *,
    identity_uid: int = 0,
    identity_gid: int = 0,
) -> tuple[ArchiveMember, ...]:
    archive_file.seek(0)
    extracted: list[ArchiveMember] = []
    with tarfile.open(fileobj=archive_file, mode="r:") as archive:
        tar_members = archive.getmembers()
        if tuple(member.name for member in tar_members) != CANONICAL_INVENTORY:
            raise GuestInputInstallError("archive-inventory-invalid")
        for member in tar_members:
            _require_canonical_member(
                archive_file,
                member,
                identity_uid=identity_uid,
                identity_gid=identity_gid,
            )
            destination = staging_root / member.name
            _create_private_parents(
                destination.parent,
                staging_root,
                identity_uid=identity_uid,
                identity_gid=identity_gid,
            )
            source = archive.extractfile(member)
            if source is None:
                raise GuestInputInstallError("archive-member-unreadable")
            digest = hashlib.sha256()
            size = 0
            descriptor = _open_exclusive(destination, 0o600)
            try:
                with os.fdopen(descriptor, "wb", closefd=False) as output:
                    while chunk := source.read(1024 * 1024):
                        output.write(chunk)
                        digest.update(chunk)
                        size += len(chunk)
                    output.flush()
                    os.fsync(output.fileno())
                os.fchown(descriptor, identity_uid, identity_gid)
                os.fchmod(descriptor, member.mode)
                os.fsync(descriptor)
            finally:
                os.close(descriptor)
            if size != member.size:
                raise GuestInputInstallError("archive-member-size-drift")
            extracted.append(
                ArchiveMember(member.name, member.mode, size, digest.hexdigest())
            )
    archive_file.seek(0)
    return tuple(extracted)


def write_evidence_exclusive(path: Path, evidence: dict[str, object]) -> None:
    payload = (
        json.dumps(evidence, ensure_ascii=True, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")
    _write_exclusive(path, payload, mode=0o600)
    _fsync_directory(path.parent)


def _require_canonical_member(
    archive_file: BinaryIO,
    member: tarfile.TarInfo,
    *,
    identity_uid: int,
    identity_gid: int,
) -> None:
    if (
        not member.isreg()
        or member.pax_headers
        or member.uid != identity_uid
        or member.gid != identity_gid
        or member.mode not in ALLOWED_FILE_MODES
        or member.size < 0
        or member.name.startswith("/")
        or ".." in Path(member.name).parts
    ):
        raise GuestInputInstallError("archive-member-identity-invalid")
    position = archive_file.tell()
    archive_file.seek(member.offset + 257)
    magic = archive_file.read(8)
    archive_file.seek(position)
    if magic != b"ustar\x0000":
        raise GuestInputInstallError("archive-member-not-ustar")


def _require_canonical_trailer(
    archive_file: BinaryIO, members: list[tarfile.TarInfo]
) -> None:
    if not members:
        raise GuestInputInstallError("archive-empty")
    last = members[-1]
    data_end = last.offset_data + ((last.size + 511) // 512) * 512
    archive_file.seek(0, os.SEEK_END)
    archive_size = archive_file.tell()
    if archive_size % 512 != 0 or archive_size - data_end < 1024:
        raise GuestInputInstallError("archive-trailer-size-invalid")
    archive_file.seek(data_end)
    trailer = archive_file.read()
    if any(trailer):
        raise GuestInputInstallError("archive-trailer-not-zero")


def _verify_tree(
    root: Path,
    members: tuple[ArchiveMember, ...],
    *,
    identity_uid: int,
    identity_gid: int,
) -> None:
    _require_directory(
        root,
        mode=0o700,
        uid=identity_uid,
        gid=identity_gid,
        label="input-root",
    )
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
                uid=identity_uid,
                gid=identity_gid,
                label="input-directory",
            )
        for name in child_directories:
            path = directory_path / name
            if path.is_symlink():
                raise GuestInputInstallError("input-directory-symlink")
        for name in filenames:
            path = directory_path / name
            relative = path.relative_to(root).as_posix()
            expected = expected_files.get(relative)
            if expected is None:
                raise GuestInputInstallError("input-file-unexpected")
            _require_regular(
                path,
                modes=frozenset((expected.mode,)),
                uid=identity_uid,
                gid=identity_gid,
                label="input-file",
            )
            if path.stat().st_size != expected.size:
                raise GuestInputInstallError("input-file-size-drift")
            if _sha256_file(path) != expected.sha256:
                raise GuestInputInstallError("input-file-hash-drift")
            observed_files.add(relative)
    if observed_files != set(expected_files):
        raise GuestInputInstallError("input-file-inventory-drift")
    if observed_directories != expected_directories:
        raise GuestInputInstallError("input-directory-inventory-drift")


def _require_bundle_identity(
    path: Path,
    request: GuestInstallRequest,
    *,
    identity_uid: int,
    identity_gid: int,
) -> None:
    info = _require_regular(
        path,
        modes=frozenset((0o600,)),
        uid=identity_uid,
        gid=identity_gid,
        label="source-bundle",
    )
    if info.st_size != request.expected_bundle_size:
        raise GuestInputInstallError("source-bundle-size-mismatch")
    if _sha256_file(path) != request.expected_bundle_sha256:
        raise GuestInputInstallError("source-bundle-sha256-mismatch")


def _require_directory(
    path: Path, *, mode: int, uid: int, gid: int, label: str
) -> os.stat_result:
    try:
        info = path.lstat()
    except OSError as exc:
        raise GuestInputInstallError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISDIR(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or stat.S_IMODE(info.st_mode) != mode
        or info.st_uid != uid
        or info.st_gid != gid
    ):
        raise GuestInputInstallError(f"{label}-identity-invalid")
    return info


def _require_regular(
    path: Path,
    *,
    modes: frozenset[int],
    uid: int,
    gid: int,
    label: str,
) -> os.stat_result:
    try:
        info = path.lstat()
    except OSError as exc:
        raise GuestInputInstallError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISREG(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or stat.S_IMODE(info.st_mode) not in modes
        or info.st_uid != uid
        or info.st_gid != gid
        or info.st_nlink != 1
    ):
        raise GuestInputInstallError(f"{label}-identity-invalid")
    return info


def _create_private_parents(
    directory: Path,
    root: Path,
    *,
    identity_uid: int,
    identity_gid: int,
) -> None:
    relative = directory.relative_to(root)
    current = root
    for part in relative.parts:
        current /= part
        created = False
        try:
            os.mkdir(current, 0o700)
            created = True
        except FileExistsError:
            _require_directory(
                current,
                mode=0o700,
                uid=identity_uid,
                gid=identity_gid,
                label="staging-directory",
            )
        if created:
            os.chown(current, identity_uid, identity_gid)
            os.chmod(current, 0o700)
        _require_directory(
            current,
            mode=0o700,
            uid=identity_uid,
            gid=identity_gid,
            label="staging-directory",
        )


def _open_exclusive(path: Path, mode: int) -> int:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return os.open(path, flags, mode)


def _write_exclusive(path: Path, payload: bytes, *, mode: int) -> None:
    descriptor = _open_exclusive(path, mode)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
        os.fchmod(descriptor, mode)
    finally:
        os.close(descriptor)


def _rename_noreplace_linux(source: Path, destination: Path) -> None:
    libc = ctypes.CDLL(None, use_errno=True)
    try:
        renameat2 = libc.renameat2
    except AttributeError as exc:
        raise GuestInputInstallError("renameat2-unavailable") from exc
    renameat2.argtypes = (
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_uint,
    )
    renameat2.restype = ctypes.c_int
    result = renameat2(
        AT_FDCWD,
        os.fsencode(source),
        AT_FDCWD,
        os.fsencode(destination),
        RENAME_NOREPLACE,
    )
    if result == 0:
        return
    error_number = ctypes.get_errno()
    if error_number == errno.EEXIST:
        raise GuestInputInstallError("final-input-root-must-be-absent")
    raise GuestInputInstallError(f"renameat2-failed-{error_number}")


def _fsync_file(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _fsync_tree_directories(root: Path) -> None:
    directories = [Path(directory) for directory, _, _ in os.walk(root)]
    for directory in sorted(directories, key=lambda item: len(item.parts), reverse=True):
        _fsync_directory(directory)


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _failure_reason(exc: BaseException) -> str:
    if isinstance(exc, GuestInputInstallError):
        return str(exc)
    if isinstance(exc, OSError):
        return f"os-error-{exc.errno or 'unknown'}"
    return f"{exc.__class__.__name__.lower()}"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Install one authorized canonical L6 input bundle into a "
            "root-owned private staging tree and atomically publish it."
        )
    )
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--transfer-root", type=Path, required=True)
    parser.add_argument("--bundle-path", type=Path, required=True)
    parser.add_argument("--expected-bundle-size", type=int, required=True)
    parser.add_argument("--expected-bundle-sha256", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = GuestInstallRequest(
        attempt_id=args.attempt_id,
        transfer_root=args.transfer_root,
        bundle_path=args.bundle_path,
        expected_bundle_size=args.expected_bundle_size,
        expected_bundle_sha256=args.expected_bundle_sha256,
    )
    try:
        evidence, exit_code = run_guest_install(request)
        write_evidence_exclusive(request.evidence_path, evidence)
    except (GuestInputInstallError, OSError, ValueError) as exc:
        print(f"l6_v4_canonical_input_install_error={exc}", file=sys.stderr)
        return 20
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())

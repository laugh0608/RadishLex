#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
import uuid
from pathlib import Path
from typing import Sequence


EVIDENCE_FORMAT = "radishlex-linux-l6-v4-boot-transport-evidence-v1"
MARKER_FORMAT = "radishlex-linux-l6-v4-boot-transport-marker-v1"
BOOT_ID_PATH = Path("/proc/sys/kernel/random/boot_id")
CONTROL_ROOT_PARENT = Path("/var/tmp")
CONTROL_SCOPE_BOOT_TRANSPORT = "boot-transport"
CONTROL_SCOPE_BOOT_START = "boot-start"
CONTROL_SCOPE_GUEST_AGENT = "guest-agent"
CONTROL_SCOPES = (
    CONTROL_SCOPE_BOOT_TRANSPORT,
    CONTROL_SCOPE_BOOT_START,
    CONTROL_SCOPE_GUEST_AGENT,
)
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")
HEX_64 = re.compile(r"[0-9a-f]{64}")


class BootTransportProbeError(ValueError):
    pass


def canonical_json(value: dict[str, object]) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=True,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("ascii")


def marker_bytes(attempt_id: str, probe_sha256: str) -> bytes:
    return canonical_json(
        {
            "attempt_id": attempt_id,
            "format": MARKER_FORMAT,
            "probe_sha256": probe_sha256,
        }
    )


def result_bytes(
    attempt_id: str,
    target_uuid: str,
    probe_sha256: str,
    boot_id_sha256: str,
) -> bytes:
    return canonical_json(
        {
            "attempt_id": attempt_id,
            "boot_id_sha256": boot_id_sha256,
            "format": EVIDENCE_FORMAT,
            "outcome": "passed",
            "probe_sha256": probe_sha256,
            "target_uuid": target_uuid,
        }
    )


def run_probe(
    *,
    attempt_id: str,
    target_uuid: str,
    expected_probe_sha256: str,
    control_scope: str,
    control_root: Path,
    boot_id_path: Path = BOOT_ID_PATH,
    probe_path: Path | None = None,
    expected_owner_uid: int = 0,
    expected_owner_gid: int = 0,
    control_root_parent: Path = CONTROL_ROOT_PARENT,
) -> str:
    if not SAFE_ATTEMPT_ID.fullmatch(attempt_id):
        raise BootTransportProbeError("attempt-id-invalid")
    try:
        canonical_target_uuid = str(uuid.UUID(target_uuid)).upper()
    except ValueError as exc:
        raise BootTransportProbeError("target-uuid-invalid") from exc
    if target_uuid != canonical_target_uuid:
        raise BootTransportProbeError("target-uuid-not-canonical")
    if not HEX_64.fullmatch(expected_probe_sha256):
        raise BootTransportProbeError("expected-probe-sha256-invalid")
    expected_root = control_root_for(
        control_scope,
        attempt_id,
        parent=control_root_parent,
    )
    if control_root != expected_root:
        raise BootTransportProbeError("control-root-invalid")
    _validate_private_root(
        control_root,
        expected_owner_uid=expected_owner_uid,
        expected_owner_gid=expected_owner_gid,
    )

    actual_probe_path = probe_path or Path(__file__)
    probe_sha256 = _sha256_file(actual_probe_path)
    if probe_sha256 != expected_probe_sha256:
        raise BootTransportProbeError("probe-identity-drift")

    marker_path = control_root / "attempt.marker.json"
    result_path = control_root / "boot-identity.evidence.json"
    _write_exclusive(
        marker_path,
        marker_bytes(attempt_id, probe_sha256),
        expected_owner_uid=expected_owner_uid,
        expected_owner_gid=expected_owner_gid,
    )

    boot_id = _read_canonical_boot_id(boot_id_path)
    boot_id_sha256 = hashlib.sha256(boot_id.encode("ascii")).hexdigest()
    _write_exclusive(
        result_path,
        result_bytes(
            attempt_id,
            target_uuid,
            probe_sha256,
            boot_id_sha256,
        ),
        expected_owner_uid=expected_owner_uid,
        expected_owner_gid=expected_owner_gid,
    )
    return boot_id_sha256


def control_root_for(
    control_scope: str,
    attempt_id: str,
    *,
    parent: Path = CONTROL_ROOT_PARENT,
) -> Path:
    if control_scope not in CONTROL_SCOPES:
        raise BootTransportProbeError("control-scope-invalid")
    if not SAFE_ATTEMPT_ID.fullmatch(attempt_id):
        raise BootTransportProbeError("attempt-id-invalid")
    return parent / f"radishlex-l6-v4-{control_scope}-{attempt_id}"


def _validate_private_root(
    path: Path, *, expected_owner_uid: int, expected_owner_gid: int
) -> None:
    try:
        info = path.lstat()
    except OSError as exc:
        raise BootTransportProbeError("control-root-unavailable") from exc
    if (
        not stat.S_ISDIR(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or stat.S_IMODE(info.st_mode) != 0o700
        or info.st_uid != expected_owner_uid
        or info.st_gid != expected_owner_gid
    ):
        raise BootTransportProbeError("control-root-identity-invalid")


def _read_canonical_boot_id(path: Path) -> str:
    try:
        raw = path.read_bytes()
    except OSError as exc:
        raise BootTransportProbeError("boot-id-unavailable") from exc
    if not raw or len(raw) > 128:
        raise BootTransportProbeError("boot-id-size-invalid")
    if raw.endswith(b"\n"):
        raw = raw[:-1]
    if b"\n" in raw or b"\r" in raw:
        raise BootTransportProbeError("boot-id-line-invalid")
    try:
        value = raw.decode("ascii")
    except UnicodeDecodeError as exc:
        raise BootTransportProbeError("boot-id-not-ascii") from exc
    try:
        canonical = str(uuid.UUID(value))
    except ValueError as exc:
        raise BootTransportProbeError("boot-id-invalid") from exc
    if value != canonical:
        raise BootTransportProbeError("boot-id-not-canonical")
    return value


def _write_exclusive(
    path: Path,
    payload: bytes,
    *,
    expected_owner_uid: int,
    expected_owner_gid: int,
) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags, 0o600)
    except OSError as exc:
        raise BootTransportProbeError(
            f"create-new-failed:{path.name}"
        ) from exc
    try:
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "wb", closefd=False) as target:
            target.write(payload)
            target.flush()
            os.fsync(target.fileno())
    finally:
        os.close(descriptor)
    info = path.lstat()
    if (
        not stat.S_ISREG(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or info.st_nlink != 1
        or stat.S_IMODE(info.st_mode) != 0o600
        or info.st_uid != expected_owner_uid
        or info.st_gid != expected_owner_gid
        or info.st_size != len(payload)
    ):
        raise BootTransportProbeError(
            f"published-file-identity-invalid:{path.name}"
        )


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            while chunk := source.read(1024 * 1024):
                digest.update(chunk)
    except OSError as exc:
        raise BootTransportProbeError("probe-unreadable") from exc
    return digest.hexdigest()


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Write one create-new SHA-256 of the canonical guest boot ID "
            "without emitting the raw boot ID or relying on stdout transport."
        )
    )
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--expected-probe-sha256", required=True)
    parser.add_argument(
        "--control-scope",
        choices=CONTROL_SCOPES,
        required=True,
    )
    parser.add_argument("--control-root", type=Path, required=True)
    return parser.parse_args(argv)


def main() -> int:
    args = parse_args()
    try:
        run_probe(
            attempt_id=args.attempt_id,
            target_uuid=args.target_uuid,
            expected_probe_sha256=args.expected_probe_sha256,
            control_scope=args.control_scope,
            control_root=args.control_root,
        )
    except BootTransportProbeError as exc:
        print(f"boot_transport_probe_error={exc}", file=sys.stderr)
        return 12
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

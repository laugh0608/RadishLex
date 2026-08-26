#!/usr/bin/env python3
from __future__ import annotations

import argparse
import fcntl
import importlib.util
import json
import os
import re
import stat
import sys
from pathlib import Path
from types import ModuleType


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-v4-install-artifacts-staged-"
    "new-boot-recovery-preflight-v1"
)
MARKER_FORMAT = f"{EVIDENCE_FORMAT}-marker"
PHASE_FORMAT = f"{EVIDENCE_FORMAT}-phase"
EXPECTED_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-new-boot-recovery-preflight-"
    "20260826-v1"
)
EXPECTED_TARGET_UUID = "50B75F88-493D-42C0-A1DC-054DEC478038"
EXPECTED_PRIOR_BOOT_ID_SHA256 = (
    "18f1ba063ecd3087a5624e6ae52a54624540a972df33cca00a34bca4ec00022a"
)
EXPECTED_CURRENT_BOOT_ID_SHA256 = (
    "b757c8fcc3c47b85b04be60ffc561e67cfe323d73ffe7981f6000d0c0fa44758"
)
EXPECTED_RESUME_DRIVER_SHA256 = (
    "0b649ecd7b90a8368cf40a3887660cade5fa90cf9119bd7baba0904bcad3d05b"
)
EXPECTED_OPERATION_ID_SHA256 = (
    "21041a89668ff577c9b7912789e3445bb33a21873bbec56530177d8574d1111a"
)
EXPECTED_CONTROL_ROOT = Path(
    "/var/tmp/radishlex-l6-v4-install-artifacts-staged-"
    f"new-boot-recovery-preflight-{EXPECTED_ATTEMPT_ID}"
)
EXPECTED_OPERATION_IN_PROGRESS_STARTUP = "0:1:3:14:2|error-absent"
EXPECTED_ACTIVE_GUARD_STARTUP = "0:1:3:10:0|error-absent"
MAX_EVIDENCE_BYTES = 1024 * 1024
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")
HEX_64 = re.compile(r"[0-9a-f]{64}")


class RecoveryPreflightError(ValueError):
    pass


def canonical_json(value: dict[str, object]) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )


def require_directory(path: Path, label: str, mode: int) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_gid != 0
        or stat.S_IMODE(metadata.st_mode) != mode
    ):
        raise RecoveryPreflightError(f"directory-identity-invalid:{label}")


def require_private_directory(path: Path, label: str) -> None:
    require_directory(path, label, 0o700)


def require_regular(
    path: Path,
    label: str,
    mode: int,
    *,
    size: int | None = None,
    sha256: str | None = None,
    driver: ModuleType | None = None,
) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_gid != 0
        or stat.S_IMODE(metadata.st_mode) != mode
        or metadata.st_nlink != 1
        or (size is not None and metadata.st_size != size)
    ):
        raise RecoveryPreflightError(f"file-identity-invalid:{label}")
    if sha256 is not None:
        if driver is None or driver.sha256_file(path) != sha256:
            raise RecoveryPreflightError(f"file-sha256-invalid:{label}")


def create_new_file(path: Path, payload: bytes) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as target:
            target.write(payload)
            target.flush()
            os.fsync(target.fileno())
    finally:
        os.close(descriptor)
    os.chmod(path, 0o600, follow_symlinks=False)
    os.chown(path, 0, 0, follow_symlinks=False)
    require_regular(path, "created-evidence", 0o600, size=len(payload))
    sync_directory(path.parent)


def sync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def replace_phase(control_root: Path, phase: str) -> None:
    incoming = control_root / ".phase.incoming.json"
    if incoming.exists() or incoming.is_symlink():
        raise RecoveryPreflightError("phase-incoming-exists")
    create_new_file(
        incoming,
        canonical_json({"format": PHASE_FORMAT, "phase": phase}),
    )
    os.replace(incoming, control_root / "phase.json")
    sync_directory(control_root)


def publish_terminal(control_root: Path, value: dict[str, object]) -> None:
    incoming = control_root / ".terminal.incoming.json"
    final = control_root / "preflight.evidence.json"
    payload = canonical_json(value)
    create_new_file(incoming, payload)
    try:
        os.link(incoming, final, follow_symlinks=False)
    except FileExistsError as exc:
        raise RecoveryPreflightError("terminal-evidence-exists") from exc
    os.unlink(incoming)
    require_regular(final, "terminal-evidence", 0o600, size=len(payload))
    sync_directory(control_root)


def load_resume_driver(path: Path, expected_sha256: str) -> ModuleType:
    require_regular(path, "resume-driver", 0o600)
    import hashlib

    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if digest != expected_sha256 or digest != EXPECTED_RESUME_DRIVER_SHA256:
        raise RecoveryPreflightError("resume-driver-identity-invalid")
    spec = importlib.util.spec_from_file_location(
        "radishlex_frozen_resume_driver", path
    )
    if spec is None or spec.loader is None:
        raise RecoveryPreflightError("resume-driver-import-invalid")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def validate_persistent_transaction(driver: ModuleType) -> str:
    require_directory(driver.STATE_ROOT, "state-root", 0o755)
    driver.require_regular(
        driver.RECEIPT_PATH,
        "receipt",
        0o644,
        driver.EXPECTED_RECEIPT_SIZE,
        driver.EXPECTED_RECEIPT_SHA256,
    )
    if (driver.STATE_ROOT / "receipt.json.tmp").exists():
        raise RecoveryPreflightError("receipt-temporary-present")
    try:
        receipt = json.loads(driver.read_bounded(driver.RECEIPT_PATH, 64 * 1024))
    except json.JSONDecodeError as exc:
        raise RecoveryPreflightError("receipt-json-invalid") from exc
    operation_id = receipt.get("operation_id") if isinstance(receipt, dict) else None
    if (
        not isinstance(operation_id, str)
        or not driver.HEX_32.fullmatch(operation_id)
        or driver.sha256_bytes(operation_id.encode("ascii"))
        != driver.EXPECTED_OPERATION_ID_SHA256
    ):
        raise RecoveryPreflightError("receipt-operation-identity-invalid")
    required = {
        "receipt_format": "radishlex-linux-install-receipt-v1",
        "product_id": "radishlex-linux",
        "distribution_identity": "debian-local-deb-v1",
        "operation_kind": "install",
        "version_relation": "not_applicable",
        "state": "artifacts_staged",
        "failure_code": None,
        "failure_after_state": None,
        "manual_recovery_required": False,
        "source_artifact": None,
        "source_proof": None,
        "target_proof": None,
    }
    if any(receipt.get(key) != value for key, value in required.items()):
        raise RecoveryPreflightError("receipt-semantics-invalid")
    if receipt.get("operation_chain") != [operation_id]:
        raise RecoveryPreflightError("receipt-operation-chain-invalid")
    target = receipt.get("target_artifact")
    staged = receipt.get("staged_artifacts")
    if not isinstance(target, dict) or not isinstance(staged, list) or len(staged) != 1:
        raise RecoveryPreflightError("receipt-target-invalid")
    staged_entry = staged[0]
    staged_artifact = (
        staged_entry.get("artifact") if isinstance(staged_entry, dict) else None
    )
    initial = receipt.get("initial_package")
    if (
        target.get("package_version") != "26.7.1+38-1"
        or target.get("package_sha256") != driver.EXPECTED_SOURCE_PACKAGE_SHA256
        or target.get("evidence_sha256") != driver.EXPECTED_SOURCE_EVIDENCE_SHA256
        or not isinstance(staged_entry, dict)
        or staged_entry.get("slot") != "target"
        or not isinstance(staged_artifact, dict)
        or staged_artifact.get("package_sha256")
        != driver.EXPECTED_SOURCE_PACKAGE_SHA256
        or staged_artifact.get("evidence_sha256")
        != driver.EXPECTED_SOURCE_EVIDENCE_SHA256
        or not isinstance(initial, dict)
        or initial.get("state") != "not_installed"
        or initial.get("artifact") is not None
    ):
        raise RecoveryPreflightError("receipt-target-invalid")
    operations_root = driver.STATE_ROOT / "operations"
    require_directory(operations_root, "operations-root", 0o755)
    operation_directories = tuple(operations_root.iterdir())
    if len(operation_directories) != 1 or operation_directories[0].name != operation_id:
        raise RecoveryPreflightError("operation-directory-count-invalid")
    operation_root = operation_directories[0]
    require_private_directory(operation_root, "operation-root")
    if {item.name for item in operation_root.iterdir()} != {
        "target.deb",
        "target.evidence.json",
    }:
        raise RecoveryPreflightError("operation-staging-inventory-invalid")
    driver.require_regular(
        operation_root / "target.deb",
        "staged-target-package",
        0o600,
        40_806_592,
        driver.EXPECTED_SOURCE_PACKAGE_SHA256,
    )
    driver.require_regular(
        operation_root / "target.evidence.json",
        "staged-target-evidence",
        0o600,
        2376,
        driver.EXPECTED_SOURCE_EVIDENCE_SHA256,
    )
    return operation_id


def classify_guard(driver: ModuleType) -> tuple[str, str]:
    guard = driver.GUARD_PATH
    try:
        metadata = guard.lstat()
    except FileNotFoundError:
        return "absent-after-reboot", EXPECTED_OPERATION_IN_PROGRESS_STARTUP
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_gid != 0
        or stat.S_IMODE(metadata.st_mode) != 0o600
        or metadata.st_nlink != 1
        or metadata.st_size != 0
    ):
        raise RecoveryPreflightError("guard-identity-invalid")
    descriptor = os.open(guard, os.O_RDWR | os.O_CLOEXEC | os.O_NOFOLLOW)
    try:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as exc:
            raise RecoveryPreflightError("guard-active") from exc
        finally:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
    finally:
        os.close(descriptor)
    return "present-valid-unlocked", EXPECTED_ACTIVE_GUARD_STARTUP


def validate_live_new_boot(driver: ModuleType, startup_output: str) -> None:
    original_boot = driver.EXPECTED_BOOT_ID_SHA256
    original_startup = driver.EXPECTED_ACTIVE_GUARD_STARTUP
    try:
        driver.EXPECTED_BOOT_ID_SHA256 = EXPECTED_CURRENT_BOOT_ID_SHA256
        driver.EXPECTED_ACTIVE_GUARD_STARTUP = startup_output
        driver.validate_live_system()
    finally:
        driver.EXPECTED_BOOT_ID_SHA256 = original_boot
        driver.EXPECTED_ACTIVE_GUARD_STARTUP = original_startup


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--prior-boot-id-sha256", required=True)
    parser.add_argument("--current-boot-id-sha256", required=True)
    parser.add_argument("--resume-driver", type=Path, required=True)
    parser.add_argument("--expected-resume-driver-sha256", required=True)
    parser.add_argument("--expected-probe-sha256", required=True)
    parser.add_argument("--control-root", type=Path, required=True)
    return parser.parse_args(argv)


def sanitize_reason(value: str) -> str:
    return re.sub(r"(?<![0-9a-f])[0-9a-f]{32}(?![0-9a-f])", "[redacted]", value)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    phase = "arguments"
    control_root_validated = False
    driver: ModuleType | None = None
    guard_profile = "not-observed"
    try:
        if os.geteuid() != 0:
            raise RecoveryPreflightError("root-required")
        expected = {
            "attempt_id": EXPECTED_ATTEMPT_ID,
            "target_uuid": EXPECTED_TARGET_UUID,
            "prior_boot_id_sha256": EXPECTED_PRIOR_BOOT_ID_SHA256,
            "current_boot_id_sha256": EXPECTED_CURRENT_BOOT_ID_SHA256,
            "expected_resume_driver_sha256": EXPECTED_RESUME_DRIVER_SHA256,
        }
        if any(getattr(args, key) != value for key, value in expected.items()):
            raise RecoveryPreflightError("argument-contract-mismatch")
        if (
            not SAFE_ATTEMPT_ID.fullmatch(args.attempt_id)
            or not HEX_64.fullmatch(args.expected_probe_sha256)
            or args.control_root != EXPECTED_CONTROL_ROOT
        ):
            raise RecoveryPreflightError("argument-invalid")
        require_private_directory(args.control_root, "control-root")
        control_root_validated = True
        probe_path = Path(__file__)
        require_regular(probe_path, "probe", 0o600)
        import hashlib

        if (
            hashlib.sha256(probe_path.read_bytes()).hexdigest()
            != args.expected_probe_sha256
        ):
            raise RecoveryPreflightError("probe-identity-drift")
        driver = load_resume_driver(
            args.resume_driver, args.expected_resume_driver_sha256
        )
        create_new_file(
            args.control_root / "attempt.marker.json",
            canonical_json(
                {
                    "attempt_id": args.attempt_id,
                    "format": MARKER_FORMAT,
                    "probe_sha256": args.expected_probe_sha256,
                    "resume_driver_sha256": args.expected_resume_driver_sha256,
                }
            ),
        )

        phase = "persistent-transaction"
        replace_phase(args.control_root, phase)
        validate_persistent_transaction(driver)

        phase = "cross-reboot-guard"
        replace_phase(args.control_root, phase)
        guard_profile, startup_output = classify_guard(driver)

        phase = "live-new-boot"
        replace_phase(args.control_root, phase)
        validate_live_new_boot(driver, startup_output)

        phase = "complete"
        replace_phase(args.control_root, phase)
        publish_terminal(
            args.control_root,
            {
                "automatic_cleanup": "not-performed",
                "automatic_quit": "not-performed",
                "automatic_retry": "not-performed",
                "automatic_stop": "not-performed",
                "current_boot_id_sha256": EXPECTED_CURRENT_BOOT_ID_SHA256,
                "dpkg_mutation_executed": False,
                "format": EVIDENCE_FORMAT,
                "guard_profile": guard_profile,
                "maintenance_resume_invocations": 0,
                "operation_id": "existing-receipt-hash-only",
                "operation_id_sha256": EXPECTED_OPERATION_ID_SHA256,
                "outcome": "recovery-qualified",
                "phase": phase,
                "prior_boot_id_sha256": EXPECTED_PRIOR_BOOT_ID_SHA256,
                "reason": "persistent-artifacts-staged-new-boot-readonly-qualified",
                "receipt_state": "artifacts_staged",
                "startup_gate": startup_output,
                "target_uuid": EXPECTED_TARGET_UUID,
                "transaction": "artifacts-staged-preserved-no-resume",
            },
        )
        return 0
    except Exception as exc:
        terminal = {
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "current_boot_id_sha256": EXPECTED_CURRENT_BOOT_ID_SHA256,
            "dpkg_mutation_executed": False,
            "format": EVIDENCE_FORMAT,
            "guard_profile": guard_profile,
            "maintenance_resume_invocations": 0,
            "operation_id": "existing-receipt-hash-only",
            "operation_id_sha256": (
                EXPECTED_OPERATION_ID_SHA256
            ),
            "outcome": "recovery-rejected",
            "phase": phase,
            "prior_boot_id_sha256": EXPECTED_PRIOR_BOOT_ID_SHA256,
            "reason": sanitize_reason(f"{type(exc).__name__}:{exc}")[:512],
            "target_uuid": EXPECTED_TARGET_UUID,
            "transaction": "artifacts-staged-preserved-no-resume",
        }
        if control_root_validated:
            try:
                replace_phase(args.control_root, "rejected")
                publish_terminal(args.control_root, terminal)
            except Exception as publish_error:
                print(
                    "recovery preflight terminal publish failed: "
                    f"{type(publish_error).__name__}:{publish_error}",
                    file=sys.stderr,
                )
        print(terminal["reason"], file=sys.stderr)
        return 10


if __name__ == "__main__":
    raise SystemExit(main())

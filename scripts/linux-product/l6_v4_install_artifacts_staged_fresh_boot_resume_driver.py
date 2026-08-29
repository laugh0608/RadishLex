#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import os
import re
import stat
import sys
from pathlib import Path
from types import ModuleType


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-v4-install-artifacts-staged-fresh-boot-resume-driver-v1"
)
MARKER_FORMAT = f"{EVIDENCE_FORMAT}-marker"
PHASE_FORMAT = f"{EVIDENCE_FORMAT}-phase"
EXPECTED_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-fresh-boot-resume-20260827-v1"
)
EXPECTED_CHECKPOINT_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-checkpoint-20260823-v1"
)
EXPECTED_TARGET_UUID = "50B75F88-493D-42C0-A1DC-054DEC478038"
EXPECTED_PRIOR_BOOT_ID_SHA256 = (
    "18f1ba063ecd3087a5624e6ae52a54624540a972df33cca00a34bca4ec00022a"
)
EXPECTED_CURRENT_BOOT_ID_SHA256 = (
    "d64b962e4fe1ad7c9540ff9055bbd0aaaea371e6342a05aed42a783a21d10b50"
)
EXPECTED_OPERATION_ID_SHA256 = (
    "21041a89668ff577c9b7912789e3445bb33a21873bbec56530177d8574d1111a"
)
EXPECTED_CHECKPOINT_SHA256 = (
    "9cb4acb856e460808a6d7f8447ea4f36833c939e7b7349dd2c2ea331004625e0"
)
EXPECTED_CRASH_STATE_SHA256 = (
    "55f705799e4d198e392f1fe4104a17fccc97bcb5bd4d43d57b2e83a67dbcaf1a"
)
EXPECTED_RESUME_DRIVER_SHA256 = (
    "0b649ecd7b90a8368cf40a3887660cade5fa90cf9119bd7baba0904bcad3d05b"
)
EXPECTED_RECOVERY_PROBE_SHA256 = (
    "e63c37e6465a6bdc0adbf13b9840111e397eaf59255829c85a3463be294187c0"
)
EXPECTED_CONTROL_ROOT = Path(
    "/var/tmp/radishlex-l6-v4-install-artifacts-staged-fresh-boot-resume-"
    f"{EXPECTED_ATTEMPT_ID}"
)
BOOT_ID_PATH = Path("/proc/sys/kernel/random/boot_id")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")
HEX_64 = re.compile(r"[0-9a-f]{64}")


class FreshBootResumeDriverError(ValueError):
    pass


def canonical_json(value: dict[str, object]) -> bytes:
    import json

    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )


def require_private_directory(path: Path, label: str) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_gid != 0
        or stat.S_IMODE(metadata.st_mode) != 0o700
    ):
        raise FreshBootResumeDriverError(f"directory-identity-invalid:{label}")


def require_regular(path: Path, label: str, mode: int) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_gid != 0
        or stat.S_IMODE(metadata.st_mode) != mode
        or metadata.st_nlink != 1
    ):
        raise FreshBootResumeDriverError(f"file-identity-invalid:{label}")


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
    require_regular(path, "created-evidence", 0o600)
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
        raise FreshBootResumeDriverError("phase-incoming-exists")
    create_new_file(
        incoming, canonical_json({"format": PHASE_FORMAT, "phase": phase})
    )
    os.replace(incoming, control_root / "phase.json")
    sync_directory(control_root)


def publish_terminal(control_root: Path, value: dict[str, object]) -> None:
    incoming = control_root / ".terminal.incoming.json"
    final = control_root / "resume.evidence.json"
    payload = canonical_json(value)
    create_new_file(incoming, payload)
    try:
        os.link(incoming, final, follow_symlinks=False)
    except FileExistsError as exc:
        raise FreshBootResumeDriverError("terminal-evidence-exists") from exc
    os.unlink(incoming)
    require_regular(final, "terminal-evidence", 0o600)
    sync_directory(control_root)


def load_module(path: Path, expected_sha256: str, name: str) -> ModuleType:
    require_regular(path, name, 0o600)
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected_sha256:
        raise FreshBootResumeDriverError(f"{name}-identity-invalid")
    spec = importlib.util.spec_from_file_location(f"radishlex_{name}", path)
    if spec is None or spec.loader is None:
        raise FreshBootResumeDriverError(f"{name}-import-invalid")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def reconstruct_transient_secret(driver: ModuleType, operation_id: str) -> None:
    if driver.RUN_ROOT.exists() or driver.RUN_ROOT.is_symlink():
        raise FreshBootResumeDriverError("transient-run-root-already-present")
    os.mkdir(driver.RUN_ROOT, 0o700)
    os.chmod(driver.RUN_ROOT, 0o700, follow_symlinks=False)
    os.chown(driver.RUN_ROOT, 0, 0, follow_symlinks=False)
    require_private_directory(driver.RUN_ROOT, "transient-run-root")
    sync_directory(driver.RUN_ROOT.parent)
    driver.create_new_file(
        driver.RUN_ROOT / "operation-id.secret", operation_id.encode("ascii"), 0o600
    )
    driver.require_regular(
        driver.RUN_ROOT / "operation-id.secret", "operation-secret", 0o600, 32
    )
    if driver.sha256_bytes(operation_id.encode("ascii")) != EXPECTED_OPERATION_ID_SHA256:
        raise FreshBootResumeDriverError("operation-secret-identity-invalid")


def validate_fresh_boot_readiness(
    driver: ModuleType, recovery: ModuleType, current_boot_id_sha256: str
) -> str:
    operation_id = recovery.validate_persistent_transaction(driver)
    guard_profile, startup_output = recovery.classify_guard(driver)
    if guard_profile != "absent-after-reboot":
        raise FreshBootResumeDriverError("fresh-boot-guard-not-absent")
    if startup_output != recovery.EXPECTED_OPERATION_IN_PROGRESS_STARTUP:
        raise FreshBootResumeDriverError("fresh-boot-startup-gate-invalid")
    recovery.validate_live_new_boot(driver, startup_output, current_boot_id_sha256)
    if driver.RUN_ROOT.exists() or driver.RUN_ROOT.is_symlink():
        raise FreshBootResumeDriverError("transient-run-root-already-present")
    return operation_id


def revalidate_before_resume(
    driver: ModuleType,
    recovery: ModuleType,
    current_boot_id_sha256: str,
    operation_id: str,
) -> None:
    if recovery.validate_persistent_transaction(driver) != operation_id:
        raise FreshBootResumeDriverError("persistent-operation-identity-drift")
    guard_profile, startup_output = recovery.classify_guard(driver)
    if guard_profile != "absent-after-reboot":
        raise FreshBootResumeDriverError("fresh-boot-guard-drift")
    recovery.validate_live_new_boot(driver, startup_output, current_boot_id_sha256)
    driver.validate_frozen_checkpoint()
    driver.require_regular(
        driver.RUN_ROOT / "operation-id.secret", "operation-secret", 0o600, 32
    )
    secret = driver.read_bounded(driver.RUN_ROOT / "operation-id.secret", 32)
    if secret != operation_id.encode("ascii"):
        raise FreshBootResumeDriverError("operation-secret-drift")


def validate_current_boot_identity(
    driver: ModuleType, current_boot_id_sha256: str
) -> None:
    try:
        raw_boot = BOOT_ID_PATH.read_text(encoding="ascii").strip()
    except (OSError, UnicodeError) as exc:
        raise FreshBootResumeDriverError("current-boot-id-unreadable") from exc
    if (
        not raw_boot
        or driver.sha256_bytes(raw_boot.encode("ascii"))
        != current_boot_id_sha256
    ):
        raise FreshBootResumeDriverError("current-boot-id-drift")


def validate_completed_result(
    driver: ModuleType,
    prior_boot_id_sha256: str,
    current_boot_id_sha256: str,
) -> dict[str, object]:
    if driver.EXPECTED_BOOT_ID_SHA256 != prior_boot_id_sha256:
        raise FreshBootResumeDriverError(
            "terminal-postflight-prior-boot-contract-invalid"
        )
    validate_current_boot_identity(driver, current_boot_id_sha256)
    completed = driver.validate_resume_result()
    if driver.EXPECTED_BOOT_ID_SHA256 != prior_boot_id_sha256:
        raise FreshBootResumeDriverError(
            "terminal-postflight-prior-boot-contract-drift"
        )
    if driver.GUARD_PATH.exists() or driver.GUARD_PATH.is_symlink():
        raise FreshBootResumeDriverError("completed-guard-still-present")
    validate_current_boot_identity(driver, current_boot_id_sha256)
    return completed


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--resume-attempt-id", required=True)
    parser.add_argument("--checkpoint-attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--prior-boot-id-sha256", required=True)
    parser.add_argument("--current-boot-id-sha256", required=True)
    parser.add_argument("--expected-operation-id-sha256", required=True)
    parser.add_argument("--expected-checkpoint-sha256", required=True)
    parser.add_argument("--expected-crash-state-sha256", required=True)
    parser.add_argument("--resume-driver", type=Path, required=True)
    parser.add_argument("--expected-resume-driver-sha256", required=True)
    parser.add_argument("--recovery-probe", type=Path, required=True)
    parser.add_argument("--expected-recovery-probe-sha256", required=True)
    parser.add_argument("--expected-driver-sha256", required=True)
    parser.add_argument("--control-root", type=Path, required=True)
    return parser.parse_args(argv)


def validate_argument_contract(args: argparse.Namespace) -> None:
    expected = {
        "resume_attempt_id": EXPECTED_ATTEMPT_ID,
        "checkpoint_attempt_id": EXPECTED_CHECKPOINT_ATTEMPT_ID,
        "target_uuid": EXPECTED_TARGET_UUID,
        "prior_boot_id_sha256": EXPECTED_PRIOR_BOOT_ID_SHA256,
        "current_boot_id_sha256": EXPECTED_CURRENT_BOOT_ID_SHA256,
        "expected_operation_id_sha256": EXPECTED_OPERATION_ID_SHA256,
        "expected_checkpoint_sha256": EXPECTED_CHECKPOINT_SHA256,
        "expected_crash_state_sha256": EXPECTED_CRASH_STATE_SHA256,
        "expected_resume_driver_sha256": EXPECTED_RESUME_DRIVER_SHA256,
        "expected_recovery_probe_sha256": EXPECTED_RECOVERY_PROBE_SHA256,
    }
    if any(getattr(args, key) != value for key, value in expected.items()):
        raise FreshBootResumeDriverError("argument-contract-mismatch")
    if (
        not SAFE_ATTEMPT_ID.fullmatch(args.resume_attempt_id)
        or not HEX_64.fullmatch(args.expected_driver_sha256)
        or args.control_root != EXPECTED_CONTROL_ROOT
    ):
        raise FreshBootResumeDriverError("argument-invalid")


def sanitize_reason(value: str) -> str:
    return re.sub(r"(?<![0-9a-f])[0-9a-f]{32}(?![0-9a-f])", "[redacted]", value)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    phase = "arguments"
    control_root_validated = False
    resume_started = False
    resume_invocations = 0
    postflight_invocations = 0
    transient_secret_reconstructed = False
    terminal_case_sha256: str | None = None
    try:
        if os.geteuid() != 0:
            raise FreshBootResumeDriverError("root-required")
        validate_argument_contract(args)
        require_private_directory(args.control_root, "control-root")
        control_root_validated = True
        driver_path = Path(__file__)
        require_regular(driver_path, "fresh-boot-resume-driver", 0o600)
        if hashlib.sha256(driver_path.read_bytes()).hexdigest() != args.expected_driver_sha256:
            raise FreshBootResumeDriverError("fresh-boot-resume-driver-identity-drift")
        driver = load_module(
            args.resume_driver, args.expected_resume_driver_sha256, "frozen_resume_driver"
        )
        recovery = load_module(
            args.recovery_probe,
            args.expected_recovery_probe_sha256,
            "frozen_recovery_probe",
        )
        create_new_file(
            args.control_root / "attempt.marker.json",
            canonical_json(
                {
                    "format": MARKER_FORMAT,
                    "fresh_boot_resume_driver_sha256": args.expected_driver_sha256,
                    "recovery_probe_sha256": args.expected_recovery_probe_sha256,
                    "resume_attempt_id": args.resume_attempt_id,
                    "resume_driver_sha256": args.expected_resume_driver_sha256,
                }
            ),
        )

        phase = "fresh-boot-readiness"
        replace_phase(args.control_root, phase)
        operation_id = validate_fresh_boot_readiness(
            driver, recovery, args.current_boot_id_sha256
        )

        phase = "transient-secret-reconstruction"
        replace_phase(args.control_root, phase)
        reconstruct_transient_secret(driver, operation_id)
        transient_secret_reconstructed = True

        phase = "resume-readiness-revalidation"
        replace_phase(args.control_root, phase)
        revalidate_before_resume(
            driver, recovery, args.current_boot_id_sha256, operation_id
        )

        phase = "terminal-case"
        replace_phase(args.control_root, phase)
        terminal_case, terminal_case_sha256 = driver.derive_terminal_case(
            args.control_root
        )

        phase = "resume"
        replace_phase(args.control_root, phase)
        revalidate_before_resume(
            driver, recovery, args.current_boot_id_sha256, operation_id
        )
        resume_started = True
        resume_invocations = 1
        driver.require_case_success(
            driver.run_case(args.control_root, terminal_case, "resume"),
            b"install_artifacts_staged_resume_outcome=completed\n",
            "resume",
        )

        phase = "postflight"
        replace_phase(args.control_root, phase)
        postflight_invocations = 1
        driver.require_case_success(
            driver.run_case(args.control_root, terminal_case, "postflight"),
            b"install_artifacts_staged_postflight_outcome=passed\n",
            "postflight",
        )
        completed = validate_completed_result(
            driver,
            args.prior_boot_id_sha256,
            args.current_boot_id_sha256,
        )

        phase = "complete"
        replace_phase(args.control_root, phase)
        publish_terminal(
            args.control_root,
            {
                "automatic_cleanup": "not-performed",
                "automatic_quit": "not-performed",
                "automatic_retry": "not-performed",
                "automatic_stop": "not-performed",
                "checkpoint_attempt_id": args.checkpoint_attempt_id,
                "checkpoint_sha256": args.expected_checkpoint_sha256,
                "current_boot_id_sha256": args.current_boot_id_sha256,
                "dpkg_mutation_executed": True,
                "format": EVIDENCE_FORMAT,
                "guard_profile_before_resume": "absent-after-reboot",
                "maintenance_resume_invocations": resume_invocations,
                "operation_id": "reconstructed-secret-hash-only",
                "operation_id_sha256": args.expected_operation_id_sha256,
                "outcome": "resume-completed",
                "phase": phase,
                "postflight_invocations": postflight_invocations,
                "prior_boot_id_sha256": args.prior_boot_id_sha256,
                "reason": "one-shot-fresh-boot-install-artifacts-staged-resume-passed",
                "resume_attempt_id": args.resume_attempt_id,
                "terminal_case_sha256": terminal_case_sha256,
                "transaction": "completed",
                "transient_secret_reconstructed": True,
                **completed,
            },
        )
        return 0
    except Exception as exc:
        outcome = "state-indeterminate" if resume_started else "resume-rejected"
        terminal = {
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "checkpoint_attempt_id": getattr(args, "checkpoint_attempt_id", "invalid"),
            "current_boot_id_sha256": getattr(
                args, "current_boot_id_sha256", "invalid"
            ),
            "dpkg_mutation_executed": "unknown" if resume_started else False,
            "format": EVIDENCE_FORMAT,
            "maintenance_resume_invocations": resume_invocations,
            "operation_id": "reconstructed-secret-hash-only",
            "operation_id_sha256": EXPECTED_OPERATION_ID_SHA256,
            "outcome": outcome,
            "phase": phase,
            "postflight_invocations": postflight_invocations,
            "prior_boot_id_sha256": getattr(args, "prior_boot_id_sha256", "invalid"),
            "reason": sanitize_reason(f"{type(exc).__name__}:{exc}")[:512],
            "resume_attempt_id": getattr(args, "resume_attempt_id", "invalid"),
            "terminal_case_sha256": terminal_case_sha256,
            "transaction": (
                "state-indeterminate"
                if resume_started
                else "artifacts-staged-preserved-no-resume"
            ),
            "transient_secret_reconstructed": transient_secret_reconstructed,
        }
        if control_root_validated:
            try:
                replace_phase(args.control_root, "indeterminate" if resume_started else "rejected")
                publish_terminal(args.control_root, terminal)
            except Exception as publish_error:
                print(
                    "fresh boot resume terminal publish failed: "
                    f"{type(publish_error).__name__}:{publish_error}",
                    file=sys.stderr,
                )
        print(terminal["reason"], file=sys.stderr)
        return 12 if resume_started else 10


if __name__ == "__main__":
    raise SystemExit(main())

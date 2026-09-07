#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
from pathlib import Path


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-v4-install-artifacts-staged-checkpoint-driver-v1"
)
MARKER_FORMAT = f"{EVIDENCE_FORMAT}-marker"
MAX_DRIVER_BYTES = 128 * 1024
MAX_EVIDENCE_BYTES = 1024 * 1024
MAX_COMMAND_OUTPUT_BYTES = 64 * 1024
HEX_32 = re.compile(r"[0-9a-f]{32}")
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")
INPUT_ROOT = Path("/var/tmp/radishlex-l6-inputs")
CASE_PATH = INPUT_ROOT / "case.sh"
SNAPSHOT_PATH = INPUT_ROOT / "snapshot-identity.evidence.txt"
RUN_ROOT = Path("/run/radishlex-l6-crash-install-artifacts-staged")
OUTPUT_ROOT = Path("/var/tmp/radishlex-l6-crash-install-artifacts-staged-output")
GUEST_IDENTITY_PATH = Path(
    "/var/tmp/radishlex-l6-crash-install-artifacts-staged-guest-identity.evidence.txt"
)
CHECKPOINT_ROOT = Path("/var/tmp/radishlex-l6-evidence")
STATE_ROOT = Path("/var/lib/radishlex/install-v1")
GUARD_PATH = Path("/run/lock/radishlex-install-v1.lock")
EXPECTED_CASE_SHA256 = (
    "30e7f0e0dc7202d32592c5b1e003d995d8063a67c278e82469eb39d33e54588d"
)
EXPECTED_CASE_SIZE = 33485
EXPECTED_SNAPSHOT_SHA256 = (
    "7f656ad3065a87e52d0abd4e87a70a6638945e9759a48ee9b23441d7c1396d7b"
)
EXPECTED_REPOSITORY_COMMIT = "d75818f764758bfc4d90102ed2a6b3208d1debcf"
EXPECTED_DPKG_STATUS_SHA256 = (
    "2c31c35c262b2b2761055fa12a55361f3d47ebfa1923dbb3cc3691499d2ce572"
)
EXPECTED_DPKG_CONFIG_SHA256 = (
    "fead43b89af3ea5691c48f32d7fe1ba0f7ab229fb5d230f612d76fe8e6f5a015"
)
EXPECTED_PREFLIGHT_STARTUP = "0:1:4:15:0|error-absent"
EXPECTED_STARTUP = "0:1:3:10:0|error-absent"


class CheckpointDriverError(ValueError):
    pass


def canonical_json(value: dict[str, object]) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


def read_bounded(path: Path, limit: int = MAX_EVIDENCE_BYTES) -> bytes:
    with path.open("rb") as source:
        value = source.read(limit + 1)
    if not value or len(value) > limit:
        raise CheckpointDriverError(f"bounded-file-invalid:{path.name}")
    return value


def read_fields(path: Path) -> dict[str, str]:
    fields: dict[str, str] = {}
    try:
        text = read_bounded(path).decode("utf-8")
    except UnicodeDecodeError as exc:
        raise CheckpointDriverError(f"field-file-not-utf8:{path.name}") from exc
    if not text.endswith("\n") or "\r" in text or "\x00" in text:
        raise CheckpointDriverError(f"field-file-not-canonical:{path.name}")
    for line in text.splitlines():
        key, separator, value = line.partition("=")
        if not separator or not key or key in fields:
            raise CheckpointDriverError(f"field-file-invalid:{path.name}")
        fields[key] = value
    return fields


def require_regular(path: Path, mode: int, size: int | None = None) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_gid != 0
        or stat.S_IMODE(metadata.st_mode) != mode
        or metadata.st_nlink != 1
        or (size is not None and metadata.st_size != size)
    ):
        raise CheckpointDriverError(f"file-identity-invalid:{path.name}")


def require_private_directory(path: Path) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_gid != 0
        or stat.S_IMODE(metadata.st_mode) != 0o700
    ):
        raise CheckpointDriverError(f"directory-identity-invalid:{path.name}")


def sync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def create_new_file(path: Path, payload: bytes, mode: int = 0o600) -> None:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    try:
        with os.fdopen(descriptor, "wb", closefd=False) as target:
            target.write(payload)
            target.flush()
            os.fsync(target.fileno())
    finally:
        os.close(descriptor)
    os.chmod(path, mode, follow_symlinks=False)
    os.chown(path, 0, 0, follow_symlinks=False)
    require_regular(path, mode, len(payload))
    sync_directory(path.parent)


def replace_phase(control_root: Path, phase: str) -> None:
    incoming = control_root / ".phase.incoming.json"
    final = control_root / "phase.json"
    if incoming.exists() or incoming.is_symlink():
        raise CheckpointDriverError("phase-incoming-exists")
    payload = canonical_json({"format": EVIDENCE_FORMAT, "phase": phase})
    create_new_file(incoming, payload)
    os.replace(incoming, final)
    sync_directory(control_root)


def publish_terminal(control_root: Path, value: dict[str, object]) -> None:
    incoming = control_root / ".checkpoint.incoming.json"
    final = control_root / "checkpoint.evidence.json"
    payload = canonical_json(value)
    create_new_file(incoming, payload)
    try:
        os.link(incoming, final, follow_symlinks=False)
    except FileExistsError as exc:
        raise CheckpointDriverError("checkpoint-evidence-exists") from exc
    os.unlink(incoming)
    require_regular(final, 0o600, len(payload))
    sync_directory(control_root)


def run_case(control_root: Path, phase: str) -> subprocess.CompletedProcess[bytes]:
    observation = subprocess.run(
        ("/bin/sh", str(CASE_PATH), phase),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=20 * 60,
        check=False,
        env={"LC_ALL": "C", "PATH": "/usr/sbin:/usr/bin:/sbin:/bin"},
    )
    if (
        len(observation.stdout) > MAX_COMMAND_OUTPUT_BYTES
        or len(observation.stderr) > MAX_COMMAND_OUTPUT_BYTES
    ):
        raise CheckpointDriverError(f"case-output-too-large:{phase}")
    create_new_file(control_root / f"case-{phase}.stdout", observation.stdout)
    create_new_file(control_root / f"case-{phase}.stderr", observation.stderr)
    return observation


def require_case_success(
    observation: subprocess.CompletedProcess[bytes], expected_stdout: bytes, phase: str
) -> None:
    if (
        observation.returncode != 0
        or observation.stdout != expected_stdout
        or observation.stderr
    ):
        raise CheckpointDriverError(f"case-{phase}-failed")


def validate_negative_preflight(
    path: Path, expected_sha256: str, expected_boot_id_sha256: str
) -> dict[str, object]:
    payload = read_bounded(path)
    if sha256_bytes(payload) != expected_sha256:
        raise CheckpointDriverError("negative-preflight-evidence-drift")
    try:
        value = json.loads(payload)
    except json.JSONDecodeError as exc:
        raise CheckpointDriverError("negative-preflight-evidence-invalid") from exc
    if not isinstance(value, dict) or canonical_json(value) != payload:
        raise CheckpointDriverError("negative-preflight-evidence-not-canonical")
    required = {
        "boot_id_sha256": expected_boot_id_sha256,
        "checkpoint_evidence": "absent",
        "dpkg_config_sha256": EXPECTED_DPKG_CONFIG_SHA256,
        "dpkg_status_sha256": EXPECTED_DPKG_STATUS_SHA256,
        "fcitx_startup": EXPECTED_PREFLIGHT_STARTUP,
        "guard": "absent",
        "input_inventory_count": 12,
        "input_inventory_identity": "matched",
        "manager_startup": EXPECTED_PREFLIGHT_STARTUP,
        "network": "loopback-only-main-routes-empty",
        "operation_id": "not-generated",
        "outcome": "passed",
        "package": "not-installed",
        "phase": "complete",
        "product_processes": "absent",
        "receipt_terminal": "absent",
        "state_root": "absent",
        "transaction": "not-performed",
        "user_xdg": "absent",
    }
    if any(value.get(key) != expected for key, expected in required.items()):
        raise CheckpointDriverError("negative-preflight-semantics-invalid")
    if (
        not HEX_64.fullmatch(str(value.get("dpkg_log_sha256")))
        or not isinstance(value.get("dpkg_log_size"), int)
        or value["dpkg_log_size"] < 0
    ):
        raise CheckpointDriverError("negative-preflight-dpkg-log-invalid")
    return value


def validate_mutation_preflight(
    guest_identity_sha256: str,
    expected_boot_id_sha256: str,
    negative_preflight: dict[str, object],
) -> dict[str, str]:
    path = OUTPUT_ROOT / "mutation-preflight.evidence.txt"
    require_regular(path, 0o600)
    mutation = read_fields(path)
    required = {
        "format": (
            "radishlex-linux-l6-install-artifacts-staged-mutation-preflight-v1"
        ),
        "boot_id_sha256": expected_boot_id_sha256,
        "guest_identity_sha256": guest_identity_sha256,
        "snapshot_identity_sha256": EXPECTED_SNAPSHOT_SHA256,
        "dpkg_status_sha256": negative_preflight["dpkg_status_sha256"],
        "dpkg_log_sha256": negative_preflight["dpkg_log_sha256"],
        "dpkg_log_size": str(negative_preflight["dpkg_log_size"]),
        "package": "not-installed",
        "state_root": "absent",
        "checkpoint_evidence": "absent",
        "guard": "absent",
        "manager_startup": EXPECTED_PREFLIGHT_STARTUP,
        "fcitx_startup": EXPECTED_PREFLIGHT_STARTUP,
        "startup_reason": "ReceiptMissing",
        "receipt_terminal": "absent",
        "user_xdg": "absent",
        "product_processes": "absent",
        "network": "loopback-only-main-routes-empty",
        "operation_id": "not-generated",
        "mutation_preflight": "passed",
    }
    if any(mutation.get(key) != expected for key, expected in required.items()):
        raise CheckpointDriverError("mutation-preflight-invalid")
    return mutation


def validate_checkpoint_state(
    guest_identity_sha256: str,
    expected_boot_id_sha256: str,
    negative_preflight: dict[str, object],
) -> dict[str, object]:
    mutation = validate_mutation_preflight(
        guest_identity_sha256, expected_boot_id_sha256, negative_preflight
    )
    crash = read_fields(OUTPUT_ROOT / "crash-result.evidence.txt")
    state = read_fields(OUTPUT_ROOT / "crash-state.evidence.txt")
    operation_hash = crash.get("operation_id_sha256", "")
    if not HEX_64.fullmatch(operation_hash):
        raise CheckpointDriverError("operation-id-hash-invalid")
    checkpoint_path = (
        CHECKPOINT_ROOT
        / "checkpoints"
        / f"install_artifacts_staged-{operation_hash[:16]}.json"
    )
    checkpoint_payload = read_bounded(checkpoint_path)
    try:
        checkpoint = json.loads(checkpoint_payload)
    except json.JSONDecodeError as exc:
        raise CheckpointDriverError("checkpoint-json-invalid") from exc
    if not isinstance(checkpoint, dict):
        raise CheckpointDriverError("checkpoint-json-invalid")
    operation = checkpoint.get("operation")
    termination = checkpoint.get("termination")
    if not isinstance(operation, dict) or not isinstance(termination, dict):
        raise CheckpointDriverError("checkpoint-contract-invalid")
    required_checkpoint = {
        "format": "radishlex-linux-l6-checkpoint-evidence-v1",
        "build_identity": "radishlex-linux-l6-acceptance-v1",
        "repository_commit": EXPECTED_REPOSITORY_COMMIT,
        "guest_identity_sha256": guest_identity_sha256,
        "scenario": "install_artifacts_staged",
        "checkpoint": "artifacts_staged",
        "expected_terminal": "completed",
    }
    if any(checkpoint.get(key) != expected for key, expected in required_checkpoint.items()):
        raise CheckpointDriverError("checkpoint-contract-invalid")
    if operation != {
        "matrix_operation": "install_source",
        "operation_id_sha256": operation_hash,
    }:
        raise CheckpointDriverError("checkpoint-operation-invalid")
    if any(
        termination.get(key) != expected
        for key, expected in {
            "checkpoint_notification": "inherited-pipe-v1",
            "process_group": "terminated",
            "signal": "sigkill",
            "worker": "signaled",
            "process_group_member_count": 0,
            "dpkg_child": "absent",
            "process_inspection": "complete",
        }.items()
    ):
        raise CheckpointDriverError("checkpoint-termination-invalid")
    if any(
        crash.get(key) != expected
        for key, expected in {
            "format": "radishlex-linux-l6-install-artifacts-staged-crash-result-v1",
            "acceptance_invocations": "1",
            "checkpoint_count": "1",
            "fault": "process_group_terminated",
            "crash_result": "passed",
        }.items()
    ):
        raise CheckpointDriverError("crash-result-invalid")
    required_state = {
        "format": "radishlex-linux-l6-install-artifacts-staged-crash-state-v1",
        "operation_id_sha256": operation_hash,
        "checkpoint_sha256": sha256_bytes(checkpoint_payload),
        "receipt": "install|not_applicable|artifacts_staged|chain-1",
        "guard": "present-valid-unlocked",
        "dpkg_mutation_executed": "false",
        "dpkg_status_sha256": mutation.get("dpkg_status_sha256"),
        "dpkg_log_sha256": mutation.get("dpkg_log_sha256"),
        "manager_startup": EXPECTED_STARTUP,
        "fcitx_startup": EXPECTED_STARTUP,
        "user_xdg": "absent",
        "product_processes": "absent",
        "network": "loopback-only-main-routes-empty",
        "crash_state": "passed",
    }
    if any(state.get(key) != expected for key, expected in required_state.items()):
        raise CheckpointDriverError("crash-state-invalid")
    secret_path = RUN_ROOT / "operation-id.secret"
    require_regular(secret_path, 0o600, 32)
    secret = read_bounded(secret_path, 32).decode("ascii")
    if not HEX_32.fullmatch(secret) or sha256_bytes(secret.encode("ascii")) != operation_hash:
        raise CheckpointDriverError("operation-secret-invalid")
    return {
        "checkpoint_sha256": sha256_bytes(checkpoint_payload),
        "checkpoint_size": len(checkpoint_payload),
        "crash_result_sha256": sha256_file(
            OUTPUT_ROOT / "crash-result.evidence.txt"
        ),
        "crash_state_sha256": sha256_file(
            OUTPUT_ROOT / "crash-state.evidence.txt"
        ),
        "mutation_preflight_sha256": sha256_file(
            OUTPUT_ROOT / "mutation-preflight.evidence.txt"
        ),
        "operation_id_sha256": operation_hash,
    }


def build_guest_identity(target_uuid: str, boot_id_sha256: str) -> bytes:
    snapshot_sha256 = sha256_file(SNAPSHOT_PATH)
    machine_id_sha256 = sha256_bytes(
        Path("/etc/machine-id").read_text(encoding="ascii").strip().encode("ascii")
    )
    return (
        "format=radishlex-linux-l6-install-artifacts-staged-guest-identity-v1\n"
        f"clone_uuid={target_uuid}\n"
        f"snapshot_identity_sha256={snapshot_sha256}\n"
        f"boot_id_sha256={boot_id_sha256}\n"
        f"machine_id_sha256={machine_id_sha256}\n"
        "guest=debian-13|arm64|aarch64\n"
        "network=loopback-only-main-routes-empty\n"
        "package=not-installed\nstate_root=absent\ncheckpoint_evidence=absent\n"
        "guard=absent\ninput=ready\noperation_id=not-generated\n"
        "user_xdg=absent\nproduct_processes=absent\nnegative_preflight=passed\n"
    ).encode("ascii")


def prepare_case_roots(
    target_uuid: str,
    boot_id_sha256: str,
    negative_preflight_sha256: str,
) -> str:
    if any(path.exists() or path.is_symlink() for path in (RUN_ROOT, OUTPUT_ROOT, GUEST_IDENTITY_PATH)):
        raise CheckpointDriverError("case-root-already-exists")
    if STATE_ROOT.exists() or CHECKPOINT_ROOT.exists() or GUARD_PATH.exists():
        raise CheckpointDriverError("transaction-state-already-exists")
    require_private_directory(INPUT_ROOT)
    require_regular(CASE_PATH, 0o600, EXPECTED_CASE_SIZE)
    if sha256_file(CASE_PATH) != EXPECTED_CASE_SHA256:
        raise CheckpointDriverError("case-identity-drift")
    require_regular(SNAPSHOT_PATH, 0o600)
    if sha256_file(SNAPSHOT_PATH) != EXPECTED_SNAPSHOT_SHA256:
        raise CheckpointDriverError("snapshot-identity-drift")
    os.mkdir(RUN_ROOT, 0o700)
    os.mkdir(OUTPUT_ROOT, 0o700)
    os.chown(RUN_ROOT, 0, 0)
    os.chown(OUTPUT_ROOT, 0, 0)
    require_private_directory(RUN_ROOT)
    require_private_directory(OUTPUT_ROOT)
    guest_identity = build_guest_identity(target_uuid, boot_id_sha256)
    create_new_file(GUEST_IDENTITY_PATH, guest_identity)
    guest_identity_sha256 = sha256_bytes(guest_identity)
    transfer_setup = (
        "format=radishlex-linux-l6-install-artifacts-staged-transfer-setup-v1\n"
        f"guest_identity_sha256={guest_identity_sha256}\n"
        f"snapshot_identity_sha256={EXPECTED_SNAPSHOT_SHA256}\n"
        f"negative_preflight_sha256={negative_preflight_sha256}\n"
        f"case_sha256={EXPECTED_CASE_SHA256}\n"
        "input_inventory=utf8-bytewise-relative-path-v1|12|verified\n"
        "network=loopback-only-main-routes-empty\npackage=not-installed\n"
        "state_root=absent\noperation_id=not-generated\ntransfer_setup=passed\n"
    ).encode("ascii")
    create_new_file(OUTPUT_ROOT / "transfer-setup.evidence.txt", transfer_setup)
    return guest_identity_sha256


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run the one-shot v4 install_artifacts_staged checkpoint driver."
    )
    parser.add_argument("--checkpoint-attempt-id", required=True)
    parser.add_argument("--preflight-attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--expected-boot-id-sha256", required=True)
    parser.add_argument("--expected-negative-preflight-sha256", required=True)
    parser.add_argument("--expected-driver-sha256", required=True)
    parser.add_argument("--control-root", type=Path, required=True)
    parser.add_argument("--negative-preflight-evidence", type=Path, required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    phase = "arguments"
    operation_started = False
    operation_hash = "not-generated"
    terminal: dict[str, object] | None = None
    control_root = args.control_root
    try:
        if os.geteuid() != 0:
            raise CheckpointDriverError("root-required")
        if (
            not SAFE_ATTEMPT_ID.fullmatch(args.checkpoint_attempt_id)
            or not SAFE_ATTEMPT_ID.fullmatch(args.preflight_attempt_id)
            or not HEX_64.fullmatch(args.expected_boot_id_sha256)
            or not HEX_64.fullmatch(args.expected_negative_preflight_sha256)
            or not HEX_64.fullmatch(args.expected_driver_sha256)
            or not args.target_uuid.isascii()
        ):
            raise CheckpointDriverError("argument-invalid")
        require_private_directory(control_root)
        driver_path = Path(__file__)
        require_regular(driver_path, 0o600)
        if sha256_file(driver_path) != args.expected_driver_sha256:
            raise CheckpointDriverError("driver-identity-drift")
        marker = canonical_json(
            {
                "checkpoint_attempt_id": args.checkpoint_attempt_id,
                "driver_sha256": args.expected_driver_sha256,
                "format": MARKER_FORMAT,
            }
        )
        create_new_file(control_root / "attempt.marker.json", marker)
        replace_phase(control_root, "negative-preflight-binding")
        phase = "negative-preflight-binding"
        negative = validate_negative_preflight(
            args.negative_preflight_evidence,
            args.expected_negative_preflight_sha256,
            args.expected_boot_id_sha256,
        )
        raw_boot = Path("/proc/sys/kernel/random/boot_id").read_text(
            encoding="ascii"
        ).strip()
        if sha256_bytes(raw_boot.encode("ascii")) != args.expected_boot_id_sha256:
            raise CheckpointDriverError("current-boot-id-drift")

        replace_phase(control_root, "case-setup")
        phase = "case-setup"
        guest_identity_sha256 = prepare_case_roots(
            args.target_uuid,
            args.expected_boot_id_sha256,
            args.expected_negative_preflight_sha256,
        )

        replace_phase(control_root, "case-preflight")
        phase = "case-preflight"
        require_case_success(
            run_case(control_root, "preflight"),
            b"install_artifacts_staged_preflight_outcome=passed\n",
            "preflight",
        )
        validate_mutation_preflight(
            guest_identity_sha256, args.expected_boot_id_sha256, negative
        )

        replace_phase(control_root, "checkpoint")
        phase = "checkpoint"
        operation_started = True
        require_case_success(
            run_case(control_root, "crash"),
            b"install_artifacts_staged_crash_outcome=checkpoint_recorded\n",
            "crash",
        )
        crash_fields = read_fields(OUTPUT_ROOT / "crash-result.evidence.txt")
        candidate_hash = crash_fields.get("operation_id_sha256", "")
        if HEX_64.fullmatch(candidate_hash):
            operation_hash = candidate_hash

        replace_phase(control_root, "inspect-crash")
        phase = "inspect-crash"
        require_case_success(
            run_case(control_root, "inspect-crash"),
            b"install_artifacts_staged_crash_state_outcome=passed\n",
            "inspect-crash",
        )
        state = validate_checkpoint_state(
            guest_identity_sha256, args.expected_boot_id_sha256, negative
        )
        operation_hash = str(state["operation_id_sha256"])
        replace_phase(control_root, "complete")
        phase = "complete"
        terminal = {
            "acceptance_invocations": 1,
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "boot_id_sha256": args.expected_boot_id_sha256,
            "case_crash_invocations": 1,
            "case_inspect_crash_invocations": 1,
            "case_preflight_invocations": 1,
            "checkpoint_attempt_id": args.checkpoint_attempt_id,
            "checkpoint_invocations": 1,
            "dpkg_mutation": "not-performed",
            "format": EVIDENCE_FORMAT,
            "guest_identity_sha256": guest_identity_sha256,
            "maintenance_resume_invocations": 0,
            "negative_preflight_outcome": negative["outcome"],
            "operation_id": "generated-once-hash-only",
            "outcome": "checkpoint-prepared",
            "phase": phase,
            "preflight_attempt_id": args.preflight_attempt_id,
            "reason": "one-shot-install-artifacts-staged-checkpoint-passed",
            "transaction": "artifacts-staged-checkpoint-only",
            **state,
        }
        publish_terminal(control_root, terminal)
        return 0
    except Exception as exc:
        reason = f"{type(exc).__name__}:{exc}"
        terminal = {
            "acceptance_invocations": 0 if not operation_started else "unknown-or-one",
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "checkpoint_attempt_id": getattr(args, "checkpoint_attempt_id", "invalid"),
            "checkpoint_invocations": 0 if not operation_started else "unknown-or-one",
            "format": EVIDENCE_FORMAT,
            "maintenance_resume_invocations": 0,
            "operation_id": (
                "not-generated" if not operation_started else "generated-not-exported"
            ),
            "operation_id_sha256": operation_hash,
            "outcome": (
                "checkpoint-rejected" if not operation_started else "state-indeterminate"
            ),
            "phase": phase,
            "reason": reason[:512],
            "transaction": (
                "not-performed" if not operation_started else "state-indeterminate"
            ),
        }
        try:
            publish_terminal(control_root, terminal)
        except Exception:
            pass
        return 10 if not operation_started else 12


if __name__ == "__main__":
    sys.exit(main())

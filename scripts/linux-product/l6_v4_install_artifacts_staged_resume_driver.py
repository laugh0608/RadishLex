#!/usr/bin/env python3
from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
from pathlib import Path


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-v4-install-artifacts-staged-resume-driver-v1"
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
STARTUP_PATH = INPUT_ROOT / "startup.py"
MAINTENANCE_PATH = INPUT_ROOT / "radishlex-linux-maintenance"
STARTUP_FFI_PATH = (
    INPUT_ROOT
    / "target-startup/usr/lib/aarch64-linux-gnu/radishlex/manager/lib/"
    "libradishlex_ime_ffi.so"
)
RUN_ROOT = Path("/run/radishlex-l6-crash-install-artifacts-staged")
OUTPUT_ROOT = Path("/var/tmp/radishlex-l6-crash-install-artifacts-staged-output")
GUEST_IDENTITY_PATH = Path(
    "/var/tmp/radishlex-l6-crash-install-artifacts-staged-guest-identity.evidence.txt"
)
CHECKPOINT_ROOT = Path("/var/tmp/radishlex-l6-evidence")
STATE_ROOT = Path("/var/lib/radishlex/install-v1")
RECEIPT_PATH = STATE_ROOT / "receipt.json"
GUARD_PATH = Path("/run/lock/radishlex-install-v1.lock")
DPKG_STATUS_PATH = Path("/var/lib/dpkg/status")
DPKG_CONFIG_PATH = Path("/etc/dpkg/dpkg.cfg")
DPKG_LOG_PATH = Path("/var/log/dpkg.log")
DPKG_UPDATES_PATH = Path("/var/lib/dpkg/updates")

EXPECTED_RESUME_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-resume-20260824-v1"
)
EXPECTED_CONTROL_ROOT = Path(
    "/var/tmp/radishlex-l6-v4-install-artifacts-staged-resume-"
    f"{EXPECTED_RESUME_ATTEMPT_ID}"
)
EXPECTED_CHECKPOINT_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-checkpoint-20260823-v1"
)
EXPECTED_TARGET_UUID = "50B75F88-493D-42C0-A1DC-054DEC478038"
EXPECTED_BOOT_ID_SHA256 = (
    "18f1ba063ecd3087a5624e6ae52a54624540a972df33cca00a34bca4ec00022a"
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
EXPECTED_RECEIPT_SHA256 = (
    "c759b5c6aa7773512b9740bb11c8f6ec8c6d8badf8bc24ee596674273c635d34"
)
EXPECTED_RECEIPT_SIZE = 2871
EXPECTED_GUEST_IDENTITY_SHA256 = (
    "08e84864ab84dbf59581032e86d9c605794ac19aa361baa1f68126491701e5aa"
)
EXPECTED_MUTATION_PREFLIGHT_SHA256 = (
    "0fa67a6633f5e727afd4f9c885218ef7ee3c4f938aa9a13a4cea3b127d644671"
)
EXPECTED_CASE_SHA256 = (
    "30e7f0e0dc7202d32592c5b1e003d995d8063a67c278e82469eb39d33e54588d"
)
EXPECTED_CASE_SIZE = 33485
EXPECTED_MAINTENANCE_SHA256 = (
    "422a5080b30fea3e4fa114674a9ec4b2636149779361e9628142bcc14c7457f3"
)
EXPECTED_MAINTENANCE_SIZE = 1_787_048
EXPECTED_STARTUP_SHA256 = (
    "0dcaef674b0e0eb43b2ff9588131e1a15a162c304500348f817bca22d54d8cb2"
)
EXPECTED_STARTUP_SIZE = 1675
EXPECTED_STARTUP_FFI_SHA256 = (
    "193145352256ba9921a61988368005c213423132f8fb3ae2635f58b366adf27d"
)
EXPECTED_STARTUP_FFI_SIZE = 7_383_672
EXPECTED_SNAPSHOT_SHA256 = (
    "7f656ad3065a87e52d0abd4e87a70a6638945e9759a48ee9b23441d7c1396d7b"
)
EXPECTED_DPKG_STATUS_SHA256 = (
    "2c31c35c262b2b2761055fa12a55361f3d47ebfa1923dbb3cc3691499d2ce572"
)
EXPECTED_DPKG_CONFIG_SHA256 = (
    "fead43b89af3ea5691c48f32d7fe1ba0f7ab229fb5d230f612d76fe8e6f5a015"
)
EXPECTED_DPKG_LOG_SHA256 = (
    "ed3a8e3291d6e2991f80983181cac4f67264460ca34513a5b18ba352395ee7d0"
)
EXPECTED_DPKG_LOG_SIZE = 879640
EXPECTED_ACTIVE_GUARD_STARTUP = "0:1:3:10:0|error-absent"
EXPECTED_ALLOWED_STARTUP = "0:1:2:2:6|error-absent"
EXPECTED_SOURCE_PACKAGE_SHA256 = (
    "09ed122804b11767b8ac7cd69c323c1f6eef511fd6ae7284d75756fb60569bec"
)
EXPECTED_SOURCE_EVIDENCE_SHA256 = (
    "fe3d6297c08dccd8cacba13d50aa44dbb1c94b0ca8c2ab4df3a5c605466fcf94"
)
EXPECTED_SOURCE_STATUS_SHA256 = (
    "33c4973d4bcccc1932de35b2b326c61140037ee46613c7a925f2cbdbd3d5cff1"
)
EXPECTED_SOURCE_MANIFEST_SHA256 = (
    "b1322933a002b46b7150334cbdad366b5d03fbc9650e381063451d458a14667b"
)
EXPECTED_SOURCE_FFI_SHA256 = (
    "d845abfec4db7745698a07b5eac796e9cced38c068c98dac32ddc946bdab6a74"
)
EXPECTED_SOURCE_MD5_SHA256 = (
    "f6afcfb73dbedd2f39c5d8a2b10ba2711a595e71b6f48cddb16298e7e83a3a50"
)

ORIGINAL_DISPATCH = (
    b'case "$1" in\n'
    b"  preflight) preflight ;;\n"
    b"  crash) crash ;;\n"
    b"  inspect-crash) inspect_crash ;;\n"
    b"  *) fail command ;;\n"
    b"esac\n"
)
TERMINAL_DISPATCH = (
    b'case "$1" in\n'
    b"  resume) resume ;;\n"
    b"  postflight) postflight ;;\n"
    b"  *) fail command ;;\n"
    b"esac\n"
)


class ResumeDriverError(ValueError):
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
        raise ResumeDriverError("bounded-file-invalid")
    return value


def read_fields(path: Path) -> dict[str, str]:
    try:
        text = read_bounded(path).decode("utf-8")
    except UnicodeDecodeError as exc:
        raise ResumeDriverError("field-file-not-utf8") from exc
    if not text.endswith("\n") or "\r" in text or "\x00" in text:
        raise ResumeDriverError("field-file-not-canonical")
    fields: dict[str, str] = {}
    for line in text.splitlines():
        key, separator, value = line.partition("=")
        if not separator or not key or key in fields:
            raise ResumeDriverError("field-file-invalid")
        fields[key] = value
    return fields


def require_regular(
    path: Path,
    label: str,
    mode: int,
    size: int | None = None,
    sha256: str | None = None,
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
        raise ResumeDriverError(f"file-identity-invalid:{label}")
    if sha256 is not None and sha256_file(path) != sha256:
        raise ResumeDriverError(f"file-sha256-invalid:{label}")


def require_private_directory(path: Path, label: str) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_gid != 0
        or stat.S_IMODE(metadata.st_mode) != 0o700
    ):
        raise ResumeDriverError(f"directory-identity-invalid:{label}")


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
    require_regular(path, "created-evidence", mode, len(payload))
    sync_directory(path.parent)


def replace_phase(control_root: Path, phase: str) -> None:
    incoming = control_root / ".phase.incoming.json"
    if incoming.exists() or incoming.is_symlink():
        raise ResumeDriverError("phase-incoming-exists")
    payload = canonical_json({"format": EVIDENCE_FORMAT, "phase": phase})
    create_new_file(incoming, payload)
    os.replace(incoming, control_root / "phase.json")
    sync_directory(control_root)


def publish_terminal(control_root: Path, value: dict[str, object]) -> None:
    incoming = control_root / ".resume.incoming.json"
    final = control_root / "resume.evidence.json"
    payload = canonical_json(value)
    create_new_file(incoming, payload)
    try:
        os.link(incoming, final, follow_symlinks=False)
    except FileExistsError as exc:
        raise ResumeDriverError("resume-evidence-exists") from exc
    os.unlink(incoming)
    require_regular(final, "resume-evidence", 0o600, len(payload))
    sync_directory(control_root)


def derive_terminal_case(control_root: Path) -> tuple[Path, str]:
    require_regular(
        CASE_PATH,
        "canonical-case",
        0o600,
        EXPECTED_CASE_SIZE,
        EXPECTED_CASE_SHA256,
    )
    source = read_bounded(CASE_PATH, EXPECTED_CASE_SIZE)
    rendered = derive_terminal_case_bytes(source)
    path = control_root / "terminal-case.sh"
    create_new_file(path, rendered)
    return path, sha256_bytes(rendered)


def derive_terminal_case_bytes(source: bytes) -> bytes:
    if source.count(ORIGINAL_DISPATCH) != 1 or TERMINAL_DISPATCH in source:
        raise ResumeDriverError("canonical-case-dispatch-invalid")
    rendered = source.replace(ORIGINAL_DISPATCH, TERMINAL_DISPATCH)
    if ORIGINAL_DISPATCH in rendered or rendered.count(TERMINAL_DISPATCH) != 1:
        raise ResumeDriverError("terminal-case-derivation-invalid")
    return rendered


def run_case(
    control_root: Path, terminal_case: Path, phase: str
) -> subprocess.CompletedProcess[bytes]:
    observation = subprocess.run(
        ("/bin/sh", str(terminal_case), phase),
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
        raise ResumeDriverError(f"case-output-too-large:{phase}")
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
        raise ResumeDriverError(f"case-{phase}-failed")


def validate_frozen_checkpoint() -> dict[str, str]:
    require_private_directory(INPUT_ROOT, "input-root")
    require_private_directory(RUN_ROOT, "run-root")
    require_private_directory(OUTPUT_ROOT, "output-root")
    require_regular(
        SNAPSHOT_PATH,
        "snapshot",
        0o600,
        sha256=EXPECTED_SNAPSHOT_SHA256,
    )
    require_regular(
        MAINTENANCE_PATH,
        "maintenance",
        0o755,
        EXPECTED_MAINTENANCE_SIZE,
        EXPECTED_MAINTENANCE_SHA256,
    )
    require_regular(
        STARTUP_PATH,
        "startup",
        0o600,
        EXPECTED_STARTUP_SIZE,
        EXPECTED_STARTUP_SHA256,
    )
    require_regular(
        STARTUP_FFI_PATH,
        "startup-ffi",
        0o600,
        EXPECTED_STARTUP_FFI_SIZE,
        EXPECTED_STARTUP_FFI_SHA256,
    )
    require_regular(
        GUEST_IDENTITY_PATH,
        "guest-identity",
        0o600,
        sha256=EXPECTED_GUEST_IDENTITY_SHA256,
    )
    mutation_path = OUTPUT_ROOT / "mutation-preflight.evidence.txt"
    crash_state_path = OUTPUT_ROOT / "crash-state.evidence.txt"
    require_regular(
        mutation_path,
        "mutation-preflight",
        0o600,
        sha256=EXPECTED_MUTATION_PREFLIGHT_SHA256,
    )
    require_regular(
        crash_state_path,
        "crash-state",
        0o600,
        sha256=EXPECTED_CRASH_STATE_SHA256,
    )
    checkpoint_path = (
        CHECKPOINT_ROOT
        / "checkpoints"
        / f"install_artifacts_staged-{EXPECTED_OPERATION_ID_SHA256[:16]}.json"
    )
    require_private_directory(CHECKPOINT_ROOT, "checkpoint-root")
    require_private_directory(checkpoint_path.parent, "checkpoint-directory")
    require_regular(
        checkpoint_path,
        "checkpoint",
        0o600,
        sha256=EXPECTED_CHECKPOINT_SHA256,
    )
    mutation = read_fields(mutation_path)
    crash_state = read_fields(crash_state_path)
    guest = read_fields(GUEST_IDENTITY_PATH)
    required_mutation = {
        "boot_id_sha256": EXPECTED_BOOT_ID_SHA256,
        "guest_identity_sha256": EXPECTED_GUEST_IDENTITY_SHA256,
        "snapshot_identity_sha256": EXPECTED_SNAPSHOT_SHA256,
        "dpkg_status_sha256": EXPECTED_DPKG_STATUS_SHA256,
        "dpkg_log_sha256": EXPECTED_DPKG_LOG_SHA256,
        "dpkg_log_size": str(EXPECTED_DPKG_LOG_SIZE),
        "package": "not-installed",
        "guard": "absent",
        "operation_id": "not-generated",
        "mutation_preflight": "passed",
    }
    if any(mutation.get(key) != value for key, value in required_mutation.items()):
        raise ResumeDriverError("mutation-preflight-semantics-invalid")
    required_crash = {
        "operation_id_sha256": EXPECTED_OPERATION_ID_SHA256,
        "checkpoint_sha256": EXPECTED_CHECKPOINT_SHA256,
        "receipt_sha256": EXPECTED_RECEIPT_SHA256,
        "receipt_size": str(EXPECTED_RECEIPT_SIZE),
        "receipt": "install|not_applicable|artifacts_staged|chain-1",
        "staged_target": (
            f"{EXPECTED_SOURCE_PACKAGE_SHA256}|{EXPECTED_SOURCE_EVIDENCE_SHA256}"
        ),
        "guard": "present-valid-unlocked",
        "dpkg_mutation_executed": "false",
        "dpkg_status_sha256": EXPECTED_DPKG_STATUS_SHA256,
        "dpkg_log_sha256": EXPECTED_DPKG_LOG_SHA256,
        "manager_startup": EXPECTED_ACTIVE_GUARD_STARTUP,
        "fcitx_startup": EXPECTED_ACTIVE_GUARD_STARTUP,
        "user_xdg": "absent",
        "product_processes": "absent",
        "network": "loopback-only-main-routes-empty",
        "crash_state": "passed",
    }
    if any(crash_state.get(key) != value for key, value in required_crash.items()):
        raise ResumeDriverError("crash-state-semantics-invalid")
    if (
        guest.get("clone_uuid") != EXPECTED_TARGET_UUID
        or guest.get("boot_id_sha256") != EXPECTED_BOOT_ID_SHA256
        or guest.get("snapshot_identity_sha256") != EXPECTED_SNAPSHOT_SHA256
        or guest.get("operation_id") != "not-generated"
    ):
        raise ResumeDriverError("guest-identity-semantics-invalid")
    try:
        checkpoint = json.loads(read_bounded(checkpoint_path))
    except json.JSONDecodeError as exc:
        raise ResumeDriverError("checkpoint-json-invalid") from exc
    operation = checkpoint.get("operation") if isinstance(checkpoint, dict) else None
    termination = checkpoint.get("termination") if isinstance(checkpoint, dict) else None
    if (
        not isinstance(checkpoint, dict)
        or checkpoint.get("scenario") != "install_artifacts_staged"
        or checkpoint.get("checkpoint") != "artifacts_staged"
        or checkpoint.get("expected_terminal") != "completed"
        or not isinstance(operation, dict)
        or operation.get("operation_id_sha256") != EXPECTED_OPERATION_ID_SHA256
        or operation.get("matrix_operation") != "install_source"
        or not isinstance(termination, dict)
        or termination.get("process_group") != "terminated"
        or termination.get("process_group_member_count") != 0
        or termination.get("dpkg_child") != "absent"
    ):
        raise ResumeDriverError("checkpoint-semantics-invalid")
    return crash_state


def validate_secret_and_receipt() -> None:
    secret_path = RUN_ROOT / "operation-id.secret"
    require_regular(secret_path, "operation-secret", 0o600, 32)
    try:
        secret = read_bounded(secret_path, 32).decode("ascii")
    except UnicodeDecodeError as exc:
        raise ResumeDriverError("operation-secret-invalid") from exc
    if (
        not HEX_32.fullmatch(secret)
        or sha256_bytes(secret.encode("ascii")) != EXPECTED_OPERATION_ID_SHA256
    ):
        raise ResumeDriverError("operation-secret-invalid")

    require_private_directory(STATE_ROOT, "state-root")
    require_regular(
        RECEIPT_PATH,
        "receipt",
        0o644,
        EXPECTED_RECEIPT_SIZE,
        EXPECTED_RECEIPT_SHA256,
    )
    if (STATE_ROOT / "receipt.json.tmp").exists():
        raise ResumeDriverError("receipt-temporary-present")
    try:
        receipt = json.loads(read_bounded(RECEIPT_PATH, 64 * 1024))
    except json.JSONDecodeError as exc:
        raise ResumeDriverError("receipt-json-invalid") from exc
    if not isinstance(receipt, dict):
        raise ResumeDriverError("receipt-json-invalid")
    target = receipt.get("target_artifact")
    staged = receipt.get("staged_artifacts")
    required_receipt = {
        "receipt_format": "radishlex-linux-install-receipt-v1",
        "product_id": "radishlex-linux",
        "distribution_identity": "debian-local-deb-v1",
        "operation_id": secret,
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
    if any(receipt.get(key) != value for key, value in required_receipt.items()):
        raise ResumeDriverError("receipt-semantics-invalid")
    if receipt.get("operation_chain") != [secret]:
        raise ResumeDriverError("receipt-operation-chain-invalid")
    if not isinstance(target, dict) or not isinstance(staged, list) or len(staged) != 1:
        raise ResumeDriverError("receipt-target-invalid")
    staged_artifact = staged[0].get("artifact") if isinstance(staged[0], dict) else None
    initial_package = receipt.get("initial_package")
    if (
        target.get("package_version") != "26.7.1+38-1"
        or target.get("package_sha256") != EXPECTED_SOURCE_PACKAGE_SHA256
        or target.get("evidence_sha256") != EXPECTED_SOURCE_EVIDENCE_SHA256
        or not isinstance(staged[0], dict)
        or staged[0].get("slot") != "target"
        or not isinstance(staged_artifact, dict)
        or staged_artifact.get("package_sha256")
        != EXPECTED_SOURCE_PACKAGE_SHA256
        or staged_artifact.get("evidence_sha256")
        != EXPECTED_SOURCE_EVIDENCE_SHA256
        or not isinstance(initial_package, dict)
        or initial_package.get("state") != "not_installed"
        or initial_package.get("artifact") is not None
    ):
        raise ResumeDriverError("receipt-target-invalid")

    operations_root = STATE_ROOT / "operations"
    require_private_directory(operations_root, "operations-root")
    operation_directories = tuple(operations_root.iterdir())
    if len(operation_directories) != 1 or operation_directories[0].name != secret:
        raise ResumeDriverError("operation-directory-count-invalid")
    operation_root = operation_directories[0]
    require_private_directory(operation_root, "operation-root")
    if {path.name for path in operation_root.iterdir()} != {
        "target.deb",
        "target.evidence.json",
    }:
        raise ResumeDriverError("operation-staging-inventory-invalid")
    require_regular(
        operation_root / "target.deb",
        "staged-target-package",
        0o600,
        40_806_592,
        EXPECTED_SOURCE_PACKAGE_SHA256,
    )
    require_regular(
        operation_root / "target.evidence.json",
        "staged-target-evidence",
        0o600,
        2376,
        EXPECTED_SOURCE_EVIDENCE_SHA256,
    )


def validate_guard() -> None:
    require_regular(GUARD_PATH, "guard", 0o600, 0)
    descriptor = os.open(GUARD_PATH, os.O_RDWR | os.O_CLOEXEC)
    try:
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as exc:
            raise ResumeDriverError("guard-still-locked") from exc
        finally:
            fcntl.flock(descriptor, fcntl.LOCK_UN)
    finally:
        os.close(descriptor)


def _run_exact(argv: tuple[str, ...], expected_stdout: bytes, label: str) -> None:
    observation = subprocess.run(
        argv,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=60,
        check=False,
        env={"LC_ALL": "C", "PATH": "/usr/sbin:/usr/bin:/sbin:/bin"},
    )
    if (
        observation.returncode != 0
        or observation.stdout != expected_stdout
        or observation.stderr
    ):
        raise ResumeDriverError(f"readonly-command-invalid:{label}")


def validate_live_system() -> None:
    raw_boot = Path("/proc/sys/kernel/random/boot_id").read_text(
        encoding="ascii"
    ).strip()
    if sha256_bytes(raw_boot.encode("ascii")) != EXPECTED_BOOT_ID_SHA256:
        raise ResumeDriverError("current-boot-id-drift")
    if sha256_file(DPKG_STATUS_PATH) != EXPECTED_DPKG_STATUS_SHA256:
        raise ResumeDriverError("dpkg-status-drift")
    if sha256_file(DPKG_CONFIG_PATH) != EXPECTED_DPKG_CONFIG_SHA256:
        raise ResumeDriverError("dpkg-config-drift")
    if (
        sha256_file(DPKG_LOG_PATH) != EXPECTED_DPKG_LOG_SHA256
        or DPKG_LOG_PATH.stat().st_size != EXPECTED_DPKG_LOG_SIZE
    ):
        raise ResumeDriverError("dpkg-log-drift")
    if any(DPKG_UPDATES_PATH.iterdir()):
        raise ResumeDriverError("dpkg-updates-present")
    _run_exact(("/usr/bin/dpkg", "--audit"), b"", "dpkg-audit")
    status = DPKG_STATUS_PATH.read_text(encoding="utf-8", errors="strict")
    if any(
        paragraph.startswith("Package: radishlex\n")
        for paragraph in status.split("\n\n")
    ):
        raise ResumeDriverError("package-present")
    if any(path.name.startswith("radishlex") for path in Path("/var/lib/dpkg/info").iterdir()):
        raise ResumeDriverError("dpkg-info-present")
    if {path.name for path in Path("/sys/class/net").iterdir()} != {"lo"}:
        raise ResumeDriverError("network-interface-drift")
    _run_exact(("/usr/sbin/ip", "-4", "route", "show", "table", "main"), b"", "ipv4-route")
    _run_exact(("/usr/sbin/ip", "-6", "route", "show", "table", "main"), b"", "ipv6-route")
    loopback = subprocess.run(
        ("/usr/sbin/ip", "-brief", "address", "show", "lo"),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=60,
        check=False,
        env={"LC_ALL": "C", "PATH": "/usr/sbin:/usr/bin:/sbin:/bin"},
    )
    loopback_fields = loopback.stdout.decode("ascii", errors="strict").split()
    if (
        loopback.returncode != 0
        or loopback.stderr
        or len(loopback_fields) != 4
        or loopback_fields[0] != "lo"
        or loopback_fields[2:] != ["127.0.0.1/8", "::1/128"]
    ):
        raise ResumeDriverError("readonly-command-invalid:loopback")
    for path in (
        Path("/home/radishlex-l6/.local/share/radishlex"),
        Path("/home/radishlex-l6/.config/radishlex"),
        Path("/home/radishlex-l6/.local/state/radishlex"),
        Path("/home/radishlex-l6/.cache/radishlex"),
        Path("/home/radishlex-l6/.config/fcitx5/profile"),
    ):
        if path.exists() or path.is_symlink():
            raise ResumeDriverError("user-xdg-present")
    forbidden_mappings = (
        b"/usr/lib/aarch64-linux-gnu/radishlex/manager",
        b"/usr/lib/aarch64-linux-gnu/fcitx5/radishlex.so",
        b"/usr/lib/aarch64-linux-gnu/fcitx5/libradishlex_ime_ffi.so",
        b"/var/tmp/radishlex-l6-inputs/radishlex-linux-maintenance",
        b"/var/tmp/radishlex-l6-inputs/radishlex-linux-l6-acceptance",
    )
    for process in Path("/proc").iterdir():
        if not process.name.isdigit():
            continue
        try:
            comm = (process / "comm").read_text(encoding="ascii").strip()
            cmdline = (process / "cmdline").read_bytes().replace(b"\0", b" ")
            mappings = (process / "maps").read_bytes()
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            continue
        if comm in {"dpkg", "dpkg-deb", "fcitx5"}:
            raise ResumeDriverError("product-or-dpkg-process-present")
        if cmdline.startswith(
            b"/usr/lib/aarch64-linux-gnu/radishlex/manager/radishlex_manager "
        ) or cmdline == b"/usr/lib/aarch64-linux-gnu/radishlex/manager/radishlex_manager":
            raise ResumeDriverError("manager-process-present")
        if any(value in mappings for value in forbidden_mappings):
            raise ResumeDriverError("product-mapping-present")
    _run_exact(
        (
            "/usr/bin/python3",
            "-B",
            str(STARTUP_PATH),
            str(STARTUP_FFI_PATH),
            "1",
            "/usr/lib/aarch64-linux-gnu/radishlex/manager/radishlex_manager",
        ),
        (EXPECTED_ACTIVE_GUARD_STARTUP + "\n").encode("ascii"),
        "manager-startup",
    )
    _run_exact(
        (
            "/usr/bin/python3",
            "-B",
            str(STARTUP_PATH),
            str(STARTUP_FFI_PATH),
            "2",
            "/usr/lib/aarch64-linux-gnu/fcitx5/radishlex.so",
        ),
        (EXPECTED_ACTIVE_GUARD_STARTUP + "\n").encode("ascii"),
        "fcitx-startup",
    )


def validate_resume_result() -> dict[str, object]:
    resume_result_path = OUTPUT_ROOT / "resume-result.evidence.txt"
    postflight_path = OUTPUT_ROOT / "terminal-postflight.evidence.txt"
    delta_path = OUTPUT_ROOT / "dpkg-delta.txt"
    require_regular(resume_result_path, "resume-result", 0o600)
    require_regular(postflight_path, "terminal-postflight", 0o600)
    require_regular(delta_path, "dpkg-delta", 0o600)
    require_regular(OUTPUT_ROOT / "resume.stdout", "maintenance-resume-stdout", 0o600)
    require_regular(OUTPUT_ROOT / "resume.stderr", "maintenance-resume-stderr", 0o600, 0)
    if read_bounded(OUTPUT_ROOT / "resume.stdout") != b"maintenance_outcome=completed\n":
        raise ResumeDriverError("maintenance-resume-stdout-invalid")
    resume = read_fields(resume_result_path)
    required_resume = {
        "format": "radishlex-linux-l6-install-artifacts-staged-resume-result-v1",
        "operation_id_sha256": EXPECTED_OPERATION_ID_SHA256,
        "crash_state_sha256": EXPECTED_CRASH_STATE_SHA256,
        "maintenance_invocations": "1",
        "maintenance_exit_code": "0",
        "maintenance_stdout": "maintenance_outcome=completed",
        "maintenance_stderr": "empty",
        "resume_result": "passed",
    }
    if resume != required_resume:
        raise ResumeDriverError("resume-result-semantics-invalid")
    postflight = read_fields(postflight_path)
    required_postflight = {
        "format": "radishlex-linux-l6-install-artifacts-staged-terminal-postflight-v1",
        "clone_uuid": EXPECTED_TARGET_UUID,
        "boot_id_sha256": EXPECTED_BOOT_ID_SHA256,
        "guest_identity_sha256": EXPECTED_GUEST_IDENTITY_SHA256,
        "snapshot_identity_sha256": EXPECTED_SNAPSHOT_SHA256,
        "operation_id_sha256": EXPECTED_OPERATION_ID_SHA256,
        "checkpoint_sha256": EXPECTED_CHECKPOINT_SHA256,
        "acceptance_invocations": "1",
        "maintenance_resume_invocations": "1",
        "operation": "install_source|install|not_applicable|completed|chain-1",
        "package": "radishlex|arm64|26.7.1+38-1|install-ok-installed",
        "target_package_sha256": EXPECTED_SOURCE_PACKAGE_SHA256,
        "target_evidence_sha256": EXPECTED_SOURCE_EVIDENCE_SHA256,
        "dpkg_status_sha256": EXPECTED_SOURCE_STATUS_SHA256,
        "dpkg_audit": "clean",
        "dpkg_verify": "clean",
        "dpkg_mutation_executed": "true",
        "installed_payload_md5_inventory": (
            f"{EXPECTED_SOURCE_MD5_SHA256}|28|verified"
        ),
        "dependency_count": "20",
        "resolved_dependency_text_digest": (
            "28fe10654a7f09b18f7f49fb32025f710fd7adf0587b2ddd1ed70bcdd6aa6aac"
        ),
        "fonts": "dejavu+noto-cjk|family+owner-satisfied",
        "manifest": EXPECTED_SOURCE_MANIFEST_SHA256,
        "ffi": f"{EXPECTED_SOURCE_FFI_SHA256}|distinct-inodes",
        "manager_startup": EXPECTED_ALLOWED_STARTUP,
        "fcitx_startup": EXPECTED_ALLOWED_STARTUP,
        "startup_negative": "invalid-component-9|1:0:0:0:0|error-present",
        "user_xdg": "absent",
        "product_processes": "absent",
        "network": "loopback-only-main-routes-empty",
        "manual_recovery_required": "false",
        "install_artifacts_staged_completed": "true",
        "terminal_postflight": "passed",
    }
    if any(postflight.get(key) != value for key, value in required_postflight.items()):
        raise ResumeDriverError("terminal-postflight-semantics-invalid")
    for key in ("receipt", "dpkg_delta"):
        digest, separator, size = postflight.get(key, "").partition("|")
        if not separator or not HEX_64.fullmatch(digest) or not size.isdigit():
            raise ResumeDriverError(f"terminal-postflight-field-invalid:{key}")
    if (
        postflight.get("dpkg_delta")
        != f"{sha256_file(delta_path)}|{delta_path.stat().st_size}"
        or postflight.get("receipt")
        != f"{sha256_file(RECEIPT_PATH)}|{RECEIPT_PATH.stat().st_size}"
    ):
        raise ResumeDriverError("terminal-postflight-artifact-identity-invalid")
    for key in ("dpkg_log_sha256",):
        if not HEX_64.fullmatch(postflight.get(key, "")):
            raise ResumeDriverError(f"terminal-postflight-field-invalid:{key}")
    if not postflight.get("dpkg_log_size", "").isdigit():
        raise ResumeDriverError("terminal-postflight-dpkg-log-size-invalid")
    return {
        "dpkg_delta_sha256": sha256_file(delta_path),
        "dpkg_delta_size": delta_path.stat().st_size,
        "dpkg_log_sha256": postflight["dpkg_log_sha256"],
        "dpkg_log_size": int(postflight["dpkg_log_size"]),
        "receipt_sha256": sha256_file(RECEIPT_PATH),
        "receipt_size": RECEIPT_PATH.stat().st_size,
        "resume_result_sha256": sha256_file(resume_result_path),
        "terminal_postflight_sha256": sha256_file(postflight_path),
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run the one-shot v4 install_artifacts_staged exact resume driver."
    )
    parser.add_argument("--resume-attempt-id", required=True)
    parser.add_argument("--checkpoint-attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--expected-boot-id-sha256", required=True)
    parser.add_argument("--expected-operation-id-sha256", required=True)
    parser.add_argument("--expected-checkpoint-sha256", required=True)
    parser.add_argument("--expected-crash-state-sha256", required=True)
    parser.add_argument("--expected-driver-sha256", required=True)
    parser.add_argument("--control-root", type=Path, required=True)
    return parser.parse_args(argv)


def sanitize_reason(value: str) -> str:
    return re.sub(r"(?<![0-9a-f])[0-9a-f]{32}(?![0-9a-f])", "[redacted]", value)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    phase = "arguments"
    resume_started = False
    resume_invocations = 0
    postflight_invocations = 0
    control_root = args.control_root
    control_root_validated = False
    terminal: dict[str, object]
    try:
        if os.geteuid() != 0:
            raise ResumeDriverError("root-required")
        expected_args = {
            "resume_attempt_id": EXPECTED_RESUME_ATTEMPT_ID,
            "checkpoint_attempt_id": EXPECTED_CHECKPOINT_ATTEMPT_ID,
            "target_uuid": EXPECTED_TARGET_UUID,
            "expected_boot_id_sha256": EXPECTED_BOOT_ID_SHA256,
            "expected_operation_id_sha256": EXPECTED_OPERATION_ID_SHA256,
            "expected_checkpoint_sha256": EXPECTED_CHECKPOINT_SHA256,
            "expected_crash_state_sha256": EXPECTED_CRASH_STATE_SHA256,
        }
        if any(getattr(args, key) != value for key, value in expected_args.items()):
            raise ResumeDriverError("argument-contract-mismatch")
        if (
            not SAFE_ATTEMPT_ID.fullmatch(args.resume_attempt_id)
            or not SAFE_ATTEMPT_ID.fullmatch(args.checkpoint_attempt_id)
            or not HEX_64.fullmatch(args.expected_driver_sha256)
            or control_root != EXPECTED_CONTROL_ROOT
        ):
            raise ResumeDriverError("argument-invalid")
        require_private_directory(control_root, "control-root")
        control_root_validated = True
        driver_path = Path(__file__)
        require_regular(driver_path, "driver", 0o600)
        if sha256_file(driver_path) != args.expected_driver_sha256:
            raise ResumeDriverError("driver-identity-drift")
        marker = canonical_json(
            {
                "driver_sha256": args.expected_driver_sha256,
                "format": MARKER_FORMAT,
                "resume_attempt_id": args.resume_attempt_id,
            }
        )
        create_new_file(control_root / "attempt.marker.json", marker)

        replace_phase(control_root, "checkpoint-binding")
        phase = "checkpoint-binding"
        validate_frozen_checkpoint()
        validate_secret_and_receipt()
        validate_guard()

        replace_phase(control_root, "resume-readiness")
        phase = "resume-readiness"
        validate_live_system()

        replace_phase(control_root, "terminal-case")
        phase = "terminal-case"
        terminal_case, terminal_case_sha256 = derive_terminal_case(control_root)

        replace_phase(control_root, "resume")
        phase = "resume"
        resume_started = True
        resume_invocations = 1
        require_case_success(
            run_case(control_root, terminal_case, "resume"),
            b"install_artifacts_staged_resume_outcome=completed\n",
            "resume",
        )

        replace_phase(control_root, "postflight")
        phase = "postflight"
        postflight_invocations = 1
        require_case_success(
            run_case(control_root, terminal_case, "postflight"),
            b"install_artifacts_staged_postflight_outcome=passed\n",
            "postflight",
        )
        completed = validate_resume_result()

        replace_phase(control_root, "complete")
        phase = "complete"
        terminal = {
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "boot_id_sha256": EXPECTED_BOOT_ID_SHA256,
            "checkpoint_attempt_id": args.checkpoint_attempt_id,
            "checkpoint_sha256": EXPECTED_CHECKPOINT_SHA256,
            "format": EVIDENCE_FORMAT,
            "maintenance_resume_invocations": resume_invocations,
            "operation_id": "existing-secret-hash-only",
            "operation_id_sha256": EXPECTED_OPERATION_ID_SHA256,
            "outcome": "resume-completed",
            "phase": phase,
            "postflight_invocations": postflight_invocations,
            "reason": "one-shot-install-artifacts-staged-resume-passed",
            "resume_attempt_id": args.resume_attempt_id,
            "terminal_case_sha256": terminal_case_sha256,
            "transaction": "completed",
            **completed,
        }
        publish_terminal(control_root, terminal)
        return 0
    except Exception as exc:
        terminal = {
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "checkpoint_attempt_id": getattr(
                args, "checkpoint_attempt_id", "invalid"
            ),
            "format": EVIDENCE_FORMAT,
            "maintenance_resume_invocations": resume_invocations,
            "operation_id": "existing-secret-hash-only",
            "operation_id_sha256": EXPECTED_OPERATION_ID_SHA256,
            "outcome": (
                "state-indeterminate" if resume_started else "resume-rejected"
            ),
            "phase": phase,
            "postflight_invocations": postflight_invocations,
            "reason": sanitize_reason(f"{type(exc).__name__}:{exc}")[:512],
            "resume_attempt_id": getattr(args, "resume_attempt_id", "invalid"),
            "transaction": (
                "state-indeterminate"
                if resume_started
                else "artifacts-staged-preserved"
            ),
        }
        if control_root_validated:
            try:
                publish_terminal(control_root, terminal)
            except Exception:
                pass
        return 12 if resume_started else 10


if __name__ == "__main__":
    sys.exit(main())

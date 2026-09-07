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
    "radishlex-linux-l6-v4-install-artifacts-staged-"
    "transaction-state-probe-v1"
)
MARKER_FORMAT = f"{EVIDENCE_FORMAT}-marker"
PHASE_FORMAT = f"{EVIDENCE_FORMAT}-phase"
EXPECTED_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-transaction-state-resolution-"
    "20260829-v2"
)
EXPECTED_TARGET_UUID = "50B75F88-493D-42C0-A1DC-054DEC478038"
EXPECTED_OPERATION_ID_SHA256 = (
    "21041a89668ff577c9b7912789e3445bb33a21873bbec56530177d8574d1111a"
)
EXPECTED_PRIOR_BOOT_ID_SHA256 = (
    "18f1ba063ecd3087a5624e6ae52a54624540a972df33cca00a34bca4ec00022a"
)
EXPECTED_GUEST_IDENTITY_SHA256 = (
    "08e84864ab84dbf59581032e86d9c605794ac19aa361baa1f68126491701e5aa"
)
EXPECTED_SNAPSHOT_SHA256 = (
    "7f656ad3065a87e52d0abd4e87a70a6638945e9759a48ee9b23441d7c1396d7b"
)
EXPECTED_CHECKPOINT_SHA256 = (
    "9cb4acb856e460808a6d7f8447ea4f36833c939e7b7349dd2c2ea331004625e0"
)
EXPECTED_RECEIPT_SHA256 = (
    "c759b5c6aa7773512b9740bb11c8f6ec8c6d8badf8bc24ee596674273c635d34"
)
EXPECTED_RECEIPT_SIZE = 2871
EXPECTED_SOURCE_PACKAGE_SHA256 = (
    "09ed122804b11767b8ac7cd69c323c1f6eef511fd6ae7284d75756fb60569bec"
)
EXPECTED_SOURCE_EVIDENCE_SHA256 = (
    "fe3d6297c08dccd8cacba13d50aa44dbb1c94b0ca8c2ab4df3a5c605466fcf94"
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
EXPECTED_INITIAL_DPKG_STATUS_SHA256 = (
    "2c31c35c262b2b2761055fa12a55361f3d47ebfa1923dbb3cc3691499d2ce572"
)
EXPECTED_INSTALLED_DPKG_STATUS_SHA256 = (
    "33c4973d4bcccc1932de35b2b326c61140037ee46613c7a925f2cbdbd3d5cff1"
)
EXPECTED_DPKG_CONFIG_SHA256 = (
    "fead43b89af3ea5691c48f32d7fe1ba0f7ab229fb5d230f612d76fe8e6f5a015"
)
EXPECTED_INITIAL_DPKG_LOG_SHA256 = (
    "ed3a8e3291d6e2991f80983181cac4f67264460ca34513a5b18ba352395ee7d0"
)
EXPECTED_INITIAL_DPKG_LOG_SIZE = 879_640
EXPECTED_OPERATION_IN_PROGRESS_STARTUP = "0:1:3:14:2|error-absent"
EXPECTED_ALLOWED_STARTUP = "0:1:2:2:6|error-absent"
EXPECTED_CONTROL_ROOT = Path(
    f"/var/tmp/radishlex-l6-transaction-state-{EXPECTED_ATTEMPT_ID}"
)

INPUT_ROOT = Path("/var/tmp/radishlex-l6-inputs")
STARTUP_PATH = INPUT_ROOT / "startup.py"
STARTUP_FFI_PATH = (
    INPUT_ROOT
    / "target-startup/usr/lib/aarch64-linux-gnu/radishlex/manager/lib/"
    "libradishlex_ime_ffi.so"
)
OUTPUT_ROOT = Path("/var/tmp/radishlex-l6-crash-install-artifacts-staged-output")
STATE_ROOT = Path("/var/lib/radishlex/install-v1")
RECEIPT_PATH = STATE_ROOT / "receipt.json"
GUARD_PATH = Path("/run/lock/radishlex-install-v1.lock")
TRANSIENT_RUN_ROOT = Path("/run/radishlex-l6-crash-install-artifacts-staged")
DPKG_STATUS_PATH = Path("/var/lib/dpkg/status")
DPKG_CONFIG_PATH = Path("/etc/dpkg/dpkg.cfg")
DPKG_LOG_PATH = Path("/var/log/dpkg.log")
DPKG_UPDATES_PATH = Path("/var/lib/dpkg/updates")
DPKG_INFO_PATH = Path("/var/lib/dpkg/info")
BOOT_ID_PATH = Path("/proc/sys/kernel/random/boot_id")

SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,95}")
HEX_32 = re.compile(r"[0-9a-f]{32}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
MAX_FILE_BYTES = 1024 * 1024
MAX_COMMAND_BYTES = 64 * 1024


class TransactionStateProbeError(ValueError):
    pass


def canonical_json(value: dict[str, object]) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode("utf-8")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            digest.update(block)
    return digest.hexdigest()


def read_bounded(path: Path, limit: int = MAX_FILE_BYTES) -> bytes:
    with path.open("rb") as source:
        value = source.read(limit + 1)
    if not value or len(value) > limit:
        raise TransactionStateProbeError("bounded-file-invalid")
    return value


def read_fields(path: Path) -> dict[str, str]:
    try:
        text = read_bounded(path).decode("utf-8")
    except UnicodeDecodeError as exc:
        raise TransactionStateProbeError("field-file-not-utf8") from exc
    if not text.endswith("\n") or "\r" in text or "\x00" in text:
        raise TransactionStateProbeError("field-file-not-canonical")
    fields: dict[str, str] = {}
    for line in text.splitlines():
        key, separator, value = line.partition("=")
        if not separator or not key or key in fields:
            raise TransactionStateProbeError("field-file-invalid")
        fields[key] = value
    return fields


def require_directory(path: Path, label: str, mode: int) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or metadata.st_uid != 0
        or metadata.st_gid != 0
        or stat.S_IMODE(metadata.st_mode) != mode
    ):
        raise TransactionStateProbeError(f"directory-identity-invalid:{label}")


def require_regular(
    path: Path,
    label: str,
    mode: int,
    *,
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
        raise TransactionStateProbeError(f"file-identity-invalid:{label}")
    if sha256 is not None and sha256_file(path) != sha256:
        raise TransactionStateProbeError(f"file-sha256-invalid:{label}")


def create_new_file(path: Path, payload: bytes) -> None:
    descriptor = os.open(
        path,
        os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
        0o600,
    )
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
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def replace_phase(control_root: Path, phase: str) -> None:
    incoming = control_root / ".phase.incoming.json"
    if incoming.exists() or incoming.is_symlink():
        raise TransactionStateProbeError("phase-incoming-exists")
    create_new_file(
        incoming,
        canonical_json({"format": PHASE_FORMAT, "phase": phase}),
    )
    os.replace(incoming, control_root / "phase.json")
    sync_directory(control_root)


def publish_result(control_root: Path, value: dict[str, object]) -> None:
    incoming = control_root / ".transaction-state.incoming.json"
    final = control_root / "transaction-state.evidence.json"
    create_new_file(incoming, canonical_json(value))
    try:
        os.link(incoming, final, follow_symlinks=False)
    except FileExistsError as exc:
        raise TransactionStateProbeError("transaction-state-evidence-exists") from exc
    os.unlink(incoming)
    require_regular(final, "transaction-state-evidence", 0o600)
    sync_directory(control_root)


def _read_receipt() -> tuple[dict[str, object], bytes]:
    require_directory(STATE_ROOT, "state-root", 0o755)
    require_regular(RECEIPT_PATH, "receipt", 0o644)
    if (STATE_ROOT / "receipt.json.tmp").exists() or (
        STATE_ROOT / "receipt.json.tmp"
    ).is_symlink():
        raise TransactionStateProbeError("receipt-temporary-present")
    payload = read_bounded(RECEIPT_PATH, 64 * 1024)
    try:
        receipt = json.loads(payload)
    except json.JSONDecodeError as exc:
        raise TransactionStateProbeError("receipt-json-invalid") from exc
    if not isinstance(receipt, dict):
        raise TransactionStateProbeError("receipt-json-invalid")
    operation_id = receipt.get("operation_id")
    if (
        not isinstance(operation_id, str)
        or not HEX_32.fullmatch(operation_id)
        or sha256_bytes(operation_id.encode("ascii"))
        != EXPECTED_OPERATION_ID_SHA256
    ):
        raise TransactionStateProbeError("receipt-operation-identity-invalid")
    return receipt, payload


def _require_common_receipt(receipt: dict[str, object]) -> str:
    operation_id = str(receipt["operation_id"])
    required = {
        "receipt_format": "radishlex-linux-install-receipt-v1",
        "product_id": "radishlex-linux",
        "distribution_identity": "debian-local-deb-v1",
        "operation_kind": "install",
        "version_relation": "not_applicable",
        "failure_code": None,
        "failure_after_state": None,
        "manual_recovery_required": False,
        "source_artifact": None,
        "source_proof": None,
    }
    if any(receipt.get(key) != value for key, value in required.items()):
        raise TransactionStateProbeError("receipt-common-semantics-invalid")
    if receipt.get("operation_chain") != [operation_id]:
        raise TransactionStateProbeError("receipt-operation-chain-invalid")
    target = receipt.get("target_artifact")
    if (
        not isinstance(target, dict)
        or target.get("package_version") != "26.7.1+38-1"
        or target.get("package_sha256") != EXPECTED_SOURCE_PACKAGE_SHA256
        or target.get("evidence_sha256") != EXPECTED_SOURCE_EVIDENCE_SHA256
    ):
        raise TransactionStateProbeError("receipt-target-invalid")
    return operation_id


def _require_guard_absent() -> None:
    if GUARD_PATH.exists() or GUARD_PATH.is_symlink():
        raise TransactionStateProbeError("guard-not-absent-after-reboot")


def _run_exact(
    argv: tuple[str, ...], expected_stdout: bytes, label: str
) -> None:
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
        or len(observation.stdout) > MAX_COMMAND_BYTES
        or len(observation.stderr) > MAX_COMMAND_BYTES
        or observation.stdout != expected_stdout
        or observation.stderr
    ):
        raise TransactionStateProbeError(f"readonly-command-invalid:{label}")


def _validate_isolation_and_startup(expected_startup: str) -> None:
    if {path.name for path in Path("/sys/class/net").iterdir()} != {"lo"}:
        raise TransactionStateProbeError("network-interface-drift")
    _run_exact(
        ("/usr/sbin/ip", "-4", "route", "show", "table", "main"),
        b"",
        "ipv4-route",
    )
    _run_exact(
        ("/usr/sbin/ip", "-6", "route", "show", "table", "main"),
        b"",
        "ipv6-route",
    )
    loopback = subprocess.run(
        ("/usr/sbin/ip", "-brief", "address", "show", "lo"),
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=60,
        check=False,
        env={"LC_ALL": "C", "PATH": "/usr/sbin:/usr/bin:/sbin:/bin"},
    )
    fields = loopback.stdout.decode("ascii", errors="strict").split()
    if (
        loopback.returncode != 0
        or loopback.stderr
        or len(fields) != 4
        or fields[0] != "lo"
        or fields[2:] != ["127.0.0.1/8", "::1/128"]
    ):
        raise TransactionStateProbeError("readonly-command-invalid:loopback")
    for path in (
        Path("/home/radishlex-l6/.local/share/radishlex"),
        Path("/home/radishlex-l6/.config/radishlex"),
        Path("/home/radishlex-l6/.local/state/radishlex"),
        Path("/home/radishlex-l6/.cache/radishlex"),
        Path("/home/radishlex-l6/.config/fcitx5/profile"),
    ):
        if path.exists() or path.is_symlink():
            raise TransactionStateProbeError("user-xdg-present")
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
            raise TransactionStateProbeError("product-or-dpkg-process-present")
        if cmdline.startswith(
            b"/usr/lib/aarch64-linux-gnu/radishlex/manager/radishlex_manager"
        ) or any(value in mappings for value in forbidden_mappings):
            raise TransactionStateProbeError("product-process-or-mapping-present")
    for component, path in (
        ("1", "/usr/lib/aarch64-linux-gnu/radishlex/manager/radishlex_manager"),
        ("2", "/usr/lib/aarch64-linux-gnu/fcitx5/radishlex.so"),
    ):
        _run_exact(
            (
                "/usr/bin/python3",
                "-B",
                str(STARTUP_PATH),
                str(STARTUP_FFI_PATH),
                component,
                path,
            ),
            (expected_startup + "\n").encode("ascii"),
            f"startup-{component}",
        )


def _validate_dpkg_common() -> None:
    if sha256_file(DPKG_CONFIG_PATH) != EXPECTED_DPKG_CONFIG_SHA256:
        raise TransactionStateProbeError("dpkg-config-drift")
    if any(DPKG_UPDATES_PATH.iterdir()):
        raise TransactionStateProbeError("dpkg-updates-present")
    _run_exact(("/usr/bin/dpkg", "--audit"), b"", "dpkg-audit")


def _validate_artifacts_staged(
    receipt: dict[str, object], payload: bytes
) -> dict[str, object]:
    operation_id = _require_common_receipt(receipt)
    if (
        receipt.get("state") != "artifacts_staged"
        or sha256_bytes(payload) != EXPECTED_RECEIPT_SHA256
        or len(payload) != EXPECTED_RECEIPT_SIZE
        or receipt.get("target_proof") is not None
    ):
        raise TransactionStateProbeError("artifacts-staged-receipt-invalid")
    target = receipt.get("target_artifact")
    staged = receipt.get("staged_artifacts")
    initial = receipt.get("initial_package")
    if (
        not isinstance(target, dict)
        or not isinstance(staged, list)
        or len(staged) != 1
        or not isinstance(staged[0], dict)
        or staged[0].get("slot") != "target"
        or not isinstance(staged[0].get("artifact"), dict)
        or staged[0]["artifact"].get("package_sha256")
        != EXPECTED_SOURCE_PACKAGE_SHA256
        or staged[0]["artifact"].get("evidence_sha256")
        != EXPECTED_SOURCE_EVIDENCE_SHA256
        or not isinstance(initial, dict)
        or initial.get("state") != "not_installed"
        or initial.get("artifact") is not None
    ):
        raise TransactionStateProbeError("artifacts-staged-evidence-invalid")
    operations = STATE_ROOT / "operations"
    require_directory(operations, "operations-root", 0o755)
    entries = tuple(operations.iterdir())
    if len(entries) != 1 or entries[0].name != operation_id:
        raise TransactionStateProbeError("operation-directory-count-invalid")
    operation_root = entries[0]
    require_directory(operation_root, "operation-root", 0o700)
    if {item.name for item in operation_root.iterdir()} != {
        "target.deb",
        "target.evidence.json",
    }:
        raise TransactionStateProbeError("operation-staging-inventory-invalid")
    require_regular(
        operation_root / "target.deb",
        "staged-target-package",
        0o600,
        size=40_806_592,
        sha256=EXPECTED_SOURCE_PACKAGE_SHA256,
    )
    require_regular(
        operation_root / "target.evidence.json",
        "staged-target-evidence",
        0o600,
        size=2376,
        sha256=EXPECTED_SOURCE_EVIDENCE_SHA256,
    )
    _require_guard_absent()
    if TRANSIENT_RUN_ROOT.exists() or TRANSIENT_RUN_ROOT.is_symlink():
        raise TransactionStateProbeError("transient-secret-root-present")
    _validate_dpkg_common()
    if (
        sha256_file(DPKG_STATUS_PATH) != EXPECTED_INITIAL_DPKG_STATUS_SHA256
        or sha256_file(DPKG_LOG_PATH) != EXPECTED_INITIAL_DPKG_LOG_SHA256
        or DPKG_LOG_PATH.stat().st_size != EXPECTED_INITIAL_DPKG_LOG_SIZE
    ):
        raise TransactionStateProbeError("artifacts-staged-dpkg-identity-invalid")
    status = DPKG_STATUS_PATH.read_text(encoding="utf-8", errors="strict")
    if any(
        paragraph.startswith("Package: radishlex\n")
        for paragraph in status.split("\n\n")
    ) or any(path.name.startswith("radishlex") for path in DPKG_INFO_PATH.iterdir()):
        raise TransactionStateProbeError("artifacts-staged-package-present")
    _validate_isolation_and_startup(EXPECTED_OPERATION_IN_PROGRESS_STARTUP)
    return {
        "dpkg_log_sha256": EXPECTED_INITIAL_DPKG_LOG_SHA256,
        "dpkg_log_size": EXPECTED_INITIAL_DPKG_LOG_SIZE,
        "dpkg_status_sha256": EXPECTED_INITIAL_DPKG_STATUS_SHA256,
        "guard_profile": "absent-after-reboot",
        "package_profile": "not-installed-staging-preserved",
        "receipt_sha256": EXPECTED_RECEIPT_SHA256,
        "receipt_size": EXPECTED_RECEIPT_SIZE,
        "startup_profile": "operation-in-progress",
        "transaction": "artifacts-staged",
    }


def _validate_completed(
    receipt: dict[str, object], payload: bytes
) -> dict[str, object]:
    _require_common_receipt(receipt)
    if receipt.get("state") != "completed":
        raise TransactionStateProbeError("completed-receipt-state-invalid")
    postflight_path = OUTPUT_ROOT / "terminal-postflight.evidence.txt"
    delta_path = OUTPUT_ROOT / "dpkg-delta.txt"
    require_regular(postflight_path, "terminal-postflight", 0o600)
    require_regular(delta_path, "dpkg-delta", 0o600)
    postflight = read_fields(postflight_path)
    required = {
        "format": "radishlex-linux-l6-install-artifacts-staged-terminal-postflight-v1",
        "clone_uuid": EXPECTED_TARGET_UUID,
        "boot_id_sha256": EXPECTED_PRIOR_BOOT_ID_SHA256,
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
        "dpkg_status_sha256": EXPECTED_INSTALLED_DPKG_STATUS_SHA256,
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
    dynamic_fields = {
        "dpkg_delta",
        "dpkg_log_sha256",
        "dpkg_log_size",
        "receipt",
    }
    if set(postflight) != set(required) | dynamic_fields or any(
        postflight.get(key) != value for key, value in required.items()
    ):
        raise TransactionStateProbeError("completed-postflight-semantics-invalid")
    receipt_digest, separator, receipt_size = postflight.get("receipt", "").partition("|")
    if (
        not separator
        or not HEX_64.fullmatch(receipt_digest)
        or not receipt_size.isdigit()
        or receipt_digest != sha256_bytes(payload)
        or int(receipt_size) != len(payload)
    ):
        raise TransactionStateProbeError("completed-receipt-identity-invalid")
    delta_digest, separator, delta_size = postflight.get("dpkg_delta", "").partition("|")
    if (
        not separator
        or not HEX_64.fullmatch(delta_digest)
        or not delta_size.isdigit()
        or delta_digest != sha256_file(delta_path)
        or int(delta_size) != delta_path.stat().st_size
    ):
        raise TransactionStateProbeError("completed-dpkg-delta-identity-invalid")
    log_digest = postflight.get("dpkg_log_sha256", "")
    log_size = postflight.get("dpkg_log_size", "")
    if (
        not HEX_64.fullmatch(log_digest)
        or not log_size.isdigit()
        or sha256_file(DPKG_LOG_PATH) != log_digest
        or DPKG_LOG_PATH.stat().st_size != int(log_size)
    ):
        raise TransactionStateProbeError("completed-dpkg-log-identity-invalid")
    _require_guard_absent()
    if TRANSIENT_RUN_ROOT.exists() or TRANSIENT_RUN_ROOT.is_symlink():
        raise TransactionStateProbeError("transient-secret-root-present")
    _validate_dpkg_common()
    if sha256_file(DPKG_STATUS_PATH) != EXPECTED_INSTALLED_DPKG_STATUS_SHA256:
        raise TransactionStateProbeError("completed-dpkg-status-invalid")
    _run_exact(
        ("/usr/bin/dpkg", "--verify", "radishlex"),
        b"",
        "dpkg-verify-radishlex",
    )
    status = DPKG_STATUS_PATH.read_text(encoding="utf-8", errors="strict")
    expected_lines = {
        "Package: radishlex",
        "Status: install ok installed",
        "Architecture: arm64",
        "Version: 26.7.1+38-1",
    }
    paragraphs = [
        set(paragraph.splitlines())
        for paragraph in status.split("\n\n")
        if paragraph.startswith("Package: radishlex\n")
    ]
    if len(paragraphs) != 1 or not expected_lines.issubset(paragraphs[0]):
        raise TransactionStateProbeError("completed-package-status-invalid")
    _validate_isolation_and_startup(EXPECTED_ALLOWED_STARTUP)
    return {
        "dpkg_log_sha256": log_digest,
        "dpkg_log_size": int(log_size),
        "dpkg_status_sha256": EXPECTED_INSTALLED_DPKG_STATUS_SHA256,
        "guard_profile": "absent-after-reboot",
        "package_profile": "installed-verified",
        "receipt_sha256": receipt_digest,
        "receipt_size": int(receipt_size),
        "startup_profile": "allowed",
        "transaction": "completed",
    }


def observe_transaction_state() -> dict[str, object]:
    raw_boot = BOOT_ID_PATH.read_text(encoding="ascii").strip()
    if not raw_boot:
        raise TransactionStateProbeError("current-boot-id-invalid")
    boot_id_sha256 = sha256_bytes(raw_boot.encode("ascii"))
    receipt, payload = _read_receipt()
    state = receipt.get("state")
    if state == "completed":
        evidence = _validate_completed(receipt, payload)
    elif state == "artifacts_staged":
        evidence = _validate_artifacts_staged(receipt, payload)
    else:
        raise TransactionStateProbeError("receipt-state-not-classifiable")
    return {"boot_id_sha256": boot_id_sha256, **evidence}


def marker_bytes(attempt_id: str, probe_sha256: str) -> bytes:
    return canonical_json(
        {
            "attempt_id": attempt_id,
            "format": MARKER_FORMAT,
            "probe_sha256": probe_sha256,
        }
    )


def result_value(
    *,
    attempt_id: str,
    target_uuid: str,
    probe_sha256: str,
    observation: dict[str, object] | None,
    reason: str,
) -> dict[str, object]:
    transaction = (
        str(observation["transaction"])
        if observation is not None
        else "state-indeterminate"
    )
    return {
        "attempt_id": attempt_id,
        "automatic_cleanup": "not-performed",
        "automatic_retry": "not-performed",
        "boot_id_sha256": (
            observation.get("boot_id_sha256") if observation else None
        ),
        "dpkg_command_profile": (
            "audit-plus-verify-read-only"
            if transaction == "completed"
            else (
                "audit-read-only"
                if transaction == "artifacts-staged"
                else "not-classified"
            )
        ),
        "dpkg_log_sha256": (
            observation.get("dpkg_log_sha256") if observation else None
        ),
        "dpkg_log_size": (
            observation.get("dpkg_log_size") if observation else None
        ),
        "dpkg_mutation": "not-performed",
        "dpkg_status_sha256": (
            observation.get("dpkg_status_sha256") if observation else None
        ),
        "format": EVIDENCE_FORMAT,
        "guard_profile": (
            observation.get("guard_profile") if observation else "not-classified"
        ),
        "maintenance_invocations": 0,
        "operation_id": "hash-only",
        "operation_id_sha256": EXPECTED_OPERATION_ID_SHA256,
        "outcome": transaction,
        "package_profile": (
            observation.get("package_profile")
            if observation
            else "not-classified"
        ),
        "probe_sha256": probe_sha256,
        "product_state_write": "not-performed",
        "reason": reason,
        "receipt_sha256": (
            observation.get("receipt_sha256") if observation else None
        ),
        "receipt_size": (
            observation.get("receipt_size") if observation else None
        ),
        "resume_invocations": 0,
        "startup_profile": (
            observation.get("startup_profile")
            if observation
            else "not-classified"
        ),
        "target_uuid": target_uuid,
        "transaction": transaction,
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--expected-operation-id-sha256", required=True)
    parser.add_argument("--expected-probe-sha256", required=True)
    parser.add_argument("--control-root", type=Path, required=True)
    return parser.parse_args(argv)


def validate_argument_contract(args: argparse.Namespace) -> None:
    if (
        args.attempt_id != EXPECTED_ATTEMPT_ID
        or args.target_uuid != EXPECTED_TARGET_UUID
        or args.expected_operation_id_sha256 != EXPECTED_OPERATION_ID_SHA256
        or not SAFE_ATTEMPT_ID.fullmatch(args.attempt_id)
        or not HEX_64.fullmatch(args.expected_probe_sha256)
        or args.control_root != EXPECTED_CONTROL_ROOT
    ):
        raise TransactionStateProbeError("argument-contract-mismatch")


def sanitize_reason(value: str) -> str:
    return re.sub(
        r"(?<![0-9a-f])[0-9a-f]{32}(?![0-9a-f])",
        "[redacted]",
        value,
    )


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    control_root_validated = False
    result: dict[str, object]
    try:
        if os.geteuid() != 0:
            raise TransactionStateProbeError("root-required")
        validate_argument_contract(args)
        require_directory(args.control_root, "control-root", 0o700)
        control_root_validated = True
        probe_path = Path(__file__).absolute()
        require_regular(probe_path, "transaction-state-probe", 0o600)
        if sha256_file(probe_path) != args.expected_probe_sha256:
            raise TransactionStateProbeError("probe-identity-invalid")
        marker = args.control_root / "attempt.marker.json"
        create_new_file(
            marker,
            marker_bytes(args.attempt_id, args.expected_probe_sha256),
        )
        replace_phase(args.control_root, "observing")
        observation = observe_transaction_state()
        result = result_value(
            attempt_id=args.attempt_id,
            target_uuid=args.target_uuid,
            probe_sha256=args.expected_probe_sha256,
            observation=observation,
            reason="read-only-transaction-state-observed",
        )
        replace_phase(args.control_root, str(observation["transaction"]))
    except (OSError, subprocess.SubprocessError, TransactionStateProbeError) as exc:
        result = result_value(
            attempt_id=getattr(args, "attempt_id", EXPECTED_ATTEMPT_ID),
            target_uuid=getattr(args, "target_uuid", EXPECTED_TARGET_UUID),
            probe_sha256=getattr(args, "expected_probe_sha256", "0" * 64),
            observation=None,
            reason=sanitize_reason(f"{type(exc).__name__}:{exc}"),
        )
        if control_root_validated:
            try:
                replace_phase(args.control_root, "state-indeterminate")
            except (OSError, TransactionStateProbeError):
                pass
    if not control_root_validated:
        print(str(result["reason"]), file=sys.stderr)
        return 12
    try:
        publish_result(args.control_root, result)
    except (OSError, TransactionStateProbeError) as exc:
        print(
            "transaction state result publish failed: "
            + sanitize_reason(str(exc)),
            file=sys.stderr,
        )
        return 12
    if result["outcome"] == "state-indeterminate":
        print(str(result["reason"]), file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

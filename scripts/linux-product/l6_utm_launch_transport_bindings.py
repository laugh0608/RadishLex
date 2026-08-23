#!/usr/bin/env python3
from __future__ import annotations

import base64
import hashlib
import json
import plistlib
import re
import stat
import xml.etree.ElementTree as element_tree
from pathlib import Path
from typing import Protocol

import l6_utm_launch_prepared_bindings as prepared_bindings
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = "radishlex-linux-l6-utm-launch-transport-v2"
TRANSPORT_ID = "foreground-applescript-v1"
CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_launch_transport_v2.py"
)
BINDING_CONTROL_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_launch_transport_bindings.py"
)
PREPARED_BINDING_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_launch_prepared_bindings.py"
)
TRANSPORT_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_utm_launch_transport_v2.applescript"
)
TRANSPORT_SOURCE = (
    'on run arguments\n'
    '    if (count of arguments) is not 1 then error '
    '"expected-one-target-uuid"\n'
    '    set targetIdentifier to item 1 of arguments\n'
    '    tell application id "com.utmapp.UTM"\n'
    '        activate\n'
    '        set targetMachine to virtual machine id targetIdentifier\n'
    '        start targetMachine saving true recovery false\n'
    '    end tell\n'
    'end run\n'
).encode("utf-8")
UTM_BUNDLE_ROOT = Path("/Applications/UTM.app")
EXPECTED_UTM_BUNDLE_ID = "com.utmapp.UTM"
EXPECTED_UTM_VERSION = "4.7.5"
EXPECTED_UTM_BUILD = "118"
V7_EVIDENCE_FORMAT = "radishlex-linux-l6-utm-launch-diagnostics-v7"
REQUIRED_V7_MANIFEST_SHA256 = (
    "206aa33511c0fef3e5d21aa43c9f5392ccade6ceb43319275db3ca617607b56c"
)
FROZEN_V7_TARGET_UUID = "5B19AEF1-0F29-40B6-8F24-1B1929117DAB"
FROZEN_V7_TARGET_NAME = (
    "RadishLex-Debian13-ARM64-L6-d75818f-"
    "crash-install-artifacts-staged-v3"
)
V7_LOG_START = "2026-08-22 08:12:00+0000"
V7_LOG_END = "2026-08-22 08:18:00+0000"
PREPARED_EVIDENCE_FORMAT = prepared_bindings.PREPARED_EVIDENCE_FORMAT
PREPARED_PREFLIGHT_FORMAT = prepared_bindings.PREPARED_PREFLIGHT_FORMAT
REQUIRED_PREPARED_MANIFEST_SHA256 = (
    prepared_bindings.REQUIRED_PREPARED_MANIFEST_SHA256
)
REQUIRED_CLONE_MANIFEST_SHA256 = (
    prepared_bindings.REQUIRED_CLONE_MANIFEST_SHA256
)
REQUIRED_PREPARED_REPOSITORY_HEAD = (
    prepared_bindings.REQUIRED_PREPARED_REPOSITORY_HEAD
)
REQUIRED_TARGET_UUID = prepared_bindings.REQUIRED_TARGET_UUID
REQUIRED_TARGET_NAME = prepared_bindings.REQUIRED_TARGET_NAME
TARGET_DISK_IDENTIFIER = prepared_bindings.TARGET_DISK_IDENTIFIER
TARGET_DISK_IMAGE_NAME = prepared_bindings.TARGET_DISK_IMAGE_NAME
HEX_64 = re.compile(r"[0-9a-f]{64}")


class BindingError(ValueError):
    pass


class LaunchTransportBindingRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    prior_v7_root: Path
    prior_v7_manifest_sha256: str
    prior_prepared_root: Path
    prior_prepared_manifest_sha256: str
    target_package_path: Path
    target_uuid: str
    target_name: str
    expected_vm_count: int


def validate_launch_bindings(
    request: LaunchTransportBindingRequest,
) -> dict[str, object]:
    try:
        repository_head = start_control._run_git(
            request.repository_root, ("rev-parse", "HEAD")
        ).decode("ascii").strip()
        repository_status = start_control._run_git(
            request.repository_root, ("status", "--porcelain")
        )
    except start_control.StartControlError as exc:
        raise BindingError(str(exc)) from exc
    if repository_head != request.expected_repository_head:
        raise BindingError("repository-head-drift")
    if repository_status:
        raise BindingError("repository-not-clean")
    control_identity = validate_control_identity(request.repository_root)
    v7_identity = validate_v7_evidence(request)
    prepared_identity = validate_prepared_evidence(request, v7_identity)
    utm_identity = validate_utm_bundle(UTM_BUNDLE_ROOT)
    return {
        **control_identity,
        "expected_current_vm_count": request.expected_vm_count,
        "format": EVIDENCE_FORMAT,
        "repository_clean": True,
        "repository_head": repository_head,
        **prepared_identity,
        **utm_identity,
        **v7_identity,
    }


def validate_control_identity(repository_root: Path) -> dict[str, object]:
    control_path = repository_root / CONTROL_RELATIVE_PATH
    binding_path = repository_root / BINDING_CONTROL_RELATIVE_PATH
    prepared_binding_path = repository_root / PREPARED_BINDING_RELATIVE_PATH
    transport_path = repository_root / TRANSPORT_RELATIVE_PATH
    for path, label in (
        (control_path, "executed-control"),
        (binding_path, "binding-control"),
        (prepared_binding_path, "prepared-binding-control"),
        (transport_path, "transport-source"),
    ):
        try:
            item_stat = path.lstat()
        except OSError as exc:
            raise BindingError(f"{label}-unavailable") from exc
        if (
            not stat.S_ISREG(item_stat.st_mode)
            or stat.S_ISLNK(item_stat.st_mode)
            or item_stat.st_nlink != 1
            or stat.S_IMODE(item_stat.st_mode) & 0o022
        ):
            raise BindingError(f"{label}-identity-invalid")
    if Path(__file__).absolute() != binding_path:
        raise BindingError("executed-binding-control-path-mismatch")
    if Path(prepared_bindings.__file__).absolute() != prepared_binding_path:
        raise BindingError("executed-prepared-binding-control-path-mismatch")
    try:
        transport_source = transport_path.read_bytes()
    except OSError as exc:
        raise BindingError("transport-source-unreadable") from exc
    if transport_source != TRANSPORT_SOURCE:
        raise BindingError("transport-source-contract-drift")
    return {
        "binding_control_sha256": _sha256_file(binding_path),
        "control_sha256": _sha256_file(control_path),
        "prepared_binding_control_sha256": _sha256_file(
            prepared_binding_path
        ),
        "transport_id": TRANSPORT_ID,
        "transport_sha256": hashlib.sha256(transport_source).hexdigest(),
    }


def validate_v7_evidence(
    request: LaunchTransportBindingRequest,
) -> dict[str, object]:
    entries = _verify_bound_manifest(
        request.prior_v7_root,
        request.prior_v7_manifest_sha256,
        "prior-v7",
    )
    expected_entries = frozenset(
        (
            "binding-preflight.json",
            "host-process-command.json",
            "host-processes.json",
            "request.json",
            "terminal.json",
            "unified-log-command.json",
            "unified-log-structure.json",
            "unified-log.json",
            "utmctl-list-live.json",
            "utmctl-status-live.json",
        )
    )
    if entries != expected_entries:
        raise BindingError("prior-v7-manifest-entry-set-invalid")

    prior_request = _read_json_object(
        request.prior_v7_root / "request.json", "prior-v7-request"
    )
    _require_json_fields(
        prior_request,
        {
            "authorization": {
                "host_launch_diagnostics": True,
                "read_system_log": True,
            },
            "expected_vm_count": 20,
            "format": V7_EVIDENCE_FORMAT,
            "log_end": V7_LOG_END,
            "log_start": V7_LOG_START,
            "target_name": FROZEN_V7_TARGET_NAME,
            "target_uuid": FROZEN_V7_TARGET_UUID,
        },
        "prior-v7-request",
    )
    if request.expected_vm_count != prior_request["expected_vm_count"] + 1:
        raise BindingError("new-target-count-not-v7-plus-one")

    terminal = _read_json_object(
        request.prior_v7_root / "terminal.json", "prior-v7-terminal"
    )
    _require_json_fields(
        terminal,
        {
            "automatic_delete": "not-performed",
            "automatic_retry": "not-performed",
            "format": V7_EVIDENCE_FORMAT,
            "guest_exec": "not-performed",
            "input_transfer": "not-performed",
            "operation_id": "not-generated",
            "outcome": "diagnostics-collected",
            "reason": "read-only-host-facts-collected",
            "root_cause": "unattributed",
            "target_name": FROZEN_V7_TARGET_NAME,
            "target_uuid": FROZEN_V7_TARGET_UUID,
            "transaction": "not-performed",
            "utm_clone": "not-performed",
            "utm_start": "not-performed",
            "utm_stop": "not-performed",
        },
        "prior-v7-terminal",
    )

    list_value = _read_json_object(
        request.prior_v7_root / "utmctl-list-live.json",
        "prior-v7-list",
    )
    baseline_inventory = start_control.parse_utmctl_list(
        _command_observation_from_json(list_value, "prior-v7-list")
    )
    if (
        len(baseline_inventory) != 20
        or any(item.status != "stopped" for item in baseline_inventory)
        or not any(
            item.uuid == FROZEN_V7_TARGET_UUID
            and item.name == FROZEN_V7_TARGET_NAME
            for item in baseline_inventory
        )
        or any(item.uuid == request.target_uuid for item in baseline_inventory)
        or any(item.name == request.target_name for item in baseline_inventory)
    ):
        raise BindingError("prior-v7-baseline-inventory-invalid")

    host_processes = _read_json_object(
        request.prior_v7_root / "host-processes.json",
        "prior-v7-host-processes",
    )
    _require_json_fields(
        host_processes,
        {
            "format": V7_EVIDENCE_FORMAT,
            "relevant_process_count": 0,
            "relevant_processes": [],
        },
        "prior-v7-host-processes",
    )
    structure = _read_json_object(
        request.prior_v7_root / "unified-log-structure.json",
        "prior-v7-log-structure",
    )
    _require_json_fields(
        structure,
        {
            "finished_marker_count": 1,
            "finished_marker_is_terminal": True,
            "finished_value_kind": "integer-one",
            "format": V7_EVIDENCE_FORMAT,
            "record_count": 4041,
        },
        "prior-v7-log-structure",
    )
    log_value = _read_json_object(
        request.prior_v7_root / "unified-log.json", "prior-v7-log"
    )
    _require_json_fields(
        log_value,
        {
            "event_count": 4040,
            "format": V7_EVIDENCE_FORMAT,
            "log_end": V7_LOG_END,
            "log_start": V7_LOG_START,
        },
        "prior-v7-log",
    )
    events = log_value.get("events")
    if not isinstance(events, list) or len(events) != 4040:
        raise BindingError("prior-v7-log-events-invalid")
    prefixes = [
        item.get("message_prefix")
        for item in events
        if isinstance(item, dict)
        and isinstance(item.get("message_prefix"), str)
    ]
    for marker in (
        "AESendMessage(UTMv,star",
        "RECEIVED:(UTMv,star)",
        "Invalid parameter not satisfying: [self canBecomeMainWindow]",
        "FAULT: NSInternalInconsistencyException",
    ):
        if sum(value.startswith(marker) for value in prefixes) != 1:
            raise BindingError("prior-v7-failure-marker-invalid")
    if any(
        isinstance(item, dict) and item.get("role") == "qemu"
        for item in events
    ):
        raise BindingError("prior-v7-qemu-event-unexpected")

    return {
        "baseline_inventory": [
            {"name": item.name, "status": item.status, "uuid": item.uuid}
            for item in baseline_inventory
        ],
        "prior_v7_entries_verified": len(entries),
        "prior_v7_event_count": 4040,
        "prior_v7_manifest_sha256": request.prior_v7_manifest_sha256,
        "prior_v7_outcome": "diagnostics-collected",
        "prior_v7_root_cause": "unattributed",
    }


def validate_prepared_evidence(
    request: LaunchTransportBindingRequest,
    v7_identity: dict[str, object],
) -> dict[str, object]:
    try:
        return prepared_bindings.validate_prepared_evidence(
            request, v7_identity
        )
    except prepared_bindings.PreparedBindingError as exc:
        raise BindingError(str(exc)) from exc


def validate_utm_bundle(bundle_root: Path) -> dict[str, object]:
    try:
        bundle_stat = bundle_root.lstat()
    except OSError as exc:
        raise BindingError("utm-bundle-unavailable") from exc
    if not stat.S_ISDIR(bundle_stat.st_mode) or stat.S_ISLNK(bundle_stat.st_mode):
        raise BindingError("utm-bundle-identity-invalid")
    info_path = bundle_root / "Contents/Info.plist"
    sdef_path = bundle_root / "Contents/Resources/UTM.sdef"
    intents_path = (
        bundle_root
        / "Contents/Resources/Metadata.appintents/extract.actionsdata"
    )
    for path, label in (
        (info_path, "utm-info-plist"),
        (sdef_path, "utm-sdef"),
        (intents_path, "utm-app-intents"),
    ):
        try:
            item_stat = path.lstat()
        except OSError as exc:
            raise BindingError(f"{label}-unavailable") from exc
        if (
            not stat.S_ISREG(item_stat.st_mode)
            or stat.S_ISLNK(item_stat.st_mode)
            or item_stat.st_nlink != 1
            or stat.S_IMODE(item_stat.st_mode) & 0o022
        ):
            raise BindingError(f"{label}-identity-invalid")
    try:
        with info_path.open("rb") as source:
            info = plistlib.load(source)
    except (OSError, plistlib.InvalidFileException) as exc:
        raise BindingError("utm-info-plist-invalid") from exc
    expected_info = {
        "CFBundleIdentifier": EXPECTED_UTM_BUNDLE_ID,
        "CFBundleShortVersionString": EXPECTED_UTM_VERSION,
        "CFBundleVersion": EXPECTED_UTM_BUILD,
    }
    for key, expected in expected_info.items():
        if info.get(key) != expected:
            raise BindingError(f"utm-info-{key}-mismatch")
    try:
        sdef_root = element_tree.parse(sdef_path).getroot()
    except (OSError, element_tree.ParseError) as exc:
        raise BindingError("utm-sdef-invalid") from exc
    start_commands = [
        item
        for item in sdef_root.iter("command")
        if item.get("name") == "start" and item.get("code") == "UTMvstar"
    ]
    if len(start_commands) != 1:
        raise BindingError("utm-sdef-start-command-invalid")
    parameters = {
        item.get("name"): (item.get("type"), item.get("optional"))
        for item in start_commands[0].findall("parameter")
    }
    if parameters != {
        "recovery": ("boolean", "yes"),
        "saving": ("boolean", "yes"),
    }:
        raise BindingError("utm-sdef-start-parameters-invalid")
    try:
        intents = json.loads(intents_path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise BindingError("utm-app-intents-invalid") from exc
    try:
        start_intent = intents["actions"]["UTMStartActionIntent"]
        parameter_names = [item["name"] for item in start_intent["parameters"]]
    except (KeyError, TypeError) as exc:
        raise BindingError("utm-start-intent-invalid") from exc
    if (
        start_intent.get("identifier") != "UTMStartActionIntent"
        or start_intent.get("openAppWhenRun") is not False
        or start_intent.get("supportedModes") != 1
        or parameter_names != ["vmEntity", "isRecovery", "isDisposible"]
    ):
        raise BindingError("utm-start-intent-contract-drift")
    return {
        "utm_app_intents_sha256": _sha256_file(intents_path),
        "utm_build": EXPECTED_UTM_BUILD,
        "utm_bundle_id": EXPECTED_UTM_BUNDLE_ID,
        "utm_info_plist_sha256": _sha256_file(info_path),
        "utm_sdef_sha256": _sha256_file(sdef_path),
        "utm_version": EXPECTED_UTM_VERSION,
    }


def expected_target_package_path(target_name: str) -> Path:
    try:
        return prepared_bindings.expected_target_package_path(target_name)
    except prepared_bindings.PreparedBindingError as exc:
        raise BindingError(str(exc)) from exc


def validate_live_target_identity(
    request: LaunchTransportBindingRequest,
    prepared_binding: dict[str, object],
) -> dict[str, object]:
    try:
        return prepared_bindings.validate_live_target_identity(
            request, prepared_binding
        )
    except prepared_bindings.PreparedBindingError as exc:
        raise BindingError(str(exc)) from exc


def _command_observation_from_json(
    value: dict[str, object], label: str
) -> start_control.CommandObservation:
    argv = value.get("argv")
    exit_code = value.get("exit_code")
    timed_out = value.get("timed_out")
    if (
        not isinstance(argv, list)
        or not all(isinstance(item, str) for item in argv)
        or not (exit_code is None or isinstance(exit_code, int))
        or not isinstance(timed_out, bool)
    ):
        raise BindingError(f"{label}-command-invalid")
    stdout = _captured_output_from_json(value.get("stdout"), f"{label}-stdout")
    stderr = _captured_output_from_json(value.get("stderr"), f"{label}-stderr")
    return start_control.CommandObservation(
        tuple(argv), exit_code, timed_out, stdout, stderr
    )


def _captured_output_from_json(
    value: object, label: str
) -> start_control.CapturedOutput:
    if not isinstance(value, dict):
        raise BindingError(f"{label}-invalid")
    prefix_base64 = value.get("prefix_base64")
    total_bytes = value.get("total_bytes")
    sha256 = value.get("sha256")
    truncated = value.get("truncated")
    if (
        not isinstance(prefix_base64, str)
        or not isinstance(total_bytes, int)
        or total_bytes < 0
        or not isinstance(sha256, str)
        or not HEX_64.fullmatch(sha256)
        or not isinstance(truncated, bool)
    ):
        raise BindingError(f"{label}-invalid")
    try:
        prefix = base64.b64decode(prefix_base64, validate=True)
    except ValueError as exc:
        raise BindingError(f"{label}-base64-invalid") from exc
    if (
        len(prefix) > total_bytes
        or (not truncated and len(prefix) != total_bytes)
        or (not truncated and hashlib.sha256(prefix).hexdigest() != sha256)
    ):
        raise BindingError(f"{label}-capture-invalid")
    return start_control.CapturedOutput(prefix, total_bytes, sha256, truncated)


def _verify_bound_manifest(
    root: Path, manifest_sha256: str, label: str
) -> frozenset[str]:
    manifest = root / "files.sha256"
    try:
        root_stat = root.lstat()
        manifest_stat = manifest.lstat()
    except OSError as exc:
        raise BindingError(f"{label}-evidence-unavailable") from exc
    if (
        not stat.S_ISDIR(root_stat.st_mode)
        or stat.S_ISLNK(root_stat.st_mode)
        or stat.S_IMODE(root_stat.st_mode) != 0o700
    ):
        raise BindingError(f"{label}-root-identity-invalid")
    if (
        not stat.S_ISREG(manifest_stat.st_mode)
        or stat.S_ISLNK(manifest_stat.st_mode)
        or stat.S_IMODE(manifest_stat.st_mode) != 0o600
        or manifest_stat.st_uid != root_stat.st_uid
        or manifest_stat.st_nlink != 1
    ):
        raise BindingError(f"{label}-manifest-identity-invalid")
    if _sha256_file(manifest) != manifest_sha256:
        raise BindingError(f"{label}-manifest-drift")
    try:
        lines = manifest.read_text(encoding="ascii").splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        raise BindingError(f"{label}-manifest-unreadable") from exc
    if not lines:
        raise BindingError(f"{label}-manifest-empty")
    entries: set[str] = set()
    for line in lines:
        if len(line) < 67 or line[64:66] != "  ":
            raise BindingError(f"{label}-manifest-line-invalid")
        expected_hash = line[:64]
        name = line[66:]
        if (
            not HEX_64.fullmatch(expected_hash)
            or not name
            or name.startswith("/")
            or "\\" in name
            or "\x00" in name
            or any(part in ("", ".", "..") for part in name.split("/"))
            or name in entries
        ):
            raise BindingError(f"{label}-manifest-entry-invalid")
        path = root / name
        try:
            item_stat = path.lstat()
        except OSError as exc:
            raise BindingError(f"{label}-entry-unavailable") from exc
        if (
            not stat.S_ISREG(item_stat.st_mode)
            or stat.S_ISLNK(item_stat.st_mode)
            or stat.S_IMODE(item_stat.st_mode) != 0o600
            or item_stat.st_uid != root_stat.st_uid
            or item_stat.st_nlink != 1
            or _sha256_file(path) != expected_hash
        ):
            raise BindingError(f"{label}-entry-identity-drift")
        entries.add(name)
    return frozenset(entries)


def _read_json_object(path: Path, label: str) -> dict[str, object]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise BindingError(f"{label}-invalid") from exc
    if not isinstance(value, dict):
        raise BindingError(f"{label}-not-object")
    return value


def _require_json_fields(
    value: dict[str, object],
    expected: dict[str, object],
    label: str,
) -> None:
    for key, expected_value in expected.items():
        if value.get(key) != expected_value:
            raise BindingError(f"{label}-{key.replace('_', '-')}")


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            while chunk := source.read(1024 * 1024):
                digest.update(chunk)
    except OSError as exc:
        raise BindingError(f"sha256-file-unreadable:{path.name}") from exc
    return digest.hexdigest()

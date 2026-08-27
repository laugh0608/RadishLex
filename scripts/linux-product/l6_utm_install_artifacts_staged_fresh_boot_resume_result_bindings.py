#!/usr/bin/env python3
from __future__ import annotations

import base64
import hashlib
import re
import stat
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_fresh_boot_recovery_result_resolution_bindings as result_bindings
import l6_utm_install_artifacts_staged_fresh_boot_resume as control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_install_artifacts_staged_fresh_boot_resume_driver as guest_driver


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "fresh-boot-resume-result-binding-v1"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_fresh_boot_resume_result_bindings.py"
)
REQUIRED_PRIOR_MANIFEST_SHA256 = (
    "ee1879e7bcb8950ac6bd83630de4102aa87522a47cc2c64119d3d88b03420798"
)
REQUIRED_PRIOR_ATTEMPT_ID = control.REQUIRED_ATTEMPT_ID
REQUIRED_PRIOR_REPOSITORY_HEAD = "fb53cc672f393d23f7c945a70e6e30b166aa90cd"
REQUIRED_TERMINAL_CASE_SHA256 = (
    "5216dd2c14464df40bcc19a3f62c46733dfb80b8c1e50c349d0308ef0822b563"
)
REQUIRED_REASON = "ResumeDriverError:terminal-postflight-semantics-invalid"
HEX_40 = re.compile(r"[0-9a-f]{40}")
REQUIRED_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "target-handle-pid-discovery.json",
    "host-process-target-handle-pid-discovery.json",
    "target-handle-pid-confirmation-001.json",
    "host-process-target-handle-pid-confirmation-001.json",
    "target-handle-pid-confirmation-002.json",
    "host-process-target-handle-pid-confirmation-002.json",
    "target-handle-pid-confirmation-003.json",
    "host-process-target-handle-pid-confirmation-003.json",
    "guest-agent-readiness-001.json",
    "guest-agent-ready.json",
    "target-files-agent-ready.json",
    "target-handle-agent-ready.json",
    "host-process-target-handle-agent-ready.json",
    "guest-fresh-boot-resume-root-create.json",
    "guest-frozen-resume-driver-push.json",
    "guest-frozen-resume-driver-chown.json",
    "guest-frozen-resume-driver-chmod.json",
    "guest-frozen-resume-driver-publish.json",
    "guest-frozen-resume-driver-readback.json",
    "guest-frozen-recovery-probe-push.json",
    "guest-frozen-recovery-probe-chown.json",
    "guest-frozen-recovery-probe-chmod.json",
    "guest-frozen-recovery-probe-publish.json",
    "guest-frozen-recovery-probe-readback.json",
    "guest-fresh-boot-resume-driver-push.json",
    "guest-fresh-boot-resume-driver-chown.json",
    "guest-fresh-boot-resume-driver-chmod.json",
    "guest-fresh-boot-resume-driver-publish.json",
    "guest-fresh-boot-resume-driver-readback.json",
    "target-files-pre-resume.json",
    "target-handle-pre-resume.json",
    "host-process-target-handle-pre-resume.json",
    "guest-fresh-boot-resume-once.json",
    "guest-fresh-boot-resume-marker-readback.json",
    "guest-fresh-boot-resume-result-readback-1.json",
    "guest-fresh-boot-resume-result-readback-2.json",
    "guest-fresh-boot-resume-result.json",
    "guest-fresh-boot-resume-phase-readback.json",
    "target-files-postflight.json",
    "target-handle-terminal.json",
    "host-process-target-handle-terminal.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class FreshBootResumeResultBindingRequest(Protocol):
    repository_root: Path
    expected_repository_head: str
    target_uuid: str
    target_name: str
    target_package_path: Path
    source_bundle_sha256: str
    source_bundle_size: int
    prior_fresh_boot_resume_root: Path
    prior_fresh_boot_resume_manifest_sha256: str
    prior_fresh_boot_resume_attempt_id: str


@dataclass(frozen=True)
class FreshBootResumeResultBinding:
    evidence: dict[str, object]
    upstream: control.FreshBootResumeBinding
    prior_repository_head: str
    current_boot_id_sha256: str
    prior_boot_id_sha256: str
    prior_backend_pid: int
    guest_resume_outcome: str
    maintenance_resume_invocations: int
    postflight_invocations: int


def validate_fresh_boot_resume_result_bindings(
    request: FreshBootResumeResultBindingRequest,
) -> FreshBootResumeResultBinding:
    if (
        request.prior_fresh_boot_resume_attempt_id != REQUIRED_PRIOR_ATTEMPT_ID
        or request.prior_fresh_boot_resume_manifest_sha256
        != REQUIRED_PRIOR_MANIFEST_SHA256
    ):
        raise ValueError("required-prior-fresh-boot-resume-mismatch")

    upstream = control.validate_fresh_boot_resume_bindings(request)
    binding_path = request.repository_root / BINDINGS_RELATIVE_PATH
    network_ready._require_committed_regular(
        binding_path, "fresh_boot_resume_result_bindings"
    )

    root = request.prior_fresh_boot_resume_root
    manifest = root / "files.sha256"
    if network_ready._sha256_file(manifest) != REQUIRED_PRIOR_MANIFEST_SHA256:
        raise ValueError("prior-fresh-boot-resume-manifest-invalid")
    entries = start_control._verify_sha256_manifest(root, manifest)
    if (
        entries != len(REQUIRED_ENTRY_NAMES)
        or runtime_bindings._manifest_names(manifest) != REQUIRED_ENTRY_NAMES
    ):
        raise ValueError("prior-fresh-boot-resume-entry-set-invalid")
    _require_private_evidence_tree(root, manifest)

    prior_repository_head = _validate_request_and_binding(request, upstream, root)
    result_bindings._validate_source_target(request, root)
    handle_signature = _validate_host_identity(request, upstream, root)
    _validate_readiness(request, root)
    _validate_delivery(request, upstream, root)
    guest_terminal = _validate_execution(request, upstream, root)
    _validate_terminal(request, upstream, root, guest_terminal)
    _validate_command_inventory(request, upstream, root)
    runtime_control._require_no_raw_operation_id(root)

    return FreshBootResumeResultBinding(
        evidence={
            "current_boot_id_sha256": upstream.current_boot_id_sha256,
            "dpkg_mutation_executed": "unknown",
            "format": EVIDENCE_FORMAT,
            "fresh_boot_resume_result_bindings_sha256": (
                network_ready._sha256_file(binding_path)
            ),
            "guest_resume_outcome": "state-indeterminate",
            "maintenance_resume_invocations": 1,
            "postflight_invocations": 1,
            "prior_boot_id_sha256": upstream.prior_boot_id_sha256,
            "prior_fresh_boot_resume_attempt_id": (
                request.prior_fresh_boot_resume_attempt_id
            ),
            "prior_fresh_boot_resume_backend_pid": upstream.prior_backend_pid,
            "prior_fresh_boot_resume_entries_verified": entries,
            "prior_fresh_boot_resume_handle_sha256": handle_signature[0],
            "prior_fresh_boot_resume_handle_size": handle_signature[1],
            "prior_fresh_boot_resume_manifest_sha256": (
                request.prior_fresh_boot_resume_manifest_sha256
            ),
            "prior_fresh_boot_resume_repository_head": prior_repository_head,
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "result_authority": "stable-double-terminal-plus-indeterminate-phase",
            "target_uuid": request.target_uuid,
            "transaction": "state-indeterminate",
        },
        upstream=upstream,
        prior_repository_head=prior_repository_head,
        current_boot_id_sha256=upstream.current_boot_id_sha256,
        prior_boot_id_sha256=upstream.prior_boot_id_sha256,
        prior_backend_pid=upstream.prior_backend_pid,
        guest_resume_outcome="state-indeterminate",
        maintenance_resume_invocations=1,
        postflight_invocations=1,
    )


def _require_private_evidence_tree(root: Path, manifest: Path) -> None:
    root_info = root.lstat()
    if (
        not stat.S_ISDIR(root_info.st_mode)
        or stat.S_ISLNK(root_info.st_mode)
        or stat.S_IMODE(root_info.st_mode) != 0o700
    ):
        raise ValueError("prior-fresh-boot-resume-root-invalid")
    for name in (*runtime_bindings._manifest_names(manifest), "files.sha256"):
        info = (root / name).lstat()
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_nlink != 1
        ):
            raise ValueError("prior-fresh-boot-resume-file-identity-invalid")


def _validate_request_and_binding(
    request: FreshBootResumeResultBindingRequest,
    upstream: control.FreshBootResumeBinding,
    root: Path,
) -> str:
    recorded_request = network_ready._read_json(root / "request.json")
    prior_repository_head = recorded_request.get("expected_repository_head")
    if (
        prior_repository_head != REQUIRED_PRIOR_REPOSITORY_HEAD
        or not isinstance(prior_repository_head, str)
        or not HEX_40.fullmatch(prior_repository_head)
        or prior_repository_head == request.expected_repository_head
    ):
        raise ValueError("prior-fresh-boot-resume-repository-head-invalid")
    expected_request = control.FreshBootResumeRequest.as_json(request)
    expected_request["expected_repository_head"] = prior_repository_head
    if recorded_request != expected_request:
        raise ValueError("prior-fresh-boot-resume-request-invalid")

    expected_binding = dict(upstream.evidence)
    expected_binding["repository_head"] = prior_repository_head
    if network_ready._read_json(root / "binding-preflight.json") != expected_binding:
        raise ValueError("prior-fresh-boot-resume-binding-invalid")
    return prior_repository_head


def _validate_host_identity(
    request: FreshBootResumeResultBindingRequest,
    upstream: control.FreshBootResumeBinding,
    root: Path,
) -> tuple[str, int]:
    suffixes = (
        "target-handle-pid-discovery",
        "target-handle-pid-confirmation-001",
        "target-handle-pid-confirmation-002",
        "target-handle-pid-confirmation-003",
        "target-handle-agent-ready",
        "target-handle-pre-resume",
        "target-handle-terminal",
    )
    signature: tuple[str, int] | None = None
    for index, suffix in enumerate(suffixes):
        process = network_ready._read_json(root / f"host-process-{suffix}.json")
        process_observation = process.get("observation")
        if (
            process.get("format") != runtime_bindings.EVIDENCE_FORMAT
            or process.get("relevant_process_count") != 0
            or process.get("relevant_processes") != []
            or not isinstance(process_observation, dict)
            or process_observation.get("argv")
            != list(launch_transport.PROCESS_COMMAND)
            or not _successful_observation(process_observation)
        ):
            raise ValueError("prior-fresh-boot-resume-process-invalid")

        handle = network_ready._read_json(root / f"{suffix}.json")
        observation = handle.get("observation")
        stdout = observation.get("stdout") if isinstance(observation, dict) else None
        expected_argv = (
            network_ready._lsof_argv(request)
            if index == 0
            else runtime_control.targeted_lsof_argv(
                request, upstream.prior_backend_pid
            )
        )
        if (
            handle.get("format") != runtime_bindings.EVIDENCE_FORMAT
            or handle.get("state") != "present"
            or handle.get("backend_pid") != upstream.prior_backend_pid
            or handle.get("backend_command") != "QEMULauncher"
            or handle.get("process_record_count") != 1
            or handle.get("efi_handle_count") != 1
            or handle.get("qcow2_handle_count") != 1
            or not isinstance(observation, dict)
            or observation.get("argv") != list(expected_argv)
            or not _successful_observation(observation)
            or not runtime_bindings._complete_stream(stdout)
        ):
            raise ValueError("prior-fresh-boot-resume-handle-invalid")
        assert isinstance(stdout, dict)
        current = (str(stdout["sha256"]), int(stdout["total_bytes"]))
        if signature is None:
            signature = current
        elif current != signature:
            raise ValueError("prior-fresh-boot-resume-handle-drift")
    if signature is None:
        raise ValueError("prior-fresh-boot-resume-handle-missing")
    return signature


def _validate_readiness(
    request: FreshBootResumeResultBindingRequest, root: Path
) -> None:
    readiness = network_ready._read_json(root / "guest-agent-readiness-001.json")
    if (
        readiness.get("argv") != list(control.readiness_argv(request))
        or not _successful_observation(readiness)
        or not runtime_bindings._empty_stream(readiness.get("stdout"))
    ):
        raise ValueError("prior-fresh-boot-resume-readiness-invalid")
    expected_ready = {
        "attempt": 1,
        "format": control.EVIDENCE_FORMAT,
        "predicate": "canonical-boot-id-readable",
        "state": "ready",
        "target_uuid": request.target_uuid,
    }
    if network_ready._read_json(root / "guest-agent-ready.json") != expected_ready:
        raise ValueError("prior-fresh-boot-resume-ready-marker-invalid")


def _validate_delivery(
    request: FreshBootResumeResultBindingRequest,
    upstream: control.FreshBootResumeBinding,
    root: Path,
) -> None:
    create = network_ready._read_json(root / "guest-fresh-boot-resume-root-create.json")
    if (
        create.get("argv")
        != [
            "utmctl",
            "exec",
            request.target_uuid,
            "--cmd",
            "/bin/mkdir",
            "-m",
            "0700",
            request.guest_fresh_boot_resume_root,
        ]
        or not _successful_empty_observation(create)
    ):
        raise ValueError("prior-fresh-boot-resume-root-create-invalid")

    deliveries = (
        (
            "frozen-resume-driver",
            request.guest_frozen_resume_driver_incoming,
            request.guest_frozen_resume_driver_path,
            upstream.frozen_resume_driver_bytes,
        ),
        (
            "frozen-recovery-probe",
            request.guest_frozen_recovery_probe_incoming,
            request.guest_frozen_recovery_probe_path,
            upstream.frozen_recovery_probe_bytes,
        ),
        (
            "fresh-boot-resume-driver",
            request.guest_fresh_boot_resume_driver_incoming,
            request.guest_fresh_boot_resume_driver_path,
            upstream.fresh_boot_resume_driver_bytes,
        ),
    )
    for label, incoming, final, payload in deliveries:
        expected = {
            "push": ["utmctl", "file", "push", request.target_uuid, incoming],
            "chown": [
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/chown",
                "root:root",
                incoming,
            ],
            "chmod": [
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/chmod",
                "0600",
                incoming,
            ],
            "publish": [
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/mv",
                "--",
                incoming,
                final,
            ],
        }
        for action, argv in expected.items():
            value = network_ready._read_json(root / f"guest-{label}-{action}.json")
            if value.get("argv") != argv or not _successful_empty_observation(value):
                raise ValueError(
                    f"prior-fresh-boot-resume-{label}-{action}-invalid"
                )
        _require_exact_payload(
            root / f"guest-{label}-readback.json",
            ["utmctl", "file", "pull", request.target_uuid, final],
            payload,
            f"prior-fresh-boot-resume-{label}-readback",
        )


def _validate_execution(
    request: FreshBootResumeResultBindingRequest,
    upstream: control.FreshBootResumeBinding,
    root: Path,
) -> dict[str, object]:
    once = network_ready._read_json(root / "guest-fresh-boot-resume-once.json")
    observation = once.get("observation")
    if (
        once.get("format") != control.EVIDENCE_FORMAT
        or once.get("raw_output_persisted") is not False
        or not isinstance(observation, dict)
        or observation.get("argv") != list(control.driver_argv(request, upstream))
        or not _successful_empty_observation(observation)
    ):
        raise ValueError("prior-fresh-boot-resume-driver-invocation-invalid")

    _require_exact_payload(
        root / "guest-fresh-boot-resume-marker-readback.json",
        [
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            request.guest_fresh_boot_resume_marker_path,
        ],
        control.marker_bytes(request, upstream),
        "prior-fresh-boot-resume-marker",
    )
    payloads = [
        _read_payload(
            root / f"guest-fresh-boot-resume-result-readback-{index}.json",
            [
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                request.guest_fresh_boot_resume_terminal_path,
            ],
            f"prior-fresh-boot-resume-result-{index}",
        )
        for index in (1, 2)
    ]
    if payloads[0] != payloads[1]:
        raise ValueError("prior-fresh-boot-resume-result-readback-drift")
    guest_terminal = control.parse_guest_terminal(payloads[0], request, upstream)
    if guest_terminal != _expected_guest_terminal(request, upstream):
        raise ValueError("prior-fresh-boot-resume-result-semantics-invalid")
    if (
        network_ready._read_json(root / "guest-fresh-boot-resume-result.json")
        != guest_terminal
    ):
        raise ValueError("prior-fresh-boot-resume-recorded-result-invalid")
    _require_exact_payload(
        root / "guest-fresh-boot-resume-phase-readback.json",
        [
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            request.guest_fresh_boot_resume_phase_path,
        ],
        guest_driver.canonical_json(
            {"format": guest_driver.PHASE_FORMAT, "phase": "indeterminate"}
        ),
        "prior-fresh-boot-resume-phase",
    )
    return guest_terminal


def _expected_guest_terminal(
    request: FreshBootResumeResultBindingRequest,
    upstream: control.FreshBootResumeBinding,
) -> dict[str, object]:
    return {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "checkpoint_attempt_id": guest_driver.EXPECTED_CHECKPOINT_ATTEMPT_ID,
        "current_boot_id_sha256": upstream.current_boot_id_sha256,
        "dpkg_mutation_executed": "unknown",
        "format": guest_driver.EVIDENCE_FORMAT,
        "maintenance_resume_invocations": 1,
        "operation_id": "reconstructed-secret-hash-only",
        "operation_id_sha256": guest_driver.EXPECTED_OPERATION_ID_SHA256,
        "outcome": "state-indeterminate",
        "phase": "postflight",
        "postflight_invocations": 1,
        "prior_boot_id_sha256": upstream.prior_boot_id_sha256,
        "reason": REQUIRED_REASON,
        "resume_attempt_id": request.fresh_boot_resume_attempt_id,
        "terminal_case_sha256": REQUIRED_TERMINAL_CASE_SHA256,
        "transaction": "state-indeterminate",
        "transient_secret_reconstructed": True,
    }


def _validate_terminal(
    request: FreshBootResumeResultBindingRequest,
    upstream: control.FreshBootResumeBinding,
    root: Path,
    guest_terminal: dict[str, object],
) -> None:
    expected = {
        "agent_readiness_invocations": 1,
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": upstream.prior_backend_pid,
        "driver_transport_exit_code": 0,
        "file_pull_invocations": 7,
        "file_push_invocations": 3,
        "format": control.EVIDENCE_FORMAT,
        "fresh_boot_resume_attempt_id": request.fresh_boot_resume_attempt_id,
        "guest_exec_invocations": 11,
        "guest_resume_outcome": guest_terminal["outcome"],
        "identity_observation_count": 3,
        "inventory_probe_invocations": 0,
        "maintenance_resume_invocations": 1,
        "operation_id": "reconstructed-secret-hash-only",
        "operation_id_sha256": guest_driver.EXPECTED_OPERATION_ID_SHA256,
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "postflight_invocations": 1,
        "reason": f"guest-resume-state-indeterminate:{REQUIRED_REASON}",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "state-indeterminate",
        "transport_exit_disambiguated_by_terminal": True,
    }
    if network_ready._read_json(root / "terminal.json") != expected:
        raise ValueError("prior-fresh-boot-resume-terminal-invalid")


def _validate_command_inventory(
    request: FreshBootResumeResultBindingRequest,
    upstream: control.FreshBootResumeBinding,
    root: Path,
) -> None:
    expected: dict[str, list[str]] = {}
    suffixes = (
        "target-handle-pid-discovery",
        "target-handle-pid-confirmation-001",
        "target-handle-pid-confirmation-002",
        "target-handle-pid-confirmation-003",
        "target-handle-agent-ready",
        "target-handle-pre-resume",
        "target-handle-terminal",
    )
    for index, suffix in enumerate(suffixes):
        expected[f"host-process-{suffix}.json"] = list(
            launch_transport.PROCESS_COMMAND
        )
        expected[f"{suffix}.json"] = list(
            network_ready._lsof_argv(request)
            if index == 0
            else runtime_control.targeted_lsof_argv(
                request, upstream.prior_backend_pid
            )
        )
    expected["guest-agent-readiness-001.json"] = list(
        control.readiness_argv(request)
    )
    expected["guest-fresh-boot-resume-root-create.json"] = [
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/bin/mkdir",
        "-m",
        "0700",
        request.guest_fresh_boot_resume_root,
    ]
    deliveries = (
        (
            "frozen-resume-driver",
            request.guest_frozen_resume_driver_incoming,
            request.guest_frozen_resume_driver_path,
        ),
        (
            "frozen-recovery-probe",
            request.guest_frozen_recovery_probe_incoming,
            request.guest_frozen_recovery_probe_path,
        ),
        (
            "fresh-boot-resume-driver",
            request.guest_fresh_boot_resume_driver_incoming,
            request.guest_fresh_boot_resume_driver_path,
        ),
    )
    for label, incoming, final in deliveries:
        expected[f"guest-{label}-push.json"] = [
            "utmctl",
            "file",
            "push",
            request.target_uuid,
            incoming,
        ]
        expected[f"guest-{label}-chown.json"] = [
            "utmctl",
            "exec",
            request.target_uuid,
            "--cmd",
            "/bin/chown",
            "root:root",
            incoming,
        ]
        expected[f"guest-{label}-chmod.json"] = [
            "utmctl",
            "exec",
            request.target_uuid,
            "--cmd",
            "/bin/chmod",
            "0600",
            incoming,
        ]
        expected[f"guest-{label}-publish.json"] = [
            "utmctl",
            "exec",
            request.target_uuid,
            "--cmd",
            "/bin/mv",
            "--",
            incoming,
            final,
        ]
        expected[f"guest-{label}-readback.json"] = [
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            final,
        ]
    expected["guest-fresh-boot-resume-once.json"] = list(
        control.driver_argv(request, upstream)
    )
    expected["guest-fresh-boot-resume-marker-readback.json"] = [
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        request.guest_fresh_boot_resume_marker_path,
    ]
    for index in (1, 2):
        expected[f"guest-fresh-boot-resume-result-readback-{index}.json"] = [
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            request.guest_fresh_boot_resume_terminal_path,
        ]
    expected["guest-fresh-boot-resume-phase-readback.json"] = [
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        request.guest_fresh_boot_resume_phase_path,
    ]

    for name in REQUIRED_ENTRY_NAMES:
        found = _all_argv(network_ready._read_json(root / name))
        wanted = [expected[name]] if name in expected else []
        if found != wanted:
            raise ValueError("prior-fresh-boot-resume-command-inventory-invalid")


def _all_argv(value: object) -> list[list[str]]:
    found: list[list[str]] = []
    if isinstance(value, dict):
        argv = value.get("argv")
        if isinstance(argv, list) and all(isinstance(item, str) for item in argv):
            found.append(argv)
        for key, child in value.items():
            if key != "argv":
                found.extend(_all_argv(child))
    elif isinstance(value, list):
        for child in value:
            found.extend(_all_argv(child))
    return found


def _require_exact_payload(
    path: Path, argv: list[str], expected: bytes, label: str
) -> None:
    if _read_payload(path, argv, label) != expected:
        raise ValueError(f"{label}-payload-invalid")


def _read_payload(path: Path, argv: list[str], label: str) -> bytes:
    value = network_ready._read_json(path)
    stdout = value.get("stdout")
    if (
        value.get("argv") != argv
        or not _successful_observation(value)
        or not isinstance(stdout, dict)
        or stdout.get("truncated") is not False
        or not isinstance(stdout.get("prefix_base64"), str)
    ):
        raise ValueError(f"{label}-observation-invalid")
    try:
        payload = base64.b64decode(stdout["prefix_base64"], validate=True)
    except ValueError as exc:
        raise ValueError(f"{label}-base64-invalid") from exc
    if (
        stdout.get("total_bytes") != len(payload)
        or stdout.get("sha256") != hashlib.sha256(payload).hexdigest()
    ):
        raise ValueError(f"{label}-stream-invalid")
    return payload


def _successful_empty_observation(value: object) -> bool:
    return (
        _successful_observation(value)
        and isinstance(value, dict)
        and runtime_bindings._empty_stream(value.get("stdout"))
    )


def _successful_observation(value: object) -> bool:
    return result_bindings._successful_observation(value)

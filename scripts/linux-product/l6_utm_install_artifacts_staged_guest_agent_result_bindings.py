#!/usr/bin/env python3
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_guest_agent_bindings as guest_bindings
import l6_utm_install_artifacts_staged_guest_agent_resolution as guest_control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_launch_transport_bindings as launch_bindings
import l6_utm_start_once as start_control


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "guest-agent-result-binding-v1"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_guest_agent_result_bindings.py"
)
PROBE_RELATIVE_PATH = guest_bindings.PROBE_RELATIVE_PATH
REQUIRED_PRIOR_GUEST_AGENT_MANIFEST_SHA256 = (
    "eb7c42f1b74701ce585188687b8cb18be6ecd2cddba8c4340fb09f775038d053"
)
REQUIRED_PRIOR_GUEST_AGENT_REPOSITORY_HEAD = (
    "1d649165062fb952471620f8444f897eef7d8938"
)
REQUIRED_PRIOR_GUEST_AGENT_ATTEMPT_ID = (
    "d75818f-v4-install-artifacts-staged-guest-agent-20260825-v1"
)
REQUIRED_BACKEND_PID = 39591
REQUIRED_HANDLE_STDOUT_SHA256 = (
    "d16eca6cf5b24692e5281c33390a806c2ae606e5601579750d70662624546bd7"
)
REQUIRED_PRIOR_PROBE_SHA256 = (
    "27ea59b4061134030f2f88bbe6b21c5787f3a27385aea3f486f2e4881981b8ba"
)
REQUIRED_PRIOR_PROBE_SIZE = 7451
REQUIRED_PRIOR_GUEST_AGENT_ENTRY_NAMES = (
    "request.json",
    "binding-preflight.json",
    "source-bundle-preflight.json",
    "target-files-preflight.json",
    "host-process-preflight.json",
    "target-handle-pid-discovery.json",
    "target-handle-pid-confirmation-001.json",
    "host-process-confirmation-001.json",
    "target-handle-pid-confirmation-002.json",
    "host-process-confirmation-002.json",
    "target-handle-pid-confirmation-003.json",
    "host-process-confirmation-003.json",
    "target-files-qualified.json",
    "guest-agent-readiness-001.json",
    "target-handle-readiness-001.json",
    "host-process-readiness-001.json",
    "guest-agent-ready.json",
    "target-files-agent-ready.json",
    "guest-control-root-create.json",
    "guest-probe-push.json",
    "guest-probe-normalize.json",
    "guest-probe-readback.json",
    "guest-boot-transport-probe-once.json",
    "guest-marker-readback.json",
    "source-bundle-postflight.json",
    "terminal.json",
)


class GuestAgentResultBindingRequest(
    guest_bindings.GuestAgentBindingRequest, Protocol
):
    prior_guest_agent_root: Path
    prior_guest_agent_manifest_sha256: str
    prior_guest_agent_attempt_id: str


@dataclass(frozen=True)
class GuestAgentResultBinding:
    evidence: dict[str, object]
    upstream: guest_bindings.GuestAgentBinding
    expected_boot_id_sha256: str
    probe_bytes: bytes
    prior_backend_pid: int


def validate_guest_agent_result_bindings(
    request: GuestAgentResultBindingRequest,
) -> GuestAgentResultBinding:
    if (
        request.guest_agent_attempt_id
        != REQUIRED_PRIOR_GUEST_AGENT_ATTEMPT_ID
        or request.prior_guest_agent_attempt_id
        != REQUIRED_PRIOR_GUEST_AGENT_ATTEMPT_ID
    ):
        raise ValueError("required-prior-guest-agent-attempt-id-mismatch")

    upstream = guest_bindings.validate_guest_agent_bindings(request)
    identities: dict[str, object] = {}
    for relative_path, label in (
        (BINDINGS_RELATIVE_PATH, "guest_agent_result_bindings"),
        (PROBE_RELATIVE_PATH, "current_boot_transport_probe"),
    ):
        path = request.repository_root / relative_path
        network_ready._require_committed_regular(path, label)
        identities[f"{label}_sha256"] = network_ready._sha256_file(path)

    root = request.prior_guest_agent_root
    manifest = root / "files.sha256"
    if (
        request.prior_guest_agent_manifest_sha256
        != REQUIRED_PRIOR_GUEST_AGENT_MANIFEST_SHA256
        or network_ready._sha256_file(manifest)
        != REQUIRED_PRIOR_GUEST_AGENT_MANIFEST_SHA256
    ):
        raise ValueError("prior-guest-agent-manifest-identity-invalid")
    entries = start_control._verify_sha256_manifest(root, manifest)
    if entries != len(REQUIRED_PRIOR_GUEST_AGENT_ENTRY_NAMES):
        raise ValueError("prior-guest-agent-entry-count-invalid")
    if (
        runtime_bindings._manifest_names(manifest)
        != REQUIRED_PRIOR_GUEST_AGENT_ENTRY_NAMES
    ):
        raise ValueError("prior-guest-agent-entry-names-invalid")

    _validate_prior_request(request, root)
    _validate_prior_binding(request, root)
    _validate_prior_source_and_target(request, root)
    handle_sha256 = _validate_prior_host_identity(request, root)
    _validate_prior_readiness(request, root)
    _validate_prior_probe_delivery(request, root)
    _validate_prior_marker_failure(request, root)
    _validate_prior_terminal(request, root)

    return GuestAgentResultBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            **identities,
            "expected_boot_id_sha256": upstream.expected_boot_id_sha256,
            "prior_guest_agent_attempt_id": (
                request.prior_guest_agent_attempt_id
            ),
            "prior_guest_agent_backend_pid": REQUIRED_BACKEND_PID,
            "prior_guest_agent_entries_verified": entries,
            "prior_guest_agent_guest_probe_invocations": 1,
            "prior_guest_agent_handle_confirmation_sha256": handle_sha256,
            "prior_guest_agent_manifest_sha256": (
                request.prior_guest_agent_manifest_sha256
            ),
            "prior_guest_agent_marker": "absent-after-probe-exec",
            "prior_guest_agent_outcome": "state-indeterminate",
            "prior_guest_agent_result_readback_invocations": 0,
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
        },
        upstream=upstream,
        expected_boot_id_sha256=upstream.expected_boot_id_sha256,
        probe_bytes=upstream.probe_bytes,
        prior_backend_pid=REQUIRED_BACKEND_PID,
    )


def _validate_prior_request(
    request: GuestAgentResultBindingRequest, root: Path
) -> None:
    expected = {
        "agent_readiness_attempts": 60,
        "authorization": {
            "bounded_read_only_guest_agent_readiness": True,
            "install_artifacts_staged_guest_agent_resolution": True,
            "no_list_status_start_resume_business_guest_retry_stop_or_quit": True,
            "one_private_guest_probe_delivery_and_execution": True,
            "stable_target_handle_pid_observations": True,
            "two_independent_result_readbacks": True,
        },
        "command_timeout_seconds": 60,
        "expected_repository_head": REQUIRED_PRIOR_GUEST_AGENT_REPOSITORY_HEAD,
        "format": guest_bindings.EVIDENCE_FORMAT,
        "guest_agent_attempt_id": request.prior_guest_agent_attempt_id,
        "guest_control_root": _guest_control_root(request),
        "identity_observations": 3,
        "poll_interval_seconds": 1,
        "prior_boot_start_attempt_id": request.prior_boot_start_attempt_id,
        "prior_boot_start_manifest_sha256": (
            request.prior_boot_start_manifest_sha256
        ),
        "source_bundle_path_sha256": network_ready._sha256_text(
            str(request.source_bundle_path)
        ),
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_package_path_sha256": network_ready._sha256_text(
            str(request.target_package_path)
        ),
        "target_uuid": request.target_uuid,
    }
    if network_ready._read_json(root / "request.json") != expected:
        raise ValueError("prior-guest-agent-request-invalid")


def _validate_prior_binding(
    request: GuestAgentResultBindingRequest, root: Path
) -> None:
    expected = {
        "boot_transport_probe_sha256": REQUIRED_PRIOR_PROBE_SHA256,
        "expected_boot_id_sha256": (
            "18f1ba063ecd3087a5624e6ae52a54624540a972df33cca00a34bca4ec00022a"
        ),
        "format": guest_bindings.EVIDENCE_FORMAT,
        "guest_agent_attempt_id": request.prior_guest_agent_attempt_id,
        "guest_agent_bindings_sha256": (
            "00bdce070e795152bfb749e19f8c461711dabfbed294d00c10563591050a7e5f"
        ),
        "guest_agent_control_sha256": (
            "210decc05e8d07db0ab84a4050d02ce6bbb029cf0a9c654a3bd87a31fdfd9543"
        ),
        "prior_boot_start_agent_readiness": "unavailable-at-first-exec",
        "prior_boot_start_attempt_id": request.prior_boot_start_attempt_id,
        "prior_boot_start_backend_pid": REQUIRED_BACKEND_PID,
        "prior_boot_start_entries_verified": 29,
        "prior_boot_start_guest_probe_invocations": 0,
        "prior_boot_start_handle_confirmation_sha256": (
            REQUIRED_HANDLE_STDOUT_SHA256
        ),
        "prior_boot_start_manifest_sha256": (
            request.prior_boot_start_manifest_sha256
        ),
        "prior_boot_start_outcome": "state-indeterminate",
        "prior_boot_start_result_readback_invocations": 0,
        "repository_clean": True,
        "repository_head": REQUIRED_PRIOR_GUEST_AGENT_REPOSITORY_HEAD,
        "target_uuid": request.target_uuid,
    }
    if network_ready._read_json(root / "binding-preflight.json") != expected:
        raise ValueError("prior-guest-agent-binding-invalid")


def _validate_prior_source_and_target(
    request: GuestAgentResultBindingRequest, root: Path
) -> None:
    source = network_ready._read_json(root / "source-bundle-preflight.json")
    descriptor = source.get("descriptor")
    if (
        source.get("format")
        != "radishlex-linux-l6-utm-canonical-input-transfer-v1"
        or source.get("inventory_count") != 12
        or not isinstance(descriptor, dict)
        or descriptor.get("sha256") != request.source_bundle_sha256
        or descriptor.get("size") != request.source_bundle_size
    ):
        raise ValueError("prior-guest-agent-source-preflight-invalid")
    if network_ready._read_json(root / "source-bundle-postflight.json") != {
        "descriptor_unchanged": True,
        "format": "radishlex-linux-l6-utm-canonical-input-transfer-v1",
        "inventory_unchanged": True,
        "sha256": request.source_bundle_sha256,
        "size": request.source_bundle_size,
    }:
        raise ValueError("prior-guest-agent-source-postflight-invalid")

    target_values = [
        network_ready._read_json(root / name)
        for name in (
            "target-files-preflight.json",
            "target-files-qualified.json",
            "target-files-agent-ready.json",
        )
    ]
    target = target_values[0]
    if (
        any(value != target for value in target_values[1:])
        or target.get("format") != network_ready.EVIDENCE_FORMAT
        or target.get("target_config_sha256")
        != network_ready.REQUIRED_TARGET_CONFIG_SHA256
        or target.get("target_package_name") != request.target_package_path.name
        or target.get("target_package_path_sha256")
        != network_ready._sha256_text(str(request.target_package_path))
    ):
        raise ValueError("prior-guest-agent-target-identity-invalid")


def _validate_prior_host_identity(
    request: GuestAgentResultBindingRequest, root: Path
) -> str:
    process_names = (
        "host-process-preflight.json",
        "host-process-confirmation-001.json",
        "host-process-confirmation-002.json",
        "host-process-confirmation-003.json",
        "host-process-readiness-001.json",
    )
    for name in process_names:
        runtime_bindings._require_process_state(
            root / name,
            0,
            expected_format=guest_bindings.EVIDENCE_FORMAT,
            error_label="prior-guest-agent",
        )

    hashes = [
        _require_handle_state(
            network_ready._read_json(root / "target-handle-pid-discovery.json"),
            network_ready._lsof_argv(request),
        )
    ]
    targeted_argv = runtime_control.targeted_lsof_argv(
        request, REQUIRED_BACKEND_PID
    )
    for name in (
        "target-handle-pid-confirmation-001.json",
        "target-handle-pid-confirmation-002.json",
        "target-handle-pid-confirmation-003.json",
        "target-handle-readiness-001.json",
    ):
        hashes.append(
            _require_handle_state(
                network_ready._read_json(root / name), targeted_argv
            )
        )
    if set(hashes) != {REQUIRED_HANDLE_STDOUT_SHA256}:
        raise ValueError("prior-guest-agent-handle-hash-drift")
    return hashes[0]


def _require_handle_state(
    value: dict[str, object], expected_argv: tuple[str, ...]
) -> str:
    observation = value.get("observation")
    if (
        value.get("format") != guest_bindings.EVIDENCE_FORMAT
        or value.get("state") != "present"
        or value.get("backend_command") != "QEMULauncher"
        or value.get("backend_pid") != REQUIRED_BACKEND_PID
        or value.get("efi_handle_count") != 1
        or value.get("process_record_count") != 1
        or value.get("qcow2_handle_count") != 1
        or not isinstance(observation, dict)
        or observation.get("argv") != list(expected_argv)
        or observation.get("exit_code") != 0
        or observation.get("timed_out") is not False
        or not runtime_bindings._empty_stream(observation.get("stderr"))
        or not runtime_bindings._complete_stream(observation.get("stdout"))
    ):
        raise ValueError("prior-guest-agent-handle-semantics-invalid")
    stdout = observation.get("stdout")
    if not isinstance(stdout, dict) or not isinstance(stdout.get("sha256"), str):
        raise ValueError("prior-guest-agent-handle-hash-invalid")
    return stdout["sha256"]


def _validate_prior_readiness(
    request: GuestAgentResultBindingRequest, root: Path
) -> None:
    value = network_ready._read_json(root / "guest-agent-readiness-001.json")
    observation = launch_bindings._command_observation_from_json(
        value, "prior-guest-agent-readiness"
    )
    _require_successful_empty_observation(
        observation,
        guest_control.readiness_argv(request),
        "prior-guest-agent-readiness",
    )
    if network_ready._read_json(root / "guest-agent-ready.json") != {
        "attempt": 1,
        "format": guest_bindings.EVIDENCE_FORMAT,
        "predicate": "canonical-boot-id-readable",
        "state": "ready",
        "target_uuid": request.target_uuid,
    }:
        raise ValueError("prior-guest-agent-ready-invalid")


def _validate_prior_probe_delivery(
    request: GuestAgentResultBindingRequest, root: Path
) -> None:
    control_root = _guest_control_root(request)
    observations = (
        (
            "guest-control-root-create.json",
            (
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/mkdir",
                "-m",
                "0700",
                control_root,
            ),
        ),
        (
            "guest-probe-push.json",
            (
                "utmctl",
                "file",
                "push",
                request.target_uuid,
                f"{control_root}/boot-transport-probe.incoming.py",
            ),
        ),
        (
            "guest-probe-normalize.json",
            (
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/usr/bin/install",
                "-o",
                "root",
                "-g",
                "root",
                "-m",
                "0600",
                "--",
                f"{control_root}/boot-transport-probe.incoming.py",
                f"{control_root}/boot-transport-probe.py",
            ),
        ),
    )
    for name, expected_argv in observations:
        observation = launch_bindings._command_observation_from_json(
            network_ready._read_json(root / name),
            f"prior-{name}",
        )
        _require_successful_empty_observation(
            observation, expected_argv, f"prior-{name}"
        )

    readback = launch_bindings._command_observation_from_json(
        network_ready._read_json(root / "guest-probe-readback.json"),
        "prior-guest-probe-readback",
    )
    if (
        readback.argv
        != (
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            f"{control_root}/boot-transport-probe.py",
        )
        or readback.exit_code != 0
        or readback.timed_out
        or readback.stderr.total_bytes != 0
        or readback.stderr.truncated
        or readback.stdout.sha256 != REQUIRED_PRIOR_PROBE_SHA256
        or readback.stdout.total_bytes != REQUIRED_PRIOR_PROBE_SIZE
        or readback.stdout.truncated
    ):
        raise ValueError("prior-guest-agent-probe-readback-invalid")

    value = network_ready._read_json(
        root / "guest-boot-transport-probe-once.json"
    )
    observation = value.get("observation")
    expected_probe_argv = (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        "-I",
        "-B",
        f"{control_root}/boot-transport-probe.py",
        "--attempt-id",
        request.prior_guest_agent_attempt_id,
        "--target-uuid",
        request.target_uuid,
        "--expected-probe-sha256",
        REQUIRED_PRIOR_PROBE_SHA256,
        "--control-root",
        control_root,
    )
    if (
        value.get("format") != guest_bindings.EVIDENCE_FORMAT
        or value.get("raw_output_persisted") is not False
        or not isinstance(observation, dict)
        or observation.get("argv") != list(expected_probe_argv)
        or observation.get("exit_code") != 0
        or observation.get("timed_out") is not False
        or not runtime_bindings._empty_stream(observation.get("stdout"))
        or not runtime_bindings._empty_stream(observation.get("stderr"))
    ):
        raise ValueError("prior-guest-agent-probe-exec-invalid")


def _validate_prior_marker_failure(
    request: GuestAgentResultBindingRequest, root: Path
) -> None:
    control_root = _guest_control_root(request)
    observation = launch_bindings._command_observation_from_json(
        network_ready._read_json(root / "guest-marker-readback.json"),
        "prior-guest-marker-readback",
    )
    expected_stderr = (
        "Error from event: The operation couldn’t be completed. "
        "(OSStatus error -2700.)\n"
        f"failed to open file '{control_root}/attempt.marker.json' "
        "(mode: 'r'): No such file or directory\n"
    ).encode("utf-8")
    if (
        observation.argv
        != (
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            f"{control_root}/attempt.marker.json",
        )
        or observation.exit_code != 0
        or observation.timed_out
        or observation.stdout.total_bytes != 0
        or observation.stdout.truncated
        or observation.stderr.prefix != expected_stderr
        or observation.stderr.total_bytes != len(expected_stderr)
        or observation.stderr.truncated
    ):
        raise ValueError("prior-guest-agent-marker-failure-invalid")


def _validate_prior_terminal(
    request: GuestAgentResultBindingRequest, root: Path
) -> None:
    expected = {
        "agent_readiness_invocations": 1,
        "agent_readiness_state": "ready",
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": REQUIRED_BACKEND_PID,
        "boot_classification": "not-generated",
        "business_guest_action": "not-performed",
        "file_pull_invocations": 2,
        "file_push_invocations": 1,
        "format": guest_bindings.EVIDENCE_FORMAT,
        "guest_agent_attempt_id": request.prior_guest_agent_attempt_id,
        "guest_exec_invocations": 4,
        "guest_probe_invocations": 1,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 0,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": None,
        "operation_id": "not-read-or-generated",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "readiness_identity_observation_count": 1,
        "reason": "guest-marker-readback:guest-marker-readback-stderr-not-empty",
        "result_readback_invocations": 0,
        "target_handles_terminal": "present",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    if network_ready._read_json(root / "terminal.json") != expected:
        raise ValueError("prior-guest-agent-terminal-invalid")


def _require_successful_empty_observation(
    observation: start_control.CommandObservation,
    expected_argv: tuple[str, ...],
    label: str,
) -> None:
    if (
        observation.argv != expected_argv
        or observation.exit_code != 0
        or observation.timed_out
        or observation.stdout.total_bytes != 0
        or observation.stdout.truncated
        or observation.stderr.total_bytes != 0
        or observation.stderr.truncated
    ):
        raise ValueError(f"{label}-invalid")


def _guest_control_root(request: GuestAgentResultBindingRequest) -> str:
    return (
        "/var/tmp/radishlex-l6-v4-guest-agent-"
        + request.prior_guest_agent_attempt_id
    )

#!/usr/bin/env python3
from __future__ import annotations

import re
import stat
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

import l6_utm_guest_network_ready as network_ready
import l6_utm_install_artifacts_staged_boot_classification_result_bindings as legacy_result
import l6_utm_install_artifacts_staged_fresh_boot_classification as control
import l6_utm_install_artifacts_staged_fresh_boot_classification_bindings as bindings
import l6_utm_install_artifacts_staged_guest_agent_bindings as guest_bindings
import l6_utm_install_artifacts_staged_guest_agent_resolution as guest_control
import l6_utm_install_artifacts_staged_runtime_resolution as runtime_control
import l6_utm_install_artifacts_staged_runtime_resolution_bindings as runtime_bindings
import l6_utm_launch_transport_bindings as launch_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_boot_transport_probe as guest_probe


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-utm-install-artifacts-staged-"
    "fresh-boot-classification-result-binding-v1"
)
BINDINGS_RELATIVE_PATH = Path(
    "scripts/linux-product/"
    "l6_utm_install_artifacts_staged_fresh_boot_classification_result_bindings.py"
)
HEX_64 = re.compile(r"[0-9a-f]{64}")
HEX_40 = re.compile(r"[0-9a-f]{40}")


class FreshBootClassificationResultBindingRequest(
    bindings.FreshBootClassificationBindingRequest,
    Protocol,
):
    prior_fresh_boot_classification_root: Path
    prior_fresh_boot_classification_manifest_sha256: str
    prior_fresh_boot_classification_attempt_id: str


@dataclass(frozen=True)
class FreshBootClassificationResultBinding:
    evidence: dict[str, object]
    upstream: bindings.FreshBootClassificationBinding
    expected_boot_id_sha256: str
    observed_boot_id_sha256: str
    probe_bytes: bytes
    prior_backend_pid: int


def validate_fresh_boot_classification_result_bindings(
    request: FreshBootClassificationResultBindingRequest,
) -> FreshBootClassificationResultBinding:
    if (
        request.prior_fresh_boot_classification_attempt_id
        != bindings.REQUIRED_ATTEMPT_ID
        or not HEX_64.fullmatch(
            request.prior_fresh_boot_classification_manifest_sha256
        )
    ):
        raise ValueError("required-prior-fresh-boot-classification-mismatch")

    prior_request = _PriorFreshBootClassificationRequestView(request)
    upstream = bindings.validate_fresh_boot_classification_bindings(prior_request)
    if (
        upstream.evidence.get("repository_clean") is not True
        or upstream.evidence.get("repository_head")
        != request.expected_repository_head
    ):
        raise ValueError("current-fresh-boot-classification-binding-invalid")
    binding_path = request.repository_root / BINDINGS_RELATIVE_PATH
    network_ready._require_committed_regular(
        binding_path, "fresh_boot_classification_result_bindings"
    )

    root = request.prior_fresh_boot_classification_root
    manifest = root / "files.sha256"
    if (
        network_ready._sha256_file(manifest)
        != request.prior_fresh_boot_classification_manifest_sha256
    ):
        raise ValueError("prior-fresh-boot-classification-manifest-invalid")
    entries = start_control._verify_sha256_manifest(root, manifest)
    _require_private_evidence_tree(root, manifest)

    terminal = network_ready._read_json(root / "terminal.json")
    backend_pid, runtime_polls, readiness_attempts = _validate_terminal_shape(
        request, terminal
    )
    expected_names = _expected_entry_names(runtime_polls, readiness_attempts)
    if (
        entries != len(expected_names)
        or runtime_bindings._manifest_names(manifest) != expected_names
    ):
        raise ValueError("prior-fresh-boot-classification-entry-set-invalid")

    recorded_request = network_ready._read_json(root / "request.json")
    prior_repository_head = recorded_request.get("expected_repository_head")
    if (
        not isinstance(prior_repository_head, str)
        or not HEX_40.fullmatch(prior_repository_head)
    ):
        raise ValueError(
            "prior-fresh-boot-classification-repository-head-invalid"
        )
    expected_request = control.FreshBootClassificationRequest.as_json(
        prior_request
    )
    expected_request["expected_repository_head"] = prior_repository_head
    if recorded_request != expected_request:
        raise ValueError("prior-fresh-boot-classification-request-invalid")
    expected_binding = dict(upstream.evidence)
    expected_binding["repository_head"] = prior_repository_head
    if (
        network_ready._read_json(root / "binding-preflight.json")
        != expected_binding
    ):
        raise ValueError("prior-fresh-boot-classification-binding-invalid")

    legacy_result._validate_prior_source_target(prior_request, root)
    _validate_inventory_and_start(prior_request, upstream, root)
    handle_signature = _validate_runtime(
        prior_request, root, backend_pid, runtime_polls
    )
    _validate_readiness(
        prior_request, root, backend_pid, readiness_attempts, handle_signature
    )
    observed_boot_id_sha256 = _validate_probe_and_result(
        prior_request, upstream, root, backend_pid, handle_signature
    )
    _validate_terminal(
        prior_request,
        root,
        terminal,
        backend_pid,
        runtime_polls,
        readiness_attempts,
        observed_boot_id_sha256,
    )
    runtime_control._require_no_raw_operation_id(root)

    return FreshBootClassificationResultBinding(
        evidence={
            "format": EVIDENCE_FORMAT,
            "expected_boot_id_sha256": upstream.expected_boot_id_sha256,
            "fresh_boot_classification_result_bindings_sha256": (
                network_ready._sha256_file(binding_path)
            ),
            "observed_boot_id_sha256": observed_boot_id_sha256,
            "prior_fresh_boot_classification_attempt_id": (
                request.prior_fresh_boot_classification_attempt_id
            ),
            "prior_fresh_boot_classification_backend_pid": backend_pid,
            "prior_fresh_boot_classification_entries_verified": entries,
            "prior_fresh_boot_classification_manifest_sha256": (
                request.prior_fresh_boot_classification_manifest_sha256
            ),
            "prior_fresh_boot_classification_outcome": "new-boot-started",
            "prior_fresh_boot_classification_repository_head": (
                prior_repository_head
            ),
            "repository_clean": True,
            "repository_head": request.expected_repository_head,
            "target_uuid": request.target_uuid,
        },
        upstream=upstream,
        expected_boot_id_sha256=upstream.expected_boot_id_sha256,
        observed_boot_id_sha256=observed_boot_id_sha256,
        probe_bytes=upstream.probe_bytes,
        prior_backend_pid=backend_pid,
    )


def _require_private_evidence_tree(root: Path, manifest: Path) -> None:
    root_info = root.lstat()
    if (
        not stat.S_ISDIR(root_info.st_mode)
        or stat.S_ISLNK(root_info.st_mode)
        or stat.S_IMODE(root_info.st_mode) != 0o700
    ):
        raise ValueError("prior-fresh-boot-classification-root-identity-invalid")
    names = (*runtime_bindings._manifest_names(manifest), "files.sha256")
    for name in names:
        info = (root / name).lstat()
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_ISLNK(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_nlink != 1
        ):
            raise ValueError(
                "prior-fresh-boot-classification-file-identity-invalid"
            )


def _validate_terminal_shape(
    request: FreshBootClassificationResultBindingRequest,
    terminal: dict[str, object],
) -> tuple[int, int, int]:
    backend_pid = terminal.get("backend_pid")
    runtime_polls = terminal.get("runtime_poll_count")
    readiness_attempts = terminal.get("agent_readiness_invocations")
    if (
        not isinstance(backend_pid, int)
        or backend_pid <= 1
        or not isinstance(runtime_polls, int)
        or not 1 <= runtime_polls <= request.runtime_poll_attempts
        or not isinstance(readiness_attempts, int)
        or not 1 <= readiness_attempts <= request.agent_readiness_attempts
    ):
        raise ValueError("prior-fresh-boot-classification-terminal-shape-invalid")
    return backend_pid, runtime_polls, readiness_attempts


def _expected_entry_names(
    runtime_polls: int, readiness_attempts: int
) -> tuple[str, ...]:
    names = [
        "request.json",
        "binding-preflight.json",
        "source-bundle-preflight.json",
        "target-files-preflight.json",
        "host-process-preflight.json",
        "target-handles-preflight.json",
        "utmctl-list-once.json",
        "inventory-classification.json",
    ]
    for index in range(1, 4):
        names.extend(
            (
                f"host-process-quiescence-{index:03d}.json",
                f"target-handles-quiescence-{index:03d}.json",
            )
        )
    names.extend(("target-files-ready.json", "foreground-start-once.json"))
    for index in range(1, runtime_polls + 1):
        names.extend(
            (
                f"host-process-runtime-{index:03d}.json",
                f"target-handles-runtime-{index:03d}.json",
            )
        )
    names.append("target-handle-pid-discovery.json")
    for index in range(1, 4):
        names.extend(
            (
                f"target-handle-pid-confirmation-{index:03d}.json",
                f"host-process-confirmation-{index:03d}.json",
            )
        )
    names.append("target-files-started.json")
    for index in range(1, readiness_attempts + 1):
        names.extend(
            (
                f"guest-agent-readiness-{index:03d}.json",
                f"target-handle-readiness-{index:03d}.json",
                f"host-process-readiness-{index:03d}.json",
            )
        )
    names.extend(
        (
            "guest-agent-ready.json",
            "target-files-agent-ready.json",
            "guest-control-root-create.json",
            "guest-probe-push.json",
            "guest-probe-normalize.json",
            "guest-probe-readback.json",
            "guest-boot-transport-probe-once.json",
            "guest-marker-readback.json",
            "guest-result-readback-1.json",
            "guest-result-readback-2.json",
            "boot-classification.json",
            "target-handle-pid-terminal.json",
            "host-process-terminal.json",
            "target-files-postflight.json",
            "source-bundle-postflight.json",
            "terminal.json",
        )
    )
    return tuple(names)


def _validate_inventory_and_start(
    request: FreshBootClassificationResultBindingRequest,
    upstream: bindings.FreshBootClassificationBinding,
    root: Path,
) -> None:
    for name in ("host-process-preflight.json",) + tuple(
        f"host-process-quiescence-{index:03d}.json" for index in range(1, 4)
    ):
        _require_process_state(root / name)
    for name in ("target-handles-preflight.json",) + tuple(
        f"target-handles-quiescence-{index:03d}.json" for index in range(1, 4)
    ):
        _require_absent_handle(request, root / name)

    list_observation = launch_bindings._command_observation_from_json(
        network_ready._read_json(root / "utmctl-list-once.json"),
        "prior-fresh-boot-classification-list",
    )
    inventory = start_control.parse_utmctl_list(list_observation)
    baseline = legacy_result._baseline_inventory(upstream)
    if (
        control.boot_control.reactivation_control._require_inventory(
            inventory, baseline, request
        )
        != "stopped"
    ):
        raise ValueError("prior-fresh-boot-classification-inventory-invalid")
    if network_ready._read_json(root / "inventory-classification.json") != {
        "format": control.EVIDENCE_FORMAT,
        "other_registered_vm_count": 20,
        "other_registered_vms": "all-stopped",
        "registered_vm_count": 21,
        "target_registered_status": "stopped",
        "target_uuid": request.target_uuid,
    }:
        raise ValueError("prior-fresh-boot-classification-inventory-evidence-invalid")

    observation = launch_bindings._command_observation_from_json(
        network_ready._read_json(root / "foreground-start-once.json"),
        "prior-fresh-boot-classification-start",
    )
    legacy_result._require_successful_empty_observation(
        observation,
        launch_transport.transport_argv(request),
        "prior-fresh-boot-classification-start",
    )


def _validate_runtime(
    request: FreshBootClassificationResultBindingRequest,
    root: Path,
    backend_pid: int,
    runtime_polls: int,
) -> tuple[str, int]:
    for index in range(1, runtime_polls + 1):
        process_path = root / f"host-process-runtime-{index:03d}.json"
        if index == runtime_polls:
            _require_process_state(process_path)
        else:
            _require_runtime_process_state(process_path)
        handle_path = root / f"target-handles-runtime-{index:03d}.json"
        if index < runtime_polls:
            _require_absent_handle(request, handle_path)
        else:
            signature = _require_present_handle(
                request,
                handle_path,
                network_ready._lsof_argv(request),
                control.EVIDENCE_FORMAT,
                backend_pid=None,
            )

    discovery_signature = _require_present_handle(
        request,
        root / "target-handle-pid-discovery.json",
        network_ready._lsof_argv(request),
        runtime_bindings.EVIDENCE_FORMAT,
        backend_pid=backend_pid,
    )
    if signature != discovery_signature:
        raise ValueError("prior-fresh-boot-classification-handle-drift")

    targeted_argv = runtime_control.targeted_lsof_argv(request, backend_pid)
    for index in range(1, 4):
        if (
            _require_present_handle(
                request,
                root / f"target-handle-pid-confirmation-{index:03d}.json",
                targeted_argv,
                runtime_bindings.EVIDENCE_FORMAT,
                backend_pid=backend_pid,
            )
            != signature
        ):
            raise ValueError("prior-fresh-boot-classification-handle-drift")
        _require_process_state(
            root / f"host-process-confirmation-{index:03d}.json"
        )
    return signature


def _validate_readiness(
    request: FreshBootClassificationResultBindingRequest,
    root: Path,
    backend_pid: int,
    readiness_attempts: int,
    handle_signature: tuple[str, int],
) -> None:
    targeted_argv = runtime_control.targeted_lsof_argv(request, backend_pid)
    for index in range(1, readiness_attempts + 1):
        name = f"guest-agent-readiness-{index:03d}.json"
        observation = launch_bindings._command_observation_from_json(
            network_ready._read_json(root / name), f"prior-{name}"
        )
        if index < readiness_attempts:
            legacy_result._require_unavailable_readiness(
                observation, request, name
            )
        else:
            legacy_result._require_successful_empty_observation(
                observation, guest_control.readiness_argv(request), name
            )
        if (
            _require_present_handle(
                request,
                root / f"target-handle-readiness-{index:03d}.json",
                targeted_argv,
                control.EVIDENCE_FORMAT,
                backend_pid=backend_pid,
            )
            != handle_signature
        ):
            raise ValueError("prior-fresh-boot-classification-readiness-handle-drift")
        _require_process_state(root / f"host-process-readiness-{index:03d}.json")
    if network_ready._read_json(root / "guest-agent-ready.json") != {
        "attempt": readiness_attempts,
        "format": control.EVIDENCE_FORMAT,
        "predicate": "canonical-boot-id-readable",
        "state": "ready",
        "target_uuid": request.target_uuid,
    }:
        raise ValueError("prior-fresh-boot-classification-readiness-invalid")


def _validate_probe_and_result(
    request: FreshBootClassificationResultBindingRequest,
    upstream: bindings.FreshBootClassificationBinding,
    root: Path,
    backend_pid: int,
    handle_signature: tuple[str, int],
) -> str:
    control_root = request.guest_control_root
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
    for name, argv in observations:
        observation = launch_bindings._command_observation_from_json(
            network_ready._read_json(root / name), f"prior-{name}"
        )
        legacy_result._require_successful_empty_observation(observation, argv, name)

    probe_sha256 = network_ready._sha256_file(
        request.repository_root / bindings.PROBE_RELATIVE_PATH
    )
    probe_size = len(upstream.probe_bytes)
    readback = launch_bindings._command_observation_from_json(
        network_ready._read_json(root / "guest-probe-readback.json"),
        "prior-fresh-boot-probe-readback",
    )
    legacy_result._require_exact_readback(
        readback,
        (
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            f"{control_root}/boot-transport-probe.py",
        ),
        probe_sha256,
        probe_size,
        "prior-fresh-boot-probe-readback",
    )

    probe_value = network_ready._read_json(
        root / "guest-boot-transport-probe-once.json"
    )
    probe_observation = probe_value.get("observation")
    if (
        probe_value.get("format") != control.EVIDENCE_FORMAT
        or probe_value.get("raw_output_persisted") is not False
        or not isinstance(probe_observation, dict)
        or probe_observation.get("argv")
        != list(control.boot_control.probe_argv(request, upstream))
        or not _successful_empty_metadata(probe_observation)
    ):
        raise ValueError("prior-fresh-boot-classification-probe-invalid")

    marker = guest_probe.marker_bytes(bindings.REQUIRED_ATTEMPT_ID, probe_sha256)
    marker_observation = launch_bindings._command_observation_from_json(
        network_ready._read_json(root / "guest-marker-readback.json"),
        "prior-fresh-boot-marker-readback",
    )
    legacy_result._require_exact_payload(
        marker_observation,
        (
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            f"{control_root}/attempt.marker.json",
        ),
        marker,
        "prior-fresh-boot-marker-readback",
    )

    classification = network_ready._read_json(root / "boot-classification.json")
    observed = classification.get("observed_boot_id_sha256")
    if (
        not isinstance(observed, str)
        or not HEX_64.fullmatch(observed)
        or observed == upstream.expected_boot_id_sha256
        or classification
        != {
            "classification": "new-boot-started",
            "expected_boot_id_sha256": upstream.expected_boot_id_sha256,
            "format": control.EVIDENCE_FORMAT,
            "observed_boot_id_sha256": observed,
            "transport": "foreground-start-private-double-readback",
        }
    ):
        raise ValueError("prior-fresh-boot-classification-result-invalid")
    result = guest_probe.result_bytes(
        bindings.REQUIRED_ATTEMPT_ID,
        request.target_uuid,
        probe_sha256,
        observed,
    )
    for index in (1, 2):
        observation = launch_bindings._command_observation_from_json(
            network_ready._read_json(root / f"guest-result-readback-{index}.json"),
            f"prior-fresh-boot-result-readback-{index}",
        )
        legacy_result._require_exact_payload(
            observation,
            (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                f"{control_root}/boot-identity.evidence.json",
            ),
            result,
            f"prior-fresh-boot-result-readback-{index}",
        )

    targeted_argv = runtime_control.targeted_lsof_argv(request, backend_pid)
    if (
        _require_present_handle(
            request,
            root / "target-handle-pid-terminal.json",
            targeted_argv,
            runtime_bindings.EVIDENCE_FORMAT,
            backend_pid=backend_pid,
        )
        != handle_signature
    ):
        raise ValueError("prior-fresh-boot-classification-terminal-handle-drift")
    _require_process_state(root / "host-process-terminal.json")
    return observed


def _validate_terminal(
    request: FreshBootClassificationResultBindingRequest,
    root: Path,
    terminal: dict[str, object],
    backend_pid: int,
    runtime_polls: int,
    readiness_attempts: int,
    observed_boot_id_sha256: str,
) -> None:
    expected = {
        "agent_readiness_invocations": readiness_attempts,
        "agent_readiness_state": "ready",
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": backend_pid,
        "boot_classification": "new-boot-started",
        "boot_start_attempt_id": bindings.REQUIRED_ATTEMPT_ID,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 4,
        "file_push_invocations": 1,
        "foreground_start_invocations": 1,
        "format": control.EVIDENCE_FORMAT,
        "guest_exec_invocations": readiness_attempts + 3,
        "guest_probe_invocations": 1,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 1,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": observed_boot_id_sha256,
        "operation_id": "not-read-or-generated",
        "outcome": "new-boot-started",
        "plain_utmctl_list": "attempted-once-as-potential-backend-reactivation",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "readiness_identity_observation_count": readiness_attempts,
        "reason": (
            "single-start-private-double-readback-new-boot-"
            "and-terminal-target-stable"
        ),
        "result_readback_invocations": 2,
        "runtime_poll_count": runtime_polls,
        "target_handles_terminal": "present",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }
    if terminal != expected:
        raise ValueError("prior-fresh-boot-classification-terminal-invalid")


def _require_process_state(path: Path) -> None:
    runtime_bindings._require_process_state(
        path,
        0,
        expected_format=runtime_bindings.EVIDENCE_FORMAT,
        error_label="prior-fresh-boot-classification",
    )


def _require_runtime_process_state(path: Path) -> None:
    value = network_ready._read_json(path)
    observation = value.get("observation")
    processes = value.get("relevant_processes")
    if (
        value.get("format") != runtime_bindings.EVIDENCE_FORMAT
        or not isinstance(processes, list)
        or value.get("relevant_process_count") != len(processes)
        or any(item.get("role") != "utmctl" for item in processes if isinstance(item, dict))
        or not all(isinstance(item, dict) for item in processes)
        or not isinstance(observation, dict)
        or observation.get("argv") != list(launch_transport.PROCESS_COMMAND)
        or not _successful_metadata(observation)
    ):
        raise ValueError("prior-fresh-boot-classification-runtime-process-invalid")


def _require_absent_handle(
    request: FreshBootClassificationResultBindingRequest, path: Path
) -> None:
    value = network_ready._read_json(path)
    observation = value.get("observation")
    if (
        value.get("format") != control.EVIDENCE_FORMAT
        or value.get("state") != "absent"
        or value.get("backend_command") is not None
        or value.get("backend_pid") is not None
        or value.get("efi_handle_count") != 0
        or value.get("process_record_count") != 0
        or value.get("qcow2_handle_count") != 0
        or not isinstance(observation, dict)
        or observation.get("argv") != list(network_ready._lsof_argv(request))
        or observation.get("exit_code") != 1
        or observation.get("timed_out") is not False
        or not runtime_bindings._empty_stream(observation.get("stdout"))
        or not runtime_bindings._empty_stream(observation.get("stderr"))
    ):
        raise ValueError("prior-fresh-boot-classification-absent-handle-invalid")


def _require_present_handle(
    request: FreshBootClassificationResultBindingRequest,
    path: Path,
    expected_argv: tuple[str, ...],
    expected_format: str,
    *,
    backend_pid: int | None,
) -> tuple[str, int]:
    del request
    value = network_ready._read_json(path)
    observation = value.get("observation")
    stdout = observation.get("stdout") if isinstance(observation, dict) else None
    backend_valid = (
        "backend_pid" not in value
        if backend_pid is None
        else value.get("backend_pid") == backend_pid
    )
    if (
        value.get("format") != expected_format
        or value.get("state") != "present"
        or value.get("backend_command") != "QEMULauncher"
        or not backend_valid
        or value.get("efi_handle_count") != 1
        or value.get("process_record_count") != 1
        or value.get("qcow2_handle_count") != 1
        or not isinstance(observation, dict)
        or observation.get("argv") != list(expected_argv)
        or observation.get("exit_code") != 0
        or observation.get("timed_out") is not False
        or not runtime_bindings._empty_stream(observation.get("stderr"))
        or not runtime_bindings._complete_stream(stdout)
        or not isinstance(stdout, dict)
        or not isinstance(stdout.get("sha256"), str)
        or not HEX_64.fullmatch(str(stdout.get("sha256")))
        or not isinstance(stdout.get("total_bytes"), int)
        or int(stdout["total_bytes"]) <= 0
    ):
        raise ValueError("prior-fresh-boot-classification-present-handle-invalid")
    return str(stdout["sha256"]), int(stdout["total_bytes"])


def _successful_metadata(value: dict[str, object]) -> bool:
    return (
        value.get("exit_code") == 0
        and value.get("timed_out") is False
        and runtime_bindings._complete_stream(value.get("stdout"))
        and runtime_bindings._empty_stream(value.get("stderr"))
    )


def _successful_empty_metadata(value: dict[str, object]) -> bool:
    return (
        value.get("exit_code") == 0
        and value.get("timed_out") is False
        and runtime_bindings._empty_stream(value.get("stdout"))
        and runtime_bindings._empty_stream(value.get("stderr"))
    )


class _PriorFreshBootClassificationRequestView:
    def __init__(
        self, request: FreshBootClassificationResultBindingRequest
    ) -> None:
        self._request = request

    @property
    def guest_control_root(self) -> str:
        return (
            "/var/tmp/radishlex-l6-v4-boot-start-"
            + bindings.REQUIRED_ATTEMPT_ID
        )

    @property
    def guest_probe_incoming(self) -> str:
        return f"{self.guest_control_root}/boot-transport-probe.incoming.py"

    @property
    def guest_probe_path(self) -> str:
        return f"{self.guest_control_root}/boot-transport-probe.py"

    @property
    def guest_marker_path(self) -> str:
        return f"{self.guest_control_root}/attempt.marker.json"

    @property
    def guest_result_path(self) -> str:
        return f"{self.guest_control_root}/boot-identity.evidence.json"

    def __getattr__(self, name: str) -> object:
        return getattr(self._request, name)

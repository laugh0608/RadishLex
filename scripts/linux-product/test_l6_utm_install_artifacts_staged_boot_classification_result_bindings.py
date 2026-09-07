#!/usr/bin/env python3
from __future__ import annotations

import contextlib
import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_boot_classification_result_bindings as bindings
import l6_utm_start_once as start_control
import test_l6_utm_install_artifacts_staged_boot_classification_resolution as classification_test


class BootClassificationResultBindingTests(unittest.TestCase):
    def test_inventory_baseline_uses_reactivation_binding(self) -> None:
        upstream = make_upstream()

        baseline = bindings._baseline_inventory(upstream)

        self.assertEqual(len(baseline), 20)
        self.assertEqual(baseline[0].uuid, "uuid-0")

    def test_first_runtime_handle_precedes_pid_discovery(self) -> None:
        argv = ("/usr/sbin/lsof", "synthetic-target")
        value = present_handle(argv, include_backend_pid=False)

        bindings._require_present_handle(
            value,
            argv,
            bindings.classification_bindings.EVIDENCE_FORMAT,
            expected_backend_pid=None,
        )

        value["backend_pid"] = bindings.REQUIRED_BACKEND_PID
        with self.assertRaisesRegex(ValueError, "present-handle-invalid"):
            bindings._require_present_handle(
                value,
                argv,
                bindings.classification_bindings.EVIDENCE_FORMAT,
                expected_backend_pid=None,
            )

    def test_manifest_binds_all_seventy_two_members_before_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_boot_classification_root
            root.mkdir(mode=0o700)
            for name in bindings.REQUIRED_ENTRY_NAMES:
                path = root / name
                path.write_bytes((name + "\n").encode("ascii"))
                path.chmod(0o600)
            manifest = root / "files.sha256"
            manifest.write_text(
                "".join(
                    f"{sha256(root / name)}  {name}\n"
                    for name in bindings.REQUIRED_ENTRY_NAMES
                ),
                encoding="ascii",
            )
            manifest.chmod(0o600)
            manifest_sha256 = sha256(manifest)
            request.prior_boot_classification_manifest_sha256 = manifest_sha256
            upstream = make_upstream()
            semantic_names = (
                "_validate_prior_request",
                "_validate_prior_binding",
                "_validate_prior_source_target",
                "_validate_prior_inventory_and_start",
                "_validate_prior_runtime",
                "_validate_prior_readiness",
                "_validate_prior_probe_and_result",
                "_validate_prior_terminal",
            )
            with contextlib.ExitStack() as stack:
                stack.enter_context(
                    mock.patch.object(
                        bindings,
                        "REQUIRED_PRIOR_MANIFEST_SHA256",
                        manifest_sha256,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        bindings.classification_bindings,
                        "validate_boot_classification_bindings",
                        return_value=upstream,
                    )
                )
                for name in semantic_names:
                    stack.enter_context(mock.patch.object(bindings, name))
                result = bindings.validate_boot_classification_result_bindings(
                    request
                )

            self.assertEqual(
                result.evidence[
                    "prior_boot_classification_entries_verified"
                ],
                72,
            )
            self.assertEqual(
                result.evidence["prior_boot_classification_outcome"],
                "new-boot-started",
            )

    def test_prior_request_rejects_authorization_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_boot_classification_root
            root.mkdir(mode=0o700)
            write_json(root / "request.json", prior_request(request))

            bindings._validate_prior_request(request, root)

            value = read_json(root / "request.json")
            value["authorization"][
                "bounded_read_only_guest_agent_readiness"
            ] = False
            write_json(root / "request.json", value)
            with self.assertRaisesRegex(
                ValueError, "prior-boot-classification-request-invalid"
            ):
                bindings._validate_prior_request(request, root)

    def test_readiness_requires_nine_unavailable_then_ready(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_boot_classification_root
            root.mkdir(mode=0o700)
            write_readiness(request, root)

            with (
                mock.patch.object(bindings, "_require_present_handle"),
                mock.patch.object(bindings, "_require_process_state"),
            ):
                bindings._validate_prior_readiness(request, root)

            value = read_json(root / "guest-agent-readiness-009.json")
            value["stderr"] = empty_stream()
            write_json(root / "guest-agent-readiness-009.json", value)
            with (
                mock.patch.object(bindings, "_require_present_handle"),
                mock.patch.object(bindings, "_require_process_state"),
                self.assertRaisesRegex(ValueError, "guest-agent-readiness-009"),
            ):
                bindings._validate_prior_readiness(request, root)

    def test_fixed_scope_probe_and_double_result_are_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_boot_classification_root
            root.mkdir(mode=0o700)
            probe_bytes = b"synthetic fixed-scope probe\n"
            probe_sha256 = hashlib.sha256(probe_bytes).hexdigest()
            write_probe_result(request, root, probe_bytes)

            with (
                mock.patch.object(
                    bindings, "REQUIRED_PROBE_SHA256", probe_sha256
                ),
                mock.patch.object(
                    bindings, "REQUIRED_PROBE_SIZE", len(probe_bytes)
                ),
            ):
                bindings._validate_prior_probe_and_result(request, root)

            value = read_json(root / "guest-boot-transport-probe-once.json")
            value["observation"]["argv"][
                value["observation"]["argv"].index("boot-start")
            ] = "guest-agent"
            write_json(root / "guest-boot-transport-probe-once.json", value)
            with (
                mock.patch.object(
                    bindings, "REQUIRED_PROBE_SHA256", probe_sha256
                ),
                mock.patch.object(
                    bindings, "REQUIRED_PROBE_SIZE", len(probe_bytes)
                ),
                self.assertRaisesRegex(ValueError, "probe-exec-invalid"),
            ):
                bindings._validate_prior_probe_and_result(request, root)

    def test_terminal_freezes_new_boot_and_no_automatic_action(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_boot_classification_root
            root.mkdir(mode=0o700)
            write_json(root / "terminal.json", prior_terminal(request))

            bindings._validate_prior_terminal(request, root)

            value = read_json(root / "terminal.json")
            value["maintenance_resume_invocations"] = 1
            write_json(root / "terminal.json", value)
            with self.assertRaisesRegex(
                ValueError, "prior-boot-classification-terminal-invalid"
            ):
                bindings._validate_prior_terminal(request, root)


def make_request(root: Path) -> SimpleNamespace:
    base = classification_test.make_request(root)
    values = dict(base.__dict__)
    values.update(
        expected_repository_head="9" * 40,
        prior_boot_classification_root=root / "prior-boot-classification",
        prior_boot_classification_manifest_sha256=(
            bindings.REQUIRED_PRIOR_MANIFEST_SHA256
        ),
        prior_boot_classification_attempt_id=(
            bindings.REQUIRED_PRIOR_ATTEMPT_ID
        ),
    )
    return SimpleNamespace(**values)


def make_upstream() -> SimpleNamespace:
    baseline = tuple(
        SimpleNamespace(uuid=f"uuid-{index}", name=f"vm-{index}", status="stopped")
        for index in range(20)
    )
    return SimpleNamespace(
        expected_boot_id_sha256=bindings.REQUIRED_EXPECTED_BOOT_ID_SHA256,
        probe_bytes=b"current probe",
        upstream=SimpleNamespace(
            upstream=SimpleNamespace(
                upstream=SimpleNamespace(baseline_inventory=baseline)
            )
        ),
    )


def present_handle(
    argv: tuple[str, ...], *, include_backend_pid: bool
) -> dict[str, object]:
    value: dict[str, object] = {
        "backend_command": "QEMULauncher",
        "efi_handle_count": 1,
        "format": bindings.classification_bindings.EVIDENCE_FORMAT,
        "observation": {
            "argv": list(argv),
            "exit_code": 0,
            "stderr": empty_stream(),
            "stdout": {
                "sha256": bindings.REQUIRED_HANDLE_STDOUT_SHA256,
                "total_bytes": 378,
                "truncated": False,
            },
            "timed_out": False,
        },
        "process_record_count": 1,
        "qcow2_handle_count": 1,
        "state": "present",
    }
    if include_backend_pid:
        value["backend_pid"] = bindings.REQUIRED_BACKEND_PID
    return value


def prior_request(request: SimpleNamespace) -> dict[str, object]:
    return {
        "agent_readiness_attempts": 60,
        "authorization": {
            "bound_prior_guest_agent_result": True,
            "bounded_read_only_guest_agent_readiness": True,
            "bounded_target_runtime_observation": True,
            "install_artifacts_staged_boot_classification_resolution": True,
            "install_artifacts_staged_boot_start_resolution": True,
            "no_status_resume_business_guest_retry_stop_or_quit": True,
            "one_foreground_start_from_stopped": True,
            "one_potential_backend_reactivation_list": True,
            "one_private_guest_probe_delivery_and_execution": True,
            "two_independent_result_readbacks": True,
        },
        "boot_classification_attempt_id": bindings.REQUIRED_PRIOR_ATTEMPT_ID,
        "boot_start_attempt_id": bindings.REQUIRED_PRIOR_ATTEMPT_ID,
        "command_timeout_seconds": 60,
        "expected_repository_head": bindings.REQUIRED_PRIOR_REPOSITORY_HEAD,
        "expected_vm_count": 21,
        "format": bindings.classification_bindings.EVIDENCE_FORMAT,
        "guest_agent_attempt_id": request.guest_agent_attempt_id,
        "guest_control_root": bindings._guest_control_root(),
        "identity_observations": 3,
        "poll_interval_seconds": 1,
        "prior_boot_start_attempt_id": request.prior_boot_start_attempt_id,
        "prior_boot_start_manifest_sha256": (
            request.prior_boot_start_manifest_sha256
        ),
        "prior_boot_transport_attempt_id": (
            request.prior_boot_transport_attempt_id
        ),
        "prior_boot_transport_manifest_sha256": (
            request.prior_boot_transport_manifest_sha256
        ),
        "prior_guest_agent_attempt_id": request.prior_guest_agent_attempt_id,
        "prior_guest_agent_manifest_sha256": (
            request.prior_guest_agent_manifest_sha256
        ),
        "prior_runtime_resolution_v2_manifest_sha256": (
            request.prior_runtime_resolution_v2_manifest_sha256
        ),
        "quiescence_observations": 3,
        "runtime_poll_attempts": 60,
        "source_bundle_path_sha256": bindings.network_ready._sha256_text(
            str(request.source_bundle_path)
        ),
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_package_path_sha256": bindings.network_ready._sha256_text(
            str(request.target_package_path)
        ),
        "target_uuid": request.target_uuid,
        "transport_id": bindings.launch_transport.TRANSPORT_ID,
        "transport_timeout_seconds": 60,
    }


def write_readiness(request: SimpleNamespace, root: Path) -> None:
    targeted = bindings.runtime_control.targeted_lsof_argv(
        request, bindings.REQUIRED_BACKEND_PID
    )
    for index in range(1, 11):
        stderr = (
            bindings.guest_bindings.REQUIRED_AGENT_UNAVAILABLE_STDERR
            if index < 10
            else b""
        )
        write_json(
            root / f"guest-agent-readiness-{index:03d}.json",
            observation(
                bindings.guest_control.readiness_argv(request), stderr=stderr
            ).as_json(),
        )
        write_json(
            root / f"target-handle-readiness-{index:03d}.json",
            {"observation": {"argv": list(targeted)}},
        )
        write_json(
            root / f"host-process-readiness-{index:03d}.json", {}
        )
    write_json(
        root / "guest-agent-ready.json",
        {
            "attempt": 10,
            "format": bindings.classification_bindings.EVIDENCE_FORMAT,
            "predicate": "canonical-boot-id-readable",
            "state": "ready",
            "target_uuid": request.target_uuid,
        },
    )


def write_probe_result(
    request: SimpleNamespace, root: Path, probe_bytes: bytes
) -> None:
    control_root = bindings._guest_control_root()
    probe_sha256 = hashlib.sha256(probe_bytes).hexdigest()
    empty_observations = {
        "guest-control-root-create.json": (
            "utmctl",
            "exec",
            request.target_uuid,
            "--cmd",
            "/bin/mkdir",
            "-m",
            "0700",
            control_root,
        ),
        "guest-probe-push.json": (
            "utmctl",
            "file",
            "push",
            request.target_uuid,
            f"{control_root}/boot-transport-probe.incoming.py",
        ),
        "guest-probe-normalize.json": (
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
    }
    for name, argv in empty_observations.items():
        write_json(root / name, observation(argv).as_json())
    write_json(
        root / "guest-probe-readback.json",
        observation(
            (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                f"{control_root}/boot-transport-probe.py",
            ),
            stdout=probe_bytes,
        ).as_json(),
    )
    with mock.patch.object(bindings, "REQUIRED_PROBE_SHA256", probe_sha256):
        probe_argv = bindings._probe_argv(request, control_root)
    write_json(
        root / "guest-boot-transport-probe-once.json",
        {
            "format": bindings.classification_bindings.EVIDENCE_FORMAT,
            "observation": observation(probe_argv).as_json(),
            "raw_output_persisted": False,
        },
    )
    marker = bindings.guest_probe.marker_bytes(
        bindings.REQUIRED_PRIOR_ATTEMPT_ID, probe_sha256
    )
    write_json(
        root / "guest-marker-readback.json",
        observation(
            (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                f"{control_root}/attempt.marker.json",
            ),
            stdout=marker,
        ).as_json(),
    )
    result = bindings.guest_probe.result_bytes(
        bindings.REQUIRED_PRIOR_ATTEMPT_ID,
        request.target_uuid,
        probe_sha256,
        bindings.REQUIRED_OBSERVED_BOOT_ID_SHA256,
    )
    for index in (1, 2):
        write_json(
            root / f"guest-result-readback-{index}.json",
            observation(
                (
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    f"{control_root}/boot-identity.evidence.json",
                ),
                stdout=result,
            ).as_json(),
        )
    write_json(
        root / "boot-classification.json",
        {
            "classification": "new-boot-started",
            "expected_boot_id_sha256": (
                bindings.REQUIRED_EXPECTED_BOOT_ID_SHA256
            ),
            "format": bindings.classification_bindings.EVIDENCE_FORMAT,
            "observed_boot_id_sha256": (
                bindings.REQUIRED_OBSERVED_BOOT_ID_SHA256
            ),
            "transport": "foreground-start-private-double-readback",
        },
    )


def prior_terminal(request: SimpleNamespace) -> dict[str, object]:
    return {
        "agent_readiness_invocations": 10,
        "agent_readiness_state": "ready",
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": bindings.REQUIRED_BACKEND_PID,
        "boot_classification": "new-boot-started",
        "boot_start_attempt_id": bindings.REQUIRED_PRIOR_ATTEMPT_ID,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 4,
        "file_push_invocations": 1,
        "foreground_start_invocations": 1,
        "format": bindings.classification_bindings.EVIDENCE_FORMAT,
        "guest_exec_invocations": 13,
        "guest_probe_invocations": 1,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 1,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": bindings.REQUIRED_OBSERVED_BOOT_ID_SHA256,
        "operation_id": "not-read-or-generated",
        "outcome": "new-boot-started",
        "plain_utmctl_list": "attempted-once-as-potential-backend-reactivation",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "readiness_identity_observation_count": 10,
        "reason": (
            "single-start-private-double-readback-new-boot-"
            "and-terminal-target-stable"
        ),
        "result_readback_invocations": 2,
        "runtime_poll_count": 1,
        "target_handles_terminal": "present",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }


def observation(
    argv: tuple[str, ...], *, stdout: bytes = b"", stderr: bytes = b""
) -> start_control.CommandObservation:
    return start_control.CommandObservation.from_bytes(
        argv, stdout=stdout, stderr=stderr
    )


def empty_stream() -> dict[str, object]:
    return {
        "prefix_base64": "",
        "prefix_utf8": "",
        "sha256": hashlib.sha256(b"").hexdigest(),
        "total_bytes": 0,
        "truncated": False,
    }


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def write_json(path: Path, value: dict[str, object]) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=True, sort_keys=True), encoding="utf-8"
    )
    os.chmod(path, 0o600)


if __name__ == "__main__":
    unittest.main()

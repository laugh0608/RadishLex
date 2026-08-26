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

import l6_utm_install_artifacts_staged_guest_agent_result_bindings as bindings
import l6_utm_start_once as start_control
import test_l6_utm_install_artifacts_staged_guest_agent_resolution as guest_test


EXPECTED_BOOT_HASH = (
    "18f1ba063ecd3087a5624e6ae52a54624540a972df33cca00a34bca4ec00022a"
)
SOURCE_PATH = Path(
    "/Users/luobo/VirtualMachines/"
    "RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v4-"
    "Canonical-Input-v1/canonical-input.ustar"
)
TARGET_PATH = Path(
    "/Users/luobo/Library/Containers/com.utmapp.UTM/Data/Documents/"
    "RadishLex-Debian13-ARM64-L6-d75818f-crash-"
    "install-artifacts-staged-v4.utm"
)


class GuestAgentResultBindingTests(unittest.TestCase):
    def test_manifest_binds_all_twenty_six_members_before_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_guest_agent_root
            root.mkdir(mode=0o700)
            for name in bindings.REQUIRED_PRIOR_GUEST_AGENT_ENTRY_NAMES:
                path = root / name
                path.write_bytes((name + "\n").encode("ascii"))
                path.chmod(0o600)
            manifest = root / "files.sha256"
            manifest.write_text(
                "".join(
                    f"{sha256(root / name)}  {name}\n"
                    for name in bindings.REQUIRED_PRIOR_GUEST_AGENT_ENTRY_NAMES
                ),
                encoding="ascii",
            )
            manifest.chmod(0o600)
            manifest_sha256 = sha256(manifest)
            request.prior_guest_agent_manifest_sha256 = manifest_sha256
            upstream = SimpleNamespace(
                expected_boot_id_sha256=EXPECTED_BOOT_HASH,
                probe_bytes=b"current fixed probe",
            )

            semantic_patches = [
                mock.patch.object(bindings, name)
                for name in (
                    "_validate_prior_request",
                    "_validate_prior_binding",
                    "_validate_prior_source_and_target",
                    "_validate_prior_readiness",
                    "_validate_prior_probe_delivery",
                    "_validate_prior_marker_failure",
                    "_validate_prior_terminal",
                )
            ]
            with contextlib.ExitStack() as stack:
                stack.enter_context(
                    mock.patch.object(
                        bindings,
                        "REQUIRED_PRIOR_GUEST_AGENT_MANIFEST_SHA256",
                        manifest_sha256,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        bindings.guest_bindings,
                        "validate_guest_agent_bindings",
                        return_value=upstream,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        bindings,
                        "_validate_prior_host_identity",
                        return_value=bindings.REQUIRED_HANDLE_STDOUT_SHA256,
                    )
                )
                for semantic_patch in semantic_patches:
                    stack.enter_context(semantic_patch)
                result = bindings.validate_guest_agent_result_bindings(request)

            self.assertEqual(
                result.evidence["prior_guest_agent_entries_verified"], 26
            )
            self.assertEqual(
                result.evidence["prior_guest_agent_marker"],
                "absent-after-probe-exec",
            )

    def test_prior_request_is_exact_and_rejects_authorization_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_guest_agent_root
            root.mkdir(mode=0o700)
            value = prior_request(request)
            write_json(root / "request.json", value)

            bindings._validate_prior_request(request, root)

            value["authorization"]["two_independent_result_readbacks"] = False
            write_json(root / "request.json", value)
            with self.assertRaisesRegex(
                ValueError, "prior-guest-agent-request-invalid"
            ):
                bindings._validate_prior_request(request, root)

    def test_legacy_probe_cli_and_marker_absence_are_bound_exactly(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_guest_agent_root
            root.mkdir(mode=0o700)
            probe_bytes = b"synthetic prior committed probe\n"
            probe_sha256 = hashlib.sha256(probe_bytes).hexdigest()
            write_probe_delivery(request, root, probe_bytes)
            write_marker_failure(request, root)

            with (
                mock.patch.object(
                    bindings, "REQUIRED_PRIOR_PROBE_SHA256", probe_sha256
                ),
                mock.patch.object(
                    bindings, "REQUIRED_PRIOR_PROBE_SIZE", len(probe_bytes)
                ),
            ):
                bindings._validate_prior_probe_delivery(request, root)
            bindings._validate_prior_marker_failure(request, root)

            value = read_json(root / "guest-boot-transport-probe-once.json")
            value["observation"]["argv"].extend(
                ["--control-scope", "guest-agent"]
            )
            write_json(root / "guest-boot-transport-probe-once.json", value)
            with (
                mock.patch.object(
                    bindings, "REQUIRED_PRIOR_PROBE_SHA256", probe_sha256
                ),
                mock.patch.object(
                    bindings, "REQUIRED_PRIOR_PROBE_SIZE", len(probe_bytes)
                ),
                self.assertRaisesRegex(
                    ValueError, "prior-guest-agent-probe-exec-invalid"
                ),
            ):
                bindings._validate_prior_probe_delivery(request, root)

    def test_terminal_freezes_no_result_and_no_automatic_action(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            root = request.prior_guest_agent_root
            root.mkdir(mode=0o700)
            value = prior_terminal(request)
            write_json(root / "terminal.json", value)

            bindings._validate_prior_terminal(request, root)

            value["result_readback_invocations"] = 1
            write_json(root / "terminal.json", value)
            with self.assertRaisesRegex(
                ValueError, "prior-guest-agent-terminal-invalid"
            ):
                bindings._validate_prior_terminal(request, root)


def make_request(root: Path) -> SimpleNamespace:
    base = guest_test.make_request(root)
    values = dict(base.__dict__)
    values.update(
        expected_repository_head="8" * 40,
        source_bundle_path=SOURCE_PATH,
        target_package_path=TARGET_PATH,
        prior_guest_agent_root=root / "prior-guest-agent",
        prior_guest_agent_manifest_sha256=(
            bindings.REQUIRED_PRIOR_GUEST_AGENT_MANIFEST_SHA256
        ),
        prior_guest_agent_attempt_id=(
            bindings.REQUIRED_PRIOR_GUEST_AGENT_ATTEMPT_ID
        ),
    )
    return SimpleNamespace(**values)


def prior_request(request: SimpleNamespace) -> dict[str, object]:
    return {
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
        "expected_repository_head": (
            bindings.REQUIRED_PRIOR_GUEST_AGENT_REPOSITORY_HEAD
        ),
        "format": bindings.guest_bindings.EVIDENCE_FORMAT,
        "guest_agent_attempt_id": request.prior_guest_agent_attempt_id,
        "guest_control_root": bindings._guest_control_root(request),
        "identity_observations": 3,
        "poll_interval_seconds": 1,
        "prior_boot_start_attempt_id": request.prior_boot_start_attempt_id,
        "prior_boot_start_manifest_sha256": (
            request.prior_boot_start_manifest_sha256
        ),
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
    }


def write_probe_delivery(
    request: SimpleNamespace, root: Path, probe_bytes: bytes
) -> None:
    control_root = bindings._guest_control_root(request)
    probe_sha256 = hashlib.sha256(probe_bytes).hexdigest()
    observations = {
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
    for name, argv in observations.items():
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
    write_json(
        root / "guest-boot-transport-probe-once.json",
        {
            "format": bindings.guest_bindings.EVIDENCE_FORMAT,
            "observation": stream_observation(
                (
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
                    probe_sha256,
                    "--control-root",
                    control_root,
                )
            ),
            "raw_output_persisted": False,
        },
    )


def write_marker_failure(request: SimpleNamespace, root: Path) -> None:
    control_root = bindings._guest_control_root(request)
    stderr = (
        "Error from event: The operation couldn’t be completed. "
        "(OSStatus error -2700.)\n"
        f"failed to open file '{control_root}/attempt.marker.json' "
        "(mode: 'r'): No such file or directory\n"
    ).encode("utf-8")
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
            stderr=stderr,
        ).as_json(),
    )


def prior_terminal(request: SimpleNamespace) -> dict[str, object]:
    return {
        "agent_readiness_invocations": 1,
        "agent_readiness_state": "ready",
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": bindings.REQUIRED_BACKEND_PID,
        "boot_classification": "not-generated",
        "business_guest_action": "not-performed",
        "file_pull_invocations": 2,
        "file_push_invocations": 1,
        "format": bindings.guest_bindings.EVIDENCE_FORMAT,
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


def observation(
    argv: tuple[str, ...], *, stdout: bytes = b"", stderr: bytes = b""
) -> start_control.CommandObservation:
    return start_control.CommandObservation.from_bytes(
        argv, stdout=stdout, stderr=stderr
    )


def stream_observation(argv: tuple[str, ...]) -> dict[str, object]:
    empty = {
        "sha256": hashlib.sha256(b"").hexdigest(),
        "total_bytes": 0,
        "truncated": False,
    }
    return {
        "argv": list(argv),
        "exit_code": 0,
        "stderr": dict(empty),
        "stdout": dict(empty),
        "timed_out": False,
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

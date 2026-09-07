#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace

import l6_utm_install_artifacts_staged_guest_agent_bindings as bindings
import l6_utm_install_artifacts_staged_guest_agent_resolution as resolution
import l6_utm_start_once as start_control
import l6_v4_boot_transport_probe as guest_probe
import test_l6_utm_install_artifacts_staged_boot_start_resolution as boot_test
import test_l6_utm_install_artifacts_staged_runtime_resolution as runtime_test


ORIGINAL_BOOT_HASH = hashlib.sha256(b"synthetic-original-boot").hexdigest()
NEW_BOOT_HASH = hashlib.sha256(b"synthetic-new-boot").hexdigest()


@dataclass
class SyntheticSource:
    file_object: object

    def as_json(self) -> dict[str, object]:
        return {
            "format": "synthetic-source-v1",
            "sha256": resolution.reactivation_control.REQUIRED_SOURCE_BUNDLE_SHA256,
            "size": resolution.reactivation_control.REQUIRED_SOURCE_BUNDLE_SIZE,
        }


class CloseTracker:
    def __init__(self) -> None:
        self.closed = False

    def close(self) -> None:
        self.closed = True


class FakeRunner:
    def __init__(
        self,
        request: resolution.GuestAgentResolutionRequest,
        probe_bytes: bytes,
        *,
        readiness_states: list[str] | None = None,
        handle_pids: list[int] | None = None,
        boot_hash: str = NEW_BOOT_HASH,
        result_payloads: list[bytes | None] | None = None,
    ) -> None:
        self.request = request
        self.probe_bytes = probe_bytes
        self.probe_sha256 = hashlib.sha256(probe_bytes).hexdigest()
        self.readiness_states = list(readiness_states or ["unavailable", "ready"])
        self.handle_pids = list(handle_pids or [])
        self.boot_hash = boot_hash
        self.result_payloads = list(result_payloads or [])
        self.calls: list[tuple[str, ...]] = []
        self.incoming_probe: bytes | None = None
        self.installed_probe: bytes | None = None

    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file=None,
    ) -> start_control.CommandObservation:
        del timeout_seconds
        self.calls.append(argv)
        if argv == resolution.runtime_control.launch_transport.PROCESS_COMMAND:
            return observation(argv, stdout=runtime_test.quiet())
        if argv[0:1] == ("/usr/sbin/lsof",):
            pid = self.handle_pids.pop(0) if self.handle_pids else 42
            return observation(
                argv,
                stdout=runtime_test.handle_payload(
                    self.request, pid, "QEMULauncher"
                ),
            )
        if argv == resolution.readiness_argv(self.request):
            if not self.readiness_states:
                raise AssertionError("unexpected readiness call")
            state = self.readiness_states.pop(0)
            if state == "ready":
                return observation(argv)
            if state == "unavailable":
                return observation(
                    argv, stderr=bindings.REQUIRED_AGENT_UNAVAILABLE_STDERR
                )
            if state == "unknown":
                return observation(argv, exit_code=1)
            raise AssertionError(f"unknown synthetic readiness: {state}")
        if argv[0:3] == ("utmctl", "file", "push"):
            if stdin_file is None:
                raise AssertionError("probe push missing stdin")
            self.incoming_probe = stdin_file.read()
            return observation(argv)
        if argv[0:3] == ("utmctl", "file", "pull"):
            path = argv[4]
            if path == self.request.guest_probe_path:
                payload = self.installed_probe
            elif path == self.request.guest_marker_path:
                payload = guest_probe.marker_bytes(
                    self.request.guest_agent_attempt_id,
                    self.probe_sha256,
                )
            elif path == self.request.guest_result_path:
                if self.result_payloads:
                    payload = self.result_payloads.pop(0)
                else:
                    payload = guest_probe.result_bytes(
                        self.request.guest_agent_attempt_id,
                        self.request.target_uuid,
                        self.probe_sha256,
                        self.boot_hash,
                    )
            else:
                raise AssertionError(f"unexpected pull path: {path}")
            if payload is None:
                return observation(argv, exit_code=1)
            return observation(argv, stdout=payload)
        if argv[0:3] == ("utmctl", "exec", self.request.target_uuid):
            if "/usr/bin/install" in argv:
                self.installed_probe = self.incoming_probe
                return observation(argv)
            if self.request.guest_probe_path in argv:
                return observation(argv)
            if "/bin/mkdir" in argv:
                return observation(argv)
        raise AssertionError(f"unexpected command: {argv}")


class GuestAgentResolutionTests(unittest.TestCase):
    def test_probe_cli_uses_guest_agent_scope_root_contract(self) -> None:
        request = make_request(Path("/tmp/radishlex-guest-agent-cli-test"))
        argv = resolution.probe_argv(request, make_binding())

        args = guest_probe.parse_args(argv[8:])

        self.assertEqual(
            args.control_scope,
            guest_probe.CONTROL_SCOPE_GUEST_AGENT,
        )
        self.assertEqual(args.control_root, Path(request.guest_control_root))
        self.assertEqual(
            guest_probe.control_root_for(
                args.control_scope,
                args.attempt_id,
            ),
            args.control_root,
        )

    def test_delayed_agent_ready_classifies_new_boot(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(request, binding.probe_bytes)

            result, source = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "new-boot-started")
            self.assertEqual(result.exit_code, resolution.EXIT_NEW_BOOT_STARTED)
            self.assertEqual(result.agent_readiness_invocations, 2)
            self.assertEqual(result.guest_probe_invocations, 1)
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertTrue(source.file_object.closed)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["guest_exec_invocations"], 5)
            self.assertEqual(terminal["file_pull_invocations"], 4)
            self.assertEqual(terminal["agent_readiness_state"], "ready")
            self.assert_forbidden_actions_absent(runner.calls)
            runtime_test.assert_manifest_valid(self, request.output_root)

    def test_original_boot_is_separate_outcome(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                readiness_states=["ready"],
                boot_hash=ORIGINAL_BOOT_HASH,
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "original-boot-restored")
            self.assertEqual(
                result.exit_code, resolution.EXIT_ORIGINAL_BOOT_RESTORED
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_agent_never_ready_exhausts_budget_without_probe(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                readiness_states=["unavailable"] * 60,
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, resolution.EXIT_STATE_INDETERMINATE)
            self.assertEqual(result.agent_readiness_invocations, 60)
            self.assertEqual(result.guest_probe_invocations, 0)
            self.assertEqual(result.result_readback_invocations, 0)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(
                terminal["reason"],
                "guest-agent-readiness:guest-agent-readiness-budget-exhausted",
            )
            self.assertEqual(terminal["file_push_invocations"], 0)
            self.assert_forbidden_actions_absent(runner.calls)
            runtime_test.assert_manifest_valid(self, request.output_root)

    def test_unknown_agent_response_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                readiness_states=["unknown"],
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.agent_readiness_invocations, 1)
            self.assertEqual(result.guest_probe_invocations, 0)
            self.assertFalse(
                (request.output_root / "guest-agent-ready.json").exists()
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_pid_drift_during_readiness_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                readiness_states=["unavailable"],
                handle_pids=[42, 42, 42, 42, 43],
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.agent_readiness_invocations, 1)
            self.assertEqual(result.guest_probe_invocations, 0)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertIn("target-backend-pid-readiness-drift", terminal["reason"])
            self.assert_forbidden_actions_absent(runner.calls)

    def test_result_drift_invalidates_classification(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            first = guest_probe.result_bytes(
                request.guest_agent_attempt_id,
                request.target_uuid,
                hashlib.sha256(binding.probe_bytes).hexdigest(),
                ORIGINAL_BOOT_HASH,
            )
            second = guest_probe.result_bytes(
                request.guest_agent_attempt_id,
                request.target_uuid,
                hashlib.sha256(binding.probe_bytes).hexdigest(),
                NEW_BOOT_HASH,
            )
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                readiness_states=["ready"],
                result_payloads=[first, second],
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertFalse(
                (request.output_root / "boot-classification.json").exists()
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_authorization_and_output_overlap_reject(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            unauthorized = replace_request(
                request,
                authorized_bounded_read_only_guest_agent_readiness=False,
            )
            with self.assertRaisesRegex(
                resolution.GuestAgentResolutionError,
                "authorized-bounded-read-only-guest-agent-readiness-required",
            ):
                unauthorized.validate()

            overlapping = replace_request(
                request,
                output_root=request.prior_boot_start_root / "new",
            )
            with self.assertRaisesRegex(
                resolution.GuestAgentResolutionError,
                "output-root-must-not-overlap-prior-boot-start-root",
            ):
                overlapping.validate()

    def test_prior_terminal_requires_exact_agent_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = make_request(root)
            terminal = prior_terminal(request)
            write_json(root / "terminal.json", terminal)
            bindings._validate_prior_terminal(request, root)

            terminal["guest_probe_invocations"] = 1
            write_json(root / "terminal.json", terminal)
            with self.assertRaisesRegex(
                ValueError, "prior-boot-start-terminal-invalid"
            ):
                bindings._validate_prior_terminal(request, root)

    def test_prior_handle_requires_runtime_v2_format(self) -> None:
        request = make_request(Path("/tmp/radishlex-guest-agent-test"))
        argv = resolution.network_ready._lsof_argv(request)
        value = {
            "backend_command": "QEMULauncher",
            "backend_pid": bindings.REQUIRED_BACKEND_PID,
            "efi_handle_count": 1,
            "format": resolution.runtime_control.EVIDENCE_FORMAT,
            "observation": {
                "argv": list(argv),
                "exit_code": 0,
                "stderr": stream_metadata(b""),
                "stdout": stream_metadata(b"synthetic-handle"),
                "timed_out": False,
            },
            "process_record_count": 1,
            "qcow2_handle_count": 1,
            "state": "present",
        }
        self.assertEqual(
            bindings._require_prior_handle_state(
                value, argv, bindings.REQUIRED_BACKEND_PID
            ),
            hashlib.sha256(b"synthetic-handle").hexdigest(),
        )
        value["format"] = resolution.runtime_bindings.PRIOR_EVIDENCE_FORMAT
        with self.assertRaisesRegex(
            ValueError, "prior-boot-start-handle-semantics-invalid"
        ):
            bindings._require_prior_handle_state(
                value, argv, bindings.REQUIRED_BACKEND_PID
            )

    def assert_forbidden_actions_absent(
        self, calls: list[tuple[str, ...]]
    ) -> None:
        flattened = [" ".join(call) for call in calls]
        for forbidden in (
            "utmctl list",
            "utmctl status",
            "utmctl start",
            "/usr/bin/osascript",
            " resume ",
            " retry ",
            " stop ",
            " quit ",
        ):
            self.assertFalse(any(forbidden in value for value in flattened))


def make_request(root: Path) -> resolution.GuestAgentResolutionRequest:
    old = boot_test.make_request(root)
    return resolution.GuestAgentResolutionRequest(
        repository_root=Path(resolution.__file__).resolve().parents[2],
        expected_repository_head="1" * 40,
        prior_v7_root=old.prior_v7_root,
        prior_v7_manifest_sha256=old.prior_v7_manifest_sha256,
        prior_prepared_root=old.prior_prepared_root,
        prior_prepared_manifest_sha256=old.prior_prepared_manifest_sha256,
        prior_network_root=old.prior_network_root,
        prior_network_manifest_sha256=old.prior_network_manifest_sha256,
        prior_transfer_root=old.prior_transfer_root,
        prior_transfer_manifest_sha256=old.prior_transfer_manifest_sha256,
        prior_resolution_root=old.prior_resolution_root,
        prior_resolution_manifest_sha256=old.prior_resolution_manifest_sha256,
        prior_preflight_root=old.prior_preflight_root,
        prior_preflight_manifest_sha256=old.prior_preflight_manifest_sha256,
        prior_checkpoint_root=old.prior_checkpoint_root,
        prior_checkpoint_manifest_sha256=old.prior_checkpoint_manifest_sha256,
        prior_resume_failure_root=old.prior_resume_failure_root,
        prior_resume_failure_manifest_sha256=(
            old.prior_resume_failure_manifest_sha256
        ),
        prior_backend_resolution_root=old.prior_backend_resolution_root,
        prior_backend_resolution_manifest_sha256=(
            old.prior_backend_resolution_manifest_sha256
        ),
        prior_reactivation_root=old.prior_reactivation_root,
        prior_reactivation_manifest_sha256=old.prior_reactivation_manifest_sha256,
        prior_runtime_resolution_root=old.prior_runtime_resolution_root,
        prior_runtime_resolution_manifest_sha256=(
            old.prior_runtime_resolution_manifest_sha256
        ),
        prior_runtime_resolution_v2_root=old.prior_runtime_resolution_v2_root,
        prior_runtime_resolution_v2_manifest_sha256=(
            old.prior_runtime_resolution_v2_manifest_sha256
        ),
        prior_boot_transport_root=old.prior_boot_transport_root,
        prior_boot_transport_manifest_sha256=(
            old.prior_boot_transport_manifest_sha256
        ),
        prior_boot_start_root=root / "prior-boot-start",
        prior_boot_start_manifest_sha256=(
            bindings.REQUIRED_PRIOR_BOOT_START_MANIFEST_SHA256
        ),
        source_bundle_path=old.source_bundle_path,
        source_bundle_size=old.source_bundle_size,
        source_bundle_sha256=old.source_bundle_sha256,
        output_root=root / "guest-agent-output",
        transfer_attempt_id=old.transfer_attempt_id,
        resolution_attempt_id=old.resolution_attempt_id,
        preflight_attempt_id=old.preflight_attempt_id,
        checkpoint_attempt_id=old.checkpoint_attempt_id,
        resume_attempt_id=old.resume_attempt_id,
        backend_resolution_attempt_id=old.backend_resolution_attempt_id,
        reactivation_attempt_id=old.reactivation_attempt_id,
        prior_runtime_resolution_attempt_id=(
            old.prior_runtime_resolution_attempt_id
        ),
        runtime_resolution_attempt_id=old.runtime_resolution_attempt_id,
        prior_runtime_resolution_v2_attempt_id=(
            old.prior_runtime_resolution_v2_attempt_id
        ),
        boot_transport_attempt_id=old.boot_transport_attempt_id,
        prior_boot_transport_attempt_id=old.prior_boot_transport_attempt_id,
        boot_start_attempt_id=old.boot_start_attempt_id,
        prior_boot_start_attempt_id=(
            bindings.REQUIRED_PRIOR_BOOT_START_ATTEMPT_ID
        ),
        guest_agent_attempt_id=bindings.REQUIRED_GUEST_AGENT_ATTEMPT_ID,
        target_uuid=old.target_uuid,
        target_name=old.target_name,
        target_package_path=old.target_package_path,
        expected_vm_count=21,
        identity_observations=3,
        agent_readiness_attempts=60,
        poll_interval_seconds=1,
        command_timeout_seconds=60,
        authorized_install_artifacts_staged_guest_agent_resolution=True,
        authorized_bounded_read_only_guest_agent_readiness=True,
        authorized_stable_target_handle_pid_observations=True,
        authorized_one_private_guest_probe_delivery_and_execution=True,
        authorized_two_independent_result_readbacks=True,
        authorized_no_list_status_start_resume_business_guest_retry_stop_or_quit=True,
    )


def make_binding() -> SimpleNamespace:
    probe_bytes = (
        Path(resolution.__file__).resolve().parents[2]
        / resolution.PROBE_RELATIVE_PATH
    ).read_bytes()
    return SimpleNamespace(
        evidence={"format": resolution.EVIDENCE_FORMAT},
        expected_boot_id_sha256=ORIGINAL_BOOT_HASH,
        probe_bytes=probe_bytes,
        prior_backend_pid=bindings.REQUIRED_BACKEND_PID,
    )


def run_case(
    request: resolution.GuestAgentResolutionRequest,
    binding: SimpleNamespace,
    runner: FakeRunner,
) -> tuple[resolution.GuestAgentResolutionResult, SyntheticSource]:
    source = SyntheticSource(CloseTracker())
    result = resolution.run_guest_agent_resolution(
        request,
        runner=runner,
        binding_validator=lambda _: binding,
        source_opener=lambda _: source,
        source_revalidator=lambda _request, _source: {
            "descriptor_unchanged": True,
            "format": "synthetic-source-v1",
            "inventory_unchanged": True,
            "sha256": resolution.reactivation_control.REQUIRED_SOURCE_BUNDLE_SHA256,
            "size": resolution.reactivation_control.REQUIRED_SOURCE_BUNDLE_SIZE,
        },
        target_validator=lambda _: runtime_test.target_identity(request),
        sleeper=lambda _: None,
    )
    return result, source


def replace_request(
    request: resolution.GuestAgentResolutionRequest, **changes: object
) -> resolution.GuestAgentResolutionRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return resolution.GuestAgentResolutionRequest(**values)


def observation(
    argv: tuple[str, ...],
    *,
    stdout: bytes = b"",
    stderr: bytes = b"",
    exit_code: int = 0,
) -> start_control.CommandObservation:
    return start_control.CommandObservation.from_bytes(
        argv, stdout=stdout, stderr=stderr, exit_code=exit_code
    )


def stream_metadata(value: bytes) -> dict[str, object]:
    return {
        "sha256": hashlib.sha256(value).hexdigest(),
        "total_bytes": len(value),
        "truncated": False,
    }


def prior_terminal(
    request: resolution.GuestAgentResolutionRequest,
) -> dict[str, object]:
    return {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": bindings.REQUIRED_BACKEND_PID,
        "boot_classification": "not-generated",
        "boot_start_attempt_id": request.prior_boot_start_attempt_id,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "foreground_start_invocations": 1,
        "format": resolution.boot_bindings.EVIDENCE_FORMAT,
        "guest_exec_invocations": 1,
        "guest_probe_invocations": 0,
        "identity_observation_count": 3,
        "inventory_probe_invocations": 1,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": None,
        "operation_id": "not-read-or-generated",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "attempted-once-as-potential-backend-reactivation",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": (
            "guest-control-root-create:"
            "guest-control-root-create-stderr-not-empty"
        ),
        "result_readback_invocations": 0,
        "runtime_poll_count": 1,
        "target_handles_terminal": "present",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def write_json(path: Path, value: dict[str, object]) -> None:
    path.write_text(json.dumps(value), encoding="utf-8")


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace

import l6_utm_install_artifacts_staged_boot_start_bindings as bindings
import l6_utm_install_artifacts_staged_boot_start_resolution as resolution
import l6_utm_start_once as start_control
import l6_v4_boot_transport_probe as guest_probe
import test_l6_utm_install_artifacts_staged_boot_transport_resolution as boot_test
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
        request: resolution.BootStartResolutionRequest,
        probe_bytes: bytes,
        *,
        target_status: str = "stopped",
        boot_hash: str = NEW_BOOT_HASH,
        handle_identities: list[tuple[int, str] | None] | None = None,
        result_payloads: list[bytes | None] | None = None,
    ) -> None:
        self.request = request
        self.probe_bytes = probe_bytes
        self.probe_sha256 = hashlib.sha256(probe_bytes).hexdigest()
        self.target_status = target_status
        self.boot_hash = boot_hash
        self.handle_identities = list(
            handle_identities
            or [
                None,
                None,
                None,
                None,
                None,
                (42, "QEMULauncher"),
                (42, "QEMULauncher"),
                (42, "QEMULauncher"),
                (42, "QEMULauncher"),
                (42, "QEMULauncher"),
            ]
        )
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
        if argv == ("utmctl", "list"):
            return observation(
                argv,
                stdout=runtime_test.inventory_stdout(
                    self.request,
                    target_status=self.target_status,
                    other_active=False,
                ),
            )
        if argv[0:1] == ("/usr/sbin/lsof",):
            if not self.handle_identities:
                raise AssertionError("unexpected handle observation")
            identity = self.handle_identities.pop(0)
            if identity is None:
                return observation(argv, exit_code=1)
            pid, command = identity
            return observation(
                argv,
                stdout=runtime_test.handle_payload(
                    self.request, pid, command
                ),
            )
        if argv == resolution.launch_transport.transport_argv(self.request):
            return observation(argv)
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
                    self.request.boot_start_attempt_id,
                    self.probe_sha256,
                )
            elif path == self.request.guest_result_path:
                if self.result_payloads:
                    payload = self.result_payloads.pop(0)
                else:
                    payload = guest_probe.result_bytes(
                        self.request.boot_start_attempt_id,
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


class BootStartResolutionTests(unittest.TestCase):
    def test_probe_cli_uses_boot_start_scope_root_contract(self) -> None:
        request = make_request(Path("/tmp/radishlex-boot-start-cli-test"))
        argv = resolution.probe_argv(request, make_binding())

        args = guest_probe.parse_args(argv[8:])

        self.assertEqual(
            args.control_scope,
            guest_probe.CONTROL_SCOPE_BOOT_START,
        )
        self.assertEqual(args.control_root, Path(request.guest_control_root))
        self.assertEqual(
            guest_probe.control_root_for(
                args.control_scope,
                args.attempt_id,
            ),
            args.control_root,
        )

    def test_stopped_target_starts_once_and_classifies_new_boot(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(request, binding.probe_bytes)

            result, source = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "new-boot-started")
            self.assertEqual(result.exit_code, resolution.EXIT_NEW_BOOT_STARTED)
            self.assertEqual(result.inventory_probe_invocations, 1)
            self.assertEqual(result.foreground_start_invocations, 1)
            self.assertEqual(result.guest_probe_invocations, 1)
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertTrue(source.file_object.closed)
            self.assertEqual(
                sum(
                    call == resolution.launch_transport.transport_argv(request)
                    for call in runner.calls
                ),
                1,
            )
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["file_pull_invocations"], 4)
            self.assertEqual(terminal["guest_exec_invocations"], 3)
            self.assertEqual(terminal["runtime_poll_count"], 2)
            self.assert_forbidden_actions_absent(runner.calls)
            runtime_test.assert_manifest_valid(self, request.output_root)

    def test_restored_original_boot_is_separate_outcome(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                boot_hash=ORIGINAL_BOOT_HASH,
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "original-boot-restored")
            self.assertEqual(
                result.exit_code, resolution.EXIT_ORIGINAL_BOOT_RESTORED
            )

    def test_nonstopped_inventory_never_starts_or_enters_guest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                target_status="started",
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.foreground_start_invocations, 0)
            self.assertEqual(result.guest_probe_invocations, 0)
            self.assertFalse(
                any(
                    call == resolution.launch_transport.transport_argv(request)
                    for call in runner.calls
                )
            )

    def test_missing_runtime_after_start_fails_closed_without_retry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                handle_identities=[None] * 64,
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.foreground_start_invocations, 1)
            self.assertEqual(result.guest_probe_invocations, 0)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["runtime_poll_count"], 60)
            self.assertEqual(terminal["automatic_retry"], "not-performed")
            self.assertEqual(terminal["automatic_stop"], "not-performed")

    def test_result_drift_invalidates_classification_without_retry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            first = guest_probe.result_bytes(
                request.boot_start_attempt_id,
                request.target_uuid,
                hashlib.sha256(binding.probe_bytes).hexdigest(),
                ORIGINAL_BOOT_HASH,
            )
            second = guest_probe.result_bytes(
                request.boot_start_attempt_id,
                request.target_uuid,
                hashlib.sha256(binding.probe_bytes).hexdigest(),
                NEW_BOOT_HASH,
            )
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                result_payloads=[first, second],
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertFalse(
                (request.output_root / "boot-classification.json").exists()
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_authorization_and_output_overlap_reject_before_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            unauthorized = replace_request(
                request,
                authorized_one_foreground_start_from_stopped=False,
            )
            with self.assertRaisesRegex(
                resolution.BootStartResolutionError,
                "authorized-one-foreground-start-from-stopped-required",
            ):
                unauthorized.validate()

            overlapping = replace_request(
                request,
                output_root=request.prior_boot_transport_root / "new",
            )
            with self.assertRaisesRegex(
                resolution.BootStartResolutionError,
                "output-root-must-not-overlap-prior-boot-transport-root",
            ):
                overlapping.validate()

    def test_prior_terminal_requires_exact_stopped_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = make_request(root)
            terminal = prior_terminal(request)
            write_json(root / "terminal.json", terminal)
            bindings._validate_prior_terminal(request, root)

            terminal["foreground_start_invocations"] = 1
            write_json(root / "terminal.json", terminal)
            with self.assertRaisesRegex(
                ValueError, "prior-boot-transport-terminal-invalid"
            ):
                bindings._validate_prior_terminal(request, root)

    def assert_forbidden_actions_absent(
        self, calls: list[tuple[str, ...]]
    ) -> None:
        flattened = [" ".join(call) for call in calls]
        for forbidden in (
            "utmctl status",
            "utmctl stop",
            " resume ",
            " retry ",
            " quit ",
        ):
            self.assertFalse(any(forbidden in value for value in flattened))


def make_request(root: Path) -> resolution.BootStartResolutionRequest:
    old = boot_test.make_request(root)
    return resolution.BootStartResolutionRequest(
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
        prior_resume_failure_manifest_sha256=old.prior_resume_failure_manifest_sha256,
        prior_backend_resolution_root=old.prior_backend_resolution_root,
        prior_backend_resolution_manifest_sha256=old.prior_backend_resolution_manifest_sha256,
        prior_reactivation_root=old.prior_reactivation_root,
        prior_reactivation_manifest_sha256=old.prior_reactivation_manifest_sha256,
        prior_runtime_resolution_root=old.prior_runtime_resolution_root,
        prior_runtime_resolution_manifest_sha256=old.prior_runtime_resolution_manifest_sha256,
        prior_runtime_resolution_v2_root=old.prior_runtime_resolution_v2_root,
        prior_runtime_resolution_v2_manifest_sha256=old.prior_runtime_resolution_v2_manifest_sha256,
        prior_boot_transport_root=root / "prior-boot-transport",
        prior_boot_transport_manifest_sha256=(
            bindings.REQUIRED_PRIOR_BOOT_TRANSPORT_MANIFEST_SHA256
        ),
        source_bundle_path=old.source_bundle_path,
        source_bundle_size=old.source_bundle_size,
        source_bundle_sha256=old.source_bundle_sha256,
        output_root=root / "boot-start-output",
        transfer_attempt_id=old.transfer_attempt_id,
        resolution_attempt_id=old.resolution_attempt_id,
        preflight_attempt_id=old.preflight_attempt_id,
        checkpoint_attempt_id=old.checkpoint_attempt_id,
        resume_attempt_id=old.resume_attempt_id,
        backend_resolution_attempt_id=old.backend_resolution_attempt_id,
        reactivation_attempt_id=old.reactivation_attempt_id,
        prior_runtime_resolution_attempt_id=old.prior_runtime_resolution_attempt_id,
        runtime_resolution_attempt_id=old.runtime_resolution_attempt_id,
        prior_runtime_resolution_v2_attempt_id=old.prior_runtime_resolution_v2_attempt_id,
        boot_transport_attempt_id=old.boot_transport_attempt_id,
        prior_boot_transport_attempt_id=(
            bindings.REQUIRED_PRIOR_BOOT_TRANSPORT_ATTEMPT_ID
        ),
        boot_start_attempt_id=bindings.REQUIRED_BOOT_START_ATTEMPT_ID,
        target_uuid=old.target_uuid,
        target_name=old.target_name,
        target_package_path=old.target_package_path,
        expected_vm_count=21,
        quiescence_observations=3,
        runtime_poll_attempts=60,
        identity_observations=3,
        poll_interval_seconds=1,
        transport_timeout_seconds=60,
        command_timeout_seconds=60,
        authorized_install_artifacts_staged_boot_start_resolution=True,
        authorized_one_potential_backend_reactivation_list=True,
        authorized_one_foreground_start_from_stopped=True,
        authorized_bounded_target_runtime_observation=True,
        authorized_one_private_guest_probe_delivery_and_execution=True,
        authorized_two_independent_result_readbacks=True,
        authorized_no_status_resume_business_guest_retry_stop_or_quit=True,
    )


def make_binding() -> SimpleNamespace:
    probe_bytes = (
        Path(resolution.__file__).resolve().parents[2]
        / resolution.PROBE_RELATIVE_PATH
    ).read_bytes()
    return SimpleNamespace(
        evidence={"format": resolution.EVIDENCE_FORMAT},
        upstream=SimpleNamespace(
            upstream=SimpleNamespace(
                upstream=SimpleNamespace(
                    baseline_inventory=runtime_test.baseline_inventory()
                )
            )
        ),
        expected_boot_id_sha256=ORIGINAL_BOOT_HASH,
        probe_bytes=probe_bytes,
    )


def run_case(
    request: resolution.BootStartResolutionRequest,
    binding: SimpleNamespace,
    runner: FakeRunner,
) -> tuple[resolution.BootStartResolutionResult, SyntheticSource]:
    source = SyntheticSource(CloseTracker())
    result = resolution.run_boot_start_resolution(
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
    request: resolution.BootStartResolutionRequest, **changes: object
) -> resolution.BootStartResolutionRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return resolution.BootStartResolutionRequest(**values)


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


def prior_terminal(
    request: resolution.BootStartResolutionRequest,
) -> dict[str, object]:
    return {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": None,
        "boot_classification": "not-generated",
        "boot_transport_attempt_id": request.prior_boot_transport_attempt_id,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 0,
        "file_push_invocations": 0,
        "foreground_start_invocations": 0,
        "format": resolution.boot_bindings.EVIDENCE_FORMAT,
        "guest_exec_invocations": 0,
        "guest_probe_invocations": 0,
        "identity_observation_count": 0,
        "inventory_probe_invocations": 1,
        "maintenance_resume_invocations": 0,
        "observed_boot_id_sha256": None,
        "operation_id": "not-read-or-generated",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "attempted-once-as-potential-backend-reactivation",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": (
            "utmctl-list-once:"
            "target-not-registered-started-for-boot-transport"
        ),
        "result_readback_invocations": 0,
        "target_handles_terminal": "not-observed",
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

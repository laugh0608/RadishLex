#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace

import l6_utm_install_artifacts_staged_boot_transport_bindings as bindings
import l6_utm_install_artifacts_staged_boot_transport_resolution as resolution
import l6_utm_start_once as start_control
import l6_v4_boot_transport_probe as guest_probe
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
        request: resolution.BootTransportResolutionRequest,
        probe_bytes: bytes,
        *,
        boot_hash: str = ORIGINAL_BOOT_HASH,
        result_payloads: list[bytes | None] | None = None,
        handle_identities: list[tuple[int, str] | None] | None = None,
        probe_exit_code: int = 0,
    ) -> None:
        self.request = request
        self.probe_bytes = probe_bytes
        self.probe_sha256 = hashlib.sha256(probe_bytes).hexdigest()
        self.boot_hash = boot_hash
        self.result_payloads = list(result_payloads or [])
        self.handle_identities = list(
            handle_identities
            or [
                (42, "QEMULauncher"),
                (42, "QEMULauncher"),
                (42, "QEMULauncher"),
                (42, "QEMULauncher"),
                (42, "QEMULauncher"),
            ]
        )
        self.process_outputs = [runtime_test.quiet() for _ in range(5)]
        self.probe_exit_code = probe_exit_code
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
            if not self.process_outputs:
                raise AssertionError("unexpected process observation")
            return observation(argv, stdout=self.process_outputs.pop(0))
        if argv == ("utmctl", "list"):
            return observation(
                argv,
                stdout=runtime_test.inventory_stdout(
                    self.request,
                    target_status="started",
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
                    self.request.boot_transport_attempt_id,
                    self.probe_sha256,
                )
            elif path == self.request.guest_result_path:
                if self.result_payloads:
                    payload = self.result_payloads.pop(0)
                else:
                    payload = guest_probe.result_bytes(
                        self.request.boot_transport_attempt_id,
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
                return observation(argv, exit_code=self.probe_exit_code)
            if "/bin/mkdir" in argv:
                return observation(argv)
        raise AssertionError(f"unexpected command: {argv}")


class BootTransportResolutionTests(unittest.TestCase):
    def test_empty_exec_output_uses_private_double_readback(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(request, binding.probe_bytes)

            result, source = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "original-boot-restored")
            self.assertEqual(
                result.exit_code, resolution.EXIT_ORIGINAL_BOOT_RESTORED
            )
            self.assertEqual(result.guest_probe_invocations, 1)
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertTrue(source.file_object.closed)
            probe_observation = read_json(
                request.output_root
                / "guest-boot-transport-probe-once.json"
            )
            self.assertEqual(probe_observation["raw_output_persisted"], False)
            metadata = probe_observation["observation"]
            self.assertIsInstance(metadata, dict)
            assert isinstance(metadata, dict)
            self.assertEqual(metadata["stdout"]["total_bytes"], 0)
            classification = read_json(
                request.output_root / "boot-classification.json"
            )
            self.assertEqual(
                classification["transport"],
                "guest-private-create-new-double-readback",
            )
            self.assert_terminal_safety(request, runner.calls)
            runtime_test.assert_manifest_valid(self, request.output_root)

    def test_new_boot_is_separate_outcome(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(
                request, binding.probe_bytes, boot_hash=NEW_BOOT_HASH
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "new-boot-started")
            self.assertEqual(
                result.exit_code, resolution.EXIT_NEW_BOOT_STARTED
            )

    def test_result_readback_drift_fails_closed_without_retry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            probe_sha256 = hashlib.sha256(binding.probe_bytes).hexdigest()
            payloads = [
                guest_probe.result_bytes(
                    request.boot_transport_attempt_id,
                    request.target_uuid,
                    probe_sha256,
                    ORIGINAL_BOOT_HASH,
                ),
                guest_probe.result_bytes(
                    request.boot_transport_attempt_id,
                    request.target_uuid,
                    probe_sha256,
                    NEW_BOOT_HASH,
                ),
            ]
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                result_payloads=payloads,
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.result_readback_invocations, 2)
            self.assertFalse(
                (request.output_root / "boot-classification.json").exists()
            )
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["automatic_retry"], "not-performed")

    def test_missing_result_is_indeterminate_and_not_reinvoked(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                result_payloads=[None],
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.guest_probe_invocations, 1)
            self.assertEqual(result.result_readback_invocations, 1)
            probe_calls = [
                call
                for call in runner.calls
                if request.guest_probe_path in call
                and call[0:3] == ("utmctl", "exec", request.target_uuid)
                and "/usr/bin/python3" in call
            ]
            self.assertEqual(len(probe_calls), 1)

    def test_unknown_result_field_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            probe_sha256 = hashlib.sha256(binding.probe_bytes).hexdigest()
            value = json.loads(
                guest_probe.result_bytes(
                    request.boot_transport_attempt_id,
                    request.target_uuid,
                    probe_sha256,
                    ORIGINAL_BOOT_HASH,
                )
            )
            value["raw_boot_id"] = "forbidden"
            payload = guest_probe.canonical_json(value)
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                result_payloads=[payload, payload],
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertIn(
                "guest-result-fields-invalid",
                read_json(request.output_root / "terminal.json")["reason"],
            )

    def test_terminal_pid_drift_invalidates_classification(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = make_binding()
            runner = FakeRunner(
                request,
                binding.probe_bytes,
                handle_identities=[
                    (42, "QEMULauncher"),
                    (42, "QEMULauncher"),
                    (42, "QEMULauncher"),
                    (42, "QEMULauncher"),
                    (43, "QEMULauncher"),
                ],
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertIn(
                "target-backend-pid-terminal-drift",
                read_json(request.output_root / "terminal.json")["reason"],
            )

    def test_authorization_and_output_overlap_fail_before_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            unauthorized = replace_request(
                request,
                authorized_two_independent_result_readbacks=False,
            )
            with self.assertRaisesRegex(
                resolution.BootTransportResolutionError,
                "authorized-two-independent-result-readbacks-required",
            ):
                unauthorized.validate()

            overlapping = replace_request(
                request,
                output_root=request.prior_runtime_resolution_v2_root / "new",
            )
            with self.assertRaisesRegex(
                resolution.BootTransportResolutionError,
                "output-root-must-not-overlap-prior-runtime-resolution-v2-root",
            ):
                overlapping.validate()

    def test_v3_binding_requires_exact_empty_v2_observation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = SimpleNamespace(target_uuid=resolution.reactivation_control.REQUIRED_TARGET_UUID)
            value = {
                "format": runtime_test.resolution.EVIDENCE_FORMAT,
                "invocation_count": 1,
                "observation": {
                    "argv": list(
                        resolution.reactivation_control.boot_id_hash_argv(
                            request
                        )
                    ),
                    "exit_code": 0,
                    "stderr": empty_stream(),
                    "stdout": empty_stream(),
                    "timed_out": False,
                },
                "raw_output_persisted": False,
            }
            write_json(root / "guest-boot-id-hash-observation.json", value)
            bindings._validate_empty_boot_observation(request, root)

            value["observation"]["stdout"]["total_bytes"] = 1
            write_json(root / "guest-boot-id-hash-observation.json", value)
            with self.assertRaisesRegex(
                ValueError,
                "prior-runtime-resolution-v2-empty-observation-invalid",
            ):
                bindings._validate_empty_boot_observation(request, root)

    def assert_terminal_safety(
        self,
        request: resolution.BootTransportResolutionRequest,
        calls: list[tuple[str, ...]],
    ) -> None:
        terminal = read_json(request.output_root / "terminal.json")
        self.assertEqual(terminal["business_guest_action"], "not-performed")
        self.assertEqual(terminal["maintenance_resume_invocations"], 0)
        self.assertEqual(terminal["foreground_start_invocations"], 0)
        self.assertEqual(terminal["file_push_invocations"], 1)
        self.assertEqual(terminal["file_pull_invocations"], 4)
        self.assertEqual(terminal["guest_exec_invocations"], 3)
        self.assertEqual(sum(call == ("utmctl", "list") for call in calls), 1)
        flattened = [" ".join(call) for call in calls]
        for forbidden in (
            "utmctl start",
            "utmctl status",
            "utmctl stop",
            " resume ",
            " retry ",
            " quit ",
        ):
            self.assertFalse(any(forbidden in value for value in flattened))


def make_request(root: Path) -> resolution.BootTransportResolutionRequest:
    old = runtime_test.make_request(root)
    repository_root = Path(resolution.__file__).resolve().parents[2]
    return resolution.BootTransportResolutionRequest(
        repository_root=repository_root,
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
        prior_runtime_resolution_v2_root=root / "prior-runtime-resolution-v2",
        prior_runtime_resolution_v2_manifest_sha256=(
            bindings.REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_MANIFEST_SHA256
        ),
        source_bundle_path=old.source_bundle_path,
        source_bundle_size=old.source_bundle_size,
        source_bundle_sha256=old.source_bundle_sha256,
        output_root=root / "output",
        transfer_attempt_id=old.transfer_attempt_id,
        resolution_attempt_id=old.resolution_attempt_id,
        preflight_attempt_id=old.preflight_attempt_id,
        checkpoint_attempt_id=old.checkpoint_attempt_id,
        resume_attempt_id=old.resume_attempt_id,
        backend_resolution_attempt_id=old.backend_resolution_attempt_id,
        reactivation_attempt_id=old.reactivation_attempt_id,
        prior_runtime_resolution_attempt_id=old.prior_runtime_resolution_attempt_id,
        runtime_resolution_attempt_id=old.runtime_resolution_attempt_id,
        prior_runtime_resolution_v2_attempt_id=(
            bindings.REQUIRED_PRIOR_RUNTIME_RESOLUTION_V2_ATTEMPT_ID
        ),
        boot_transport_attempt_id=bindings.REQUIRED_BOOT_TRANSPORT_ATTEMPT_ID,
        target_uuid=old.target_uuid,
        target_name=old.target_name,
        target_package_path=old.target_package_path,
        expected_vm_count=21,
        identity_observations=3,
        poll_interval_seconds=1,
        command_timeout_seconds=60,
        authorized_install_artifacts_staged_boot_transport_resolution=True,
        authorized_one_potential_backend_reactivation_list=True,
        authorized_stable_target_handle_pid_observations=True,
        authorized_one_private_guest_probe_delivery_and_execution=True,
        authorized_two_independent_result_readbacks=True,
        authorized_no_start_status_resume_business_guest_retry_stop_or_quit=True,
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
                baseline_inventory=runtime_test.baseline_inventory()
            )
        ),
        expected_boot_id_sha256=ORIGINAL_BOOT_HASH,
        probe_bytes=probe_bytes,
    )


def run_case(
    request: resolution.BootTransportResolutionRequest,
    binding: SimpleNamespace,
    runner: FakeRunner,
) -> tuple[resolution.BootTransportResolutionResult, SyntheticSource]:
    source = SyntheticSource(CloseTracker())
    result = resolution.run_boot_transport_resolution(
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
    request: resolution.BootTransportResolutionRequest, **changes: object
) -> resolution.BootTransportResolutionRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return resolution.BootTransportResolutionRequest(**values)


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


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def write_json(path: Path, value: dict[str, object]) -> None:
    path.write_text(json.dumps(value), encoding="utf-8")


def empty_stream() -> dict[str, object]:
    return {
        "sha256": bindings.EMPTY_SHA256,
        "total_bytes": 0,
        "truncated": False,
    }


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace

import l6_utm_install_artifacts_staged_runtime_resolution as resolution
import l6_utm_start_once as start_control


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
        request: resolution.RuntimeResolutionRequest,
        *,
        list_status: str = "started",
        other_active: bool = False,
        process_outputs: list[bytes],
        handle_identities: list[tuple[int, str] | None],
        boot_hash: str = ORIGINAL_BOOT_HASH,
        boot_payload: bytes | None = None,
    ) -> None:
        self.request = request
        self.list_status = list_status
        self.other_active = other_active
        self.process_outputs = list(process_outputs)
        self.handle_identities = list(handle_identities)
        self.boot_hash = boot_hash
        self.boot_payload = boot_payload
        self.calls: list[tuple[str, ...]] = []

    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> start_control.CommandObservation:
        del timeout_seconds
        self.calls.append(argv)
        if argv == resolution.launch_transport.PROCESS_COMMAND:
            if not self.process_outputs:
                raise AssertionError("unexpected process observation")
            return start_control.CommandObservation.from_bytes(
                argv, stdout=self.process_outputs.pop(0)
            )
        if argv == ("utmctl", "list"):
            return start_control.CommandObservation.from_bytes(
                argv,
                stdout=inventory_stdout(
                    self.request,
                    target_status=self.list_status,
                    other_active=self.other_active,
                ),
            )
        if argv[0:1] == ("/usr/sbin/lsof",):
            if not self.handle_identities:
                raise AssertionError("unexpected handle observation")
            identity = self.handle_identities.pop(0)
            if identity is None:
                return start_control.CommandObservation.from_bytes(
                    argv, exit_code=1
                )
            pid, command = identity
            return start_control.CommandObservation.from_bytes(
                argv, stdout=handle_payload(self.request, pid, command)
            )
        if argv == resolution.reactivation_control.boot_id_hash_argv(
            self.request
        ):
            payload = self.boot_payload
            if payload is None:
                payload = f"{self.boot_hash}\n".encode("ascii")
            return start_control.CommandObservation.from_bytes(
                argv, stdout=payload
            )
        raise AssertionError(f"unexpected command: {argv}")


class RuntimeResolutionTests(unittest.TestCase):
    def test_stable_pid_restores_original_boot(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = success_runner(request)

            result, source = run_case(request, runner)

            self.assertEqual(result.outcome, "original-boot-restored")
            self.assertEqual(
                result.exit_code, resolution.EXIT_ORIGINAL_BOOT_RESTORED
            )
            self.assertEqual(result.inventory_probe_invocations, 1)
            self.assertEqual(result.guest_boot_hash_invocations, 1)
            self.assertEqual(result.identity_observation_count, 3)
            self.assertTrue(source.file_object.closed)
            observation = read_json(
                request.output_root / "guest-boot-id-hash-observation.json"
            )
            self.assertEqual(
                observation["raw_output_persisted"], False
            )
            self.assertNotIn("prefix_base64", json.dumps(observation))
            classification = read_json(
                request.output_root / "guest-boot-id-hash-classification.json"
            )
            self.assertEqual(
                classification["classification"], "original-boot-restored"
            )
            self.assert_terminal_safety(request, runner.calls)
            assert_manifest_valid(self, request.output_root)

    def test_new_boot_is_separate_terminal_outcome(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = success_runner(request, boot_hash=NEW_BOOT_HASH)

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "new-boot-started")
            self.assertEqual(result.exit_code, resolution.EXIT_NEW_BOOT_STARTED)
            boot = read_json(
                request.output_root / "guest-boot-id-hash-classification.json"
            )
            self.assertEqual(boot["classification"], "new-boot-started")
            self.assertEqual(boot["observed_boot_id_sha256"], NEW_BOOT_HASH)

    def test_stopped_target_fails_closed_before_handle_or_guest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = FakeRunner(
                request,
                list_status="stopped",
                process_outputs=[quiet()],
                handle_identities=[],
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.guest_boot_hash_invocations, 0)
            self.assertFalse(
                any(call[0:1] == ("/usr/sbin/lsof",) for call in runner.calls)
            )

    def test_other_registered_vm_active_blocks_identity_resolution(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = FakeRunner(
                request,
                other_active=True,
                process_outputs=[quiet()],
                handle_identities=[],
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.identity_observation_count, 0)
            self.assertEqual(result.guest_boot_hash_invocations, 0)

    def test_absent_target_handles_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = FakeRunner(
                request,
                process_outputs=[quiet()],
                handle_identities=[None],
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.identity_observation_count, 0)
            self.assertEqual(result.guest_boot_hash_invocations, 0)

    def test_target_pid_drift_blocks_guest_probe(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = FakeRunner(
                request,
                process_outputs=[quiet(), quiet()],
                handle_identities=[
                    (42, "QEMULauncher"),
                    (42, "QEMULauncher"),
                    (43, "QEMULauncher"),
                ],
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.identity_observation_count, 2)
            self.assertEqual(result.guest_boot_hash_invocations, 0)
            self.assertIn(
                "target-backend-pid-drift",
                read_json(request.output_root / "terminal.json")["reason"],
            )

    def test_unexpected_backend_command_blocks_guest_probe(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = FakeRunner(
                request,
                process_outputs=[quiet()],
                handle_identities=[(42, "QEMUHelper")],
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.guest_boot_hash_invocations, 0)
            self.assertIn(
                "target-handle-command-unexpected",
                read_json(request.output_root / "terminal.json")["reason"],
            )

    def test_active_utmctl_process_blocks_guest_probe(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = FakeRunner(
                request,
                process_outputs=[quiet(), active_utmctl()],
                handle_identities=[
                    (42, "QEMULauncher"),
                    (42, "QEMULauncher"),
                ],
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.guest_boot_hash_invocations, 0)
            self.assertIn(
                "utmctl-process-active:confirmation-001",
                read_json(request.output_root / "terminal.json")["reason"],
            )

    def test_malformed_boot_hash_is_indeterminate_and_not_retried(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = success_runner(
                request, boot_payload=b"not-a-boot-hash\n"
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.guest_boot_hash_invocations, 1)
            observation_path = (
                request.output_root / "guest-boot-id-hash-observation.json"
            )
            classification_path = (
                request.output_root / "guest-boot-id-hash-classification.json"
            )
            self.assertTrue(observation_path.is_file())
            self.assertFalse(classification_path.exists())
            observation = read_json(observation_path)
            self.assertEqual(observation["invocation_count"], 1)
            self.assertEqual(observation["raw_output_persisted"], False)
            metadata = observation["observation"]
            self.assertIsInstance(metadata, dict)
            assert isinstance(metadata, dict)
            stdout = metadata["stdout"]
            self.assertIsInstance(stdout, dict)
            assert isinstance(stdout, dict)
            self.assertEqual(stdout["total_bytes"], len(b"not-a-boot-hash\n"))
            self.assertEqual(
                stdout["sha256"],
                hashlib.sha256(b"not-a-boot-hash\n").hexdigest(),
            )
            self.assertNotIn("not-a-boot-hash", observation_path.read_text())
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(
                terminal["guest_boot_observation"],
                "persisted-before-parse",
            )
            self.assertEqual(
                terminal["guest_boot_classification"], "not-performed"
            )
            self.assertEqual(
                sum(
                    call
                    == resolution.reactivation_control.boot_id_hash_argv(
                        request
                    )
                    for call in runner.calls
                ),
                1,
            )

    def test_observation_is_persisted_before_boot_parser_runs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = success_runner(request)
            parser_called = False

            def parse_after_evidence(
                observation: start_control.CommandObservation,
            ) -> str:
                nonlocal parser_called
                parser_called = True
                path = (
                    request.output_root
                    / "guest-boot-id-hash-observation.json"
                )
                self.assertTrue(path.is_file())
                persisted = read_json(path)
                self.assertEqual(persisted["raw_output_persisted"], False)
                return resolution.reactivation_control.parse_boot_id_hash(
                    observation
                )

            result, _ = run_case(
                request,
                runner,
                boot_hash_parser=parse_after_evidence,
            )

            self.assertTrue(parser_called)
            self.assertEqual(result.outcome, "original-boot-restored")

    def test_terminal_pid_drift_is_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = FakeRunner(
                request,
                process_outputs=[quiet(), quiet(), quiet(), quiet()],
                handle_identities=[
                    (42, "QEMULauncher"),
                    (42, "QEMULauncher"),
                    (42, "QEMULauncher"),
                    (42, "QEMULauncher"),
                    (43, "QEMULauncher"),
                ],
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.guest_boot_hash_invocations, 1)
            self.assertIn(
                "target-backend-pid-terminal-drift",
                read_json(request.output_root / "terminal.json")["reason"],
            )

    def test_authorization_and_output_overlap_fail_before_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            unauthorized = replace_request(
                request,
                authorized_stable_target_handle_pid_observations=False,
            )
            with self.assertRaisesRegex(
                resolution.RuntimeResolutionError,
                "authorized-stable-target-handle-pid-observations-required",
            ):
                unauthorized.validate()

            overlapping = replace_request(
                request,
                output_root=request.prior_reactivation_root / "new",
            )
            with self.assertRaisesRegex(
                resolution.RuntimeResolutionError,
                "output-root-must-not-overlap-prior-reactivation-root",
            ):
                overlapping.validate()

            wrong_prior = replace_request(
                request,
                prior_runtime_resolution_manifest_sha256="0" * 64,
            )
            with self.assertRaisesRegex(
                resolution.RuntimeResolutionError,
                "required-prior-runtime-resolution-manifest-mismatch",
            ):
                wrong_prior.validate()

    def assert_terminal_safety(
        self,
        request: resolution.RuntimeResolutionRequest,
        calls: list[tuple[str, ...]],
    ) -> None:
        terminal = read_json(request.output_root / "terminal.json")
        self.assertEqual(terminal["business_guest_action"], "not-performed")
        self.assertEqual(terminal["foreground_start_invocations"], 0)
        self.assertEqual(terminal["maintenance_resume_invocations"], 0)
        self.assertEqual(terminal["automatic_retry"], "not-performed")
        self.assertEqual(terminal["automatic_stop"], "not-performed")
        self.assertEqual(sum(call == ("utmctl", "list") for call in calls), 1)
        flattened = [" ".join(call) for call in calls]
        for forbidden in (
            "utmctl start",
            "utmctl status",
            "utmctl stop",
            "utmctl file",
            " resume ",
            " retry ",
            " quit ",
        ):
            self.assertFalse(any(forbidden in value for value in flattened))


def run_case(
    request: resolution.RuntimeResolutionRequest,
    runner: FakeRunner,
    *,
    boot_hash_parser=None,
) -> tuple[resolution.RuntimeResolutionResult, SyntheticSource]:
    source = SyntheticSource(CloseTracker())
    result = resolution.run_runtime_resolution(
        request,
        runner=runner,
        binding_validator=lambda _: SimpleNamespace(
            evidence={"format": resolution.EVIDENCE_FORMAT},
            upstream=SimpleNamespace(baseline_inventory=baseline_inventory()),
            expected_boot_id_sha256=ORIGINAL_BOOT_HASH,
        ),
        source_opener=lambda _: source,
        source_revalidator=lambda _request, _source: {
            "descriptor_unchanged": True,
            "format": "synthetic-source-v1",
            "inventory_unchanged": True,
            "sha256": resolution.reactivation_control.REQUIRED_SOURCE_BUNDLE_SHA256,
            "size": resolution.reactivation_control.REQUIRED_SOURCE_BUNDLE_SIZE,
        },
        target_validator=lambda _: target_identity(request),
        boot_hash_parser=boot_hash_parser,
        sleeper=lambda _: None,
    )
    return result, source


def make_request(root: Path) -> resolution.RuntimeResolutionRequest:
    old = resolution.reactivation_control
    target_name = old.REQUIRED_TARGET_NAME
    return resolution.RuntimeResolutionRequest(
        repository_root=root / "repo",
        expected_repository_head="1" * 40,
        prior_v7_root=root / "prior-v7",
        prior_v7_manifest_sha256=old.REQUIRED_PRIOR_V7_MANIFEST_SHA256,
        prior_prepared_root=root / "prior-prepared",
        prior_prepared_manifest_sha256=(
            old.REQUIRED_PRIOR_PREPARED_MANIFEST_SHA256
        ),
        prior_network_root=root / "prior-network",
        prior_network_manifest_sha256=(
            old.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
        ),
        prior_transfer_root=root / "prior-transfer",
        prior_transfer_manifest_sha256=(
            old.REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256
        ),
        prior_resolution_root=root / "prior-resolution",
        prior_resolution_manifest_sha256=(
            old.REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256
        ),
        prior_preflight_root=root / "prior-preflight",
        prior_preflight_manifest_sha256=(
            old.REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256
        ),
        prior_checkpoint_root=root / "prior-checkpoint",
        prior_checkpoint_manifest_sha256=(
            old.REQUIRED_PRIOR_CHECKPOINT_MANIFEST_SHA256
        ),
        prior_resume_failure_root=root / "prior-resume-failure",
        prior_resume_failure_manifest_sha256=(
            old.REQUIRED_PRIOR_RESUME_FAILURE_MANIFEST_SHA256
        ),
        prior_backend_resolution_root=root / "prior-backend-resolution",
        prior_backend_resolution_manifest_sha256=(
            old.REQUIRED_PRIOR_BACKEND_RESOLUTION_MANIFEST_SHA256
        ),
        prior_reactivation_root=root / "prior-reactivation",
        prior_reactivation_manifest_sha256=(
            resolution.REQUIRED_PRIOR_REACTIVATION_MANIFEST_SHA256
        ),
        prior_runtime_resolution_root=root / "prior-runtime-resolution",
        prior_runtime_resolution_manifest_sha256=(
            resolution.REQUIRED_PRIOR_RUNTIME_RESOLUTION_MANIFEST_SHA256
        ),
        source_bundle_path=root / "source.tar",
        source_bundle_size=old.REQUIRED_SOURCE_BUNDLE_SIZE,
        source_bundle_sha256=old.REQUIRED_SOURCE_BUNDLE_SHA256,
        output_root=root / "output",
        transfer_attempt_id=old.REQUIRED_TRANSFER_ATTEMPT_ID,
        resolution_attempt_id=old.REQUIRED_RESOLUTION_ATTEMPT_ID,
        preflight_attempt_id=old.REQUIRED_PREFLIGHT_ATTEMPT_ID,
        checkpoint_attempt_id=old.REQUIRED_CHECKPOINT_ATTEMPT_ID,
        resume_attempt_id=old.REQUIRED_RESUME_ATTEMPT_ID,
        backend_resolution_attempt_id=(
            old.REQUIRED_BACKEND_RESOLUTION_ATTEMPT_ID
        ),
        reactivation_attempt_id=old.REQUIRED_REACTIVATION_ATTEMPT_ID,
        prior_runtime_resolution_attempt_id=(
            resolution.REQUIRED_PRIOR_RUNTIME_RESOLUTION_ATTEMPT_ID
        ),
        runtime_resolution_attempt_id=(
            resolution.REQUIRED_RUNTIME_RESOLUTION_ATTEMPT_ID
        ),
        target_uuid=old.REQUIRED_TARGET_UUID,
        target_name=target_name,
        target_package_path=(
            old.launch_bindings.expected_target_package_path(target_name)
        ),
        expected_vm_count=21,
        identity_observations=3,
        poll_interval_seconds=1,
        command_timeout_seconds=60,
        authorized_install_artifacts_staged_runtime_resolution=True,
        authorized_one_potential_backend_reactivation_list=True,
        authorized_stable_target_handle_pid_observations=True,
        authorized_one_read_only_boot_id_hash=True,
        authorized_no_start_status_resume_business_guest_retry_stop_or_quit=True,
    )


def replace_request(
    request: resolution.RuntimeResolutionRequest, **changes: object
) -> resolution.RuntimeResolutionRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return resolution.RuntimeResolutionRequest(**values)


def baseline_inventory() -> tuple[start_control.RegisteredVm, ...]:
    return tuple(
        start_control.RegisteredVm(
            f"00000000-0000-4000-8000-{index:012d}", "stopped", f"base-{index}"
        )
        for index in range(1, 21)
    )


def inventory_stdout(
    request: resolution.RuntimeResolutionRequest,
    *,
    target_status: str,
    other_active: bool,
) -> bytes:
    rows = ["UUID Status Name"]
    for index, item in enumerate(baseline_inventory()):
        status = "started" if other_active and index == 0 else item.status
        rows.append(f"{item.uuid} {status} {item.name}")
    rows.append(f"{request.target_uuid} {target_status} {request.target_name}")
    return ("\n".join(rows) + "\n").encode("utf-8")


def quiet() -> bytes:
    return b"1 0 0 launchd\n"


def active_utmctl() -> bytes:
    return b"1 0 0 launchd\n99 1 501 utmctl\n"


def handle_payload(
    request: resolution.RuntimeResolutionRequest, pid: int, command: str
) -> bytes:
    efi = request.target_package_path / "Data/efi_vars.fd"
    qcow2 = (
        request.target_package_path
        / "Data"
        / resolution.network_ready.REQUIRED_QCOW2_NAME
    )
    return (
        f"p{pid}\nc{command}\nf1\ntREG\nn{efi}\nf2\ntREG\nn{qcow2}\n"
    ).encode("utf-8")


def target_identity(
    request: resolution.RuntimeResolutionRequest,
) -> dict[str, object]:
    return {
        "format": "synthetic-target-v1",
        "target_config_sha256": "2" * 64,
        "target_package_name": request.target_package_path.name,
        "target_package_path_sha256": resolution.network_ready._sha256_text(
            str(request.target_package_path)
        ),
        "target_descriptors": {
            "config": {"device": 1, "inode": 2, "mode": 0o600, "size": 100},
            "efi": {"device": 1, "inode": 3, "mode": 0o600, "size": 200},
            "qcow2": {"device": 1, "inode": 4, "mode": 0o600, "size": 300},
        },
    }


def success_runner(
    request: resolution.RuntimeResolutionRequest,
    *,
    boot_hash: str = ORIGINAL_BOOT_HASH,
    boot_payload: bytes | None = None,
) -> FakeRunner:
    return FakeRunner(
        request,
        process_outputs=[quiet(), quiet(), quiet(), quiet(), quiet()],
        handle_identities=[
            (42, "QEMULauncher"),
            (42, "QEMULauncher"),
            (42, "QEMULauncher"),
            (42, "QEMULauncher"),
            (42, "QEMULauncher"),
        ],
        boot_hash=boot_hash,
        boot_payload=boot_payload,
    )


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def assert_manifest_valid(test: unittest.TestCase, root: Path) -> None:
    for line in (root / "files.sha256").read_text(
        encoding="ascii"
    ).splitlines():
        digest, name = line.split("  ", 1)
        test.assertEqual(
            hashlib.sha256((root / name).read_bytes()).hexdigest(), digest
        )


if __name__ == "__main__":
    unittest.main()

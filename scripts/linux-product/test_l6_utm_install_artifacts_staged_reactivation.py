#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace

import l6_utm_install_artifacts_staged_reactivation as reactivation
import l6_utm_start_once as start_control


ORIGINAL_BOOT_HASH = hashlib.sha256(b"synthetic-original-boot").hexdigest()
NEW_BOOT_HASH = hashlib.sha256(b"synthetic-new-boot").hexdigest()


@dataclass
class SyntheticSource:
    file_object: object

    def as_json(self) -> dict[str, object]:
        return {
            "format": "synthetic-source-v1",
            "sha256": reactivation.REQUIRED_SOURCE_BUNDLE_SHA256,
            "size": reactivation.REQUIRED_SOURCE_BUNDLE_SIZE,
        }


class CloseTracker:
    def __init__(self) -> None:
        self.closed = False

    def close(self) -> None:
        self.closed = True


class FakeRunner:
    def __init__(
        self,
        request: reactivation.ReactivationRequest,
        *,
        list_status: str = "stopped",
        other_active: bool = False,
        process_outputs: list[bytes],
        handle_observations: list[start_control.CommandObservation],
        transport_observation: start_control.CommandObservation | None = None,
        boot_hash: str = ORIGINAL_BOOT_HASH,
        boot_payload: bytes | None = None,
    ) -> None:
        self.request = request
        self.process_outputs = list(process_outputs)
        self.handle_observations = list(handle_observations)
        self.transport_observation = transport_observation
        self.boot_hash = boot_hash
        self.boot_payload = boot_payload
        self.list_status = list_status
        self.other_active = other_active
        self.calls: list[tuple[str, ...]] = []

    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> start_control.CommandObservation:
        del timeout_seconds
        self.calls.append(argv)
        if argv == reactivation.PROCESS_COMMAND:
            if not self.process_outputs:
                raise AssertionError("unexpected process observation")
            return start_control.CommandObservation.from_bytes(
                argv, stdout=self.process_outputs.pop(0)
            )
        if argv == reactivation.network_ready._lsof_argv(self.request):
            if not self.handle_observations:
                raise AssertionError("unexpected handle observation")
            return self.handle_observations.pop(0)
        if argv == ("utmctl", "list"):
            return start_control.CommandObservation.from_bytes(
                argv,
                stdout=inventory_stdout(
                    self.request,
                    target_status=self.list_status,
                    other_active=self.other_active,
                ),
            )
        if argv == reactivation.launch_transport.transport_argv(self.request):
            return self.transport_observation or (
                start_control.CommandObservation.from_bytes(argv)
            )
        if argv == reactivation.boot_id_hash_argv(self.request):
            payload = self.boot_payload
            if payload is None:
                payload = f"{self.boot_hash}\n".encode("ascii")
            return start_control.CommandObservation.from_bytes(
                argv, stdout=payload
            )
        raise AssertionError(f"unexpected command: {argv}")


class ReactivationTests(unittest.TestCase):
    def test_stopped_target_starts_once_and_restores_original_boot(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary), settle_attempts=2)
            runner = FakeRunner(
                request,
                process_outputs=[quiet(), quiet(), quiet(), quiet(), runtime()],
                handle_observations=[
                    absent_handles(request),
                    absent_handles(request),
                    absent_handles(request),
                    absent_handles(request),
                    present_handles(request),
                ],
            )

            result, source = run_case(request, runner)

            self.assertEqual(result.outcome, "original-boot-restored")
            self.assertEqual(
                result.exit_code, reactivation.EXIT_ORIGINAL_BOOT_RESTORED
            )
            self.assertEqual(result.inventory_probe_invocations, 1)
            self.assertEqual(result.foreground_start_invocations, 1)
            self.assertEqual(result.guest_boot_hash_invocations, 1)
            self.assertTrue(source.file_object.closed)
            self.assertEqual(
                sum(
                    call == reactivation.launch_transport.transport_argv(request)
                    for call in runner.calls
                ),
                1,
            )
            self.assert_terminal_safety(request, runner.calls)
            assert_manifest_valid(self, request.output_root)

    def test_new_boot_is_separate_terminal_outcome(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = stopped_success_runner(
                request, boot_hash=NEW_BOOT_HASH
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "new-boot-started")
            self.assertEqual(result.exit_code, reactivation.EXIT_NEW_BOOT_STARTED)
            boot = read_json(request.output_root / "guest-boot-id-hash-once.json")
            self.assertEqual(boot["classification"], "new-boot-started")
            self.assertEqual(boot["observed_boot_id_sha256"], NEW_BOOT_HASH)
            self.assert_terminal_safety(request, runner.calls)

    def test_inventory_probe_reactivated_target_skips_foreground_start(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary), settle_attempts=2)
            runner = FakeRunner(
                request,
                list_status="started",
                process_outputs=[quiet(), runtime()],
                handle_observations=[
                    absent_handles(request),
                    present_handles(request),
                ],
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "original-boot-restored")
            self.assertEqual(result.foreground_start_invocations, 0)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(
                terminal["runtime_observation_source"],
                "inventory-probe-reactivated",
            )
            self.assertFalse(
                any(
                    call == reactivation.launch_transport.transport_argv(request)
                    for call in runner.calls
                )
            )

    def test_registered_started_without_runtime_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary), settle_attempts=2)
            runner = FakeRunner(
                request,
                list_status="started",
                process_outputs=[quiet(), quiet(), quiet()],
                handle_observations=[
                    absent_handles(request),
                    absent_handles(request),
                    absent_handles(request),
                ],
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(
                result.exit_code, reactivation.EXIT_STATE_INDETERMINATE
            )
            self.assertEqual(result.foreground_start_invocations, 0)
            self.assertEqual(result.guest_boot_hash_invocations, 0)

    def test_other_registered_vm_active_blocks_start(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = FakeRunner(
                request,
                other_active=True,
                process_outputs=[quiet(), quiet()],
                handle_observations=[
                    absent_handles(request),
                    absent_handles(request),
                ],
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.foreground_start_invocations, 0)
            self.assertEqual(result.guest_boot_hash_invocations, 0)
            self.assertIn(
                "inventory-unresolved",
                read_json(request.output_root / "terminal.json")["reason"],
            )

    def test_stopped_inventory_with_partial_backend_blocks_start(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary), settle_attempts=2)
            runner = FakeRunner(
                request,
                process_outputs=[quiet(), quiet(), runtime()],
                handle_observations=[
                    absent_handles(request),
                    absent_handles(request),
                    present_handles(request),
                ],
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.foreground_start_invocations, 0)
            self.assertEqual(result.guest_boot_hash_invocations, 0)
            self.assertIn(
                "registered-stopped-with-nonquiescent",
                read_json(request.output_root / "terminal.json")["reason"],
            )

    def test_transport_failure_without_runtime_is_not_retried(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary), runtime_poll_attempts=2)
            transport_argv = reactivation.launch_transport.transport_argv(request)
            runner = FakeRunner(
                request,
                process_outputs=[quiet(), quiet(), quiet(), quiet(), quiet()],
                handle_observations=[
                    absent_handles(request),
                    absent_handles(request),
                    absent_handles(request),
                    absent_handles(request),
                    absent_handles(request),
                ],
                transport_observation=start_control.CommandObservation.from_bytes(
                    transport_argv, exit_code=1, stderr=b"synthetic failure\n"
                ),
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.foreground_start_invocations, 1)
            self.assertEqual(result.guest_boot_hash_invocations, 0)
            self.assertEqual(sum(call == transport_argv for call in runner.calls), 1)

    def test_malformed_boot_hash_is_indeterminate_and_not_retried(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = stopped_success_runner(
                request, boot_payload=b"not-a-boot-hash\n"
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.guest_boot_hash_invocations, 1)
            self.assertEqual(
                sum(
                    call == reactivation.boot_id_hash_argv(request)
                    for call in runner.calls
                ),
                1,
            )

    def test_preflight_target_handles_reject_before_global_probe(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            runner = FakeRunner(
                request,
                process_outputs=[quiet()],
                handle_observations=[present_handles(request)],
            )

            result, _ = run_case(request, runner)

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.inventory_probe_invocations, 0)
            self.assertNotIn(("utmctl", "list"), runner.calls)

    def test_authorization_and_output_overlap_fail_before_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            unauthorized = replace_request(
                request, authorized_one_read_only_boot_id_hash=False
            )
            with self.assertRaisesRegex(
                reactivation.ReactivationError,
                "authorized-one-read-only-boot-id-hash-required",
            ):
                unauthorized.validate()

            overlapping = replace_request(
                request,
                output_root=request.prior_backend_resolution_root / "new",
            )
            with self.assertRaisesRegex(
                reactivation.ReactivationError,
                "output-root-must-not-overlap-prior-backend-resolution-root",
            ):
                overlapping.validate()

    def assert_terminal_safety(
        self,
        request: reactivation.ReactivationRequest,
        calls: list[tuple[str, ...]],
    ) -> None:
        terminal = read_json(request.output_root / "terminal.json")
        self.assertEqual(terminal["business_guest_action"], "not-performed")
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
    request: reactivation.ReactivationRequest,
    runner: FakeRunner,
) -> tuple[reactivation.ReactivationResult, SyntheticSource]:
    source = SyntheticSource(CloseTracker())
    result = reactivation.run_reactivation(
        request,
        runner=runner,
        binding_validator=lambda _: SimpleNamespace(
            evidence={"format": reactivation.EVIDENCE_FORMAT},
            baseline_inventory=baseline_inventory(),
            expected_boot_id_sha256=ORIGINAL_BOOT_HASH,
        ),
        source_opener=lambda _: source,
        source_revalidator=lambda _request, _source: {
            "descriptor_unchanged": True,
            "format": "synthetic-source-v1",
            "inventory_unchanged": True,
            "sha256": reactivation.REQUIRED_SOURCE_BUNDLE_SHA256,
            "size": reactivation.REQUIRED_SOURCE_BUNDLE_SIZE,
        },
        target_validator=lambda _: {
            "format": "synthetic-target-v1",
            "target_uuid": request.target_uuid,
        },
        sleeper=lambda _: None,
    )
    return result, source


def make_request(
    root: Path,
    *,
    settle_attempts: int = 1,
    runtime_poll_attempts: int = 1,
) -> reactivation.ReactivationRequest:
    target_name = reactivation.REQUIRED_TARGET_NAME
    return reactivation.ReactivationRequest(
        repository_root=root / "repo",
        expected_repository_head="1" * 40,
        prior_v7_root=root / "prior-v7",
        prior_v7_manifest_sha256=(
            reactivation.REQUIRED_PRIOR_V7_MANIFEST_SHA256
        ),
        prior_prepared_root=root / "prior-prepared",
        prior_prepared_manifest_sha256=(
            reactivation.REQUIRED_PRIOR_PREPARED_MANIFEST_SHA256
        ),
        prior_network_root=root / "prior-network",
        prior_network_manifest_sha256=(
            reactivation.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
        ),
        prior_transfer_root=root / "prior-transfer",
        prior_transfer_manifest_sha256=(
            reactivation.REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256
        ),
        prior_resolution_root=root / "prior-resolution",
        prior_resolution_manifest_sha256=(
            reactivation.REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256
        ),
        prior_preflight_root=root / "prior-preflight",
        prior_preflight_manifest_sha256=(
            reactivation.REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256
        ),
        prior_checkpoint_root=root / "prior-checkpoint",
        prior_checkpoint_manifest_sha256=(
            reactivation.REQUIRED_PRIOR_CHECKPOINT_MANIFEST_SHA256
        ),
        prior_resume_failure_root=root / "prior-resume-failure",
        prior_resume_failure_manifest_sha256=(
            reactivation.REQUIRED_PRIOR_RESUME_FAILURE_MANIFEST_SHA256
        ),
        prior_backend_resolution_root=root / "prior-backend-resolution",
        prior_backend_resolution_manifest_sha256=(
            reactivation.REQUIRED_PRIOR_BACKEND_RESOLUTION_MANIFEST_SHA256
        ),
        source_bundle_path=root / "source.tar",
        source_bundle_size=reactivation.REQUIRED_SOURCE_BUNDLE_SIZE,
        source_bundle_sha256=reactivation.REQUIRED_SOURCE_BUNDLE_SHA256,
        output_root=root / "output",
        transfer_attempt_id=reactivation.REQUIRED_TRANSFER_ATTEMPT_ID,
        resolution_attempt_id=reactivation.REQUIRED_RESOLUTION_ATTEMPT_ID,
        preflight_attempt_id=reactivation.REQUIRED_PREFLIGHT_ATTEMPT_ID,
        checkpoint_attempt_id=reactivation.REQUIRED_CHECKPOINT_ATTEMPT_ID,
        resume_attempt_id=reactivation.REQUIRED_RESUME_ATTEMPT_ID,
        backend_resolution_attempt_id=(
            reactivation.REQUIRED_BACKEND_RESOLUTION_ATTEMPT_ID
        ),
        reactivation_attempt_id=reactivation.REQUIRED_REACTIVATION_ATTEMPT_ID,
        target_uuid=reactivation.REQUIRED_TARGET_UUID,
        target_name=target_name,
        target_package_path=(
            reactivation.launch_bindings.expected_target_package_path(
                target_name
            )
        ),
        expected_vm_count=21,
        settle_attempts=settle_attempts,
        runtime_poll_attempts=runtime_poll_attempts,
        poll_interval_seconds=1,
        transport_timeout_seconds=60,
        command_timeout_seconds=60,
        authorized_install_artifacts_staged_reactivation=True,
        authorized_one_potential_backend_reactivation_list=True,
        authorized_one_foreground_start_if_stopped_quiescent=True,
        authorized_one_read_only_boot_id_hash=True,
        authorized_no_business_guest_resume_retry_stop_or_quit=True,
    )


def replace_request(
    request: reactivation.ReactivationRequest, **changes: object
) -> reactivation.ReactivationRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return reactivation.ReactivationRequest(**values)


def baseline_inventory() -> tuple[start_control.RegisteredVm, ...]:
    return tuple(
        start_control.RegisteredVm(
            f"00000000-0000-4000-8000-{index:012d}", "stopped", f"base-{index}"
        )
        for index in range(1, 21)
    )


def inventory_stdout(
    request: reactivation.ReactivationRequest,
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


def runtime() -> bytes:
    return b"1 0 0 launchd\n42 1 501 QEMULauncher\n"


def absent_handles(
    request: reactivation.ReactivationRequest,
) -> start_control.CommandObservation:
    argv = reactivation.network_ready._lsof_argv(request)
    return start_control.CommandObservation.from_bytes(argv, exit_code=1)


def present_handles(
    request: reactivation.ReactivationRequest,
) -> start_control.CommandObservation:
    efi = request.target_package_path / "Data/efi_vars.fd"
    qcow2 = (
        request.target_package_path
        / "Data"
        / reactivation.network_ready.REQUIRED_QCOW2_NAME
    )
    payload = (
        f"p42\ncQEMULauncher\nf1\ntREG\nn{efi}\nf2\ntREG\nn{qcow2}\n"
    ).encode("utf-8")
    return start_control.CommandObservation.from_bytes(
        reactivation.network_ready._lsof_argv(request), stdout=payload
    )


def stopped_success_runner(
    request: reactivation.ReactivationRequest,
    *,
    boot_hash: str = ORIGINAL_BOOT_HASH,
    boot_payload: bytes | None = None,
) -> FakeRunner:
    return FakeRunner(
        request,
        process_outputs=[quiet(), quiet(), quiet(), runtime()],
        handle_observations=[
            absent_handles(request),
            absent_handles(request),
            absent_handles(request),
            present_handles(request),
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

#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace

import l6_utm_install_artifacts_staged_backend_resolution as resolution
import l6_utm_start_once as start_control


@dataclass
class SyntheticSource:
    file_object: object

    def as_json(self) -> dict[str, object]:
        return {
            "format": "synthetic-source-v1",
            "sha256": resolution.REQUIRED_SOURCE_BUNDLE_SHA256,
            "size": resolution.REQUIRED_SOURCE_BUNDLE_SIZE,
        }


class CloseTracker:
    def __init__(self) -> None:
        self.closed = False

    def close(self) -> None:
        self.closed = True


class FakeRunner:
    def __init__(
        self,
        request: resolution.BackendResolutionRequest,
        *,
        status: bytes = b"started\n",
        status_exit_code: int = 0,
        polls: list[tuple[bytes, start_control.CommandObservation]] | None = None,
        preflight_handles: start_control.CommandObservation | None = None,
    ) -> None:
        self.request = request
        self.status = status
        self.status_exit_code = status_exit_code
        self.polls = polls or [(b"1 0 0 launchd\n", absent_handles(request))]
        self.preflight_handles = preflight_handles or absent_handles(request)
        self.calls: list[tuple[str, ...]] = []
        self.process_calls = 0
        self.handle_calls = 0

    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> start_control.CommandObservation:
        del timeout_seconds
        self.calls.append(argv)
        if argv == resolution.PROCESS_COMMAND:
            self.process_calls += 1
            if self.process_calls == 1:
                stdout = b"1 0 0 launchd\n"
            else:
                stdout = self.polls[self.process_calls - 2][0]
            return start_control.CommandObservation.from_bytes(
                argv, stdout=stdout
            )
        if argv == resolution.network_ready._lsof_argv(self.request):
            self.handle_calls += 1
            if self.handle_calls == 1:
                return self.preflight_handles
            return self.polls[self.handle_calls - 2][1]
        if argv == ("utmctl", "status", self.request.target_uuid):
            return start_control.CommandObservation.from_bytes(
                argv,
                exit_code=self.status_exit_code,
                stdout=self.status,
            )
        raise AssertionError(f"unexpected command: {argv}")


class BackendResolutionTests(unittest.TestCase):
    def test_single_status_observes_runtime_without_follow_on_action(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary), poll_attempts=3)
            polls = [
                (b"1 0 0 launchd\n", absent_handles(request)),
                (
                    b"1 0 0 launchd\n42 1 501 QEMULauncher\n",
                    present_handles(request),
                ),
            ]
            runner = FakeRunner(request, polls=polls)

            result, source = run_resolution(request, runner)

            self.assertEqual(result.outcome, "runtime-reactivated")
            self.assertEqual(result.exit_code, resolution.EXIT_RUNTIME_REACTIVATED)
            self.assertEqual(result.backend_reactivation_probe_invocations, 1)
            self.assertEqual(result.observation_poll_count, 2)
            self.assertTrue(source.file_object.closed)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["registered_status"], "started")
            self.assertEqual(terminal["target_handles_terminal"], "present")
            self.assertEqual(terminal["maintenance_resume_invocations"], 0)
            self.assertEqual(
                terminal["plain_utmctl_status"],
                "attempted-once-as-potential-backend-reactivation",
            )
            self.assert_forbidden_actions_absent(runner.calls)
            assert_manifest_valid(self, request.output_root)

    def test_registered_stopped_requires_all_bounded_polls_quiescent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary), poll_attempts=2)
            runner = FakeRunner(
                request,
                status=b"stopped\n",
                polls=[
                    (b"1 0 0 launchd\n", absent_handles(request)),
                    (b"1 0 0 launchd\n", absent_handles(request)),
                ],
            )

            result, _ = run_resolution(request, runner)

            self.assertEqual(result.outcome, "registered-stopped")
            self.assertEqual(result.exit_code, resolution.EXIT_REGISTERED_STOPPED)
            self.assertEqual(result.observation_poll_count, 2)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(
                terminal["saved_state"],
                "registered-stopped-saved-state-not-inferred",
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_started_without_backend_does_not_infer_saved_state(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary), poll_attempts=2)
            runner = FakeRunner(
                request,
                polls=[
                    (b"1 0 0 launchd\n", absent_handles(request)),
                    (b"1 0 0 launchd\n", absent_handles(request)),
                ],
            )

            result, _ = run_resolution(request, runner)

            self.assertEqual(
                result.outcome, "registered-started-runtime-unavailable"
            )
            self.assertEqual(result.exit_code, resolution.EXIT_STATE_INDETERMINATE)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(
                terminal["saved_state"],
                "saved-or-runtime-unavailable-not-distinguished-by-status",
            )

    def test_status_failure_still_runs_postflight_and_stays_indeterminate(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary), poll_attempts=1)
            runner = FakeRunner(
                request,
                status=b"",
                status_exit_code=1,
            )

            result, _ = run_resolution(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.backend_reactivation_probe_invocations, 1)
            self.assertEqual(result.observation_poll_count, 1)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertIn("utmctl-status-exit-1", terminal["reason"])
            self.assert_forbidden_actions_absent(runner.calls)

    def test_preflight_handles_present_rejects_before_status(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary), poll_attempts=1)
            runner = FakeRunner(
                request, preflight_handles=present_handles(request)
            )

            result, _ = run_resolution(request, runner)

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(
                result.exit_code, resolution.EXIT_PRECONDITION_REJECTED
            )
            self.assertEqual(result.backend_reactivation_probe_invocations, 0)
            self.assertFalse(
                any(call[:2] == ("utmctl", "status") for call in runner.calls)
            )

    def test_partial_backend_observation_is_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary), poll_attempts=1)
            runner = FakeRunner(
                request,
                polls=[
                    (
                        b"1 0 0 launchd\n42 1 501 QEMULauncher\n",
                        absent_handles(request),
                    )
                ],
            )

            result, _ = run_resolution(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertIn(
                "inconsistent",
                read_json(request.output_root / "terminal.json")["reason"],
            )

    def test_authorization_and_output_overlap_fail_before_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = make_request(root)
            unauthorized = replace_request(
                request,
                authorized_one_potential_backend_reactivation_status=False,
            )
            with self.assertRaisesRegex(
                resolution.BackendResolutionError,
                "authorized-one-potential-backend-reactivation-status-required",
            ):
                unauthorized.validate()

            overlapping = replace_request(
                request, output_root=request.prior_resume_failure_root / "new"
            )
            with self.assertRaisesRegex(
                resolution.BackendResolutionError,
                "output-root-must-not-overlap-prior-resume-failure-root",
            ):
                overlapping.validate()

    def assert_forbidden_actions_absent(
        self, calls: list[tuple[str, ...]]
    ) -> None:
        self.assertEqual(
            sum(call[:2] == ("utmctl", "status") for call in calls), 1
        )
        flattened = [" ".join(call) for call in calls]
        for forbidden in (
            "utmctl list",
            "utmctl start",
            "utmctl exec",
            "utmctl file",
            "utmctl stop",
            " resume ",
            " retry ",
            " quit ",
        ):
            self.assertFalse(any(forbidden in value for value in flattened))


def run_resolution(
    request: resolution.BackendResolutionRequest,
    runner: FakeRunner,
) -> tuple[resolution.BackendResolutionResult, SyntheticSource]:
    source = SyntheticSource(CloseTracker())
    result = resolution.run_backend_resolution(
        request,
        runner=runner,
        binding_validator=lambda _: SimpleNamespace(
            evidence={
                "format": resolution.EVIDENCE_FORMAT,
                "prior_resume_failure_entries_verified": 7,
            }
        ),
        source_opener=lambda _: source,
        source_revalidator=lambda _request, _source: {
            "descriptor_unchanged": True,
            "format": "synthetic-source-v1",
            "inventory_unchanged": True,
            "sha256": resolution.REQUIRED_SOURCE_BUNDLE_SHA256,
            "size": resolution.REQUIRED_SOURCE_BUNDLE_SIZE,
        },
        target_validator=lambda _: {
            "format": "synthetic-target-v1",
            "target_uuid": request.target_uuid,
        },
        sleeper=lambda _: None,
    )
    return result, source


def make_request(
    root: Path, *, poll_attempts: int = 1
) -> resolution.BackendResolutionRequest:
    target_name = resolution.REQUIRED_TARGET_NAME
    return resolution.BackendResolutionRequest(
        repository_root=root / "repo",
        expected_repository_head="1" * 40,
        prior_network_root=root / "prior-network",
        prior_network_manifest_sha256=(
            resolution.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
        ),
        prior_transfer_root=root / "prior-transfer",
        prior_transfer_manifest_sha256=(
            resolution.REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256
        ),
        prior_resolution_root=root / "prior-resolution",
        prior_resolution_manifest_sha256=(
            resolution.REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256
        ),
        prior_preflight_root=root / "prior-preflight",
        prior_preflight_manifest_sha256=(
            resolution.REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256
        ),
        prior_checkpoint_root=root / "prior-checkpoint",
        prior_checkpoint_manifest_sha256=(
            resolution.REQUIRED_PRIOR_CHECKPOINT_MANIFEST_SHA256
        ),
        prior_resume_failure_root=root / "prior-resume-failure",
        prior_resume_failure_manifest_sha256=(
            resolution.REQUIRED_PRIOR_RESUME_FAILURE_MANIFEST_SHA256
        ),
        source_bundle_path=root / "source.tar",
        source_bundle_size=resolution.REQUIRED_SOURCE_BUNDLE_SIZE,
        source_bundle_sha256=resolution.REQUIRED_SOURCE_BUNDLE_SHA256,
        output_root=root / "output",
        transfer_attempt_id=resolution.REQUIRED_TRANSFER_ATTEMPT_ID,
        resolution_attempt_id=resolution.REQUIRED_RESOLUTION_ATTEMPT_ID,
        preflight_attempt_id=resolution.REQUIRED_PREFLIGHT_ATTEMPT_ID,
        checkpoint_attempt_id=resolution.REQUIRED_CHECKPOINT_ATTEMPT_ID,
        resume_attempt_id=resolution.REQUIRED_RESUME_ATTEMPT_ID,
        backend_resolution_attempt_id=(
            resolution.REQUIRED_BACKEND_RESOLUTION_ATTEMPT_ID
        ),
        target_uuid=resolution.REQUIRED_TARGET_UUID,
        target_name=target_name,
        target_package_path=(
            Path("/Users/luobo/Library/Containers/com.utmapp.UTM/Data/Documents")
            / f"{target_name}.utm"
        ),
        poll_attempts=poll_attempts,
        poll_interval_seconds=1,
        command_timeout_seconds=60,
        authorized_install_artifacts_staged_backend_resolution=True,
        authorized_one_potential_backend_reactivation_status=True,
        authorized_no_list_start_resume_guest_retry_stop_or_quit=True,
    )


def replace_request(
    request: resolution.BackendResolutionRequest, **changes: object
) -> resolution.BackendResolutionRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return resolution.BackendResolutionRequest(**values)


def absent_handles(
    request: resolution.BackendResolutionRequest,
) -> start_control.CommandObservation:
    argv = resolution.network_ready._lsof_argv(request)
    return start_control.CommandObservation.from_bytes(argv, exit_code=1)


def present_handles(
    request: resolution.BackendResolutionRequest,
) -> start_control.CommandObservation:
    efi = request.target_package_path / "Data/efi_vars.fd"
    qcow2 = (
        request.target_package_path
        / "Data"
        / resolution.network_ready.REQUIRED_QCOW2_NAME
    )
    payload = (
        f"p42\ncQEMULauncher\nf1\ntREG\nn{efi}\nf2\ntREG\nn{qcow2}\n"
    ).encode("utf-8")
    return start_control.CommandObservation.from_bytes(
        resolution.network_ready._lsof_argv(request), stdout=payload
    )


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


def assert_manifest_valid(test: unittest.TestCase, root: Path) -> None:
    manifest = root / "files.sha256"
    for line in manifest.read_text(encoding="ascii").splitlines():
        digest, name = line.split("  ", 1)
        test.assertEqual(
            hashlib.sha256((root / name).read_bytes()).hexdigest(), digest
        )


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import io
import json
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path

import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_install_artifacts_staged_resume as resume
import l6_utm_install_artifacts_staged_resume_bindings as bindings
import l6_utm_install_artifacts_staged_resume_evidence as evidence
import l6_utm_start_once as start_control
import l6_v4_install_artifacts_staged_resume_driver as driver


NETWORK_EVIDENCE = b"synthetic frozen loopback network evidence\n"
LIVE_ARTIFACTS = {
    "guest-identity": (
        "/var/tmp/guest-identity.evidence.txt",
        b"synthetic guest identity\n",
    ),
    "mutation-preflight": (
        "/var/tmp/mutation-preflight.evidence.txt",
        b"synthetic mutation preflight\n",
    ),
    "checkpoint": (
        "/var/tmp/checkpoint.evidence.json",
        b'{"synthetic":"checkpoint"}\n',
    ),
    "crash-state": (
        "/var/tmp/crash-state.evidence.txt",
        b"synthetic crash state\n",
    ),
}


@dataclass
class SyntheticResume:
    terminal: bytes
    artifacts: dict[str, bytes]


class FakeRunner:
    def __init__(
        self,
        request: resume.InstallArtifactsStagedResumeRequest,
        binding: bindings.ResumeBinding,
        synthetic: SyntheticResume,
        *,
        driver_exit_code: int = 0,
        drift_second_terminal: bool = False,
    ) -> None:
        self.request = request
        self.binding = binding
        self.synthetic = synthetic
        self.driver_exit_code = driver_exit_code
        self.drift_second_terminal = drift_second_terminal
        self.calls: list[tuple[str, ...]] = []
        self.terminal_pulls = 0

    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file=None,
    ) -> start_control.CommandObservation:
        del timeout_seconds
        self.calls.append(argv)
        stdout = b""
        exit_code = 0
        if argv == resume.PROCESS_COMMAND:
            stdout = b"1 0 0 launchd\n"
        elif argv == resume.network_ready._lsof_argv(self.request):
            stdout = lsof_output(self.request)
        elif argv[:3] == ("utmctl", "file", "push"):
            if stdin_file is None:
                raise AssertionError("driver push requires stdin")
            stdin_file.seek(0)
            if stdin_file.read() != self.binding.driver_bytes:
                raise AssertionError("driver push drift")
            stdin_file.seek(0)
        elif argv[:3] == ("utmctl", "file", "pull"):
            stdout = self.payload_for_path(argv[4])
        elif argv == resume.driver_argv(self.request, self.binding):
            exit_code = self.driver_exit_code
        elif argv[:2] == ("utmctl", "exec"):
            pass
        else:
            raise AssertionError(f"unexpected command: {argv}")
        return start_control.CommandObservation.from_bytes(
            argv, exit_code=exit_code, timed_out=False, stdout=stdout, stderr=b""
        )

    def payload_for_path(self, path: str) -> bytes:
        if path == self.binding.network.guest_evidence_path:
            return NETWORK_EVIDENCE
        for _, (guest_path, payload) in self.binding.live_artifacts.items():
            if path == guest_path:
                return payload
        if path == self.request.guest_driver_path:
            return self.binding.driver_bytes
        if path == self.request.guest_marker_path:
            return driver.canonical_json(
                {
                    "driver_sha256": hashlib.sha256(
                        self.binding.driver_bytes
                    ).hexdigest(),
                    "format": driver.MARKER_FORMAT,
                    "resume_attempt_id": self.request.resume_attempt_id,
                }
            )
        if path == self.request.guest_terminal_path:
            self.terminal_pulls += 1
            if self.drift_second_terminal and self.terminal_pulls == 2:
                return self.synthetic.terminal + b"drift"
            return self.synthetic.terminal
        if path == self.request.guest_phase_path:
            terminal = json.loads(self.synthetic.terminal)
            return driver.canonical_json(
                {"format": driver.EVIDENCE_FORMAT, "phase": terminal["phase"]}
            )
        for name, payload in self.synthetic.artifacts.items():
            if artifact_guest_path(self.request, name) == path:
                return payload
        raise AssertionError(f"unexpected pull path: {path}")


class LinuxL6InstallArtifactsStagedResumeTests(unittest.TestCase):
    def test_one_shot_resume_completes_once_without_stop_or_retry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = valid_binding(request)
            synthetic = successful_resume(request, binding)
            runner = FakeRunner(request, binding, synthetic)

            result = run_resume(request, binding, runner)

            self.assertEqual(result.outcome, "resume-completed")
            self.assertEqual(result.exit_code, resume.EXIT_RESUME_COMPLETED)
            self.assertEqual(result.resume_invocations, 1)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["maintenance_resume_invocations"], 1)
            self.assertEqual(terminal["postflight_invocations"], 1)
            self.assertEqual(terminal["operation_id"], "existing-secret-hash-only")
            self.assertEqual(terminal["automatic_retry"], "not-performed")
            self.assertEqual(terminal["automatic_stop"], "not-performed")
            self.assertEqual(terminal["automatic_next_checkpoint"], "not-performed")
            self.assertEqual(terminal["file_push_invocations"], 1)
            self.assertEqual(terminal["guest_exec_invocations"], 5)
            assert_manifest_valid(self, request.output_root)
            commands = [" ".join(call) for call in runner.calls]
            driver_call = resume.driver_argv(request, binding)
            self.assertEqual(sum(call == driver_call for call in runner.calls), 1)
            for forbidden in (
                "utmctl list",
                "utmctl status",
                "utmctl start",
                "utmctl stop",
                " cleanup ",
                " retry ",
                " quit ",
            ):
                self.assertFalse(any(forbidden in command for command in commands))
            all_evidence = b"".join(
                path.read_bytes()
                for path in request.output_root.iterdir()
                if path.is_file()
            )
            self.assertNotRegex(all_evidence, evidence.RAW_OPERATION_TOKEN)

    def test_terminal_double_readback_drift_is_indeterminate_without_retry(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = valid_binding(request)
            runner = FakeRunner(
                request,
                binding,
                successful_resume(request, binding),
                drift_second_terminal=True,
            )

            result = run_resume(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, resume.EXIT_STATE_INDETERMINATE)
            self.assertEqual(
                sum(
                    call == resume.driver_argv(request, binding)
                    for call in runner.calls
                ),
                1,
            )

    def test_pre_resume_rejection_preserves_checkpoint(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = valid_binding(request)
            rejected = rejected_resume(request, binding)
            runner = FakeRunner(
                request, binding, rejected, driver_exit_code=10
            )

            result = run_resume(request, binding, runner)

            self.assertEqual(result.outcome, "resume-rejected")
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["maintenance_resume_invocations"], 0)
            self.assertEqual(terminal["transaction"], "artifacts-staged-preserved")

    def test_success_artifact_hash_drift_is_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = valid_binding(request)
            synthetic = successful_resume(request, binding)
            synthetic.artifacts["dpkg-delta"] += b"drift\n"
            runner = FakeRunner(request, binding, synthetic)

            result = run_resume(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.resume_invocations, 1)
            self.assertEqual(
                sum(
                    call == resume.driver_argv(request, binding)
                    for call in runner.calls
                ),
                1,
            )

    def test_authorization_and_output_overlap_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            unauthorized = resume.InstallArtifactsStagedResumeRequest(
                **{
                    **request.__dict__,
                    "authorized_one_resume_and_one_postflight": False,
                }
            )
            with self.assertRaisesRegex(
                resume.InstallArtifactsStagedResumeError,
                "authorized-one-resume-and-one-postflight-required",
            ):
                unauthorized.validate()
            overlap = resume.InstallArtifactsStagedResumeRequest(
                **{
                    **request.__dict__,
                    "output_root": request.prior_checkpoint_root / "overwrite",
                }
            )
            with self.assertRaisesRegex(
                resume.InstallArtifactsStagedResumeError,
                "output-root-must-not-overlap-prior-checkpoint-root",
            ):
                overlap.validate()

    def test_binding_names_all_49_frozen_checkpoint_members(self) -> None:
        self.assertEqual(len(bindings.REQUIRED_CHECKPOINT_ENTRY_NAMES), 49)
        self.assertEqual(
            bindings.REQUIRED_CHECKPOINT_ENTRY_NAMES[-1], "terminal.json"
        )
        self.assertEqual(
            bindings.REQUIRED_PRIOR_CHECKPOINT_MANIFEST_SHA256,
            "3aca0576eb91ae2916374d1f175d98276758ccf594f2e48152eb7814ec60a4f7",
        )

    def test_evidence_rejects_raw_operation_id(self) -> None:
        request = synthetic_protocol_request()
        binding = synthetic_protocol_binding()
        payload = successful_resume(request, binding).terminal
        value = json.loads(payload)
        value["diagnostic"] = "0" * 32

        with self.assertRaisesRegex(
            evidence.ResumeEvidenceError, "contains-raw-operation-id"
        ):
            evidence.parse_driver_terminal(
                driver.canonical_json(value), request, binding
            )


def make_request(root: Path) -> resume.InstallArtifactsStagedResumeRequest:
    repository = root / "repository"
    source = root / "source" / "canonical-input.ustar"
    for directory in (
        repository,
        root / "network",
        root / "transfer",
        root / "resolution",
        root / "preflight",
        root / "checkpoint",
        source.parent,
    ):
        directory.mkdir(parents=True, exist_ok=True)
    driver_source = Path(driver.__file__).resolve()
    driver_destination = repository / resume.DRIVER_RELATIVE_PATH
    driver_destination.parent.mkdir(parents=True, exist_ok=True)
    driver_destination.write_bytes(driver_source.read_bytes())
    return resume.InstallArtifactsStagedResumeRequest(
        repository_root=repository,
        expected_repository_head="b" * 40,
        prior_network_root=root / "network",
        prior_network_manifest_sha256=resume.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256,
        prior_transfer_root=root / "transfer",
        prior_transfer_manifest_sha256=resume.REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256,
        prior_resolution_root=root / "resolution",
        prior_resolution_manifest_sha256=resume.REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256,
        prior_preflight_root=root / "preflight",
        prior_preflight_manifest_sha256=resume.REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256,
        prior_checkpoint_root=root / "checkpoint",
        prior_checkpoint_manifest_sha256=resume.REQUIRED_PRIOR_CHECKPOINT_MANIFEST_SHA256,
        source_bundle_path=source,
        source_bundle_size=resume.REQUIRED_SOURCE_BUNDLE_SIZE,
        source_bundle_sha256=resume.REQUIRED_SOURCE_BUNDLE_SHA256,
        output_root=root / "output",
        transfer_attempt_id=resume.REQUIRED_TRANSFER_ATTEMPT_ID,
        resolution_attempt_id=resume.REQUIRED_RESOLUTION_ATTEMPT_ID,
        preflight_attempt_id=resume.REQUIRED_PREFLIGHT_ATTEMPT_ID,
        checkpoint_attempt_id=resume.REQUIRED_CHECKPOINT_ATTEMPT_ID,
        resume_attempt_id=resume.REQUIRED_RESUME_ATTEMPT_ID,
        target_uuid=resume.REQUIRED_TARGET_UUID,
        target_name=resume.REQUIRED_TARGET_NAME,
        target_package_path=resume.transport_bindings.expected_target_package_path(
            resume.REQUIRED_TARGET_NAME
        ),
        command_timeout_seconds=60,
        resume_timeout_seconds=120,
        evidence_settle_seconds=10,
        authorized_install_artifacts_staged_exact_resume=True,
        authorized_existing_operation_secret_hash_only=True,
        authorized_one_resume_and_one_postflight=True,
        authorized_no_retry_cleanup_stop_quit_or_next_checkpoint=True,
    )


def valid_binding(
    request: resume.InstallArtifactsStagedResumeRequest,
) -> bindings.ResumeBinding:
    driver_bytes = (request.repository_root / resume.DRIVER_RELATIVE_PATH).read_bytes()
    return bindings.ResumeBinding(
        evidence={"format": resume.EVIDENCE_FORMAT},
        network=input_transfer.NetworkBinding(
            evidence={"format": resume.EVIDENCE_FORMAT},
            guest_evidence_path="/var/tmp/network-v2/network.evidence",
            guest_evidence_bytes=NETWORK_EVIDENCE,
        ),
        driver_bytes=driver_bytes,
        boot_id_sha256=driver.EXPECTED_BOOT_ID_SHA256,
        operation_id_sha256=driver.EXPECTED_OPERATION_ID_SHA256,
        checkpoint_sha256=driver.EXPECTED_CHECKPOINT_SHA256,
        crash_state_sha256=driver.EXPECTED_CRASH_STATE_SHA256,
        receipt_sha256=driver.EXPECTED_RECEIPT_SHA256,
        live_artifacts=dict(LIVE_ARTIFACTS),
    )


def run_resume(
    request: resume.InstallArtifactsStagedResumeRequest,
    binding: bindings.ResumeBinding,
    runner: FakeRunner,
) -> resume.InstallArtifactsStagedResumeResult:
    source = input_transfer.SourceBundle(
        file_object=io.BytesIO(b""), descriptor={"synthetic": True}, members=()
    )
    return resume.run_install_artifacts_staged_resume(
        request,
        runner=runner,
        binding_validator=lambda _: binding,
        source_opener=lambda _: source,
        source_revalidator=lambda _request, _source: {
            "format": resume.EVIDENCE_FORMAT,
            "source_bundle_identity": "matched",
        },
        target_validator=lambda _: {
            "format": resume.EVIDENCE_FORMAT,
            "target_uuid": request.target_uuid,
        },
        sleeper=lambda _: None,
    )


def successful_resume(
    request: object,
    binding: object,
) -> SyntheticResume:
    resume_result = fields(
        format="radishlex-linux-l6-install-artifacts-staged-resume-result-v1",
        operation_id_sha256=binding.operation_id_sha256,
        crash_state_sha256=binding.crash_state_sha256,
        maintenance_invocations="1",
        maintenance_exit_code="0",
        maintenance_stdout="maintenance_outcome=completed",
        maintenance_stderr="empty",
        resume_result="passed",
    )
    delta = b"2026-08-24 00:00:00 status installed radishlex:arm64 26.7.1+38-1\n"
    receipt_sha256 = "2" * 64
    receipt_size = 4000
    postflight = fields(
        format="radishlex-linux-l6-install-artifacts-staged-terminal-postflight-v1",
        clone_uuid=driver.EXPECTED_TARGET_UUID,
        boot_id_sha256=binding.boot_id_sha256,
        guest_identity_sha256=driver.EXPECTED_GUEST_IDENTITY_SHA256,
        snapshot_identity_sha256=driver.EXPECTED_SNAPSHOT_SHA256,
        operation_id_sha256=binding.operation_id_sha256,
        checkpoint_sha256=binding.checkpoint_sha256,
        acceptance_invocations="1",
        maintenance_resume_invocations="1",
        operation="install_source|install|not_applicable|completed|chain-1",
        package="radishlex|arm64|26.7.1+38-1|install-ok-installed",
        receipt=f"{receipt_sha256}|{receipt_size}",
        target_package_sha256=driver.EXPECTED_SOURCE_PACKAGE_SHA256,
        target_evidence_sha256=driver.EXPECTED_SOURCE_EVIDENCE_SHA256,
        dpkg_status_sha256=driver.EXPECTED_SOURCE_STATUS_SHA256,
        dpkg_log_sha256="3" * 64,
        dpkg_log_size="881152",
        dpkg_delta=f"{hashlib.sha256(delta).hexdigest()}|{len(delta)}",
        dpkg_audit="clean",
        dpkg_verify="clean",
        dpkg_mutation_executed="true",
        installed_payload_md5_inventory=(
            f"{driver.EXPECTED_SOURCE_MD5_SHA256}|28|verified"
        ),
        dependency_count="20",
        resolved_dependency_text_digest=(
            "28fe10654a7f09b18f7f49fb32025f710fd7adf0587b2ddd1ed70bcdd6aa6aac"
        ),
        fonts="dejavu+noto-cjk|family+owner-satisfied",
        manifest=driver.EXPECTED_SOURCE_MANIFEST_SHA256,
        ffi=f"{driver.EXPECTED_SOURCE_FFI_SHA256}|distinct-inodes",
        manager_startup=driver.EXPECTED_ALLOWED_STARTUP,
        fcitx_startup=driver.EXPECTED_ALLOWED_STARTUP,
        startup_negative="invalid-component-9|1:0:0:0:0|error-present",
        user_xdg="absent",
        product_processes="absent",
        network="loopback-only-main-routes-empty",
        manual_recovery_required="false",
        install_artifacts_staged_completed="true",
        terminal_postflight="passed",
    )
    artifacts = {
        "case-resume-stdout": (
            b"install_artifacts_staged_resume_outcome=completed\n"
        ),
        "case-resume-stderr": b"",
        "case-postflight-stdout": (
            b"install_artifacts_staged_postflight_outcome=passed\n"
        ),
        "case-postflight-stderr": b"",
        "maintenance-resume-stdout": b"maintenance_outcome=completed\n",
        "maintenance-resume-stderr": b"",
        "resume-result": resume_result,
        "terminal-postflight": postflight,
        "dpkg-delta": delta,
    }
    terminal = driver.canonical_json(
        {
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "boot_id_sha256": binding.boot_id_sha256,
            "checkpoint_attempt_id": request.checkpoint_attempt_id,
            "checkpoint_sha256": binding.checkpoint_sha256,
            "dpkg_delta_sha256": hashlib.sha256(delta).hexdigest(),
            "dpkg_delta_size": len(delta),
            "dpkg_log_sha256": "3" * 64,
            "dpkg_log_size": 881152,
            "format": driver.EVIDENCE_FORMAT,
            "maintenance_resume_invocations": 1,
            "operation_id": "existing-secret-hash-only",
            "operation_id_sha256": binding.operation_id_sha256,
            "outcome": "resume-completed",
            "phase": "complete",
            "postflight_invocations": 1,
            "reason": "one-shot-install-artifacts-staged-resume-passed",
            "receipt_sha256": receipt_sha256,
            "receipt_size": receipt_size,
            "resume_attempt_id": request.resume_attempt_id,
            "resume_result_sha256": hashlib.sha256(resume_result).hexdigest(),
            "terminal_case_sha256": "4" * 64,
            "terminal_postflight_sha256": hashlib.sha256(postflight).hexdigest(),
            "transaction": "completed",
        }
    )
    return SyntheticResume(terminal, artifacts)


def rejected_resume(request, binding) -> SyntheticResume:
    terminal = driver.canonical_json(
        {
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "checkpoint_attempt_id": request.checkpoint_attempt_id,
            "format": driver.EVIDENCE_FORMAT,
            "maintenance_resume_invocations": 0,
            "operation_id": "existing-secret-hash-only",
            "operation_id_sha256": binding.operation_id_sha256,
            "outcome": "resume-rejected",
            "phase": "resume-readiness",
            "postflight_invocations": 0,
            "reason": "ResumeDriverError:synthetic-readiness-drift",
            "resume_attempt_id": request.resume_attempt_id,
            "transaction": "artifacts-staged-preserved",
        }
    )
    return SyntheticResume(terminal, {})


def artifact_guest_path(
    request: resume.InstallArtifactsStagedResumeRequest, name: str
) -> str:
    output = "/var/tmp/radishlex-l6-crash-install-artifacts-staged-output"
    return {
        "case-resume-stdout": f"{request.guest_resume_root}/case-resume.stdout",
        "case-resume-stderr": f"{request.guest_resume_root}/case-resume.stderr",
        "case-postflight-stdout": (
            f"{request.guest_resume_root}/case-postflight.stdout"
        ),
        "case-postflight-stderr": (
            f"{request.guest_resume_root}/case-postflight.stderr"
        ),
        "maintenance-resume-stdout": f"{output}/resume.stdout",
        "maintenance-resume-stderr": f"{output}/resume.stderr",
        "resume-result": f"{output}/resume-result.evidence.txt",
        "terminal-postflight": f"{output}/terminal-postflight.evidence.txt",
        "dpkg-delta": f"{output}/dpkg-delta.txt",
    }[name]


def fields(**values: str) -> bytes:
    return "".join(f"{key}={value}\n" for key, value in values.items()).encode(
        "utf-8"
    )


def lsof_output(request: resume.InstallArtifactsStagedResumeRequest) -> bytes:
    data = request.target_package_path / "Data"
    return (
        "p2177\ncQEMULauncher\nf17\ntREG\n"
        f"n{data / 'efi_vars.fd'}\n"
        "f18\ntREG\n"
        f"n{data / resume.network_ready.REQUIRED_QCOW2_NAME}\n"
    ).encode("utf-8")


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise AssertionError("expected JSON object")
    return value


def assert_manifest_valid(test: unittest.TestCase, root: Path) -> None:
    for line in (root / "files.sha256").read_text(encoding="ascii").splitlines():
        expected_hash, name = line.split("  ", maxsplit=1)
        test.assertEqual(
            hashlib.sha256((root / name).read_bytes()).hexdigest(), expected_hash
        )


def synthetic_protocol_request():
    return type(
        "Request",
        (),
        {
            "resume_attempt_id": resume.REQUIRED_RESUME_ATTEMPT_ID,
            "checkpoint_attempt_id": resume.REQUIRED_CHECKPOINT_ATTEMPT_ID,
            "guest_resume_root": "/var/tmp/synthetic-resume",
        },
    )()


def synthetic_protocol_binding():
    return type(
        "Binding",
        (),
        {
            "boot_id_sha256": driver.EXPECTED_BOOT_ID_SHA256,
            "operation_id_sha256": driver.EXPECTED_OPERATION_ID_SHA256,
            "checkpoint_sha256": driver.EXPECTED_CHECKPOINT_SHA256,
            "crash_state_sha256": driver.EXPECTED_CRASH_STATE_SHA256,
        },
    )()


if __name__ == "__main__":
    unittest.main()

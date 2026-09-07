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
import l6_utm_install_artifacts_staged_checkpoint as checkpoint
import l6_utm_install_artifacts_staged_checkpoint_bindings as bindings
import l6_utm_install_artifacts_staged_checkpoint_evidence as evidence
import l6_utm_start_once as start_control
import l6_v4_install_artifacts_staged_checkpoint_driver as driver


NETWORK_EVIDENCE = b"synthetic frozen loopback network evidence\n"
PREFLIGHT_EVIDENCE = b"synthetic frozen negative preflight evidence\n"
BOOT_SHA256 = hashlib.sha256(b"synthetic-boot-id").hexdigest()
OPERATION_ID_SHA256 = hashlib.sha256(b"0" * 32).hexdigest()


@dataclass
class SyntheticCheckpoint:
    terminal: bytes
    artifacts: dict[str, bytes]


class FakeRunner:
    def __init__(
        self,
        request: checkpoint.InstallArtifactsStagedCheckpointRequest,
        binding: bindings.CheckpointBinding,
        synthetic: SyntheticCheckpoint,
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
        if argv == checkpoint.PROCESS_COMMAND:
            stdout = b"1 0 0 launchd\n"
        elif argv == checkpoint.network_ready._lsof_argv(self.request):
            stdout = lsof_output(self.request)
        elif argv[:3] == ("utmctl", "file", "push"):
            if stdin_file is None:
                raise AssertionError("driver push requires stdin")
            stdin_file.seek(0)
            self.assert_equal(
                hashlib.sha256(stdin_file.read()).hexdigest(),
                hashlib.sha256(self.binding.driver_bytes).hexdigest(),
            )
            stdin_file.seek(0)
        elif argv[:3] == ("utmctl", "file", "pull"):
            path = argv[4]
            stdout = self.payload_for_path(path)
        elif argv == checkpoint.driver_argv(self.request, self.binding):
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
        if path == self.binding.preflight_evidence_path:
            return PREFLIGHT_EVIDENCE
        if path == self.request.guest_driver_path:
            return self.binding.driver_bytes
        if path == self.request.guest_marker_path:
            return driver.canonical_json(
                {
                    "checkpoint_attempt_id": self.request.checkpoint_attempt_id,
                    "driver_sha256": hashlib.sha256(
                        self.binding.driver_bytes
                    ).hexdigest(),
                    "format": driver.MARKER_FORMAT,
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

    def assert_equal(self, actual: object, expected: object) -> None:
        if actual != expected:
            raise AssertionError(f"{actual!r} != {expected!r}")


class LinuxL6InstallArtifactsStagedCheckpointTests(unittest.TestCase):
    def test_one_shot_checkpoint_reaches_prepared_without_resume(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = valid_binding(request)
            synthetic = successful_checkpoint(request, binding)
            runner = FakeRunner(request, binding, synthetic)

            result = run_checkpoint(request, binding, runner)

            self.assertEqual(result.outcome, "checkpoint-prepared")
            self.assertEqual(result.exit_code, checkpoint.EXIT_CHECKPOINT_PREPARED)
            self.assertEqual(result.checkpoint_invocations, 1)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["operation_id"], "generated-once-hash-only")
            self.assertEqual(terminal["operation_id_sha256"], OPERATION_ID_SHA256)
            self.assertEqual(terminal["maintenance_resume_invocations"], 0)
            self.assertEqual(terminal["automatic_retry"], "not-performed")
            self.assertEqual(terminal["automatic_stop"], "not-performed")
            self.assertEqual(terminal["file_push_invocations"], 1)
            self.assertEqual(terminal["guest_exec_invocations"], 5)
            assert_manifest_valid(self, request.output_root)
            commands = [" ".join(call) for call in runner.calls]
            for forbidden in (
                "utmctl list",
                "utmctl status",
                "utmctl start",
                "utmctl stop",
                " resume ",
                " cleanup ",
                " retry ",
            ):
                self.assertFalse(any(forbidden in command for command in commands))
            all_evidence = b"".join(
                path.read_bytes()
                for path in request.output_root.iterdir()
                if path.is_file()
            )
            self.assertNotIn(b"00000000000000000000000000000000", all_evidence)

    def test_terminal_double_readback_drift_is_indeterminate_without_retry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = valid_binding(request)
            runner = FakeRunner(
                request,
                binding,
                successful_checkpoint(request, binding),
                drift_second_terminal=True,
            )

            result = run_checkpoint(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, checkpoint.EXIT_STATE_INDETERMINATE)
            self.assertEqual(sum(call == checkpoint.driver_argv(request, binding) for call in runner.calls), 1)
            self.assertFalse(any("crash-state" in " ".join(call) for call in runner.calls))

    def test_driver_exit_must_match_terminal(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = valid_binding(request)
            runner = FakeRunner(
                request,
                binding,
                successful_checkpoint(request, binding),
                driver_exit_code=12,
            )

            result = run_checkpoint(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertIn(
                "exit-evidence-mismatch",
                read_json(request.output_root / "terminal.json")["reason"],
            )

    def test_preoperation_rejection_keeps_operation_absent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = valid_binding(request)
            rejected = rejected_checkpoint(request)
            runner = FakeRunner(
                request, binding, rejected, driver_exit_code=10
            )

            result = run_checkpoint(request, binding, runner)

            self.assertEqual(result.outcome, "checkpoint-rejected")
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["operation_id"], "not-generated")
            self.assertEqual(terminal["transaction"], "not-performed")

    def test_checkpoint_artifact_receipt_drift_is_rejected(self) -> None:
        request = synthetic_protocol_request()
        binding = synthetic_protocol_binding()
        synthetic = successful_checkpoint(request, binding)
        value = dict(synthetic.artifacts)
        value["crash-state"] = value["crash-state"].replace(
            b"artifacts_staged", b"prepared"
        )
        terminal = json.loads(synthetic.terminal)
        terminal["crash_state_sha256"] = hashlib.sha256(
            value["crash-state"]
        ).hexdigest()

        with self.assertRaisesRegex(
            evidence.CheckpointEvidenceError, "artifact-semantics-invalid"
        ):
            evidence.validate_checkpoint_artifacts(value, terminal, binding)

    def test_authorization_and_output_overlap_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            unauthorized = checkpoint.InstallArtifactsStagedCheckpointRequest(
                **{
                    **request.__dict__,
                    "authorized_generate_one_operation_id": False,
                }
            )
            with self.assertRaisesRegex(
                checkpoint.InstallArtifactsStagedCheckpointError,
                "authorized-generate-one-operation-id-required",
            ):
                unauthorized.validate()
            overlap = checkpoint.InstallArtifactsStagedCheckpointRequest(
                **{
                    **request.__dict__,
                    "output_root": request.prior_preflight_root / "overwrite",
                }
            )
            with self.assertRaisesRegex(
                checkpoint.InstallArtifactsStagedCheckpointError,
                "output-root-must-not-overlap-prior-preflight-root",
            ):
                overlap.validate()

    def test_binding_names_all_25_frozen_preflight_members(self) -> None:
        self.assertEqual(len(bindings.REQUIRED_PREFLIGHT_ENTRY_NAMES), 25)
        self.assertEqual(bindings.REQUIRED_PREFLIGHT_ENTRY_NAMES[-1], "terminal.json")
        self.assertEqual(
            bindings.REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256,
            "aba59811e62a2f16f149f8fffc71c9f57f823561afe053babd21dd903c360d7a",
        )

    def test_guest_driver_has_no_resume_or_retry_execution_path(self) -> None:
        source = Path(driver.__file__).read_text(encoding="utf-8")
        self.assertNotIn('run_case(control_root, "resume")', source)
        self.assertNotIn('run_case(control_root, "retry")', source)
        self.assertNotIn("utmctl", source)
        self.assertIn('run_case(control_root, "crash")', source)
        self.assertIn('run_case(control_root, "inspect-crash")', source)


def make_request(root: Path) -> checkpoint.InstallArtifactsStagedCheckpointRequest:
    repository = root / "repository"
    source = root / "source" / "canonical-input.ustar"
    for directory in (
        repository,
        root / "network",
        root / "transfer",
        root / "resolution",
        root / "preflight",
        source.parent,
    ):
        directory.mkdir(parents=True, exist_ok=True)
    driver_source = Path(driver.__file__).resolve()
    driver_destination = repository / checkpoint.DRIVER_RELATIVE_PATH
    driver_destination.parent.mkdir(parents=True, exist_ok=True)
    driver_destination.write_bytes(driver_source.read_bytes())
    return checkpoint.InstallArtifactsStagedCheckpointRequest(
        repository_root=repository,
        expected_repository_head="b" * 40,
        prior_network_root=root / "network",
        prior_network_manifest_sha256=checkpoint.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256,
        prior_transfer_root=root / "transfer",
        prior_transfer_manifest_sha256=checkpoint.REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256,
        prior_resolution_root=root / "resolution",
        prior_resolution_manifest_sha256=checkpoint.REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256,
        prior_preflight_root=root / "preflight",
        prior_preflight_manifest_sha256=checkpoint.REQUIRED_PRIOR_PREFLIGHT_MANIFEST_SHA256,
        source_bundle_path=source,
        source_bundle_size=checkpoint.REQUIRED_SOURCE_BUNDLE_SIZE,
        source_bundle_sha256=checkpoint.REQUIRED_SOURCE_BUNDLE_SHA256,
        output_root=root / "output",
        transfer_attempt_id=checkpoint.REQUIRED_TRANSFER_ATTEMPT_ID,
        resolution_attempt_id=checkpoint.REQUIRED_RESOLUTION_ATTEMPT_ID,
        preflight_attempt_id=checkpoint.REQUIRED_PREFLIGHT_ATTEMPT_ID,
        checkpoint_attempt_id=checkpoint.REQUIRED_CHECKPOINT_ATTEMPT_ID,
        target_uuid=checkpoint.REQUIRED_TARGET_UUID,
        target_name=checkpoint.REQUIRED_TARGET_NAME,
        target_package_path=checkpoint.transport_bindings.expected_target_package_path(
            checkpoint.REQUIRED_TARGET_NAME
        ),
        command_timeout_seconds=60,
        checkpoint_timeout_seconds=120,
        evidence_settle_seconds=10,
        authorized_install_artifacts_staged_checkpoint=True,
        authorized_generate_one_operation_id=True,
        authorized_one_acceptance_checkpoint_and_process_group_termination=True,
        authorized_no_resume_retry_cleanup_stop_or_quit=True,
    )


def valid_binding(
    request: checkpoint.InstallArtifactsStagedCheckpointRequest,
) -> bindings.CheckpointBinding:
    driver_bytes = (request.repository_root / checkpoint.DRIVER_RELATIVE_PATH).read_bytes()
    return bindings.CheckpointBinding(
        evidence={"format": checkpoint.EVIDENCE_FORMAT},
        network=input_transfer.NetworkBinding(
            evidence={"format": checkpoint.EVIDENCE_FORMAT},
            guest_evidence_path="/var/tmp/network-v2/network.evidence",
            guest_evidence_bytes=NETWORK_EVIDENCE,
        ),
        driver_bytes=driver_bytes,
        preflight_evidence_bytes=PREFLIGHT_EVIDENCE,
        preflight_evidence_path=(
            "/var/tmp/negative-preflight/negative-preflight.evidence.json"
        ),
        boot_id_sha256=BOOT_SHA256,
    )


def run_checkpoint(
    request: checkpoint.InstallArtifactsStagedCheckpointRequest,
    binding: bindings.CheckpointBinding,
    runner: FakeRunner,
) -> checkpoint.InstallArtifactsStagedCheckpointResult:
    source = input_transfer.SourceBundle(
        file_object=io.BytesIO(b""), descriptor={"synthetic": True}, members=()
    )
    return checkpoint.run_install_artifacts_staged_checkpoint(
        request,
        runner=runner,
        binding_validator=lambda _: binding,
        source_opener=lambda _: source,
        source_revalidator=lambda _request, _source: {
            "format": checkpoint.EVIDENCE_FORMAT,
            "source_bundle_identity": "matched",
        },
        target_validator=lambda _: {
            "format": checkpoint.EVIDENCE_FORMAT,
            "target_uuid": request.target_uuid,
        },
        sleeper=lambda _: None,
    )


def successful_checkpoint(
    request: checkpoint.InstallArtifactsStagedCheckpointRequest,
    binding: bindings.CheckpointBinding,
) -> SyntheticCheckpoint:
    guest_identity = fields(
        format="radishlex-linux-l6-install-artifacts-staged-guest-identity-v1",
        clone_uuid=request.target_uuid,
        boot_id_sha256=binding.boot_id_sha256,
        operation_id="not-generated",
    )
    mutation = fields(
        format="radishlex-linux-l6-install-artifacts-staged-mutation-preflight-v1",
        boot_id_sha256=binding.boot_id_sha256,
        dpkg_status_sha256=driver.EXPECTED_DPKG_STATUS_SHA256,
        dpkg_log_sha256="1" * 64,
        package="not-installed",
        state_root="absent",
        checkpoint_evidence="absent",
        guard="absent",
        operation_id="not-generated",
        mutation_preflight="passed",
    )
    crash_result = fields(
        format="radishlex-linux-l6-install-artifacts-staged-crash-result-v1",
        operation_id_sha256=OPERATION_ID_SHA256,
        preflight_sha256=hashlib.sha256(mutation).hexdigest(),
        acceptance_invocations="1",
        checkpoint_count="1",
        fault="process_group_terminated",
        crash_result="passed",
    )
    checkpoint_payload = json.dumps(
        {
            "build_identity": "radishlex-linux-l6-acceptance-v1",
            "checkpoint": "artifacts_staged",
            "expected_terminal": "completed",
            "format": "radishlex-linux-l6-checkpoint-evidence-v1",
            "guest_identity_sha256": hashlib.sha256(guest_identity).hexdigest(),
            "operation": {
                "matrix_operation": "install_source",
                "operation_id_sha256": OPERATION_ID_SHA256,
            },
            "repository_commit": driver.EXPECTED_REPOSITORY_COMMIT,
            "scenario": "install_artifacts_staged",
            "termination": {
                "checkpoint_notification": "inherited-pipe-v1",
                "dpkg_child": "absent",
                "process_group": "terminated",
                "process_group_member_count": 0,
                "process_inspection": "complete",
                "signal": "sigkill",
                "worker": "signaled",
            },
        },
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8") + b"\n"
    crash_state = fields(
        format="radishlex-linux-l6-install-artifacts-staged-crash-state-v1",
        operation_id_sha256=OPERATION_ID_SHA256,
        checkpoint_sha256=hashlib.sha256(checkpoint_payload).hexdigest(),
        receipt="install|not_applicable|artifacts_staged|chain-1",
        guard="present-valid-unlocked",
        dpkg_mutation_executed="false",
        dpkg_status_sha256=driver.EXPECTED_DPKG_STATUS_SHA256,
        dpkg_log_sha256="1" * 64,
        manager_startup=driver.EXPECTED_STARTUP,
        fcitx_startup=driver.EXPECTED_STARTUP,
        user_xdg="absent",
        product_processes="absent",
        network="loopback-only-main-routes-empty",
        crash_state="passed",
    )
    artifacts = {
        "guest-identity": guest_identity,
        "transfer-setup": fields(
            format="radishlex-linux-l6-install-artifacts-staged-transfer-setup-v1",
            negative_preflight_sha256=hashlib.sha256(
                binding.preflight_evidence_bytes
            ).hexdigest(),
            operation_id="not-generated",
        ),
        "mutation-preflight": mutation,
        "crash-result": crash_result,
        "checkpoint": checkpoint_payload,
        "crash-state": crash_state,
        "case-preflight-stdout": b"install_artifacts_staged_preflight_outcome=passed\n",
        "case-preflight-stderr": b"",
        "case-crash-stdout": b"install_artifacts_staged_crash_outcome=checkpoint_recorded\n",
        "case-crash-stderr": b"",
        "case-inspect-crash-stdout": b"install_artifacts_staged_crash_state_outcome=passed\n",
        "case-inspect-crash-stderr": b"",
    }
    terminal = driver.canonical_json(
        {
            "acceptance_invocations": 1,
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "boot_id_sha256": binding.boot_id_sha256,
            "case_crash_invocations": 1,
            "case_inspect_crash_invocations": 1,
            "case_preflight_invocations": 1,
            "checkpoint_attempt_id": request.checkpoint_attempt_id,
            "checkpoint_invocations": 1,
            "checkpoint_sha256": hashlib.sha256(checkpoint_payload).hexdigest(),
            "checkpoint_size": len(checkpoint_payload),
            "crash_result_sha256": hashlib.sha256(crash_result).hexdigest(),
            "crash_state_sha256": hashlib.sha256(crash_state).hexdigest(),
            "dpkg_mutation": "not-performed",
            "format": driver.EVIDENCE_FORMAT,
            "guest_identity_sha256": hashlib.sha256(guest_identity).hexdigest(),
            "maintenance_resume_invocations": 0,
            "mutation_preflight_sha256": hashlib.sha256(mutation).hexdigest(),
            "negative_preflight_outcome": "passed",
            "operation_id": "generated-once-hash-only",
            "operation_id_sha256": OPERATION_ID_SHA256,
            "outcome": "checkpoint-prepared",
            "phase": "complete",
            "preflight_attempt_id": request.preflight_attempt_id,
            "reason": "one-shot-install-artifacts-staged-checkpoint-passed",
            "transaction": "artifacts-staged-checkpoint-only",
        }
    )
    return SyntheticCheckpoint(terminal, artifacts)


def rejected_checkpoint(
    request: checkpoint.InstallArtifactsStagedCheckpointRequest,
) -> SyntheticCheckpoint:
    terminal = driver.canonical_json(
        {
            "acceptance_invocations": 0,
            "automatic_cleanup": "not-performed",
            "automatic_quit": "not-performed",
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "checkpoint_attempt_id": request.checkpoint_attempt_id,
            "checkpoint_invocations": 0,
            "format": driver.EVIDENCE_FORMAT,
            "maintenance_resume_invocations": 0,
            "operation_id": "not-generated",
            "operation_id_sha256": "not-generated",
            "outcome": "checkpoint-rejected",
            "phase": "case-preflight",
            "reason": "CheckpointDriverError:synthetic-preflight-drift",
            "transaction": "not-performed",
        }
    )
    return SyntheticCheckpoint(terminal, {})


def artifact_guest_path(
    request: checkpoint.InstallArtifactsStagedCheckpointRequest, name: str
) -> str:
    mapping = {
        "guest-identity": "/var/tmp/radishlex-l6-crash-install-artifacts-staged-guest-identity.evidence.txt",
        "transfer-setup": "/var/tmp/radishlex-l6-crash-install-artifacts-staged-output/transfer-setup.evidence.txt",
        "mutation-preflight": "/var/tmp/radishlex-l6-crash-install-artifacts-staged-output/mutation-preflight.evidence.txt",
        "crash-result": "/var/tmp/radishlex-l6-crash-install-artifacts-staged-output/crash-result.evidence.txt",
        "checkpoint": f"/var/tmp/radishlex-l6-evidence/checkpoints/install_artifacts_staged-{OPERATION_ID_SHA256[:16]}.json",
        "crash-state": "/var/tmp/radishlex-l6-crash-install-artifacts-staged-output/crash-state.evidence.txt",
        "case-preflight-stdout": f"{request.guest_checkpoint_root}/case-preflight.stdout",
        "case-preflight-stderr": f"{request.guest_checkpoint_root}/case-preflight.stderr",
        "case-crash-stdout": f"{request.guest_checkpoint_root}/case-crash.stdout",
        "case-crash-stderr": f"{request.guest_checkpoint_root}/case-crash.stderr",
        "case-inspect-crash-stdout": f"{request.guest_checkpoint_root}/case-inspect-crash.stdout",
        "case-inspect-crash-stderr": f"{request.guest_checkpoint_root}/case-inspect-crash.stderr",
    }
    return mapping[name]


def fields(**values: str) -> bytes:
    return "".join(f"{key}={value}\n" for key, value in values.items()).encode("utf-8")


def lsof_output(
    request: checkpoint.InstallArtifactsStagedCheckpointRequest,
) -> bytes:
    data = request.target_package_path / "Data"
    return (
        "p2177\ncQEMULauncher\nf17\ntREG\n"
        f"n{data / 'efi_vars.fd'}\n"
        "f18\ntREG\n"
        f"n{data / checkpoint.network_ready.REQUIRED_QCOW2_NAME}\n"
    ).encode("utf-8")


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise AssertionError("expected JSON object")
    return value


def assert_manifest_valid(test: unittest.TestCase, root: Path) -> None:
    for line in (root / "files.sha256").read_text(encoding="ascii").splitlines():
        expected_hash, name = line.split("  ", maxsplit=1)
        test.assertEqual(hashlib.sha256((root / name).read_bytes()).hexdigest(), expected_hash)


def synthetic_protocol_request():
    return type(
        "Request",
        (),
        {
            "checkpoint_attempt_id": checkpoint.REQUIRED_CHECKPOINT_ATTEMPT_ID,
            "preflight_attempt_id": checkpoint.REQUIRED_PREFLIGHT_ATTEMPT_ID,
            "guest_checkpoint_root": "/var/tmp/synthetic-checkpoint",
            "target_uuid": checkpoint.REQUIRED_TARGET_UUID,
        },
    )()


def synthetic_protocol_binding():
    return type(
        "Binding",
        (),
        {
            "boot_id_sha256": BOOT_SHA256,
            "preflight_evidence_bytes": PREFLIGHT_EVIDENCE,
        },
    )()


if __name__ == "__main__":
    unittest.main()

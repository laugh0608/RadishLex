#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import io
import json
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path

import l6_utm_canonical_input_preflight as preflight
import l6_utm_canonical_input_preflight_bindings as bindings
import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_start_once as start_control
import l6_v4_canonical_input_preflight_probe as probe


NETWORK_EVIDENCE = b"synthetic frozen loopback network evidence\n"
RESOLUTION_EVIDENCE = b"synthetic frozen input-ready evidence\n"
BOOT_SHA256 = hashlib.sha256(b"synthetic-boot-id").hexdigest()


@dataclass(frozen=True)
class ExpectedCall:
    argv: tuple[str, ...]
    observation: start_control.CommandObservation
    stdin_sha256: str | None = None


class FakeRunner:
    def __init__(self, expected: list[ExpectedCall]) -> None:
        self.expected = expected
        self.calls: list[tuple[str, ...]] = []

    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file=None,
    ) -> start_control.CommandObservation:
        del timeout_seconds
        if not self.expected:
            raise AssertionError(f"unexpected command: {argv}")
        item = self.expected.pop(0)
        if item.argv != argv:
            raise AssertionError(f"expected {item.argv}, received {argv}")
        if item.stdin_sha256 is None:
            if stdin_file is not None:
                raise AssertionError(f"unexpected stdin: {argv}")
        else:
            if stdin_file is None:
                raise AssertionError(f"missing stdin: {argv}")
            stdin_file.seek(0)
            actual = hashlib.sha256(stdin_file.read()).hexdigest()
            stdin_file.seek(0)
            if actual != item.stdin_sha256:
                raise AssertionError(f"stdin mismatch: {argv}")
        self.calls.append(argv)
        return item.observation


class LinuxL6CanonicalInputPreflightTests(unittest.TestCase):
    def test_one_shot_negative_preflight_reaches_ready(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = valid_binding(request)
            evidence = passed_evidence(request)
            runner = FakeRunner(successful_calls(request, binding, evidence))

            result = run_preflight(request, binding, runner)

            self.assertEqual(result.outcome, "preflight-ready")
            self.assertEqual(result.exit_code, preflight.EXIT_PREFLIGHT_READY)
            self.assertEqual(result.preflight_invocations, 1)
            self.assertEqual(runner.expected, [])
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["case_invocations"], 0)
            self.assertEqual(terminal["maintenance_invocations"], 0)
            self.assertEqual(terminal["acceptance_invocations"], 0)
            self.assertEqual(terminal["dpkg_invocations"], 0)
            self.assertEqual(terminal["installer_invocations"], 0)
            self.assertEqual(terminal["operation_id"], "not-generated")
            self.assertEqual(terminal["transaction"], "not-performed")
            self.assertEqual(terminal["automatic_retry"], "not-performed")
            assert_manifest_valid(self, request.output_root)
            commands = [" ".join(call) for call in runner.calls]
            forbidden = (
                "utmctl list",
                "utmctl status",
                "utmctl start",
                "utmctl stop",
                "case.sh",
                "radishlex-linux-maintenance",
                "radishlex-linux-l6-acceptance",
                "/usr/bin/dpkg",
                "install-canonical-input.py",
            )
            for text in forbidden:
                self.assertFalse(any(text in command for command in commands), text)

    def test_resolution_readback_drift_stops_before_guest_root_creation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = valid_binding(request)
            calls = host_preflight_calls(request, binding)
            calls.extend(
                [
                    expected(
                        resolution_pull_argv(request),
                        stdout=RESOLUTION_EVIDENCE,
                    ),
                    expected(
                        resolution_pull_argv(request),
                        stdout=RESOLUTION_EVIDENCE + b"drift",
                    ),
                ]
            )
            runner = FakeRunner(calls)

            result = run_preflight(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, preflight.EXIT_STATE_INDETERMINATE)
            self.assertEqual(result.preflight_invocations, 0)
            self.assertEqual(runner.expected, [])
            self.assertFalse(
                any(
                    call[:5]
                    == (
                        "utmctl",
                        "exec",
                        request.target_uuid,
                        "--cmd",
                        "/bin/mkdir",
                    )
                    for call in runner.calls
                )
            )

    def test_guest_rejection_is_frozen_as_failed_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = valid_binding(request)
            evidence = rejected_evidence(request)
            runner = FakeRunner(
                successful_calls(
                    request,
                    binding,
                    evidence,
                    probe_exit_code=probe.EXIT_REJECTED,
                )
            )

            result = run_preflight(request, binding, runner)

            self.assertEqual(result.outcome, "preflight-rejected")
            self.assertEqual(result.exit_code, preflight.EXIT_PREFLIGHT_REJECTED)
            self.assertEqual(result.preflight_invocations, 1)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["operation_id"], "not-generated")
            self.assertIn("synthetic-package-drift", terminal["reason"])

    def test_probe_exit_must_match_double_readback_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = valid_binding(request)
            runner = FakeRunner(
                successful_calls(
                    request,
                    binding,
                    passed_evidence(request),
                    probe_exit_code=probe.EXIT_REJECTED,
                )
            )

            result = run_preflight(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, preflight.EXIT_STATE_INDETERMINATE)
            self.assertEqual(result.preflight_invocations, 1)

    def test_evidence_rejects_generated_operation_id(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            value = json.loads(passed_evidence(request))
            value["operation_id"] = "generated"
            payload = probe.canonical_json(value)

            with self.assertRaisesRegex(
                preflight.CanonicalInputPreflightError,
                "evidence-identity-invalid",
            ):
                preflight.parse_preflight_evidence(
                    payload, request, BOOT_SHA256
                )

    def test_all_authorization_switches_are_required(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            rejected = preflight.CanonicalInputPreflightRequest(
                **{
                    **request.__dict__,
                    "authorized_create_new_guest_preflight_root": False,
                }
            )
            with self.assertRaisesRegex(
                preflight.CanonicalInputPreflightError,
                "authorized-create-new-guest-preflight-root-required",
            ):
                rejected.validate()

    def test_request_rejects_output_overlap_with_frozen_resolution(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            rejected = preflight.CanonicalInputPreflightRequest(
                **{
                    **request.__dict__,
                    "output_root": request.prior_resolution_root / "overwrite",
                }
            )
            with self.assertRaisesRegex(
                preflight.CanonicalInputPreflightError,
                "output-root-must-not-overlap-prior-resolution",
            ):
                rejected.validate()

    def test_binding_contract_names_all_21_frozen_members(self) -> None:
        self.assertEqual(len(bindings.REQUIRED_RESOLUTION_ENTRY_NAMES), 21)
        self.assertEqual(
            bindings.REQUIRED_RESOLUTION_ENTRY_NAMES[-1], "terminal.json"
        )
        self.assertEqual(
            bindings.REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256,
            "7bab8f2095769e7d5764d36330c678f693260ad576ce31437cea3552ddfd4e4a",
        )


def make_request(root: Path) -> preflight.CanonicalInputPreflightRequest:
    repository = root / "repository"
    network = root / "network"
    transfer = root / "transfer"
    resolution = root / "resolution"
    source = root / "source" / "canonical-input.ustar"
    for directory in (
        repository,
        network,
        transfer,
        resolution,
        source.parent,
    ):
        directory.mkdir(parents=True, exist_ok=True)
    probe_source = (
        Path(__file__).resolve().parents[2] / preflight.PROBE_RELATIVE_PATH
    )
    destination = repository / preflight.PROBE_RELATIVE_PATH
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(probe_source.read_bytes())
    return preflight.CanonicalInputPreflightRequest(
        repository_root=repository,
        expected_repository_head="b" * 40,
        prior_network_root=network,
        prior_network_manifest_sha256=(
            preflight.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
        ),
        prior_transfer_root=transfer,
        prior_transfer_manifest_sha256=(
            preflight.REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256
        ),
        prior_resolution_root=resolution,
        prior_resolution_manifest_sha256=(
            preflight.REQUIRED_PRIOR_RESOLUTION_MANIFEST_SHA256
        ),
        source_bundle_path=source,
        source_bundle_size=preflight.REQUIRED_SOURCE_BUNDLE_SIZE,
        source_bundle_sha256=preflight.REQUIRED_SOURCE_BUNDLE_SHA256,
        output_root=root / "output",
        transfer_attempt_id=preflight.REQUIRED_TRANSFER_ATTEMPT_ID,
        resolution_attempt_id=preflight.REQUIRED_RESOLUTION_ATTEMPT_ID,
        preflight_attempt_id="d75818f-v4-negative-preflight-20260823-v1",
        target_uuid=preflight.REQUIRED_TARGET_UUID,
        target_name=preflight.REQUIRED_TARGET_NAME,
        target_package_path=preflight.transport_bindings.expected_target_package_path(
            preflight.REQUIRED_TARGET_NAME
        ),
        command_timeout_seconds=60,
        preflight_timeout_seconds=120,
        evidence_settle_seconds=10,
        authorized_canonical_input_negative_preflight=True,
        authorized_existing_resolution_double_readback=True,
        authorized_create_new_guest_preflight_root=True,
        authorized_read_only_startup_without_case_maintenance_acceptance_or_dpkg=True,
    )


def valid_binding(
    request: preflight.CanonicalInputPreflightRequest,
) -> bindings.PreflightBinding:
    probe_bytes = (request.repository_root / preflight.PROBE_RELATIVE_PATH).read_bytes()
    return bindings.PreflightBinding(
        evidence={
            "format": preflight.EVIDENCE_FORMAT,
            "prior_network_boot_id_sha256": BOOT_SHA256,
            "repository_head": request.expected_repository_head,
        },
        network=input_transfer.NetworkBinding(
            evidence={"format": input_transfer.EVIDENCE_FORMAT},
            guest_evidence_path="/var/tmp/network-v2/network.evidence",
            guest_evidence_bytes=NETWORK_EVIDENCE,
        ),
        probe_bytes=probe_bytes,
        resolution_evidence_bytes=RESOLUTION_EVIDENCE,
    )


def run_preflight(
    request: preflight.CanonicalInputPreflightRequest,
    binding: bindings.PreflightBinding,
    runner: FakeRunner,
) -> preflight.CanonicalInputPreflightResult:
    source = input_transfer.SourceBundle(
        file_object=io.BytesIO(b""),
        descriptor={"synthetic": True},
        members=(),
    )
    return preflight.run_canonical_input_preflight(
        request,
        runner=runner,
        binding_validator=lambda _: binding,
        source_opener=lambda _: source,
        source_revalidator=lambda _request, _source: {
            "format": preflight.EVIDENCE_FORMAT,
            "source_bundle_identity": "matched",
        },
        target_validator=lambda _: {
            "format": preflight.EVIDENCE_FORMAT,
            "target_uuid": request.target_uuid,
        },
        sleeper=lambda _: None,
    )


def successful_calls(
    request: preflight.CanonicalInputPreflightRequest,
    binding: bindings.PreflightBinding,
    evidence_payload: bytes,
    *,
    probe_exit_code: int = probe.EXIT_PASSED,
) -> list[ExpectedCall]:
    probe_sha256 = hashlib.sha256(binding.probe_bytes).hexdigest()
    marker = probe.expected_marker_bytes(request.preflight_attempt_id, probe_sha256)
    calls = host_preflight_calls(request, binding)
    calls.extend(
        [
            expected(resolution_pull_argv(request), stdout=RESOLUTION_EVIDENCE),
            expected(resolution_pull_argv(request), stdout=RESOLUTION_EVIDENCE),
            expected(root_create_argv(request)),
            expected(
                probe_push_argv(request),
                stdin_sha256=probe_sha256,
            ),
            expected(probe_chown_argv(request)),
            expected(probe_chmod_argv(request)),
            expected(probe_publish_argv(request)),
            expected(probe_pull_argv(request), stdout=binding.probe_bytes),
            expected(
                preflight.probe_argv(request, binding),
                exit_code=probe_exit_code,
            ),
            expected(marker_pull_argv(request), stdout=marker),
            expected(evidence_pull_argv(request), stdout=evidence_payload),
            expected(evidence_pull_argv(request), stdout=evidence_payload),
            expected(
                phase_pull_argv(request),
                stdout=probe.canonical_json(
                    {
                        "format": probe.EVIDENCE_FORMAT,
                        "phase": json.loads(evidence_payload)["phase"],
                    }
                ),
            ),
            expected(network_pull_argv(request), stdout=NETWORK_EVIDENCE),
        ]
    )
    return calls


def host_preflight_calls(
    request: preflight.CanonicalInputPreflightRequest,
    binding: bindings.PreflightBinding,
) -> list[ExpectedCall]:
    del binding
    return [
        expected(preflight.PROCESS_COMMAND, stdout=b"1 0 0 launchd\n"),
        expected(
            preflight.network_ready._lsof_argv(request),
            stdout=lsof_output(request),
        ),
        expected(network_pull_argv(request), stdout=NETWORK_EVIDENCE),
        expected(network_pull_argv(request), stdout=NETWORK_EVIDENCE),
    ]


def passed_evidence(
    request: preflight.CanonicalInputPreflightRequest,
) -> bytes:
    value = evidence_base(request)
    value.update(
        {
            "boot_id_sha256": BOOT_SHA256,
            "checkpoint_evidence": "absent",
            "dependency_count": 20,
            "dependency_identity": "matched",
            "dpkg_config_sha256": probe.EXPECTED_DPKG_CONFIG_SHA256,
            "dpkg_log_sha256": hashlib.sha256(b"synthetic-log").hexdigest(),
            "dpkg_log_size": 42,
            "dpkg_status_sha256": probe.EXPECTED_DPKG_STATUS_SHA256,
            "fcitx_startup": probe.EXPECTED_STARTUP_OUTPUT,
            "font_identity": "dejavu+noto-cjk|matched",
            "guard": "absent",
            "input_inventory_count": len(probe.INPUT_MEMBERS),
            "input_inventory_identity": "matched",
            "input_root_identity": "private-directory",
            "manager_startup": probe.EXPECTED_STARTUP_OUTPUT,
            "network": "loopback-only-main-routes-empty",
            "outcome": "passed",
            "package": "not-installed",
            "phase": "complete",
            "product_processes": "absent",
            "reason": "none",
            "receipt_terminal": "absent",
            "release_pair_identity": "matched",
            "startup_reason": "ReceiptMissing",
            "state_root": "absent",
            "user_xdg": "absent",
        }
    )
    return probe.canonical_json(value)


def rejected_evidence(
    request: preflight.CanonicalInputPreflightRequest,
) -> bytes:
    value = evidence_base(request)
    value.update(
        {
            "outcome": "predicate_failed",
            "phase": "package-dependencies",
            "reason": "NegativePreflightProbeError:synthetic-package-drift",
        }
    )
    return probe.canonical_json(value)


def evidence_base(
    request: preflight.CanonicalInputPreflightRequest,
) -> dict[str, object]:
    return probe.base_evidence(
        type(
            "Args",
            (),
            {
                "preflight_attempt_id": request.preflight_attempt_id,
                "resolution_attempt_id": request.resolution_attempt_id,
                "transfer_attempt_id": request.transfer_attempt_id,
            },
        )()
    )


def network_pull_argv(
    request: preflight.CanonicalInputPreflightRequest,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        "/var/tmp/network-v2/network.evidence",
    )


def resolution_pull_argv(
    request: preflight.CanonicalInputPreflightRequest,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        request.guest_resolution_evidence_path,
    )


def root_create_argv(
    request: preflight.CanonicalInputPreflightRequest,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/bin/mkdir",
        "-m",
        "0700",
        request.guest_preflight_root,
    )


def probe_push_argv(request: preflight.CanonicalInputPreflightRequest) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "push",
        request.target_uuid,
        request.guest_probe_incoming,
    )


def probe_chown_argv(request: preflight.CanonicalInputPreflightRequest) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/bin/chown",
        "root:root",
        request.guest_probe_incoming,
    )


def probe_chmod_argv(request: preflight.CanonicalInputPreflightRequest) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/bin/chmod",
        "0600",
        request.guest_probe_incoming,
    )


def probe_publish_argv(request: preflight.CanonicalInputPreflightRequest) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/bin/mv",
        "--",
        request.guest_probe_incoming,
        request.guest_probe_path,
    )


def probe_pull_argv(request: preflight.CanonicalInputPreflightRequest) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        request.guest_probe_path,
    )


def marker_pull_argv(request: preflight.CanonicalInputPreflightRequest) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        request.guest_marker_path,
    )


def evidence_pull_argv(request: preflight.CanonicalInputPreflightRequest) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        request.guest_evidence_path,
    )


def phase_pull_argv(request: preflight.CanonicalInputPreflightRequest) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        request.guest_phase_path,
    )


def lsof_output(request: preflight.CanonicalInputPreflightRequest) -> bytes:
    data = request.target_package_path / "Data"
    return (
        "p2177\n"
        "cQEMULauncher\n"
        "f17\n"
        "tREG\n"
        f"n{data / 'efi_vars.fd'}\n"
        "f18\n"
        "tREG\n"
        f"n{data / preflight.network_ready.REQUIRED_QCOW2_NAME}\n"
    ).encode("utf-8")


def expected(
    argv: tuple[str, ...],
    *,
    stdout: bytes = b"",
    stderr: bytes = b"",
    exit_code: int | None = 0,
    timed_out: bool = False,
    stdin_sha256: str | None = None,
) -> ExpectedCall:
    return ExpectedCall(
        argv,
        start_control.CommandObservation.from_bytes(
            argv,
            exit_code=exit_code,
            timed_out=timed_out,
            stdout=stdout,
            stderr=stderr,
        ),
        stdin_sha256,
    )


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise AssertionError("expected JSON object")
    return value


def assert_manifest_valid(
    test: unittest.TestCase, root: Path
) -> None:
    for line in (root / "files.sha256").read_text(
        encoding="ascii"
    ).splitlines():
        expected_hash, name = line.split("  ", maxsplit=1)
        test.assertEqual(
            hashlib.sha256((root / name).read_bytes()).hexdigest(),
            expected_hash,
        )


if __name__ == "__main__":
    unittest.main()

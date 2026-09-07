#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import io
import json
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from unittest import mock

import l6_utm_canonical_input_resolution as resolution
import l6_utm_canonical_input_resolution_bindings as resolution_bindings
import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer
import l6_v4_canonical_input_resolution_probe as probe


NETWORK_EVIDENCE = b"synthetic-loopback-only-network-evidence\n"
MEMBERS = (
    guest_installer.ArchiveMember(
        "source/synthetic-input.txt",
        0o600,
        7,
        hashlib.sha256(b"fixture").hexdigest(),
    ),
)


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
        expected_call = self.expected.pop(0)
        if argv != expected_call.argv:
            raise AssertionError(
                f"expected {expected_call.argv}, received {argv}"
            )
        if expected_call.stdin_sha256 is None:
            if stdin_file is not None:
                raise AssertionError(f"unexpected stdin for {argv}")
        else:
            if stdin_file is None:
                raise AssertionError(f"missing stdin for {argv}")
            stdin_file.seek(0)
            digest = hashlib.sha256(stdin_file.read()).hexdigest()
            stdin_file.seek(0)
            if digest != expected_call.stdin_sha256:
                raise AssertionError(f"stdin hash mismatch for {argv}")
        self.calls.append(argv)
        return expected_call.observation


class LinuxL6CanonicalInputResolutionTests(unittest.TestCase):
    def test_binding_requires_exact_frozen_transfer_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            populate_prior_transfer_root(request)
            original_sha256 = resolution_bindings.network_ready._sha256_file

            def frozen_sha256(path: Path) -> str:
                if path == request.prior_transfer_root / "files.sha256":
                    return request.prior_transfer_manifest_sha256
                return original_sha256(path)

            with mock.patch.object(
                resolution_bindings.start_control,
                "_run_git",
                side_effect=lambda _root, args: (
                    (request.expected_repository_head + "\n").encode("ascii")
                    if args == ("rev-parse", "HEAD")
                    else b""
                ),
            ), mock.patch.object(
                resolution_bindings.network_ready,
                "_sha256_file",
                side_effect=frozen_sha256,
            ):
                binding = resolution_bindings.validate_resolution_bindings(
                    request,
                    network_validator=lambda _: valid_binding(request).network,
                )

            self.assertEqual(
                binding.evidence["prior_transfer_entries_verified"], 19
            )
            self.assertEqual(
                binding.evidence["prior_transfer_outcome"],
                "state-indeterminate",
            )
            self.assertEqual(
                hashlib.sha256(binding.installer_bytes).hexdigest(),
                hashlib.sha256(
                    (
                        request.repository_root
                        / input_transfer.GUEST_INSTALLER_RELATIVE_PATH
                    ).read_bytes()
                ).hexdigest(),
            )

    def test_binding_rejects_frozen_terminal_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            populate_prior_transfer_root(
                request, terminal_overrides={"bundle_push_invocations": 2}
            )
            original_sha256 = resolution_bindings.network_ready._sha256_file

            def frozen_sha256(path: Path) -> str:
                if path == request.prior_transfer_root / "files.sha256":
                    return request.prior_transfer_manifest_sha256
                return original_sha256(path)

            with mock.patch.object(
                resolution_bindings.start_control,
                "_run_git",
                side_effect=lambda _root, args: (
                    (request.expected_repository_head + "\n").encode("ascii")
                    if args == ("rev-parse", "HEAD")
                    else b""
                ),
            ), mock.patch.object(
                resolution_bindings.network_ready,
                "_sha256_file",
                side_effect=frozen_sha256,
            ):
                with self.assertRaisesRegex(
                    ValueError, "prior-transfer-terminal-semantics-invalid"
                ):
                    resolution_bindings.validate_resolution_bindings(
                        request,
                        network_validator=lambda _: valid_binding(request).network,
                    )

    def test_stable_passed_result_and_probe_resolve_input_ready(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            transfer_payload = transfer_evidence(request)
            probe_payload = probe_evidence(request)
            runner = FakeRunner(
                successful_calls(request, transfer_payload, probe_payload)
            )

            result = run_resolution(request, runner)

            self.assertEqual(result.outcome, "input-ready")
            self.assertEqual(result.exit_code, resolution.EXIT_INPUT_READY)
            self.assertEqual(result.transfer_result_readback_invocations, 2)
            self.assertEqual(result.probe_invocations, 1)
            self.assertEqual(runner.expected, [])
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["bundle_push_invocations"], 0)
            self.assertEqual(terminal["installer_invocations"], 0)
            self.assertEqual(terminal["operation_id"], "not-generated")
            self.assertEqual(terminal["transaction"], "not-performed")
            self.assertEqual(terminal["automatic_retry"], "not-performed")
            assert_manifest_valid(self, request.output_root)
            forbidden = (
                "utmctl list",
                "utmctl status",
                "utmctl start",
                "utmctl stop",
                "utmctl delete",
                "radishlex-linux-maintenance",
                "radishlex-linux-l6-acceptance",
                "/usr/bin/dpkg",
            )
            commands = [" ".join(call) for call in runner.calls]
            for value in forbidden:
                self.assertFalse(
                    any(value in command for command in commands), value
                )
            original_installer_argv = (
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/usr/bin/python3",
                f"{request.guest_transfer_root}/install-canonical-input.py",
                "--attempt-id",
                request.transfer_attempt_id,
                "--transfer-root",
                request.guest_transfer_root,
                "--bundle-path",
                f"{request.guest_transfer_root}/canonical-input.ustar.incoming",
                "--expected-bundle-size",
                str(request.source_bundle_size),
                "--expected-bundle-sha256",
                request.source_bundle_sha256,
            )
            self.assertNotIn(original_installer_argv, runner.calls)
            self.assertEqual(
                [
                    call
                    for call in runner.calls
                    if call[:3] == ("utmctl", "file", "push")
                ],
                [
                    (
                        "utmctl",
                        "file",
                        "push",
                        request.target_uuid,
                        request.guest_probe_incoming,
                    )
                ],
            )

    def test_missing_existing_result_stays_indeterminate_without_guest_probe(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            calls = preflight_calls(request)
            calls.append(
                expected(
                    transfer_result_pull_argv(request),
                    stderr=b"missing transfer evidence\n",
                )
            )
            runner = FakeRunner(calls)

            result = run_resolution(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, resolution.EXIT_STATE_INDETERMINATE)
            self.assertEqual(result.transfer_result_readback_invocations, 1)
            self.assertEqual(result.probe_invocations, 0)
            self.assertEqual(runner.expected, [])
            self.assertFalse(
                any(call[:2] == ("utmctl", "exec") for call in runner.calls)
            )
            self.assertFalse(
                any(call[:3] == ("utmctl", "file", "push") for call in runner.calls)
            )

    def test_existing_result_drift_stays_indeterminate_without_probe(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            payload = transfer_evidence(request)
            runner = FakeRunner(
                preflight_calls(request)
                + [
                    expected(transfer_result_pull_argv(request), stdout=payload),
                    expected(
                        transfer_result_pull_argv(request),
                        stdout=payload + b"drift",
                    ),
                ]
            )

            result = run_resolution(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.probe_invocations, 0)
            self.assertEqual(runner.expected, [])

    def test_active_installer_probe_never_resolves_passed_result(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            transfer_payload = transfer_evidence(request)
            active_probe = probe_evidence(
                request,
                outcome="indeterminate",
                phase="process-static",
                reason="installer-or-related-process-active",
                active_installer_count=1,
                installer_identity="not-observed",
                bundle_identity="not-observed",
                final_input_inventory_identity="not-observed",
                transfer_marker_identity="not-observed",
                transfer_evidence_identity="not-observed",
                transfer_root_state="unknown",
                staging_root_state="unknown",
                final_input_root_state="unknown",
            )
            runner = FakeRunner(
                successful_calls(
                    request,
                    transfer_payload,
                    active_probe,
                    probe_exit_code=probe.EXIT_INDETERMINATE,
                )
            )

            result = run_resolution(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, resolution.EXIT_STATE_INDETERMINATE)
            self.assertEqual(result.probe_invocations, 1)
            self.assertEqual(runner.expected, [])

    def test_probe_exit_must_match_stable_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            transfer_payload = transfer_evidence(request)
            probe_payload = probe_evidence(request)
            runner = FakeRunner(
                successful_calls(
                    request,
                    transfer_payload,
                    probe_payload,
                    probe_exit_code=probe.EXIT_INDETERMINATE,
                )
            )

            result = run_resolution(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.exit_code, resolution.EXIT_STATE_INDETERMINATE)
            self.assertEqual(result.probe_invocations, 1)
            self.assertEqual(runner.expected, [])

    def test_failed_before_switch_with_absent_final_root_is_failed_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            transfer_payload = transfer_evidence(
                request,
                outcome="failed",
                phase="root-preflight",
                reason="final-input-root-must-be-absent",
                final_switch="not-performed",
                inventory=[],
            )
            probe_payload = probe_evidence(
                request,
                final_input_inventory_identity="not-observed",
                final_input_root_state="absent",
                staging_root_state="absent",
            )
            runner = FakeRunner(
                successful_calls(request, transfer_payload, probe_payload)
            )

            result = run_resolution(request, runner)

            self.assertEqual(result.outcome, "transfer-failed-closed")
            self.assertEqual(
                result.exit_code, resolution.EXIT_TRANSFER_FAILED_CLOSED
            )
            self.assertEqual(runner.expected, [])

    def test_failed_after_switch_stops_before_probe(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            transfer_payload = transfer_evidence(
                request,
                outcome="failed",
                phase="final-readback",
                reason="input-file-hash-drift",
                final_switch="performed",
            )
            runner = FakeRunner(
                preflight_calls(request)
                + [
                    expected(
                        transfer_result_pull_argv(request),
                        stdout=transfer_payload,
                    ),
                    expected(
                        transfer_result_pull_argv(request),
                        stdout=transfer_payload,
                    ),
                ]
            )

            result = run_resolution(request, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.probe_invocations, 0)
            self.assertEqual(runner.expected, [])

    def test_authorization_and_frozen_identity_are_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for overrides, reason in (
                (
                    {"authorized_create_new_read_only_probe": False},
                    "authorized-create-new-read-only-probe-required",
                ),
                (
                    {"transfer_attempt_id": "different-attempt"},
                    "required-transfer-attempt-id-mismatch",
                ),
                (
                    {"prior_transfer_manifest_sha256": "0" * 64},
                    "required-prior-transfer-manifest-mismatch",
                ),
            ):
                with self.subTest(reason=reason):
                    request = make_request(root / reason, **overrides)
                    with self.assertRaisesRegex(
                        resolution.CanonicalInputResolutionError, reason
                    ):
                        request.validate()


def run_resolution(
    request: resolution.CanonicalInputResolutionRequest,
    runner: FakeRunner,
) -> resolution.CanonicalInputResolutionResult:
    return resolution.run_canonical_input_resolution(
        request,
        runner=runner,
        binding_validator=valid_binding,
        source_opener=lambda _: input_transfer.SourceBundle(
            io.BytesIO(b"synthetic"), {}, MEMBERS
        ),
        source_revalidator=lambda _request, _source: {
            "descriptor_unchanged": True,
            "format": resolution.EVIDENCE_FORMAT,
            "inventory_unchanged": True,
            "sha256": request.source_bundle_sha256,
            "size": request.source_bundle_size,
        },
        target_validator=valid_target,
        sleeper=lambda _: None,
    )


def successful_calls(
    request: resolution.CanonicalInputResolutionRequest,
    transfer_payload: bytes,
    probe_payload: bytes,
    *,
    probe_exit_code: int = probe.EXIT_PASSED,
) -> list[ExpectedCall]:
    binding = valid_binding(request)
    probe_bytes = binding.probe_bytes
    installer_sha256 = hashlib.sha256(binding.installer_bytes).hexdigest()
    transfer_sha256 = hashlib.sha256(transfer_payload).hexdigest()
    calls = preflight_calls(request)
    calls.extend(
        [
            expected(transfer_result_pull_argv(request), stdout=transfer_payload),
            expected(transfer_result_pull_argv(request), stdout=transfer_payload),
            expected(
                (
                    "utmctl",
                    "exec",
                    request.target_uuid,
                    "--cmd",
                    "/bin/mkdir",
                    "-m",
                    "0700",
                    request.guest_resolution_root,
                )
            ),
            expected(
                (
                    "utmctl",
                    "file",
                    "push",
                    request.target_uuid,
                    request.guest_probe_incoming,
                ),
                stdin_sha256=hashlib.sha256(probe_bytes).hexdigest(),
            ),
            expected(
                (
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
                    request.guest_probe_incoming,
                    request.guest_probe_path,
                )
            ),
            expected(
                (
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    request.guest_probe_path,
                ),
                stdout=probe_bytes,
            ),
            expected(
                resolution.probe_argv(
                    request,
                    installer_size=len(binding.installer_bytes),
                    installer_sha256=installer_sha256,
                    transfer_evidence_size=len(transfer_payload),
                    transfer_evidence_sha256=transfer_sha256,
                ),
                exit_code=probe_exit_code,
            ),
            expected(probe_result_pull_argv(request), stdout=probe_payload),
            expected(probe_result_pull_argv(request), stdout=probe_payload),
        ]
    )
    return calls


def preflight_calls(
    request: resolution.CanonicalInputResolutionRequest,
) -> list[ExpectedCall]:
    network_argv = (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        "/var/tmp/network-v2/network.evidence",
    )
    return [
        expected(resolution.PROCESS_COMMAND, stdout=b"1 0 0 launchd\n"),
        expected(
            resolution.network_ready._lsof_argv(request),
            stdout=lsof_output(request),
        ),
        expected(network_argv, stdout=NETWORK_EVIDENCE),
        expected(network_argv, stdout=NETWORK_EVIDENCE),
    ]


def valid_binding(
    request: resolution.CanonicalInputResolutionRequest,
) -> resolution_bindings.ResolutionBinding:
    repository = request.repository_root
    installer = (
        repository
        / resolution.input_transfer.GUEST_INSTALLER_RELATIVE_PATH
    ).read_bytes()
    probe_bytes = (repository / resolution.PROBE_RELATIVE_PATH).read_bytes()
    return resolution_bindings.ResolutionBinding(
        evidence={
            "format": resolution.EVIDENCE_FORMAT,
            "repository_head": request.expected_repository_head,
        },
        network=resolution.input_transfer.NetworkBinding(
            evidence={"format": resolution.input_transfer.EVIDENCE_FORMAT},
            guest_evidence_path="/var/tmp/network-v2/network.evidence",
            guest_evidence_bytes=NETWORK_EVIDENCE,
        ),
        installer_bytes=installer,
        probe_bytes=probe_bytes,
    )


def valid_target(
    request: resolution.CanonicalInputResolutionRequest,
) -> dict[str, object]:
    return {"format": resolution.EVIDENCE_FORMAT, "target_uuid": request.target_uuid}


def transfer_evidence(
    request: resolution.CanonicalInputResolutionRequest,
    *,
    outcome: str = "passed",
    phase: str = "final-readback",
    reason: str = "none",
    final_switch: str = "performed",
    inventory: list[dict[str, object]] | None = None,
) -> bytes:
    actual_inventory = (
        [member.as_json() for member in MEMBERS]
        if inventory is None
        else inventory
    )
    value = {
        "attempt_id": request.transfer_attempt_id,
        "automatic_cleanup": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "bundle_sha256": request.source_bundle_sha256,
        "bundle_size": request.source_bundle_size,
        "final_input_root": str(guest_installer.FINAL_INPUT_ROOT),
        "final_switch": final_switch,
        "format": guest_installer.EVIDENCE_FORMAT,
        "inventory": actual_inventory,
        "inventory_count": len(actual_inventory),
        "operation_id": "not-generated",
        "outcome": outcome,
        "phase": phase,
        "reason": reason,
        "transaction": "not-performed",
    }
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")


def probe_evidence(
    request: resolution.CanonicalInputResolutionRequest,
    *,
    outcome: str = "passed",
    phase: str = "complete",
    reason: str = "none",
    active_installer_count: int = 0,
    suspicious_reference_count: int = 0,
    installer_identity: str = "matched",
    bundle_identity: str = "matched",
    final_input_inventory_identity: str = "matched",
    transfer_marker_identity: str = "matched",
    transfer_evidence_identity: str = "matched",
    transfer_root_state: str = "private-directory",
    staging_root_state: str = "absent",
    final_input_root_state: str = "private-directory",
) -> bytes:
    value = {
        "active_installer_count": active_installer_count,
        "automatic_cleanup": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "bundle_identity": bundle_identity,
        "final_input_root": str(guest_installer.FINAL_INPUT_ROOT),
        "final_input_inventory_identity": final_input_inventory_identity,
        "final_input_root_state": final_input_root_state,
        "format": probe.EVIDENCE_FORMAT,
        "installer_identity": installer_identity,
        "operation_id": "not-generated",
        "outcome": outcome,
        "phase": phase,
        "reason": reason,
        "resolution_attempt_id": request.resolution_attempt_id,
        "staging_root_state": staging_root_state,
        "suspicious_reference_count": suspicious_reference_count,
        "transaction": "not-performed",
        "transfer_attempt_id": request.transfer_attempt_id,
        "transfer_evidence_identity": transfer_evidence_identity,
        "transfer_marker_identity": transfer_marker_identity,
        "transfer_root_state": transfer_root_state,
    }
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")


def make_request(
    temporary_root: Path, **overrides: object
) -> resolution.CanonicalInputResolutionRequest:
    temporary_root.mkdir(parents=True, exist_ok=True)
    source = temporary_root / "canonical-input.ustar"
    source.write_bytes(b"synthetic-placeholder")
    source.chmod(0o600)
    values: dict[str, object] = {
        "repository_root": Path(__file__).resolve().parents[2],
        "expected_repository_head": "a" * 40,
        "prior_network_root": temporary_root / "network-v2",
        "prior_network_manifest_sha256": (
            resolution.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
        ),
        "prior_transfer_root": temporary_root / "input-transfer-v1",
        "prior_transfer_manifest_sha256": (
            resolution.REQUIRED_PRIOR_TRANSFER_MANIFEST_SHA256
        ),
        "source_bundle_path": source,
        "source_bundle_size": resolution.REQUIRED_SOURCE_BUNDLE_SIZE,
        "source_bundle_sha256": resolution.REQUIRED_SOURCE_BUNDLE_SHA256,
        "output_root": temporary_root / "input-resolution-v1",
        "transfer_attempt_id": resolution.REQUIRED_TRANSFER_ATTEMPT_ID,
        "resolution_attempt_id": "d75818f-v4-input-resolution-20260823-v1",
        "target_uuid": resolution.REQUIRED_TARGET_UUID,
        "target_name": resolution.REQUIRED_TARGET_NAME,
        "target_package_path": (
            resolution.transport_bindings.expected_target_package_path(
                resolution.REQUIRED_TARGET_NAME
            )
        ),
        "command_timeout_seconds": 60,
        "probe_timeout_seconds": 120,
        "probe_settle_seconds": 10,
        "authorized_canonical_input_resolution": True,
        "authorized_existing_result_double_readback": True,
        "authorized_create_new_read_only_probe": True,
        "authorized_no_transfer_operation_transaction_stop_or_retry": True,
    }
    values.update(overrides)
    return resolution.CanonicalInputResolutionRequest(  # type: ignore[arg-type]
        **values
    )


def populate_prior_transfer_root(
    request: resolution.CanonicalInputResolutionRequest,
    *,
    terminal_overrides: dict[str, object] | None = None,
) -> None:
    root = request.prior_transfer_root
    root.mkdir(parents=True)
    root.chmod(0o700)
    guest_root = request.guest_transfer_root
    installer_bytes = (
        request.repository_root / input_transfer.GUEST_INSTALLER_RELATIVE_PATH
    ).read_bytes()
    empty_sha = hashlib.sha256(b"").hexdigest()
    request_json = {
        "attempt_id": request.transfer_attempt_id,
        "authorization": {
            "canonical_input_transfer": True,
            "exact_source_bundle_identity": True,
            "no_operation_transaction_stop_or_retry": True,
            "private_staging_and_atomic_switch": True,
        },
        "command_timeout_seconds": 60,
        "expected_repository_head": (
            resolution_bindings.REQUIRED_PRIOR_TRANSFER_REPOSITORY_HEAD
        ),
        "format": input_transfer.EVIDENCE_FORMAT,
        "guest_final_input_root": str(guest_installer.FINAL_INPUT_ROOT),
        "guest_root": guest_root,
        "prior_network_manifest_sha256": request.prior_network_manifest_sha256,
        "source_bundle_path_sha256": hashlib.sha256(
            str(request.source_bundle_path).encode("utf-8")
        ).hexdigest(),
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_package_path_sha256": hashlib.sha256(
            str(request.target_package_path).encode("utf-8")
        ).hexdigest(),
        "target_uuid": request.target_uuid,
        "transfer_timeout_seconds": 600,
    }
    terminal = {
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "bundle_push_invocations": 1,
        "bundle_readback_invocations": 2,
        "file_pull_invocations": 6,
        "file_push_invocations": 2,
        "format": input_transfer.EVIDENCE_FORMAT,
        "guest_exec_invocations": 3,
        "guest_installer_invocations": 1,
        "guest_terminal_outcome": None,
        "input_root": str(guest_installer.FINAL_INPUT_ROOT),
        "operation_id": "not-generated",
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": (
            "guest-transfer-evidence-readback-1:"
            "guest-transfer-evidence-readback-1-stderr-not-empty"
        ),
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
    }
    if terminal_overrides:
        terminal.update(terminal_overrides)
    installer_argv = (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        f"{guest_root}/install-canonical-input.py",
        "--attempt-id",
        request.transfer_attempt_id,
        "--transfer-root",
        guest_root,
        "--bundle-path",
        f"{guest_root}/canonical-input.ustar.incoming",
        "--expected-bundle-size",
        str(request.source_bundle_size),
        "--expected-bundle-sha256",
        request.source_bundle_sha256,
    )
    installer_pull_argv = (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        f"{guest_root}/install-canonical-input.py",
    )
    result_pull_argv = (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        f"{guest_root}/transfer.evidence.json",
    )
    bundle_pull_argv = [
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        f"{guest_root}/canonical-input.ustar.incoming",
    ]
    bundle_observation = {
        "argv": bundle_pull_argv,
        "exit_code": 0,
        "stderr": {
            "sha256": empty_sha,
            "total_bytes": 0,
            "truncated": False,
        },
        "stdout": {
            "sha256": request.source_bundle_sha256,
            "total_bytes": request.source_bundle_size,
            "truncated": True,
        },
        "timed_out": False,
    }
    first_result = start_control.CommandObservation.from_bytes(
        result_pull_argv
    ).as_json()
    first_result["stderr"] = {
        "prefix_base64": "",
        "prefix_utf8": "",
        "sha256": resolution_bindings.REQUIRED_FIRST_RESULT_STDERR_SHA256,
        "total_bytes": resolution_bindings.REQUIRED_FIRST_RESULT_STDERR_SIZE,
        "truncated": False,
    }
    payloads: dict[str, object] = {
        "request.json": request_json,
        "guest-installer-readback.json": (
            start_control.CommandObservation.from_bytes(
                installer_pull_argv, stdout=installer_bytes
            ).as_json()
        ),
        "guest-installer.json": (
            start_control.CommandObservation.from_bytes(
                installer_argv
            ).as_json()
        ),
        "guest-transfer-evidence-readback-1.json": first_result,
        "source-bundle-readback-1.json": {
            "format": input_transfer.EVIDENCE_FORMAT,
            "observation": bundle_observation,
        },
        "source-bundle-readback-2.json": {
            "format": input_transfer.EVIDENCE_FORMAT,
            "observation": bundle_observation,
        },
        "source-bundle-postflight.json": {
            "descriptor_unchanged": True,
            "format": input_transfer.EVIDENCE_FORMAT,
            "inventory_unchanged": True,
            "sha256": request.source_bundle_sha256,
            "size": request.source_bundle_size,
        },
        "terminal.json": terminal,
    }
    for name in resolution_bindings.REQUIRED_TRANSFER_ENTRY_NAMES:
        value = payloads.get(name, {})
        (root / name).write_text(
            json.dumps(value, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        (root / name).chmod(0o600)
    manifest_lines = []
    for name in resolution_bindings.REQUIRED_TRANSFER_ENTRY_NAMES:
        manifest_lines.append(
            f"{hashlib.sha256((root / name).read_bytes()).hexdigest()}  {name}\n"
        )
    (root / "files.sha256").write_text(
        "".join(manifest_lines), encoding="ascii"
    )
    (root / "files.sha256").chmod(0o600)


def transfer_result_pull_argv(
    request: resolution.CanonicalInputResolutionRequest,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        request.guest_transfer_evidence_path,
    )


def probe_result_pull_argv(
    request: resolution.CanonicalInputResolutionRequest,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        request.guest_probe_evidence_path,
    )


def lsof_output(request: resolution.CanonicalInputResolutionRequest) -> bytes:
    data = request.target_package_path / "Data"
    return (
        "p2177\n"
        "cQEMULauncher\n"
        "f17\n"
        "tREG\n"
        f"n{data / 'efi_vars.fd'}\n"
        "f18\n"
        "tREG\n"
        f"n{data / resolution.network_ready.REQUIRED_QCOW2_NAME}\n"
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


def assert_manifest_valid(test: unittest.TestCase, root: Path) -> None:
    for line in (root / "files.sha256").read_text(
        encoding="ascii"
    ).splitlines():
        expected_hash, name = line.split("  ", maxsplit=1)
        path = root / name
        test.assertEqual(
            hashlib.sha256(path.read_bytes()).hexdigest(), expected_hash
        )


if __name__ == "__main__":
    unittest.main()

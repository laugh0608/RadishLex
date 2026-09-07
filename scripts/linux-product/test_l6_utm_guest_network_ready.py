#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from unittest import mock

import l6_utm_guest_network_ready as network_ready
import l6_utm_start_once as start_control


BOOT_ID = "12345678-1234-4234-8234-123456789abc"


@dataclass(frozen=True)
class ExpectedCall:
    argv: tuple[str, ...]
    observation: start_control.CommandObservation
    stdin_bytes: bytes | None = None


class FakeRunner:
    def __init__(self, expected: list[ExpectedCall]) -> None:
        self.expected = expected
        self.calls: list[tuple[str, ...]] = []

    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_bytes: bytes | None = None,
    ) -> start_control.CommandObservation:
        del timeout_seconds
        if not self.expected:
            raise AssertionError(f"unexpected command: {argv}")
        expected = self.expected.pop(0)
        if expected.argv != argv:
            raise AssertionError(f"expected {expected.argv}, received {argv}")
        if expected.stdin_bytes != stdin_bytes:
            raise AssertionError(f"stdin mismatch for {argv}")
        self.calls.append(argv)
        return expected.observation


class LinuxL6UtmGuestNetworkReadyTests(unittest.TestCase):
    def test_network_ready_requires_exact_script_and_double_readback(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            script = guest_script(request)
            evidence = guest_evidence(request)
            runner = FakeRunner(
                successful_prefix(request, script)
                + [
                    expected(
                        network_exec_argv(request),
                        exit_code=0,
                    ),
                    expected(
                        evidence_pull_argv(request), stdout=evidence
                    ),
                    expected(
                        evidence_pull_argv(request), stdout=evidence
                    ),
                ]
            )

            with mock.patch.object(
                network_ready,
                "validate_target_files",
                return_value={"format": network_ready.EVIDENCE_FORMAT},
            ):
                result = network_ready.run_guest_network_ready(
                    request,
                    runner=runner,
                    binding_validator=valid_binding,
                )

            self.assertEqual(result.outcome, "network-ready")
            self.assertEqual(result.exit_code, network_ready.EXIT_NETWORK_READY)
            self.assertEqual(result.network_script_invocations, 1)
            self.assertEqual(result.file_pull_invocations, 3)
            self.assertEqual(runner.expected, [])
            self.assertFalse(
                any(
                    call[:2]
                    in {
                        ("utmctl", "list"),
                        ("utmctl", "start"),
                        ("utmctl", "status"),
                        ("utmctl", "stop"),
                    }
                    for call in runner.calls
                )
            )
            self.assertTrue(
                all(
                    len(call) >= 5 and call[3] == "--cmd"
                    for call in runner.calls
                    if call[:2] == ("utmctl", "exec")
                )
            )
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["network_evidence_outcome"], "passed")
            self.assertEqual(terminal["business_input"], "not-performed")
            self.assertEqual(terminal["transaction"], "not-performed")
            self.assert_manifest_valid(request.output_root)

    def test_missing_qcow2_handle_rejects_before_guest_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                [
                    expected(network_ready.PROCESS_COMMAND, stdout=ps_output()),
                    expected(
                        network_ready._lsof_argv(request),
                        stdout=lsof_output(request, include_qcow2=False),
                    ),
                ]
            )

            with mock.patch.object(
                network_ready,
                "validate_target_files",
                return_value={"format": network_ready.EVIDENCE_FORMAT},
            ):
                result = network_ready.run_guest_network_ready(
                    request,
                    runner=runner,
                    binding_validator=valid_binding,
                )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.network_script_invocations, 0)
            self.assertFalse(any(call[0] == "utmctl" for call in runner.calls))

    def test_different_second_evidence_readback_is_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            script = guest_script(request)
            evidence = guest_evidence(request)
            drifted = evidence.replace(
                b"ipv6_main_route_count=0\n",
                b"ipv6_main_route_count=1\n",
            )
            runner = FakeRunner(
                successful_prefix(request, script)
                + [
                    expected(network_exec_argv(request)),
                    expected(evidence_pull_argv(request), stdout=evidence),
                    expected(evidence_pull_argv(request), stdout=drifted),
                ]
            )

            with mock.patch.object(
                network_ready,
                "validate_target_files",
                return_value={"format": network_ready.EVIDENCE_FORMAT},
            ):
                result = network_ready.run_guest_network_ready(
                    request,
                    runner=runner,
                    binding_validator=valid_binding,
                )

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(
                result.exit_code, network_ready.EXIT_STATE_INDETERMINATE
            )
            self.assertEqual(result.network_script_invocations, 1)

    def test_canonical_failed_guest_evidence_is_not_ready(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            script = guest_script(request)
            evidence = guest_evidence(
                request,
                outcome="failed",
                reason="interface-shutdown-failed",
            )
            runner = FakeRunner(
                successful_prefix(request, script)
                + [
                    expected(network_exec_argv(request), exit_code=10),
                    expected(evidence_pull_argv(request), stdout=evidence),
                    expected(evidence_pull_argv(request), stdout=evidence),
                ]
            )

            with mock.patch.object(
                network_ready,
                "validate_target_files",
                return_value={"format": network_ready.EVIDENCE_FORMAT},
            ):
                result = network_ready.run_guest_network_ready(
                    request,
                    runner=runner,
                    binding_validator=valid_binding,
                )

            self.assertEqual(result.outcome, "network-not-ready")
            self.assertEqual(
                result.exit_code, network_ready.EXIT_NETWORK_NOT_READY
            )

    def test_authorization_and_target_are_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(
                Path(temporary),
                authorized_guest_network_shutdown_and_verify=False,
            )
            with self.assertRaisesRegex(
                network_ready.NetworkReadyError,
                "authorized-guest-network-shutdown-and-verify-required",
            ):
                request.validate()

            failure_root = Path(temporary) / "failure-drift"
            failure_root.mkdir()
            drifted_failure = self.request(
                failure_root,
                prior_network_failure_manifest_sha256="b" * 64,
            )
            with self.assertRaisesRegex(
                network_ready.NetworkReadyError,
                "required-prior-network-failure-manifest-mismatch",
            ):
                drifted_failure.validate()

    def request(
        self, temporary_root: Path, **overrides: object
    ) -> network_ready.NetworkReadyRequest:
        prior = temporary_root / "prior-launch"
        prior.mkdir()
        prior_failure = temporary_root / "prior-network-failure"
        prior_failure.mkdir()
        values: dict[str, object] = {
            "repository_root": Path(__file__).resolve().parents[2],
            "expected_repository_head": "a" * 40,
            "prior_launch_root": prior,
            "prior_launch_manifest_sha256": (
                network_ready.REQUIRED_PRIOR_LAUNCH_MANIFEST_SHA256
            ),
            "prior_network_failure_root": prior_failure,
            "prior_network_failure_manifest_sha256": (
                network_ready.REQUIRED_PRIOR_NETWORK_FAILURE_MANIFEST_SHA256
            ),
            "output_root": temporary_root / "network-evidence",
            "attempt_id": "d75818f-v4-20260823",
            "target_uuid": network_ready.REQUIRED_TARGET_UUID,
            "target_name": network_ready.REQUIRED_TARGET_NAME,
            "target_package_path": (
                network_ready.transport_bindings.expected_target_package_path(
                    network_ready.REQUIRED_TARGET_NAME
                )
            ),
            "command_timeout_seconds": 60,
            "authorized_guest_network_shutdown_and_verify": True,
            "authorized_control_transfer": True,
            "authorized_no_business_input_transaction_or_stop": True,
        }
        values.update(overrides)
        return network_ready.NetworkReadyRequest(  # type: ignore[arg-type]
            **values
        )

    def assert_manifest_valid(self, root: Path) -> None:
        for line in (root / "files.sha256").read_text(
            encoding="ascii"
        ).splitlines():
            expected_hash, name = line.split("  ", maxsplit=1)
            path = root / name
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertEqual(
                hashlib.sha256(path.read_bytes()).hexdigest(), expected_hash
            )


def successful_prefix(
    request: network_ready.NetworkReadyRequest, script: bytes
) -> list[ExpectedCall]:
    guest_root = request.guest_root
    incoming = f"{guest_root}/network-ready.incoming.sh"
    installed = f"{guest_root}/network-ready.sh"
    return [
        expected(network_ready.PROCESS_COMMAND, stdout=ps_output()),
        expected(
            network_ready._lsof_argv(request), stdout=lsof_output(request)
        ),
        expected(
            (
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/mkdir",
                "-m",
                "0700",
                guest_root,
            )
        ),
        expected(
            (
                "utmctl",
                "file",
                "push",
                request.target_uuid,
                incoming,
            ),
            stdin_bytes=script,
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
                incoming,
                installed,
            )
        ),
        expected(
            (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                installed,
            ),
            stdout=script,
        ),
    ]


def network_exec_argv(
    request: network_ready.NetworkReadyRequest,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/bin/sh",
        f"{request.guest_root}/network-ready.sh",
        request.guest_root,
        request.attempt_id,
    )


def evidence_pull_argv(
    request: network_ready.NetworkReadyRequest,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        f"{request.guest_root}/network.evidence",
    )


def expected(
    argv: tuple[str, ...],
    *,
    stdout: bytes = b"",
    stderr: bytes = b"",
    exit_code: int | None = 0,
    timed_out: bool = False,
    stdin_bytes: bytes | None = None,
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
        stdin_bytes,
    )


def ps_output() -> bytes:
    return b"1 0 0 launchd\n"


def lsof_output(
    request: network_ready.NetworkReadyRequest,
    *,
    include_qcow2: bool = True,
) -> bytes:
    data = request.target_package_path / "Data"
    lines = [
        "p2177",
        "cQEMULauncher",
        "f17",
        "tREG",
        f"n{data / 'efi_vars.fd'}",
    ]
    if include_qcow2:
        lines.extend(
            [
                "f18",
                "tREG",
                f"n{data / network_ready.REQUIRED_QCOW2_NAME}",
            ]
        )
    return ("\n".join(lines) + "\n").encode("utf-8")


def guest_script(request: network_ready.NetworkReadyRequest) -> bytes:
    return (
        request.repository_root / network_ready.GUEST_SCRIPT_RELATIVE_PATH
    ).read_bytes()


def guest_evidence(
    request: network_ready.NetworkReadyRequest,
    *,
    outcome: str = "passed",
    reason: str = "none",
) -> bytes:
    return (
        f"format={network_ready.GUEST_EVIDENCE_FORMAT}\n"
        f"attempt_id={request.attempt_id}\n"
        f"boot_id={BOOT_ID}\n"
        f"outcome={outcome}\n"
        f"reason={reason}\n"
        "active_interface_count=1\n"
        "active_interfaces=lo\n"
        "non_loopback_interface_count=1\n"
        "non_loopback_up_count=0\n"
        "ipv4_main_route_count=0\n"
        "ipv6_main_route_count=0\n"
    ).encode("ascii")


def valid_binding(
    request: network_ready.NetworkReadyRequest,
) -> dict[str, object]:
    return {
        "format": network_ready.EVIDENCE_FORMAT,
        "repository_head": request.expected_repository_head,
    }


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise AssertionError("expected JSON object")
    return value


if __name__ == "__main__":
    unittest.main()

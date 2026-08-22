#!/usr/bin/env python3
from __future__ import annotations

import base64
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

import l6_utm_start_once


TARGET_UUID = "B0B826F6-D7D3-433C-8987-E2D6993A87B3"
TARGET_NAME = (
    "RadishLex-Debian13-ARM64-L6-d75818f-crash-install-artifacts-staged"
)
OTHER_UUID = "11111111-1111-4111-8111-111111111111"


class FakeRunner:
    def __init__(
        self,
        observations: list[l6_utm_start_once.CommandObservation],
    ) -> None:
        self.observations = observations
        self.calls: list[tuple[str, ...]] = []

    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> l6_utm_start_once.CommandObservation:
        del timeout_seconds
        if not self.observations:
            raise AssertionError(f"unexpected command: {argv}")
        observation = self.observations.pop(0)
        if observation.argv != argv:
            raise AssertionError(
                f"expected command {observation.argv}, received {argv}"
            )
        self.calls.append(argv)
        return observation


class LinuxL6UtmStartOnceTests(unittest.TestCase):
    def test_started_requires_status_and_terminal_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary), poll_attempts=3)
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    observation(
                        ("utmctl", "status", TARGET_UUID), stdout=b"stopped\n"
                    ),
                    observation(("utmctl", "start", TARGET_UUID)),
                    observation(
                        ("utmctl", "status", TARGET_UUID), stdout=b"stopped\n"
                    ),
                    observation(
                        ("utmctl", "status", TARGET_UUID), stdout=b"started\n"
                    ),
                    observation(("utmctl", "list"), stdout=vm_list("started")),
                ]
            )

            result = l6_utm_start_once.run_start_once(
                request,
                runner=runner,
                sleeper=lambda _: None,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "started-observed")
            self.assertEqual(result.exit_code, l6_utm_start_once.EXIT_STARTED)
            self.assertEqual(result.start_invocations, 1)
            self.assertEqual(result.status_poll_count, 2)
            self.assertEqual(
                runner.calls.count(("utmctl", "start", TARGET_UUID)), 1
            )
            self.assert_manifest_valid(request.output_root)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["automatic_retry"], "not-performed")
            self.assertEqual(terminal["automatic_stop"], "not-performed")
            self.assertEqual(terminal["guest_exec"], "not-performed")

    def test_osstatus_diagnostic_and_each_stopped_poll_are_preserved(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary), poll_attempts=3)
            diagnostic = (
                b"Error from event: The operation couldn't be completed. "
                b"(OSStatus error -1712.)\n"
            )
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    observation(
                        ("utmctl", "status", TARGET_UUID), stdout=b"stopped\n"
                    ),
                    observation(
                        ("utmctl", "start", TARGET_UUID), stderr=diagnostic
                    ),
                    *[
                        observation(
                            ("utmctl", "status", TARGET_UUID),
                            stdout=b"stopped\n",
                        )
                        for _ in range(3)
                    ],
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                ]
            )

            result = l6_utm_start_once.run_start_once(
                request,
                runner=runner,
                sleeper=lambda _: None,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "failed-closed-stopped")
            self.assertEqual(
                result.exit_code,
                l6_utm_start_once.EXIT_FAILED_CLOSED_STOPPED,
            )
            self.assertEqual(result.status_poll_count, 3)
            start = read_json(request.output_root / "utmctl-start.json")
            captured = base64.b64decode(start["stderr"]["prefix_base64"])
            self.assertEqual(captured, diagnostic)
            poll_files = sorted(request.output_root.glob("utmctl-status-poll-*.json"))
            self.assertEqual(len(poll_files), 3)
            self.assertTrue(
                all(call[1] in ("list", "status", "start") for call in runner.calls)
            )
            self.assertNotIn(("utmctl", "stop", TARGET_UUID), runner.calls)
            self.assert_manifest_valid(request.output_root)

    def test_start_timeout_can_still_be_proven_started_by_state(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary), poll_attempts=2)
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    observation(
                        ("utmctl", "status", TARGET_UUID), stdout=b"stopped\n"
                    ),
                    observation(
                        ("utmctl", "start", TARGET_UUID),
                        exit_code=None,
                        timed_out=True,
                        stderr=b"synthetic timeout diagnostic\n",
                    ),
                    observation(
                        ("utmctl", "status", TARGET_UUID), stdout=b"started\n"
                    ),
                    observation(("utmctl", "list"), stdout=vm_list("started")),
                ]
            )

            result = l6_utm_start_once.run_start_once(
                request,
                runner=runner,
                sleeper=lambda _: None,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "started-observed")
            terminal = read_json(request.output_root / "terminal.json")
            self.assertTrue(terminal["start_command_timed_out"])
            self.assertIsNone(terminal["start_command_exit_code"])

    def test_started_terminal_without_started_status_is_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary), poll_attempts=2)
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    observation(
                        ("utmctl", "status", TARGET_UUID), stdout=b"stopped\n"
                    ),
                    observation(("utmctl", "start", TARGET_UUID)),
                    observation(
                        ("utmctl", "status", TARGET_UUID), stdout=b"stopped\n"
                    ),
                    observation(
                        ("utmctl", "status", TARGET_UUID), stdout=b"stopped\n"
                    ),
                    observation(("utmctl", "list"), stdout=vm_list("started")),
                ]
            )

            result = l6_utm_start_once.run_start_once(
                request,
                runner=runner,
                sleeper=lambda _: None,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(
                result.exit_code, l6_utm_start_once.EXIT_STATE_INDETERMINATE
            )
            self.assertEqual(result.start_invocations, 1)

    def test_running_peer_rejects_precondition_without_start(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary), poll_attempts=2)
            runner = FakeRunner(
                [
                    observation(
                        ("utmctl", "list"),
                        stdout=vm_list("stopped", other_status="started"),
                    )
                ]
            )

            result = l6_utm_start_once.run_start_once(
                request,
                runner=runner,
                sleeper=lambda _: None,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.start_invocations, 0)
            self.assertEqual(runner.calls, [("utmctl", "list")])
            self.assert_manifest_valid(request.output_root)

    def test_authorization_and_absent_output_root_are_mandatory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            unauthorized = self.request(
                root, authorized_l6_vm_start=False
            )
            with self.assertRaises(l6_utm_start_once.StartControlError):
                l6_utm_start_once.run_start_once(
                    unauthorized,
                    runner=FakeRunner([]),
                    binding_validator=valid_binding,
                )

            existing = self.request(root, attempt_id="existing")
            existing.output_root.mkdir()
            with self.assertRaises(l6_utm_start_once.StartControlError):
                l6_utm_start_once.run_start_once(
                    existing,
                    runner=FakeRunner([]),
                    binding_validator=valid_binding,
                )

            inside_repository = self.request(
                root,
                attempt_id="inside-repository",
                output_root=root / "repo" / "evidence",
            )
            with self.assertRaises(l6_utm_start_once.StartControlError):
                inside_repository.validate()

    def test_prior_failure_manifest_is_verified_entry_by_entry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            evidence = root / "failure"
            evidence.mkdir(mode=0o700)
            payload = evidence / "failure.json"
            payload.write_bytes(b'{"outcome":"failed-closed"}\n')
            payload.chmod(0o600)
            payload_hash = hashlib.sha256(payload.read_bytes()).hexdigest()
            manifest = evidence / "files.sha256"
            manifest.write_text(
                f"{payload_hash}  failure.json\n", encoding="ascii"
            )
            manifest.chmod(0o600)

            self.assertEqual(
                l6_utm_start_once._verify_sha256_manifest(evidence, manifest),
                1,
            )
            payload.write_bytes(b'{"outcome":"changed"}\n')
            with self.assertRaises(l6_utm_start_once.StartControlError):
                l6_utm_start_once._verify_sha256_manifest(evidence, manifest)

    def test_executed_control_is_bound_to_the_repository_copy(self) -> None:
        repository_root = Path(__file__).resolve().parents[2]
        digest = l6_utm_start_once._validate_control_identity(repository_root)
        self.assertRegex(digest, r"^[0-9a-f]{64}$")
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaises(l6_utm_start_once.StartControlError):
                l6_utm_start_once._validate_control_identity(Path(temporary))

    def request(self, root: Path, **overrides: object) -> l6_utm_start_once.StartRequest:
        values: dict[str, object] = {
            "repository_root": root / "repo",
            "expected_repository_head": "a" * 40,
            "prior_failure_root": root / "failure",
            "prior_failure_manifest_sha256": "b" * 64,
            "output_root": root / "start-evidence",
            "attempt_id": "synthetic-attempt",
            "clone_uuid": TARGET_UUID,
            "clone_name": TARGET_NAME,
            "expected_vm_count": 2,
            "poll_attempts": 3,
            "poll_interval_seconds": 1,
            "start_timeout_seconds": 90,
            "command_timeout_seconds": 15,
            "authorized_l6_vm_start": True,
            "authorized_no_automatic_stop_or_retry": True,
        }
        values.update(overrides)
        return l6_utm_start_once.StartRequest(**values)  # type: ignore[arg-type]

    def assert_manifest_valid(self, root: Path) -> None:
        lines = (root / "files.sha256").read_text(encoding="ascii").splitlines()
        self.assertGreaterEqual(len(lines), 1)
        for line in lines:
            expected_hash, name = line.split("  ", maxsplit=1)
            actual_hash = hashlib.sha256((root / name).read_bytes()).hexdigest()
            self.assertEqual(actual_hash, expected_hash)


def valid_binding(
    request: l6_utm_start_once.StartRequest,
) -> dict[str, object]:
    return {
        "format": l6_utm_start_once.EVIDENCE_FORMAT,
        "prior_failure_entries_verified": 7,
        "prior_failure_manifest_sha256": request.prior_failure_manifest_sha256,
        "repository_clean": True,
        "repository_head": request.expected_repository_head,
    }


def observation(
    argv: tuple[str, ...],
    *,
    exit_code: int | None = 0,
    timed_out: bool = False,
    stdout: bytes = b"",
    stderr: bytes = b"",
) -> l6_utm_start_once.CommandObservation:
    return l6_utm_start_once.CommandObservation.from_bytes(
        argv,
        exit_code=exit_code,
        timed_out=timed_out,
        stdout=stdout,
        stderr=stderr,
    )


def vm_list(target_status: str, *, other_status: str = "stopped") -> bytes:
    return (
        "UUID Status Name\n"
        f"{TARGET_UUID} {target_status} {TARGET_NAME}\n"
        f"{OTHER_UUID} {other_status} Synthetic-Peer\n"
    ).encode("utf-8")


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()

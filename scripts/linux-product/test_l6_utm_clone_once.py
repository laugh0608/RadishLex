#!/usr/bin/env python3
from __future__ import annotations

import base64
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

import l6_utm_clone_once


SOURCE_UUID = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"
SOURCE_NAME = "RadishLex-Debian13-ARM64-DependencyFrozen"
TARGET_UUID = "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB"
TARGET_NAME = (
    "RadishLex-Debian13-ARM64-L6-d75818f-crash-install-artifacts-staged-v2"
)
OTHER_UUID = "11111111-1111-4111-8111-111111111111"


class FakeRunner:
    def __init__(
        self,
        observations: list[l6_utm_clone_once.CommandObservation],
        *,
        after_clone: object | None = None,
    ) -> None:
        self.observations = observations
        self.after_clone = after_clone
        self.calls: list[tuple[str, ...]] = []

    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> l6_utm_clone_once.CommandObservation:
        del timeout_seconds
        if not self.observations:
            raise AssertionError(f"unexpected command: {argv}")
        observation = self.observations.pop(0)
        if observation.argv != argv:
            raise AssertionError(
                f"expected command {observation.argv}, received {argv}"
            )
        self.calls.append(argv)
        if argv[1] == "clone" and callable(self.after_clone):
            self.after_clone()
        return observation


class LinuxL6UtmCloneOnceTests(unittest.TestCase):
    def test_created_requires_command_registry_and_exact_package(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=preclone_list()),
                    observation(clone_argv()),
                    observation(("utmctl", "list"), stdout=created_list()),
                ],
                after_clone=request.target_package_path.mkdir,
            )

            result = l6_utm_clone_once.run_clone_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "created")
            self.assertEqual(result.exit_code, l6_utm_clone_once.EXIT_CREATED)
            self.assertEqual(result.clone_invocations, 1)
            self.assertEqual(runner.calls.count(clone_argv()), 1)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["automatic_retry"], "not-performed")
            self.assertEqual(terminal["automatic_delete"], "not-performed")
            self.assertEqual(terminal["automatic_start"], "not-performed")
            self.assertEqual(terminal["guest_exec"], "not-performed")
            self.assert_manifest_valid(request.output_root)

    def test_exit_zero_with_osstatus_and_no_landing_fails_closed_absent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            diagnostic = (
                b"Error from event: The operation couldn't be completed. "
                b"(OSStatus error -1712.)\n"
            )
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=preclone_list()),
                    observation(clone_argv(), stderr=diagnostic),
                    observation(("utmctl", "list"), stdout=preclone_list()),
                ]
            )

            result = l6_utm_clone_once.run_clone_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "failed-closed-absent")
            self.assertEqual(
                result.exit_code,
                l6_utm_clone_once.EXIT_FAILED_CLOSED_ABSENT,
            )
            captured = read_json(request.output_root / "utmctl-clone.json")
            self.assertEqual(
                base64.b64decode(captured["stderr"]["prefix_base64"]),
                diagnostic,
            )
            self.assertEqual(runner.calls.count(clone_argv()), 1)
            self.assert_manifest_valid(request.output_root)

    def test_large_output_keeps_bounded_prefix_and_full_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            output = b"x" * (l6_utm_clone_once.MAX_CAPTURE_BYTES + 123)
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=preclone_list()),
                    observation(clone_argv(), stdout=output),
                    observation(("utmctl", "list"), stdout=preclone_list()),
                ]
            )

            result = l6_utm_clone_once.run_clone_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "failed-closed-absent")
            captured = read_json(request.output_root / "utmctl-clone.json")[
                "stdout"
            ]
            self.assertTrue(captured["truncated"])
            self.assertEqual(captured["total_bytes"], len(output))
            self.assertEqual(captured["sha256"], hashlib.sha256(output).hexdigest())
            self.assertEqual(
                len(base64.b64decode(captured["prefix_base64"])),
                l6_utm_clone_once.MAX_CAPTURE_BYTES,
            )

    def test_timeout_with_complete_landing_is_state_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=preclone_list()),
                    observation(
                        clone_argv(),
                        exit_code=None,
                        timed_out=True,
                        stderr=b"synthetic timeout\n",
                    ),
                    observation(("utmctl", "list"), stdout=created_list()),
                ],
                after_clone=request.target_package_path.mkdir,
            )

            result = l6_utm_clone_once.run_clone_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(
                result.exit_code, l6_utm_clone_once.EXIT_STATE_INDETERMINATE
            )
            self.assertEqual(result.clone_invocations, 1)

    def test_registry_only_landing_is_state_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=preclone_list()),
                    observation(clone_argv()),
                    observation(("utmctl", "list"), stdout=created_list()),
                ]
            )

            result = l6_utm_clone_once.run_clone_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.clone_invocations, 1)

    def test_package_only_landing_is_state_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=preclone_list()),
                    observation(clone_argv()),
                    observation(("utmctl", "list"), stdout=preclone_list()),
                ],
                after_clone=request.target_package_path.mkdir,
            )

            result = l6_utm_clone_once.run_clone_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.clone_invocations, 1)

    def test_duplicate_target_name_and_count_drift_are_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            duplicate = (
                created_list()
                + (
                    "CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC stopped "
                    f"{TARGET_NAME}\n"
                ).encode("utf-8")
            )
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=preclone_list()),
                    observation(clone_argv()),
                    observation(("utmctl", "list"), stdout=duplicate),
                ],
                after_clone=request.target_package_path.mkdir,
            )

            result = l6_utm_clone_once.run_clone_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.clone_invocations, 1)

    def test_running_peer_rejects_precondition_without_clone(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                [
                    observation(
                        ("utmctl", "list"),
                        stdout=preclone_list(other_status="started"),
                    )
                ]
            )

            result = l6_utm_clone_once.run_clone_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.clone_invocations, 0)
            self.assertEqual(runner.calls, [("utmctl", "list")])
            self.assert_manifest_valid(request.output_root)

    def test_inventory_hash_drift_rejects_precondition_without_clone(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(
                Path(temporary), expected_preclone_inventory_sha256="c" * 64
            )
            runner = FakeRunner(
                [observation(("utmctl", "list"), stdout=preclone_list())]
            )

            result = l6_utm_clone_once.run_clone_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.clone_invocations, 0)

    def test_dual_authorization_absent_output_and_absent_package_are_required(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for override in (
                {"authorized_l6_vm_clone": False},
                {"authorized_no_automatic_retry_delete_or_start": False},
            ):
                with self.assertRaises(l6_utm_clone_once.CloneControlError):
                    self.request(root, **override).validate()

            existing_output = self.request(root, attempt_id="existing-output")
            existing_output.output_root.mkdir()
            with self.assertRaises(l6_utm_clone_once.CloneControlError):
                l6_utm_clone_once.run_clone_once(
                    existing_output,
                    runner=FakeRunner([]),
                    binding_validator=valid_binding,
                )

            existing_package = self.request(root, attempt_id="existing-package")
            existing_package.target_package_path.mkdir()
            result = l6_utm_clone_once.run_clone_once(
                existing_package,
                runner=FakeRunner([]),
                binding_validator=valid_binding,
            )
            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.clone_invocations, 0)

    def test_prior_manifest_and_executed_control_identity_are_verified(self) -> None:
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
                l6_utm_clone_once._verify_sha256_manifest(evidence, manifest),
                1,
            )
            payload.write_bytes(b'{"outcome":"changed"}\n')
            with self.assertRaises(l6_utm_clone_once.CloneControlError):
                l6_utm_clone_once._verify_sha256_manifest(evidence, manifest)

            repository_root = Path(__file__).resolve().parents[2]
            digest = l6_utm_clone_once._validate_control_identity(repository_root)
            self.assertRegex(digest, r"^[0-9a-f]{64}$")
            with self.assertRaises(l6_utm_clone_once.CloneControlError):
                l6_utm_clone_once._validate_control_identity(root)

    def request(
        self, root: Path, **overrides: object
    ) -> l6_utm_clone_once.CloneRequest:
        package_parent = root / "utm-packages"
        package_parent.mkdir(exist_ok=True)
        values: dict[str, object] = {
            "repository_root": root / "repo",
            "expected_repository_head": "a" * 40,
            "prior_failure_root": root / "failure",
            "prior_failure_manifest_sha256": "b" * 64,
            "output_root": root / f"clone-evidence-{overrides.get('attempt_id', 'default')}",
            "attempt_id": "synthetic-attempt",
            "source_uuid": SOURCE_UUID,
            "source_name": SOURCE_NAME,
            "target_name": TARGET_NAME,
            "target_package_path": package_parent / f"{TARGET_NAME}.utm",
            "expected_vm_count": 2,
            "expected_preclone_inventory_sha256": inventory_sha256(
                preclone_list()
            ),
            "clone_timeout_seconds": 120,
            "command_timeout_seconds": 15,
            "authorized_l6_vm_clone": True,
            "authorized_no_automatic_retry_delete_or_start": True,
        }
        values.update(overrides)
        return l6_utm_clone_once.CloneRequest(**values)  # type: ignore[arg-type]

    def assert_manifest_valid(self, root: Path) -> None:
        lines = (root / "files.sha256").read_text(encoding="ascii").splitlines()
        self.assertGreaterEqual(len(lines), 1)
        for line in lines:
            expected_hash, name = line.split("  ", maxsplit=1)
            actual_hash = hashlib.sha256((root / name).read_bytes()).hexdigest()
            self.assertEqual(actual_hash, expected_hash)


def valid_binding(
    request: l6_utm_clone_once.CloneRequest,
) -> dict[str, object]:
    return {
        "control_sha256": "d" * 64,
        "format": l6_utm_clone_once.EVIDENCE_FORMAT,
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
) -> l6_utm_clone_once.CommandObservation:
    return l6_utm_clone_once.CommandObservation.from_bytes(
        argv,
        exit_code=exit_code,
        timed_out=timed_out,
        stdout=stdout,
        stderr=stderr,
    )


def clone_argv() -> tuple[str, ...]:
    return ("utmctl", "clone", SOURCE_UUID, TARGET_NAME)


def preclone_list(*, other_status: str = "stopped") -> bytes:
    return (
        "UUID Status Name\n"
        f"{SOURCE_UUID} stopped {SOURCE_NAME}\n"
        f"{OTHER_UUID} {other_status} Synthetic-Peer\n"
    ).encode("utf-8")


def created_list() -> bytes:
    return (
        "UUID Status Name\n"
        f"{OTHER_UUID} stopped Synthetic-Peer\n"
        f"{TARGET_UUID} stopped {TARGET_NAME}\n"
        f"{SOURCE_UUID} stopped {SOURCE_NAME}\n"
    ).encode("utf-8")


def inventory_sha256(value: bytes) -> str:
    parsed = l6_utm_clone_once.parse_utmctl_list(
        observation(("utmctl", "list"), stdout=value)
    )
    return l6_utm_clone_once.canonical_inventory_sha256(parsed)


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()

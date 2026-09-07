#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import os
import plistlib
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest import mock

import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control


TARGET_UUID = launch_transport.REQUIRED_TARGET_UUID
TARGET_NAME = launch_transport.REQUIRED_TARGET_NAME
PEERS = tuple(
    (str(uuid.UUID(int=index)).upper(), f"Synthetic-Frozen-Peer-{index:02d}")
    for index in range(1, 21)
)
PEER_UUID, PEER_NAME = PEERS[0]


class FakeRunner:
    def __init__(
        self, observations: list[start_control.CommandObservation]
    ) -> None:
        self.observations = observations
        self.calls: list[tuple[str, ...]] = []

    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> start_control.CommandObservation:
        del timeout_seconds
        if not self.observations:
            raise AssertionError(f"unexpected command: {argv}")
        expected = self.observations.pop(0)
        if expected.argv != argv:
            raise AssertionError(
                f"expected command {expected.argv}, received {argv}"
            )
        self.calls.append(argv)
        return expected


class LinuxL6UtmLaunchTransportV2Tests(unittest.TestCase):
    def test_started_requires_exact_transport_inventory_and_backend(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary), poll_attempts=2)
            runner = FakeRunner(
                [
                    process_observation(),
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"stopped\n",
                    ),
                    process_observation(),
                    observation(launch_transport.transport_argv(request)),
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"started\n",
                    ),
                    process_observation("QEMULauncher"),
                    observation(("utmctl", "list"), stdout=vm_list("started")),
                    process_observation("UTM", "QEMULauncher"),
                ]
            )

            result = launch_transport.run_launch_transport_once(
                request,
                runner=runner,
                sleeper=lambda _: None,
                binding_validator=valid_binding,
                target_identity_validator=valid_target_identity,
            )

            self.assertEqual(result.outcome, "started-observed")
            self.assertEqual(result.exit_code, launch_transport.EXIT_STARTED)
            self.assertEqual(result.launch_invocations, 1)
            self.assertEqual(result.status_poll_count, 1)
            self.assertEqual(
                runner.calls.count(launch_transport.transport_argv(request)), 1
            )
            self.assertFalse(
                any(call[:2] == ("utmctl", "start") for call in runner.calls)
            )
            self.assertFalse(
                any(call[:2] == ("utmctl", "stop") for call in runner.calls)
            )
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["backend_process_count"], 1)
            self.assertEqual(terminal["plain_utmctl_start"], "not-performed")
            self.assertEqual(terminal["utm_hide"], "not-performed")
            self.assert_manifest_valid(request.output_root)

    def test_transport_error_all_stopped_and_no_backend_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary), poll_attempts=2)
            runner = FakeRunner(
                [
                    process_observation(),
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"stopped\n",
                    ),
                    process_observation(),
                    observation(
                        launch_transport.transport_argv(request),
                        exit_code=1,
                        stderr=b"synthetic AppleEvent failure\n",
                    ),
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"stopped\n",
                    ),
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"stopped\n",
                    ),
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    process_observation("UTM"),
                ]
            )

            result = launch_transport.run_launch_transport_once(
                request,
                runner=runner,
                sleeper=lambda _: None,
                binding_validator=valid_binding,
                target_identity_validator=valid_target_identity,
            )

            self.assertEqual(result.outcome, "failed-closed-stopped")
            self.assertEqual(
                result.exit_code,
                launch_transport.EXIT_FAILED_CLOSED_STOPPED,
            )
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["terminal_relevant_process_count"], 1)
            self.assertEqual(terminal["automatic_retry"], "not-performed")
            self.assertEqual(terminal["automatic_quit"], "not-performed")
            self.assertEqual(terminal["automatic_stop"], "not-performed")
            self.assertEqual(terminal["automatic_delete"], "not-performed")

    def test_timed_out_transport_can_only_succeed_with_postconditions(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                [
                    process_observation(),
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"stopped\n",
                    ),
                    process_observation(),
                    observation(
                        launch_transport.transport_argv(request),
                        exit_code=None,
                        timed_out=True,
                    ),
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"started\n",
                    ),
                    process_observation("QEMULauncher"),
                    observation(("utmctl", "list"), stdout=vm_list("started")),
                    process_observation("QEMULauncher"),
                ]
            )

            result = launch_transport.run_launch_transport_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
                target_identity_validator=valid_target_identity,
            )

            self.assertEqual(result.outcome, "started-observed")
            terminal = read_json(request.output_root / "terminal.json")
            self.assertTrue(terminal["transport_command_timed_out"])

    def test_started_without_backend_is_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                successful_prefix(request)
                + [
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"started\n",
                    ),
                    process_observation(),
                    observation(("utmctl", "list"), stdout=vm_list("started")),
                    process_observation("UTM"),
                ]
            )

            result = launch_transport.run_launch_transport_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
                target_identity_validator=valid_target_identity,
            )

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(
                result.exit_code, launch_transport.EXIT_STATE_INDETERMINATE
            )

    def test_started_status_waits_for_delayed_backend(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary), poll_attempts=2)
            runner = FakeRunner(
                successful_prefix(request)
                + [
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"started\n",
                    ),
                    process_observation(),
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"started\n",
                    ),
                    process_observation("QEMULauncher"),
                    observation(
                        ("utmctl", "list"), stdout=vm_list("started")
                    ),
                    process_observation("QEMULauncher"),
                ]
            )

            result = launch_transport.run_launch_transport_once(
                request,
                runner=runner,
                sleeper=lambda _: None,
                binding_validator=valid_binding,
                target_identity_validator=valid_target_identity,
            )

            self.assertEqual(result.outcome, "started-observed")
            self.assertEqual(result.status_poll_count, 2)
            self.assertEqual(
                runner.calls.count(launch_transport.transport_argv(request)), 1
            )
            self.assertTrue(
                (request.output_root / "host-processes-poll-001.json").is_file()
            )
            self.assertTrue(
                (request.output_root / "host-processes-poll-002.json").is_file()
            )

    def test_stopped_inventory_with_backend_is_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                successful_prefix(request)
                + [
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"stopped\n",
                    ),
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    process_observation("QEMULauncher"),
                ]
            )

            result = launch_transport.run_launch_transport_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
                target_identity_validator=valid_target_identity,
            )

            self.assertEqual(result.outcome, "state-indeterminate")

    def test_preflight_process_rejects_before_inventory_or_transport(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner([process_observation("UTM")])

            result = launch_transport.run_launch_transport_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
                target_identity_validator=valid_target_identity,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.launch_invocations, 0)
            self.assertEqual(runner.calls, [launch_transport.PROCESS_COMMAND])

    def test_process_after_read_only_inventory_rejects_transport(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                [
                    process_observation(),
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"stopped\n",
                    ),
                    process_observation("utmctl"),
                ]
            )

            result = launch_transport.run_launch_transport_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
                target_identity_validator=valid_target_identity,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.launch_invocations, 0)

    def test_target_identity_drift_before_transport_rejects_launch(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                [
                    process_observation(),
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    observation(
                        ("utmctl", "status", TARGET_UUID),
                        stdout=b"stopped\n",
                    ),
                    process_observation(),
                ]
            )
            calls = 0

            def drifting_target_identity(
                candidate: launch_transport.LaunchTransportRequest,
                binding: dict[str, object],
            ) -> dict[str, object]:
                nonlocal calls
                calls += 1
                value = valid_target_identity(candidate, binding)
                if calls == 2:
                    value["target_qcow2"] = {"sha256": "f" * 64}
                return value

            result = launch_transport.run_launch_transport_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
                target_identity_validator=drifting_target_identity,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.launch_invocations, 0)
            self.assertEqual(calls, 2)
            self.assertFalse(
                any(
                    call == launch_transport.transport_argv(request)
                    for call in runner.calls
                )
            )
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["target_identity_checks"], 2)
            self.assertIn(
                "target-identity-changed-before-transport",
                str(terminal["reason"]),
            )

    def test_inventory_must_be_exact_v7_plus_one_new_target(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            drifted = (
                "UUID Status Name\n"
                f"{TARGET_UUID} stopped {TARGET_NAME}\n"
                f"{PEER_UUID} started {PEER_NAME}\n"
            ).encode("utf-8")
            runner = FakeRunner(
                [
                    process_observation(),
                    observation(("utmctl", "list"), stdout=drifted),
                ]
            )

            result = launch_transport.run_launch_transport_once(
                request,
                runner=runner,
                binding_validator=valid_binding,
                target_identity_validator=valid_target_identity,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.launch_invocations, 0)

    def test_frozen_v7_target_cannot_be_reused(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(
                Path(temporary),
                target_uuid=launch_transport.FROZEN_V7_TARGET_UUID,
                target_name=launch_transport.FROZEN_V7_TARGET_NAME,
            )

            with self.assertRaisesRegex(
                launch_transport.LaunchTransportError,
                "frozen-v7-target-reuse-forbidden",
            ):
                request.validate()

    def test_committed_transport_source_has_exact_narrow_contract(self) -> None:
        repository_root = Path(__file__).resolve().parents[2]

        identity = launch_transport.validate_control_identity(repository_root)

        self.assertEqual(identity["transport_id"], launch_transport.TRANSPORT_ID)
        self.assertRegex(
            str(identity["prepared_binding_control_sha256"]),
            r"^[0-9a-f]{64}$",
        )
        self.assertRegex(str(identity["transport_sha256"]), r"^[0-9a-f]{64}$")

    def test_utm_bundle_contract_binds_version_sdef_and_start_intent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            bundle_root = Path(temporary) / "UTM.app"
            resources = bundle_root / "Contents/Resources"
            intents_root = resources / "Metadata.appintents"
            intents_root.mkdir(parents=True)
            with (bundle_root / "Contents/Info.plist").open("wb") as output:
                plistlib.dump(
                    {
                        "CFBundleIdentifier": launch_transport.EXPECTED_UTM_BUNDLE_ID,
                        "CFBundleShortVersionString": launch_transport.EXPECTED_UTM_VERSION,
                        "CFBundleVersion": launch_transport.EXPECTED_UTM_BUILD,
                    },
                    output,
                )
            (resources / "UTM.sdef").write_text(
                "<dictionary><suite>"
                '<command name="start" code="UTMvstar">'
                '<direct-parameter type="virtual machine"/>'
                '<parameter name="saving" type="boolean" optional="yes"/>'
                '<parameter name="recovery" type="boolean" optional="yes"/>'
                "</command></suite></dictionary>\n",
                encoding="utf-8",
            )
            (intents_root / "extract.actionsdata").write_text(
                json.dumps(
                    {
                        "actions": {
                            "UTMStartActionIntent": {
                                "identifier": "UTMStartActionIntent",
                                "openAppWhenRun": False,
                                "parameters": [
                                    {"name": "vmEntity"},
                                    {"name": "isRecovery"},
                                    {"name": "isDisposible"},
                                ],
                                "supportedModes": 1,
                            }
                        }
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            result = launch_transport.validate_utm_bundle(bundle_root)

            self.assertEqual(result["utm_version"], "4.7.5")
            self.assertEqual(result["utm_build"], "118")
            self.assertRegex(str(result["utm_sdef_sha256"]), r"^[0-9a-f]{64}$")

    def test_v7_binding_requires_exact_terminal_inventory_and_markers(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_root = Path(temporary)
            v7_root, manifest_sha256 = create_v7_evidence(temporary_root)
            with mock.patch.object(
                launch_transport,
                "REQUIRED_V7_MANIFEST_SHA256",
                manifest_sha256,
            ):
                request = self.request(
                    temporary_root,
                    prior_v7_root=v7_root,
                    prior_v7_manifest_sha256=manifest_sha256,
                    expected_vm_count=21,
                )
                request.validate()
                result = launch_transport.validate_v7_evidence(request)

            self.assertEqual(result["prior_v7_entries_verified"], 10)
            self.assertEqual(result["prior_v7_event_count"], 4040)
            self.assertEqual(len(result["baseline_inventory"]), 20)

    def request(
        self, temporary_root: Path, **overrides: object
    ) -> launch_transport.LaunchTransportRequest:
        repository_root = temporary_root / "repo"
        prior_v7_root = temporary_root / "prior-v7"
        repository_root.mkdir(exist_ok=True)
        prior_v7_root.mkdir(exist_ok=True)
        values: dict[str, object] = {
            "repository_root": repository_root,
            "expected_repository_head": "a" * 40,
            "prior_v7_root": prior_v7_root,
            "prior_v7_manifest_sha256": (
                launch_transport.REQUIRED_V7_MANIFEST_SHA256
            ),
            "prior_prepared_root": temporary_root / "prior-prepared",
            "prior_prepared_manifest_sha256": (
                launch_transport.REQUIRED_PREPARED_MANIFEST_SHA256
            ),
            "output_root": temporary_root / "evidence",
            "attempt_id": "synthetic-launch-transport-v2",
            "target_uuid": TARGET_UUID,
            "target_name": TARGET_NAME,
            "target_package_path": (
                launch_transport.expected_target_package_path(TARGET_NAME)
            ),
            "expected_vm_count": 21,
            "poll_attempts": 1,
            "poll_interval_seconds": 1,
            "transport_timeout_seconds": 15,
            "command_timeout_seconds": 15,
            "authorized_l6_vm_start": True,
            "authorized_foreground_applescript_transport": True,
            "authorized_no_automatic_stop_retry_delete": True,
        }
        values.update(overrides)
        return launch_transport.LaunchTransportRequest(  # type: ignore[arg-type]
            **values
        )

    def assert_manifest_valid(self, root: Path) -> None:
        self.assertEqual(stat_mode(root), 0o700)
        lines = (root / "files.sha256").read_text(
            encoding="ascii"
        ).splitlines()
        self.assertGreaterEqual(len(lines), 1)
        for line in lines:
            expected_hash, name = line.split("  ", maxsplit=1)
            path = root / name
            self.assertEqual(stat_mode(path), 0o600)
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), expected_hash)


def successful_prefix(
    request: launch_transport.LaunchTransportRequest,
) -> list[start_control.CommandObservation]:
    return [
        process_observation(),
        observation(("utmctl", "list"), stdout=vm_list("stopped")),
        observation(
            ("utmctl", "status", TARGET_UUID), stdout=b"stopped\n"
        ),
        process_observation(),
        observation(launch_transport.transport_argv(request)),
    ]


def valid_binding(
    request: launch_transport.LaunchTransportRequest,
) -> dict[str, object]:
    return {
        "baseline_inventory": [
            {"name": name, "status": "stopped", "uuid": vm_uuid}
            for vm_uuid, name in PEERS
        ],
        "expected_current_vm_count": request.expected_vm_count,
        "format": launch_transport.EVIDENCE_FORMAT,
        "prepared_target_config_sha256": "c" * 64,
        "prepared_target_efi_sha256": "d" * 64,
        "prepared_target_qcow2_sha256": "e" * 64,
        "prior_prepared_manifest_sha256": (
            request.prior_prepared_manifest_sha256
        ),
        "prior_v7_manifest_sha256": request.prior_v7_manifest_sha256,
        "repository_clean": True,
        "repository_head": request.expected_repository_head,
        "transport_id": launch_transport.TRANSPORT_ID,
        "utm_build": launch_transport.EXPECTED_UTM_BUILD,
        "utm_version": launch_transport.EXPECTED_UTM_VERSION,
    }


def valid_target_identity(
    request: launch_transport.LaunchTransportRequest,
    binding: dict[str, object],
) -> dict[str, object]:
    del binding
    return {
        "format": launch_transport.EVIDENCE_FORMAT,
        "target_config": {"sha256": "c" * 64},
        "target_efi": {"sha256": "d" * 64},
        "target_name": request.target_name,
        "target_package_name": request.target_package_path.name,
        "target_package_path_sha256": hashlib.sha256(
            str(request.target_package_path).encode("utf-8")
        ).hexdigest(),
        "target_qcow2": {"sha256": "e" * 64},
        "target_uuid": request.target_uuid,
    }


def observation(
    argv: tuple[str, ...],
    *,
    exit_code: int | None = 0,
    timed_out: bool = False,
    stdout: bytes = b"",
    stderr: bytes = b"",
) -> start_control.CommandObservation:
    return start_control.CommandObservation.from_bytes(
        argv,
        exit_code=exit_code,
        timed_out=timed_out,
        stdout=stdout,
        stderr=stderr,
    )


def process_observation(*accounting_names: str) -> start_control.CommandObservation:
    lines = ["1 0 0 launchd\n"]
    for index, name in enumerate(accounting_names, start=100):
        lines.append(f"{index} 1 501 {name}\n")
    return observation(
        launch_transport.PROCESS_COMMAND,
        stdout="".join(lines).encode("utf-8"),
    )


def vm_list(target_status: str) -> bytes:
    return (
        "UUID Status Name\n"
        f"{TARGET_UUID} {target_status} {TARGET_NAME}\n"
        + "".join(
            f"{vm_uuid} stopped {name}\n" for vm_uuid, name in PEERS
        )
    ).encode("utf-8")


def create_v7_evidence(temporary_root: Path) -> tuple[Path, str]:
    root = temporary_root / "bound-v7"
    root.mkdir(mode=0o700)
    baseline = []
    for index in range(1, 20):
        vm_uuid = str(uuid.UUID(int=index)).upper()
        baseline.append((vm_uuid, f"Synthetic-Frozen-{index:02d}"))
    baseline.append(
        (
            launch_transport.FROZEN_V7_TARGET_UUID,
            launch_transport.FROZEN_V7_TARGET_NAME,
        )
    )
    list_stdout = "UUID Status Name\n" + "".join(
        f"{vm_uuid} stopped {name}\n" for vm_uuid, name in baseline
    )
    events: list[dict[str, object]] = [
        {"message_prefix": "AESendMessage(UTMv,star synthetic", "role": "utmctl"},
        {"message_prefix": "RECEIVED:(UTMv,star) synthetic", "role": "utm-app"},
        {
            "message_prefix": (
                "Invalid parameter not satisfying: [self canBecomeMainWindow]"
            ),
            "role": "utm-app",
        },
        {
            "message_prefix": "FAULT: NSInternalInconsistencyException: synthetic",
            "role": "utm-app",
        },
    ]
    events.extend(
        {"message_prefix": f"synthetic-{index}", "role": "utmctl"}
        for index in range(4036)
    )
    values: dict[str, dict[str, object]] = {
        "binding-preflight.json": {},
        "host-process-command.json": {},
        "host-processes.json": {
            "format": launch_transport.V7_EVIDENCE_FORMAT,
            "relevant_process_count": 0,
            "relevant_processes": [],
        },
        "request.json": {
            "authorization": {
                "host_launch_diagnostics": True,
                "read_system_log": True,
            },
            "expected_vm_count": 20,
            "format": launch_transport.V7_EVIDENCE_FORMAT,
            "log_end": launch_transport.V7_LOG_END,
            "log_start": launch_transport.V7_LOG_START,
            "target_name": launch_transport.FROZEN_V7_TARGET_NAME,
            "target_uuid": launch_transport.FROZEN_V7_TARGET_UUID,
        },
        "terminal.json": {
            "automatic_delete": "not-performed",
            "automatic_retry": "not-performed",
            "format": launch_transport.V7_EVIDENCE_FORMAT,
            "guest_exec": "not-performed",
            "input_transfer": "not-performed",
            "operation_id": "not-generated",
            "outcome": "diagnostics-collected",
            "reason": "read-only-host-facts-collected",
            "root_cause": "unattributed",
            "target_name": launch_transport.FROZEN_V7_TARGET_NAME,
            "target_uuid": launch_transport.FROZEN_V7_TARGET_UUID,
            "transaction": "not-performed",
            "utm_clone": "not-performed",
            "utm_start": "not-performed",
            "utm_stop": "not-performed",
        },
        "unified-log-command.json": {},
        "unified-log-structure.json": {
            "finished_marker_count": 1,
            "finished_marker_is_terminal": True,
            "finished_value_kind": "integer-one",
            "format": launch_transport.V7_EVIDENCE_FORMAT,
            "record_count": 4041,
        },
        "unified-log.json": {
            "event_count": 4040,
            "events": events,
            "format": launch_transport.V7_EVIDENCE_FORMAT,
            "log_end": launch_transport.V7_LOG_END,
            "log_start": launch_transport.V7_LOG_START,
        },
        "utmctl-list-live.json": observation(
            ("utmctl", "list"), stdout=list_stdout.encode("utf-8")
        ).as_json(),
        "utmctl-status-live.json": {},
    }
    manifest_lines = []
    for name in sorted(values):
        path = root / name
        path.write_text(
            json.dumps(values[name], ensure_ascii=True, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        path.chmod(0o600)
        manifest_lines.append(
            f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {name}\n"
        )
    manifest = root / "files.sha256"
    manifest.write_text("".join(manifest_lines), encoding="ascii")
    manifest.chmod(0o600)
    return root, hashlib.sha256(manifest.read_bytes()).hexdigest()


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def stat_mode(path: Path) -> int:
    return os.stat(path, follow_symlinks=False).st_mode & 0o777


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

import l6_utm_launch_diagnostics


TARGET_UUID = "5B19AEF1-0F29-40B6-8F24-1B1929117DAB"
TARGET_NAME = (
    "RadishLex-Debian13-ARM64-L6-d75818f-crash-install-artifacts-staged-v3"
)
OTHER_UUID = "11111111-1111-4111-8111-111111111111"
LOG_START = "2026-08-22 10:00:00+0000"
LOG_END = "2026-08-22 10:10:00+0000"


class FakeRunner:
    def __init__(
        self,
        observations: list[l6_utm_launch_diagnostics.CommandObservation],
    ) -> None:
        self.observations = observations
        self.calls: list[tuple[str, ...]] = []

    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> l6_utm_launch_diagnostics.CommandObservation:
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


class LinuxL6UtmLaunchDiagnosticsTests(unittest.TestCase):
    def test_collects_only_read_only_bounded_redacted_host_facts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    observation(
                        ("utmctl", "status", TARGET_UUID), stdout=b"stopped\n"
                    ),
                    observation(
                        l6_utm_launch_diagnostics.PROCESS_COMMAND,
                        stdout=process_inventory(),
                    ),
                    observation(
                        log_command(),
                        stdout=unified_log_events(),
                    ),
                ]
            )

            result = l6_utm_launch_diagnostics.run_launch_diagnostics(
                request,
                runner=runner,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "diagnostics-collected")
            self.assertEqual(
                result.exit_code, l6_utm_launch_diagnostics.EXIT_COLLECTED
            )
            self.assertEqual(
                runner.calls,
                [
                    ("utmctl", "list"),
                    ("utmctl", "status", TARGET_UUID),
                    l6_utm_launch_diagnostics.PROCESS_COMMAND,
                    log_command(),
                ],
            )
            self.assert_manifest_valid(request.output_root)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["root_cause"], "unattributed")
            self.assertEqual(terminal["utm_start"], "not-performed")
            self.assertEqual(terminal["utm_stop"], "not-performed")
            self.assertEqual(terminal["guest_exec"], "not-performed")
            processes = read_json(request.output_root / "host-processes.json")
            self.assertEqual(processes["relevant_process_count"], 3)
            serialized_processes = json.dumps(processes, sort_keys=True)
            self.assertNotIn("/Applications/UTM.app", serialized_processes)
            self.assertNotIn("/opt/homebrew", serialized_processes)
            self.assertNotIn("path", serialized_processes)
            events = read_json(request.output_root / "unified-log.json")
            self.assertEqual(events["event_count"], 2)
            serialized_events = json.dumps(events, sort_keys=True)
            self.assertIn("/Users/<redacted>/", serialized_events)
            self.assertNotIn("/Users/luobo/", serialized_events)

    def test_running_target_rejects_before_process_or_log_collection(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                [observation(("utmctl", "list"), stdout=vm_list("started"))]
            )

            result = l6_utm_launch_diagnostics.run_launch_diagnostics(
                request,
                runner=runner,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(runner.calls, [("utmctl", "list")])
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["host_process_observation"], "not-performed")
            self.assertEqual(terminal["unified_log_observation"], "not-performed")
            self.assertEqual(terminal["utm_start"], "not-performed")
            self.assert_manifest_valid(request.output_root)

    def test_log_command_failure_is_diagnostics_incomplete_without_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = self.request(Path(temporary))
            runner = FakeRunner(
                [
                    observation(("utmctl", "list"), stdout=vm_list("stopped")),
                    observation(
                        ("utmctl", "status", TARGET_UUID), stdout=b"stopped\n"
                    ),
                    observation(
                        l6_utm_launch_diagnostics.PROCESS_COMMAND,
                        stdout=process_inventory(),
                    ),
                    observation(
                        log_command(), exit_code=1, stderr=b"synthetic log error\n"
                    ),
                ]
            )

            result = l6_utm_launch_diagnostics.run_launch_diagnostics(
                request,
                runner=runner,
                binding_validator=valid_binding,
            )

            self.assertEqual(result.outcome, "diagnostics-incomplete")
            self.assertEqual(
                result.exit_code,
                l6_utm_launch_diagnostics.EXIT_DIAGNOSTICS_INCOMPLETE,
            )
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["root_cause"], "unattributed")
            self.assertEqual(terminal["utm_start"], "not-performed")
            self.assert_manifest_valid(request.output_root)

    def test_malformed_or_unrelated_log_event_fails_closed(self) -> None:
        malformed_values = (
            b"not-json\n",
            unified_log_events().replace(b'{"finished":true}\n', b""),
            b'{"finished":false}\n',
            (
                json.dumps(
                    {
                        "timestamp": "2026-08-22 10:00:01.000000+0000",
                        "processImagePath": "/usr/bin/unrelated",
                        "eventMessage": "unexpected",
                    }
                )
                + "\n"
            ).encode("utf-8"),
        )
        for index, malformed in enumerate(malformed_values):
            with self.subTest(index=index), tempfile.TemporaryDirectory() as temporary:
                request = self.request(Path(temporary))
                runner = FakeRunner(
                    [
                        observation(("utmctl", "list"), stdout=vm_list("stopped")),
                        observation(
                            ("utmctl", "status", TARGET_UUID),
                            stdout=b"stopped\n",
                        ),
                        observation(
                            l6_utm_launch_diagnostics.PROCESS_COMMAND,
                            stdout=process_inventory(),
                        ),
                        observation(log_command(), stdout=malformed),
                    ]
                )

                result = l6_utm_launch_diagnostics.run_launch_diagnostics(
                    request,
                    runner=runner,
                    binding_validator=valid_binding,
                )

                self.assertEqual(result.outcome, "diagnostics-incomplete")
                self.assert_manifest_valid(request.output_root)

    def test_authorizations_window_and_absent_output_are_mandatory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for override in (
                {"authorized_host_launch_diagnostics": False},
                {"authorized_read_system_log": False},
                {"log_end": "2026-08-22 10:30:01+0000"},
                {"log_start": "2026-08-22 10:00:00"},
            ):
                request = self.request(root, **override)
                with self.assertRaises(
                    l6_utm_launch_diagnostics.LaunchDiagnosticError
                ):
                    request.validate()

            existing = self.request(root, attempt_id="existing")
            existing.output_root.mkdir()
            with self.assertRaises(
                l6_utm_launch_diagnostics.LaunchDiagnosticError
            ):
                l6_utm_launch_diagnostics.run_launch_diagnostics(
                    existing,
                    runner=FakeRunner([]),
                    binding_validator=valid_binding,
                )

            inside_repository = self.request(
                root,
                attempt_id="inside-repository",
                output_root=root / "repo" / "diagnostics",
            )
            with self.assertRaises(
                l6_utm_launch_diagnostics.LaunchDiagnosticError
            ):
                inside_repository.validate()

    def test_prior_start_manifest_and_terminal_are_semantically_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            start_root, manifest_hash = write_prior_start_evidence(root)
            request = self.request(
                root,
                prior_start_root=start_root,
                prior_start_manifest_sha256=manifest_hash,
            )

            result = l6_utm_launch_diagnostics._validate_prior_start_evidence(
                request
            )
            self.assertEqual(result["prior_start_entries_verified"], 2)
            self.assertEqual(result["prior_start_outcome"], "failed-closed-stopped")

            terminal = start_root / "terminal.json"
            value = json.loads(terminal.read_text(encoding="utf-8"))
            value["guest_exec"] = "performed"
            terminal.write_text(json.dumps(value) + "\n", encoding="utf-8")
            terminal.chmod(0o600)
            manifest_hash = rewrite_prior_start_manifest(start_root)
            tampered_request = self.request(
                root,
                prior_start_root=start_root,
                prior_start_manifest_sha256=manifest_hash,
            )
            with self.assertRaises(
                l6_utm_launch_diagnostics.LaunchDiagnosticError
            ):
                l6_utm_launch_diagnostics._validate_prior_start_evidence(
                    tampered_request
                )

    def test_failure_and_postverify_manifests_are_semantically_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            start_hash = "b" * 64
            failure_root, failure_hash = write_key_value_evidence(
                root / "prior-failure",
                "failure.evidence.txt",
                {
                    "format": "synthetic-failure",
                    "target_uuid": TARGET_UUID,
                    "failure_stage": "start-controller",
                    "controller_invocations": "1",
                    "guest_exec_invocations": "0",
                    "file_pull_invocations": "0",
                    "automatic_stop": "not-performed",
                    "automatic_retry": "not-performed",
                    "automatic_delete": "not-performed",
                    "input_transfer": "not-performed",
                    "operation_id": "not-generated",
                },
            )
            postverify_root, postverify_hash = write_key_value_evidence(
                root / "prior-postverify",
                "postverify.evidence.txt",
                postverify_values(start_hash, failure_hash),
            )
            request = self.request(
                root,
                prior_start_manifest_sha256=start_hash,
                prior_failure_root=failure_root,
                prior_failure_manifest_sha256=failure_hash,
                prior_postverify_root=postverify_root,
                prior_postverify_manifest_sha256=postverify_hash,
            )

            result = l6_utm_launch_diagnostics._validate_related_evidence(
                request
            )

            self.assertEqual(result["prior_failure_entries_verified"], 1)
            self.assertEqual(result["prior_postverify_entries_verified"], 1)

            unrelated = postverify_root / "unrelated.txt"
            unrelated.write_text("synthetic\n", encoding="utf-8")
            unrelated.chmod(0o600)
            unbound_manifest = postverify_root / "files.sha256"
            unbound_manifest.write_text(
                f"{hashlib.sha256(unrelated.read_bytes()).hexdigest()}  "
                "unrelated.txt\n",
                encoding="ascii",
            )
            unbound_manifest.chmod(0o600)
            unbound_request = self.request(
                root,
                prior_start_manifest_sha256=start_hash,
                prior_failure_root=failure_root,
                prior_failure_manifest_sha256=failure_hash,
                prior_postverify_root=postverify_root,
                prior_postverify_manifest_sha256=hashlib.sha256(
                    unbound_manifest.read_bytes()
                ).hexdigest(),
            )
            with self.assertRaises(
                l6_utm_launch_diagnostics.LaunchDiagnosticError
            ):
                l6_utm_launch_diagnostics._validate_related_evidence(
                    unbound_request
                )

            tampered_values = postverify_values(start_hash, failure_hash)
            tampered_values["guest_exec"] = "performed"
            _, tampered_postverify_hash = write_key_value_evidence(
                postverify_root,
                "postverify.evidence.txt",
                tampered_values,
            )
            tampered_request = self.request(
                root,
                prior_start_manifest_sha256=start_hash,
                prior_failure_root=failure_root,
                prior_failure_manifest_sha256=failure_hash,
                prior_postverify_root=postverify_root,
                prior_postverify_manifest_sha256=tampered_postverify_hash,
            )
            with self.assertRaises(
                l6_utm_launch_diagnostics.LaunchDiagnosticError
            ):
                l6_utm_launch_diagnostics._validate_related_evidence(
                    tampered_request
                )

    def test_prior_incomplete_diagnostic_is_semantically_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            initial_root, initial_hash = write_initial_diagnostic_evidence(root)
            diagnostic_root, manifest_hash = write_prior_diagnostic_evidence(
                root, initial_hash
            )
            latest_root, latest_hash = write_latest_diagnostic_evidence(
                root, initial_hash, manifest_hash
            )
            request = self.request(
                root,
                initial_diagnostic_root=initial_root,
                initial_diagnostic_manifest_sha256=initial_hash,
                prior_diagnostic_root=diagnostic_root,
                prior_diagnostic_manifest_sha256=manifest_hash,
                latest_diagnostic_root=latest_root,
                latest_diagnostic_manifest_sha256=latest_hash,
            )

            initial_result = (
                l6_utm_launch_diagnostics._validate_initial_diagnostic_evidence(
                    request
                )
            )
            result = (
                l6_utm_launch_diagnostics._validate_prior_diagnostic_evidence(
                    request
                )
            )
            latest_result = (
                l6_utm_launch_diagnostics._validate_latest_diagnostic_evidence(
                    request
                )
            )

            self.assertEqual(
                initial_result["initial_diagnostic_entries_verified"], 6
            )
            self.assertEqual(result["prior_diagnostic_entries_verified"], 6)
            self.assertEqual(
                result["prior_diagnostic_outcome"], "diagnostics-incomplete"
            )
            self.assertEqual(
                latest_result["latest_diagnostic_entries_verified"], 6
            )

            latest_parent_drift = self.request(
                root,
                initial_diagnostic_root=initial_root,
                initial_diagnostic_manifest_sha256=initial_hash,
                prior_diagnostic_root=diagnostic_root,
                prior_diagnostic_manifest_sha256="9" * 64,
                latest_diagnostic_root=latest_root,
                latest_diagnostic_manifest_sha256=latest_hash,
            )
            with self.assertRaises(
                l6_utm_launch_diagnostics.LaunchDiagnosticError
            ):
                l6_utm_launch_diagnostics._validate_latest_diagnostic_evidence(
                    latest_parent_drift
                )

            prior_request = diagnostic_root / "request.json"
            value = json.loads(prior_request.read_text(encoding="utf-8"))
            value["log_start"] = "2026-08-22 09:59:59+0000"
            prior_request.write_text(json.dumps(value) + "\n", encoding="utf-8")
            prior_request.chmod(0o600)
            drifted_manifest_hash = rewrite_json_manifest(diagnostic_root)
            drifted_request = self.request(
                root,
                initial_diagnostic_root=initial_root,
                initial_diagnostic_manifest_sha256=initial_hash,
                prior_diagnostic_root=diagnostic_root,
                prior_diagnostic_manifest_sha256=drifted_manifest_hash,
            )
            with self.assertRaises(
                l6_utm_launch_diagnostics.LaunchDiagnosticError
            ):
                l6_utm_launch_diagnostics._validate_prior_diagnostic_evidence(
                    drifted_request
                )

            value["log_start"] = LOG_START
            prior_request.write_text(json.dumps(value) + "\n", encoding="utf-8")
            prior_request.chmod(0o600)
            rewrite_json_manifest(diagnostic_root)

            terminal = diagnostic_root / "terminal.json"
            value = json.loads(terminal.read_text(encoding="utf-8"))
            value["unified_log_observation"] = "performed"
            terminal.write_text(json.dumps(value) + "\n", encoding="utf-8")
            terminal.chmod(0o600)
            manifest_hash = rewrite_json_manifest(diagnostic_root)
            tampered_request = self.request(
                root,
                initial_diagnostic_root=initial_root,
                initial_diagnostic_manifest_sha256=initial_hash,
                prior_diagnostic_root=diagnostic_root,
                prior_diagnostic_manifest_sha256=manifest_hash,
            )
            with self.assertRaises(
                l6_utm_launch_diagnostics.LaunchDiagnosticError
            ):
                l6_utm_launch_diagnostics._validate_prior_diagnostic_evidence(
                    tampered_request
                )

    def test_process_inventory_accepts_macos_system_ids_and_rejects_invalid_ids(
        self,
    ) -> None:
        parsed = l6_utm_launch_diagnostics.parse_relevant_processes(
            observation(
                l6_utm_launch_diagnostics.PROCESS_COMMAND,
                stdout=process_inventory(),
            )
        )
        self.assertEqual(len(parsed), 3)

        for index, payload in enumerate(
            (
                b"-1 0 0 kernel_task\n",
                b"1 -1 0 launchd\n",
                b"1 0 -1 launchd\n",
                b"1 0 -02 launchd\n",
                b"1 0 -3 launchd\n",
                b"1 0 0 launchd\n1 0 0 duplicate\n",
                b"0 1 0 kernel_task\n",
                b"0 0 501 kernel_task\n",
                b"0 0 0 UTM\n",
            )
        ):
            with self.subTest(index=index), self.assertRaises(
                l6_utm_launch_diagnostics.LaunchDiagnosticError
            ):
                l6_utm_launch_diagnostics.parse_relevant_processes(
                    observation(
                        l6_utm_launch_diagnostics.PROCESS_COMMAND,
                        stdout=payload,
                    )
                )

    def test_executed_control_is_bound_to_repository_copy(self) -> None:
        repository_root = Path(__file__).resolve().parents[2]
        digest = l6_utm_launch_diagnostics._validate_control_identity(
            repository_root
        )
        self.assertRegex(digest, r"^[0-9a-f]{64}$")
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaises(
                l6_utm_launch_diagnostics.LaunchDiagnosticError
            ):
                l6_utm_launch_diagnostics._validate_control_identity(
                    Path(temporary)
                )

    def request(
        self, root: Path, **overrides: object
    ) -> l6_utm_launch_diagnostics.LaunchDiagnosticRequest:
        values: dict[str, object] = {
            "repository_root": root / "repo",
            "expected_repository_head": "a" * 40,
            "prior_start_root": root / "start-evidence",
            "prior_start_manifest_sha256": "b" * 64,
            "prior_failure_root": root / "failure-evidence",
            "prior_failure_manifest_sha256": "d" * 64,
            "prior_postverify_root": root / "postverify-evidence",
            "prior_postverify_manifest_sha256": "e" * 64,
            "initial_diagnostic_root": root / "initial-diagnostic-evidence",
            "initial_diagnostic_manifest_sha256": "f" * 64,
            "prior_diagnostic_root": root / "diagnostic-evidence",
            "prior_diagnostic_manifest_sha256": "1" * 64,
            "latest_diagnostic_root": root / "latest-diagnostic-evidence",
            "latest_diagnostic_manifest_sha256": "2" * 64,
            "prior_log_diagnostic_root": root / "prior-log-diagnostic-evidence",
            "prior_log_diagnostic_manifest_sha256": "3" * 64,
            "prior_schema_diagnostic_root": (
                root / "prior-schema-diagnostic-evidence"
            ),
            "prior_schema_diagnostic_manifest_sha256": "4" * 64,
            "output_root": root / "launch-diagnostics",
            "attempt_id": "synthetic-diagnostics",
            "target_uuid": TARGET_UUID,
            "target_name": TARGET_NAME,
            "expected_vm_count": 2,
            "log_start": LOG_START,
            "log_end": LOG_END,
            "command_timeout_seconds": 15,
            "log_timeout_seconds": 30,
            "authorized_host_launch_diagnostics": True,
            "authorized_read_system_log": True,
        }
        values.update(overrides)
        return l6_utm_launch_diagnostics.LaunchDiagnosticRequest(  # type: ignore[arg-type]
            **values
        )

    def assert_manifest_valid(self, root: Path) -> None:
        lines = (root / "files.sha256").read_text(encoding="ascii").splitlines()
        self.assertGreaterEqual(len(lines), 1)
        for line in lines:
            expected_hash, name = line.split("  ", maxsplit=1)
            actual_hash = hashlib.sha256((root / name).read_bytes()).hexdigest()
            self.assertEqual(actual_hash, expected_hash)


def valid_binding(
    request: l6_utm_launch_diagnostics.LaunchDiagnosticRequest,
) -> dict[str, object]:
    return {
        "binding_control_sha256": "a" * 64,
        "control_sha256": "c" * 64,
        "format": l6_utm_launch_diagnostics.EVIDENCE_FORMAT,
        "prior_start_entries_verified": 65,
        "prior_start_manifest_sha256": request.prior_start_manifest_sha256,
        "prior_start_outcome": "failed-closed-stopped",
        "prior_failure_entries_verified": 12,
        "prior_failure_manifest_sha256": (
            request.prior_failure_manifest_sha256
        ),
        "prior_postverify_entries_verified": 9,
        "prior_postverify_manifest_sha256": (
            request.prior_postverify_manifest_sha256
        ),
        "initial_diagnostic_entries_verified": 6,
        "initial_diagnostic_manifest_sha256": (
            request.initial_diagnostic_manifest_sha256
        ),
        "initial_diagnostic_outcome": "diagnostics-incomplete",
        "prior_diagnostic_entries_verified": 6,
        "prior_diagnostic_manifest_sha256": (
            request.prior_diagnostic_manifest_sha256
        ),
        "prior_diagnostic_outcome": "diagnostics-incomplete",
        "latest_diagnostic_entries_verified": 6,
        "latest_diagnostic_manifest_sha256": (
            request.latest_diagnostic_manifest_sha256
        ),
        "latest_diagnostic_outcome": "diagnostics-incomplete",
        "prior_log_diagnostic_entries_verified": 8,
        "prior_log_diagnostic_manifest_sha256": (
            request.prior_log_diagnostic_manifest_sha256
        ),
        "prior_log_diagnostic_outcome": "diagnostics-incomplete",
        "prior_schema_diagnostic_entries_verified": 8,
        "prior_schema_diagnostic_manifest_sha256": (
            request.prior_schema_diagnostic_manifest_sha256
        ),
        "prior_schema_diagnostic_outcome": "diagnostics-incomplete",
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
) -> l6_utm_launch_diagnostics.CommandObservation:
    return l6_utm_launch_diagnostics.CommandObservation.from_bytes(
        argv,
        exit_code=exit_code,
        timed_out=timed_out,
        stdout=stdout,
        stderr=stderr,
    )


def vm_list(target_status: str) -> bytes:
    return (
        "UUID Status Name\n"
        f"{TARGET_UUID} {target_status} {TARGET_NAME}\n"
        f"{OTHER_UUID} stopped Synthetic-Peer\n"
    ).encode("utf-8")


def process_inventory() -> bytes:
    return (
        "0 0 0 kernel_task\n"
        "101 1 501 UTM\n"
        "102 101 501 qemu-aarch64-so\n"
        "103 1 501 utmctl\n"
        "104 1 501 Finder\n"
        "105 1 -2 nobody-helper\n"
    ).encode("utf-8")


def log_command() -> tuple[str, ...]:
    return (
        "/usr/bin/log",
        "show",
        "--style",
        "ndjson",
        "--no-pager",
        "--timezone",
        "UTC",
        "--info",
        "--no-debug",
        "--no-signpost",
        "--no-loss",
        "--start",
        LOG_START,
        "--end",
        LOG_END,
        "--predicate",
        l6_utm_launch_diagnostics.LOG_PREDICATE,
    )


def unified_log_events() -> bytes:
    values = [
        {
            "timestamp": "2026-08-22 10:00:01.000000+0000",
            "processImagePath": "/Applications/UTM.app/Contents/MacOS/UTM",
            "subsystem": "com.utmapp.UTM",
            "category": "VirtualMachine",
            "messageType": "Default",
            "eventMessage": "opening /Users/luobo/VirtualMachines/Synthetic.utm",
        },
        {
            "timestamp": "2026-08-22 10:00:02.000000+0000",
            "processImagePath": "/opt/homebrew/bin/utmctl",
            "subsystem": "com.apple.appleevents",
            "category": "AppleEvents",
            "messageType": "Error",
            "eventMessage": "event timeout for /Users/luobo/Synthetic",
        },
    ]
    return b"".join(
        (json.dumps(value, separators=(",", ":")) + "\n").encode("utf-8")
        for value in (*values, {"finished": True})
    )


def write_prior_start_evidence(root: Path) -> tuple[Path, str]:
    evidence = root / "prior-start"
    evidence.mkdir(mode=0o700)
    request = evidence / "request.json"
    request.write_text(
        json.dumps(
            {
                "attempt_id": "synthetic-start",
                "clone_name": TARGET_NAME,
                "clone_uuid": TARGET_UUID,
                "format": "radishlex-linux-l6-utm-start-once-v1",
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    terminal = evidence / "terminal.json"
    terminal.write_text(
        json.dumps(
            {
                "automatic_retry": "not-performed",
                "automatic_stop": "not-performed",
                "clone_name": TARGET_NAME,
                "clone_uuid": TARGET_UUID,
                "format": "radishlex-linux-l6-utm-start-once-v1",
                "guest_exec": "not-performed",
                "input_transfer": "not-performed",
                "operation_id": "not-generated",
                "outcome": "failed-closed-stopped",
                "reason": "started-not-observed-and-all-vms-stopped",
                "start_command_exit_code": None,
                "start_command_timed_out": True,
                "start_invocations": 1,
                "status_poll_count": 60,
                "transaction": "not-performed",
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    for path in (request, terminal):
        path.chmod(0o600)
    return evidence, rewrite_prior_start_manifest(evidence)


def rewrite_prior_start_manifest(evidence: Path) -> str:
    manifest = evidence / "files.sha256"
    payloads = (evidence / "request.json", evidence / "terminal.json")
    manifest.write_text(
        "".join(
            f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n"
            for path in payloads
        ),
        encoding="ascii",
    )
    manifest.chmod(0o600)
    return hashlib.sha256(manifest.read_bytes()).hexdigest()


def write_initial_diagnostic_evidence(root: Path) -> tuple[Path, str]:
    return write_diagnostic_evidence(
        root / "initial-diagnostic",
        evidence_format=l6_utm_launch_diagnostics.INITIAL_EVIDENCE_FORMAT,
        process_command=l6_utm_launch_diagnostics.INITIAL_PROCESS_COMMAND,
        process_stdout_size=95_549,
        process_stdout_truncated=True,
        reason=(
            "host-process-observation:"
            "host-process-observation-output-truncated"
        ),
    )


def write_prior_diagnostic_evidence(
    root: Path, initial_manifest_sha256: str
) -> tuple[Path, str]:
    return write_diagnostic_evidence(
        root / "prior-diagnostic",
        evidence_format=l6_utm_launch_diagnostics.PRIOR_EVIDENCE_FORMAT,
        process_command=l6_utm_launch_diagnostics.PRIOR_PROCESS_COMMAND,
        process_stdout_size=31_570,
        process_stdout_truncated=False,
        reason="host-process-observation:host-process-identifier-invalid",
        request_diagnostic_fields={
            "prior_diagnostic_manifest_sha256": initial_manifest_sha256,
        },
        binding_diagnostic_fields={
            "binding_control_sha256": "a" * 64,
            "prior_diagnostic_entries_verified": 6,
            "prior_diagnostic_manifest_sha256": initial_manifest_sha256,
            "prior_diagnostic_outcome": "diagnostics-incomplete",
            "prior_diagnostic_reason": (
                "host-process-observation:"
                "host-process-observation-output-truncated"
            ),
        },
    )


def write_latest_diagnostic_evidence(
    root: Path,
    initial_manifest_sha256: str,
    prior_manifest_sha256: str,
) -> tuple[Path, str]:
    return write_diagnostic_evidence(
        root / "latest-diagnostic",
        evidence_format=l6_utm_launch_diagnostics.LATEST_EVIDENCE_FORMAT,
        process_command=l6_utm_launch_diagnostics.LATEST_PROCESS_COMMAND,
        process_stdout_size=31_080,
        process_stdout_truncated=False,
        reason="host-process-observation:host-process-identifier-invalid",
        request_diagnostic_fields={
            "initial_diagnostic_manifest_sha256": initial_manifest_sha256,
            "prior_diagnostic_manifest_sha256": prior_manifest_sha256,
        },
        binding_diagnostic_fields={
            "binding_control_sha256": "a" * 64,
            "initial_diagnostic_entries_verified": 6,
            "initial_diagnostic_manifest_sha256": initial_manifest_sha256,
            "initial_diagnostic_outcome": "diagnostics-incomplete",
            "initial_diagnostic_reason": (
                "host-process-observation:"
                "host-process-observation-output-truncated"
            ),
            "prior_diagnostic_entries_verified": 6,
            "prior_diagnostic_manifest_sha256": prior_manifest_sha256,
            "prior_diagnostic_outcome": "diagnostics-incomplete",
            "prior_diagnostic_reason": (
                "host-process-observation:host-process-identifier-invalid"
            ),
        },
    )


def write_diagnostic_evidence(
    evidence: Path,
    *,
    evidence_format: str,
    process_command: tuple[str, ...],
    process_stdout_size: int,
    process_stdout_truncated: bool,
    reason: str,
    request_diagnostic_fields: dict[str, object] | None = None,
    binding_diagnostic_fields: dict[str, object] | None = None,
) -> tuple[Path, str]:
    evidence.mkdir(mode=0o700)
    values: dict[str, dict[str, object]] = {
        "request.json": {
            "authorization": {
                "host_launch_diagnostics": True,
                "read_system_log": True,
            },
            "expected_repository_head": "a" * 40,
            "expected_vm_count": 2,
            "format": evidence_format,
            "log_end": LOG_END,
            "log_start": LOG_START,
            "prior_failure_manifest_sha256": "d" * 64,
            "prior_postverify_manifest_sha256": "e" * 64,
            "prior_start_manifest_sha256": "b" * 64,
            "target_name": TARGET_NAME,
            "target_uuid": TARGET_UUID,
        },
        "binding-preflight.json": {
            "control_sha256": "c" * 64,
            "format": evidence_format,
            "prior_failure_manifest_sha256": "d" * 64,
            "prior_postverify_manifest_sha256": "e" * 64,
            "prior_start_manifest_sha256": "b" * 64,
            "repository_clean": True,
            "repository_head": "a" * 40,
        },
        "utmctl-list-live.json": {"synthetic": True},
        "utmctl-status-live.json": {"synthetic": True},
        "host-process-command.json": {
            "argv": list(process_command),
            "exit_code": 0,
            "stderr": {
                "prefix_utf8": "",
                "sha256": hashlib.sha256(b"").hexdigest(),
                "total_bytes": 0,
                "truncated": False,
            },
            "stdout": {
                "sha256": "1" * 64,
                "total_bytes": process_stdout_size,
                "truncated": process_stdout_truncated,
            },
            "timed_out": False,
        },
        "terminal.json": {
            "automatic_delete": "not-performed",
            "automatic_retry": "not-performed",
            "format": evidence_format,
            "guest_exec": "not-performed",
            "host_process_observation": "attempted",
            "input_transfer": "not-performed",
            "operation_id": "not-generated",
            "outcome": "diagnostics-incomplete",
            "reason": reason,
            "root_cause": "unattributed",
            "target_name": TARGET_NAME,
            "target_uuid": TARGET_UUID,
            "transaction": "not-performed",
            "unified_log_observation": "not-performed",
            "utm_clone": "not-performed",
            "utm_start": "not-performed",
            "utm_stop": "not-performed",
        },
    }
    if request_diagnostic_fields is not None:
        values["request.json"].update(request_diagnostic_fields)
    if binding_diagnostic_fields is not None:
        values["binding-preflight.json"].update(binding_diagnostic_fields)
    for name, value in values.items():
        path = evidence / name
        path.write_text(json.dumps(value) + "\n", encoding="utf-8")
        path.chmod(0o600)
    return evidence, rewrite_json_manifest(evidence)


def rewrite_json_manifest(evidence: Path) -> str:
    manifest = evidence / "files.sha256"
    payloads = sorted(evidence.glob("*.json"))
    manifest.write_text(
        "".join(
            f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n"
            for path in payloads
        ),
        encoding="ascii",
    )
    manifest.chmod(0o600)
    return hashlib.sha256(manifest.read_bytes()).hexdigest()


def postverify_values(
    start_manifest_sha256: str, failure_manifest_sha256: str
) -> dict[str, str]:
    return {
        "format": "synthetic-postverify",
        "start_manifest_sha256": start_manifest_sha256,
        "failure_manifest_sha256": failure_manifest_sha256,
        "target_uuid": TARGET_UUID,
        "registered_vms": "all-stopped",
        "target_vm": "stopped",
        "terminal": "failed-closed-stopped",
        "guest_exec": "not-performed",
        "file_pull_invocations": "0",
        "input_transfer": "not-performed",
        "operation_id": "not-generated",
        "transaction": "not-performed",
        "postverify": "failed-closed-preserved",
    }


def write_key_value_evidence(
    root: Path, name: str, values: dict[str, str]
) -> tuple[Path, str]:
    if not root.exists():
        root.mkdir(mode=0o700)
    payload = root / name
    payload.write_text(
        "".join(f"{key}={value}\n" for key, value in values.items()),
        encoding="utf-8",
    )
    payload.chmod(0o600)
    manifest = root / "files.sha256"
    payload_hash = hashlib.sha256(payload.read_bytes()).hexdigest()
    manifest.write_text(f"{payload_hash}  {name}\n", encoding="ascii")
    manifest.chmod(0o600)
    return root, hashlib.sha256(manifest.read_bytes()).hexdigest()


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()

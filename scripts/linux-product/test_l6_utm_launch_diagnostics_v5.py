#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

import l6_utm_launch_diagnostics
import test_l6_utm_launch_diagnostics as existing_tests


class LinuxL6UtmLaunchDiagnosticsV5Tests(unittest.TestCase):
    def test_capture_above_legacy_limit_remains_bounded_and_parseable(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = existing_tests.LinuxL6UtmLaunchDiagnosticsTests().request(
                Path(temporary)
            )
            log_payload = large_unified_log_events()
            self.assertGreater(
                len(log_payload),
                l6_utm_launch_diagnostics.start_control.MAX_CAPTURE_BYTES,
            )
            self.assertLess(
                len(log_payload),
                l6_utm_launch_diagnostics.MAX_DIAGNOSTIC_CAPTURE_BYTES,
            )
            log_observation = l6_utm_launch_diagnostics.CommandObservation(
                argv=existing_tests.log_command(),
                exit_code=0,
                timed_out=False,
                stdout=(
                    l6_utm_launch_diagnostics.start_control.CapturedOutput.from_bytes(
                        log_payload,
                        max_capture_bytes=(
                            l6_utm_launch_diagnostics.MAX_DIAGNOSTIC_CAPTURE_BYTES
                        ),
                    )
                ),
                stderr=(
                    l6_utm_launch_diagnostics.start_control.CapturedOutput.from_bytes(
                        b"",
                        max_capture_bytes=(
                            l6_utm_launch_diagnostics.MAX_DIAGNOSTIC_CAPTURE_BYTES
                        ),
                    )
                ),
            )
            runner = existing_tests.FakeRunner(
                [
                    existing_tests.observation(
                        ("utmctl", "list"),
                        stdout=existing_tests.vm_list("stopped"),
                    ),
                    existing_tests.observation(
                        ("utmctl", "status", existing_tests.TARGET_UUID),
                        stdout=b"stopped\n",
                    ),
                    existing_tests.observation(
                        l6_utm_launch_diagnostics.PROCESS_COMMAND,
                        stdout=existing_tests.process_inventory(),
                    ),
                    log_observation,
                ]
            )

            result = l6_utm_launch_diagnostics.run_launch_diagnostics(
                request,
                runner=runner,
                binding_validator=existing_tests.valid_binding,
            )

            self.assertEqual(result.outcome, "diagnostics-collected")
            command = read_json(
                request.output_root / "unified-log-command.json"
            )
            self.assertFalse(command["stdout"]["truncated"])
            events = read_json(request.output_root / "unified-log.json")
            self.assertEqual(events["event_count"], 96)

    def test_v4_log_truncation_evidence_is_semantically_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            initial_root, initial_hash = (
                existing_tests.write_initial_diagnostic_evidence(root)
            )
            prior_root, prior_hash = (
                existing_tests.write_prior_diagnostic_evidence(
                    root, initial_hash
                )
            )
            latest_root, latest_hash = (
                existing_tests.write_latest_diagnostic_evidence(
                    root, initial_hash, prior_hash
                )
            )
            prior_log_root, prior_log_hash = write_prior_log_evidence(
                root, initial_hash, prior_hash, latest_hash
            )
            request = existing_tests.LinuxL6UtmLaunchDiagnosticsTests().request(
                root,
                initial_diagnostic_root=initial_root,
                initial_diagnostic_manifest_sha256=initial_hash,
                prior_diagnostic_root=prior_root,
                prior_diagnostic_manifest_sha256=prior_hash,
                latest_diagnostic_root=latest_root,
                latest_diagnostic_manifest_sha256=latest_hash,
                prior_log_diagnostic_root=prior_log_root,
                prior_log_diagnostic_manifest_sha256=prior_log_hash,
            )

            result = (
                l6_utm_launch_diagnostics._validate_prior_log_diagnostic_evidence(
                    request
                )
            )

            self.assertEqual(result["prior_log_diagnostic_entries_verified"], 8)
            self.assertEqual(
                result["prior_log_diagnostic_reason"],
                "unified-log-observation:"
                "unified-log-observation-output-truncated",
            )

            host_processes = prior_log_root / "host-processes.json"
            value = read_json(host_processes)
            value["relevant_process_count"] = 1
            write_json(host_processes, value)
            drifted_hash = existing_tests.rewrite_json_manifest(prior_log_root)
            drifted_request = (
                existing_tests.LinuxL6UtmLaunchDiagnosticsTests().request(
                    root,
                    initial_diagnostic_root=initial_root,
                    initial_diagnostic_manifest_sha256=initial_hash,
                    prior_diagnostic_root=prior_root,
                    prior_diagnostic_manifest_sha256=prior_hash,
                    latest_diagnostic_root=latest_root,
                    latest_diagnostic_manifest_sha256=latest_hash,
                    prior_log_diagnostic_root=prior_log_root,
                    prior_log_diagnostic_manifest_sha256=drifted_hash,
                )
            )
            with self.assertRaises(
                l6_utm_launch_diagnostics.LaunchDiagnosticError
            ):
                l6_utm_launch_diagnostics._validate_prior_log_diagnostic_evidence(
                    drifted_request
                )


def large_unified_log_events() -> bytes:
    values = []
    for index in range(96):
        values.append(
            {
                "timestamp": f"2026-08-22 10:00:{index % 60:02d}.000000+0000",
                "processImagePath": (
                    "/Applications/UTM.app/Contents/MacOS/UTM"
                ),
                "subsystem": "com.utmapp.UTM",
                "category": "VirtualMachine",
                "messageType": "Default",
                "eventMessage": (
                    f"synthetic event {index} /Users/luobo/VirtualMachines/"
                    + "x" * 900
                ),
            }
        )
    return b"".join(
        (json.dumps(value, separators=(",", ":")) + "\n").encode("utf-8")
        for value in (*values, {"finished": True})
    )


def write_prior_log_evidence(
    root: Path,
    initial_hash: str,
    prior_hash: str,
    latest_hash: str,
) -> tuple[Path, str]:
    evidence = root / "prior-log-diagnostic"
    evidence.mkdir(mode=0o700)
    values: dict[str, dict[str, object]] = {
        "request.json": {
            "authorization": {
                "host_launch_diagnostics": True,
                "read_system_log": True,
            },
            "expected_repository_head": "a" * 40,
            "expected_vm_count": 2,
            "format": l6_utm_launch_diagnostics.PRIOR_LOG_EVIDENCE_FORMAT,
            "initial_diagnostic_manifest_sha256": initial_hash,
            "latest_diagnostic_manifest_sha256": latest_hash,
            "log_end": existing_tests.LOG_END,
            "log_start": existing_tests.LOG_START,
            "prior_diagnostic_manifest_sha256": prior_hash,
            "prior_failure_manifest_sha256": "d" * 64,
            "prior_postverify_manifest_sha256": "e" * 64,
            "prior_start_manifest_sha256": "b" * 64,
            "target_name": existing_tests.TARGET_NAME,
            "target_uuid": existing_tests.TARGET_UUID,
        },
        "binding-preflight.json": {
            "binding_control_sha256": "a" * 64,
            "control_sha256": "c" * 64,
            "format": l6_utm_launch_diagnostics.PRIOR_LOG_EVIDENCE_FORMAT,
            "initial_diagnostic_entries_verified": 6,
            "initial_diagnostic_manifest_sha256": initial_hash,
            "initial_diagnostic_outcome": "diagnostics-incomplete",
            "latest_diagnostic_entries_verified": 6,
            "latest_diagnostic_manifest_sha256": latest_hash,
            "latest_diagnostic_outcome": "diagnostics-incomplete",
            "prior_diagnostic_entries_verified": 6,
            "prior_diagnostic_manifest_sha256": prior_hash,
            "prior_diagnostic_outcome": "diagnostics-incomplete",
            "prior_failure_manifest_sha256": "d" * 64,
            "prior_postverify_manifest_sha256": "e" * 64,
            "prior_start_manifest_sha256": "b" * 64,
            "repository_clean": True,
            "repository_head": "a" * 40,
        },
        "utmctl-list-live.json": {"synthetic": True},
        "utmctl-status-live.json": {"synthetic": True},
        "host-process-command.json": command_metadata(
            l6_utm_launch_diagnostics.PROCESS_COMMAND,
            total_bytes=31_605,
            truncated=False,
        ),
        "host-processes.json": {
            "format": l6_utm_launch_diagnostics.PRIOR_LOG_EVIDENCE_FORMAT,
            "relevant_process_count": 0,
            "relevant_processes": [],
        },
        "unified-log-command.json": command_metadata(
            existing_tests.log_command(),
            total_bytes=3_916_860,
            truncated=True,
        ),
        "terminal.json": {
            "automatic_delete": "not-performed",
            "automatic_retry": "not-performed",
            "format": l6_utm_launch_diagnostics.PRIOR_LOG_EVIDENCE_FORMAT,
            "guest_exec": "not-performed",
            "host_process_observation": "performed",
            "input_transfer": "not-performed",
            "operation_id": "not-generated",
            "outcome": "diagnostics-incomplete",
            "reason": (
                "unified-log-observation:"
                "unified-log-observation-output-truncated"
            ),
            "root_cause": "unattributed",
            "target_name": existing_tests.TARGET_NAME,
            "target_uuid": existing_tests.TARGET_UUID,
            "transaction": "not-performed",
            "unified_log_observation": "attempted",
            "utm_clone": "not-performed",
            "utm_start": "not-performed",
            "utm_stop": "not-performed",
        },
    }
    for name, value in values.items():
        write_json(evidence / name, value)
    return evidence, existing_tests.rewrite_json_manifest(evidence)


def command_metadata(
    argv: tuple[str, ...], *, total_bytes: int, truncated: bool
) -> dict[str, object]:
    return {
        "argv": list(argv),
        "exit_code": 0,
        "stderr": {
            "prefix_utf8": "",
            "sha256": hashlib.sha256(b"").hexdigest(),
            "total_bytes": 0,
            "truncated": False,
        },
        "stdout": {
            "sha256": "4" * 64,
            "total_bytes": total_bytes,
            "truncated": truncated,
        },
        "timed_out": False,
    }


def write_json(path: Path, value: dict[str, object]) -> None:
    path.write_text(json.dumps(value) + "\n", encoding="utf-8")
    path.chmod(0o600)


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise AssertionError(f"expected object: {path}")
    return value


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import l6_utm_launch_diagnostics
import test_l6_utm_launch_diagnostics as base_tests
import test_l6_utm_launch_diagnostics_v5 as v5_tests
import test_l6_utm_launch_diagnostics_v6 as v6_tests


class LinuxL6UtmLaunchDiagnosticsV7Tests(unittest.TestCase):
    def test_numeric_finished_marker_is_supported_and_summarized(self) -> None:
        numeric = observation(log_payload(1))
        parsed = l6_utm_launch_diagnostics.parse_unified_log_events(numeric)

        self.assertEqual(len(parsed), 1)
        self.assertEqual(
            l6_utm_launch_diagnostics.summarize_unified_log_structure(numeric),
            {
                "finished_marker_count": 1,
                "finished_marker_is_terminal": True,
                "finished_value_kind": "integer-one",
                "format": l6_utm_launch_diagnostics.EVIDENCE_FORMAT,
                "record_count": 2,
            },
        )
        boolean = l6_utm_launch_diagnostics.summarize_unified_log_structure(
            observation(log_payload(True))
        )
        self.assertEqual(boolean["finished_value_kind"], "boolean-true")

        for value, kind in (
            (False, "boolean-false"),
            (0, "integer-other"),
            (2, "integer-other"),
            ("true", "string"),
            (None, "null"),
            ([], "array"),
        ):
            with self.subTest(value=value):
                invalid = observation(log_payload(value))
                self.assertEqual(
                    l6_utm_launch_diagnostics.summarize_unified_log_structure(
                        invalid
                    )["finished_value_kind"],
                    kind,
                )
                with self.assertRaises(
                    l6_utm_launch_diagnostics.LaunchDiagnosticError
                ):
                    l6_utm_launch_diagnostics.parse_unified_log_events(invalid)

    def test_structure_is_persisted_before_marker_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = base_tests.LinuxL6UtmLaunchDiagnosticsTests().request(
                Path(temporary)
            )
            runner = base_tests.FakeRunner(
                [
                    base_tests.observation(
                        ("utmctl", "list"),
                        stdout=base_tests.vm_list("stopped"),
                    ),
                    base_tests.observation(
                        ("utmctl", "status", base_tests.TARGET_UUID),
                        stdout=b"stopped\n",
                    ),
                    base_tests.observation(
                        l6_utm_launch_diagnostics.PROCESS_COMMAND,
                        stdout=base_tests.process_inventory(),
                    ),
                    base_tests.observation(
                        base_tests.log_command(),
                        stdout=log_payload(False),
                    ),
                ]
            )

            result = l6_utm_launch_diagnostics.run_launch_diagnostics(
                request,
                runner=runner,
                binding_validator=base_tests.valid_binding,
            )

            self.assertEqual(result.outcome, "diagnostics-incomplete")
            structure = read_json(
                request.output_root / "unified-log-structure.json"
            )
            self.assertEqual(structure["finished_value_kind"], "boolean-false")
            self.assertTrue(structure["finished_marker_is_terminal"])
            self.assertFalse((request.output_root / "unified-log.json").exists())
            base_tests.LinuxL6UtmLaunchDiagnosticsTests().assert_manifest_valid(
                request.output_root
            )

    def test_v6_finished_failure_evidence_is_semantically_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            initial_root, initial_hash = (
                base_tests.write_initial_diagnostic_evidence(root)
            )
            prior_root, prior_hash = base_tests.write_prior_diagnostic_evidence(
                root, initial_hash
            )
            latest_root, latest_hash = (
                base_tests.write_latest_diagnostic_evidence(
                    root, initial_hash, prior_hash
                )
            )
            prior_log_root, prior_log_hash = v5_tests.write_prior_log_evidence(
                root, initial_hash, prior_hash, latest_hash
            )
            schema_root, schema_hash = v6_tests.write_prior_schema_evidence(
                root, initial_hash, prior_hash, latest_hash, prior_log_hash
            )
            finished_root, finished_hash = write_prior_finished_evidence(
                root,
                initial_hash,
                prior_hash,
                latest_hash,
                prior_log_hash,
                schema_hash,
            )
            request = base_tests.LinuxL6UtmLaunchDiagnosticsTests().request(
                root,
                initial_diagnostic_root=initial_root,
                initial_diagnostic_manifest_sha256=initial_hash,
                prior_diagnostic_root=prior_root,
                prior_diagnostic_manifest_sha256=prior_hash,
                latest_diagnostic_root=latest_root,
                latest_diagnostic_manifest_sha256=latest_hash,
                prior_log_diagnostic_root=prior_log_root,
                prior_log_diagnostic_manifest_sha256=prior_log_hash,
                prior_schema_diagnostic_root=schema_root,
                prior_schema_diagnostic_manifest_sha256=schema_hash,
                prior_finished_diagnostic_root=finished_root,
                prior_finished_diagnostic_manifest_sha256=finished_hash,
            )

            result = l6_utm_launch_diagnostics._validate_prior_finished_diagnostic_evidence(
                request
            )

            self.assertEqual(
                result["prior_finished_diagnostic_entries_verified"], 8
            )
            self.assertEqual(
                result["prior_finished_diagnostic_reason"],
                "unified-log-observation:unified-log-finished-invalid",
            )

            terminal = finished_root / "terminal.json"
            value = read_json(terminal)
            value["unified_log_observation"] = "performed"
            v6_tests.write_json(terminal, value)
            drifted_hash = base_tests.rewrite_json_manifest(finished_root)
            drifted_request = (
                base_tests.LinuxL6UtmLaunchDiagnosticsTests().request(
                    root,
                    initial_diagnostic_root=initial_root,
                    initial_diagnostic_manifest_sha256=initial_hash,
                    prior_diagnostic_root=prior_root,
                    prior_diagnostic_manifest_sha256=prior_hash,
                    latest_diagnostic_root=latest_root,
                    latest_diagnostic_manifest_sha256=latest_hash,
                    prior_log_diagnostic_root=prior_log_root,
                    prior_log_diagnostic_manifest_sha256=prior_log_hash,
                    prior_schema_diagnostic_root=schema_root,
                    prior_schema_diagnostic_manifest_sha256=schema_hash,
                    prior_finished_diagnostic_root=finished_root,
                    prior_finished_diagnostic_manifest_sha256=drifted_hash,
                )
            )
            with self.assertRaises(
                l6_utm_launch_diagnostics.LaunchDiagnosticError
            ):
                l6_utm_launch_diagnostics._validate_prior_finished_diagnostic_evidence(
                    drifted_request
                )


def log_payload(finished: object) -> bytes:
    event = {
        "timestamp": "2026-08-22 10:00:01.000000+0000",
        "processImagePath": "/Applications/UTM.app/Contents/MacOS/UTM",
        "subsystem": "",
        "category": "",
        "messageType": "",
        "eventMessage": "synthetic event",
    }
    return b"".join(
        (json.dumps(value, separators=(",", ":")) + "\n").encode("utf-8")
        for value in (event, {"finished": finished})
    )


def observation(
    payload: bytes,
) -> l6_utm_launch_diagnostics.CommandObservation:
    return l6_utm_launch_diagnostics.CommandObservation.from_bytes(
        base_tests.log_command(), stdout=payload
    )


def write_prior_finished_evidence(
    root: Path,
    initial_hash: str,
    prior_hash: str,
    latest_hash: str,
    prior_log_hash: str,
    schema_hash: str,
) -> tuple[Path, str]:
    evidence = root / "prior-finished-diagnostic"
    evidence.mkdir(mode=0o700)
    values: dict[str, dict[str, object]] = {
        "request.json": {
            "authorization": {
                "host_launch_diagnostics": True,
                "read_system_log": True,
            },
            "expected_repository_head": "a" * 40,
            "expected_vm_count": 2,
            "format": (
                l6_utm_launch_diagnostics.PRIOR_FINISHED_EVIDENCE_FORMAT
            ),
            "initial_diagnostic_manifest_sha256": initial_hash,
            "latest_diagnostic_manifest_sha256": latest_hash,
            "log_end": base_tests.LOG_END,
            "log_start": base_tests.LOG_START,
            "prior_diagnostic_manifest_sha256": prior_hash,
            "prior_failure_manifest_sha256": "d" * 64,
            "prior_log_diagnostic_manifest_sha256": prior_log_hash,
            "prior_postverify_manifest_sha256": "e" * 64,
            "prior_schema_diagnostic_manifest_sha256": schema_hash,
            "prior_start_manifest_sha256": "b" * 64,
            "target_name": base_tests.TARGET_NAME,
            "target_uuid": base_tests.TARGET_UUID,
        },
        "binding-preflight.json": {
            "binding_control_sha256": "a" * 64,
            "control_sha256": "c" * 64,
            "format": (
                l6_utm_launch_diagnostics.PRIOR_FINISHED_EVIDENCE_FORMAT
            ),
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
            "prior_log_diagnostic_entries_verified": 8,
            "prior_log_diagnostic_manifest_sha256": prior_log_hash,
            "prior_log_diagnostic_outcome": "diagnostics-incomplete",
            "prior_log_diagnostic_reason": (
                "unified-log-observation:"
                "unified-log-observation-output-truncated"
            ),
            "prior_postverify_manifest_sha256": "e" * 64,
            "prior_schema_diagnostic_entries_verified": 8,
            "prior_schema_diagnostic_manifest_sha256": schema_hash,
            "prior_schema_diagnostic_outcome": "diagnostics-incomplete",
            "prior_schema_diagnostic_reason": (
                "unified-log-observation:unified-log-category-invalid"
            ),
            "prior_start_manifest_sha256": "b" * 64,
            "repository_clean": True,
            "repository_head": "a" * 40,
        },
        "utmctl-list-live.json": {"synthetic": True},
        "utmctl-status-live.json": {"synthetic": True},
        "host-process-command.json": v5_tests.command_metadata(
            l6_utm_launch_diagnostics.PROCESS_COMMAND,
            total_bytes=31_605,
            truncated=False,
        ),
        "host-processes.json": {
            "format": (
                l6_utm_launch_diagnostics.PRIOR_FINISHED_EVIDENCE_FORMAT
            ),
            "relevant_process_count": 0,
            "relevant_processes": [],
        },
        "unified-log-command.json": v5_tests.command_metadata(
            base_tests.log_command(),
            total_bytes=3_916_860,
            truncated=False,
        ),
        "terminal.json": {
            "automatic_delete": "not-performed",
            "automatic_retry": "not-performed",
            "format": (
                l6_utm_launch_diagnostics.PRIOR_FINISHED_EVIDENCE_FORMAT
            ),
            "guest_exec": "not-performed",
            "host_process_observation": "performed",
            "input_transfer": "not-performed",
            "operation_id": "not-generated",
            "outcome": "diagnostics-incomplete",
            "reason": (
                "unified-log-observation:unified-log-finished-invalid"
            ),
            "root_cause": "unattributed",
            "target_name": base_tests.TARGET_NAME,
            "target_uuid": base_tests.TARGET_UUID,
            "transaction": "not-performed",
            "unified_log_observation": "attempted",
            "utm_clone": "not-performed",
            "utm_start": "not-performed",
            "utm_stop": "not-performed",
        },
    }
    for name, value in values.items():
        v6_tests.write_json(evidence / name, value)
    return evidence, base_tests.rewrite_json_manifest(evidence)


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise AssertionError(f"expected object: {path}")
    return value


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
from __future__ import annotations

import contextlib
import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_fresh_boot_recovery_result_resolution_bindings as bindings
import l6_v4_install_artifacts_staged_new_boot_recovery_preflight as guest_probe
import test_l6_utm_install_artifacts_staged_fresh_boot_recovery_result_resolution as resolution_test


class RecoveryResultResolutionBindingTests(unittest.TestCase):
    def test_manifest_binds_exactly_twenty_one_members(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_fresh_boot_recovery_result_resolution_root,
                bindings.REQUIRED_ENTRY_NAMES,
            )
            request.prior_fresh_boot_recovery_result_resolution_manifest_sha256 = (
                manifest_sha256
            )

            with patched_validation(manifest_sha256):
                result = bindings.validate_fresh_boot_recovery_result_resolution_result_bindings(
                    request
                )

            self.assertEqual(
                result.evidence[
                    "prior_recovery_result_resolution_entries_verified"
                ],
                21,
            )
            self.assertEqual(
                result.evidence["guest_recovery_outcome"], "recovery-qualified"
            )
            self.assertEqual(
                result.evidence["result_authority"],
                "double-readback-plus-phase-complete",
            )

    def test_manifest_order_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_fresh_boot_recovery_result_resolution_root,
                tuple(reversed(bindings.REQUIRED_ENTRY_NAMES)),
            )
            request.prior_fresh_boot_recovery_result_resolution_manifest_sha256 = (
                manifest_sha256
            )
            with patched_validation(manifest_sha256), self.assertRaisesRegex(
                ValueError, "prior-recovery-result-resolution-entry-set-invalid"
            ):
                bindings.validate_fresh_boot_recovery_result_resolution_result_bindings(
                    request
                )

    def test_double_result_and_complete_phase_are_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = make_request(root)
            upstream = make_upstream()
            evidence = request.prior_fresh_boot_recovery_result_resolution_root
            evidence.mkdir(mode=0o700)
            payload = resolution_test.qualified_payload(request, upstream)
            recorded = json.loads(payload)
            write_json(evidence / "existing-recovery-result.json", recorded)
            for index in (1, 2):
                write_json(
                    evidence / f"existing-recovery-result-readback-{index}.json",
                    observation(
                        [
                            "utmctl",
                            "file",
                            "pull",
                            request.target_uuid,
                            request.guest_terminal_path,
                        ],
                        payload,
                    ),
                )
            phase = guest_probe.canonical_json(
                {"format": guest_probe.PHASE_FORMAT, "phase": "complete"}
            )
            write_json(
                evidence / "existing-recovery-phase-readback.json",
                observation(
                    [
                        "utmctl",
                        "file",
                        "pull",
                        request.target_uuid,
                        request.guest_phase_path,
                    ],
                    phase,
                ),
            )

            value = bindings._validate_result_and_phase(
                request, upstream, evidence
            )
            self.assertEqual(value["outcome"], "recovery-qualified")

            drift = json.loads(
                (evidence / "existing-recovery-result-readback-2.json").read_text()
            )
            drift["stdout"]["total_bytes"] += 1
            write_json(
                evidence / "existing-recovery-result-readback-2.json", drift
            )
            with self.assertRaisesRegex(
                ValueError, "prior-recovery-result-readback-drift"
            ):
                bindings._validate_result_and_phase(request, upstream, evidence)

    def test_terminal_requires_zero_mutation_and_exact_counts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = make_request(root)
            upstream = make_upstream()
            evidence = request.prior_fresh_boot_recovery_result_resolution_root
            evidence.mkdir(mode=0o700)
            guest = json.loads(resolution_test.qualified_payload(request, upstream))
            write_json(evidence / "terminal.json", terminal(request, upstream))

            bindings._validate_terminal(request, upstream, evidence, guest)

            value = json.loads((evidence / "terminal.json").read_text())
            value["maintenance_resume_invocations"] = 1
            write_json(evidence / "terminal.json", value)
            with self.assertRaisesRegex(
                ValueError, "prior-recovery-result-resolution-terminal-invalid"
            ):
                bindings._validate_terminal(request, upstream, evidence, guest)


def make_request(root: Path) -> SimpleNamespace:
    base = resolution_test.make_request(root)
    values = dict(base.__dict__)
    values.update(
        guest_terminal_path=base.guest_terminal_path,
        guest_phase_path=base.guest_phase_path,
        prior_fresh_boot_recovery_result_resolution_root=(
            root / "prior-recovery-result-resolution"
        ),
        prior_fresh_boot_recovery_result_resolution_manifest_sha256=(
            bindings.REQUIRED_PRIOR_MANIFEST_SHA256
        ),
        prior_fresh_boot_recovery_result_resolution_attempt_id=(
            bindings.REQUIRED_PRIOR_ATTEMPT_ID
        ),
    )
    return SimpleNamespace(**values)


def make_upstream() -> SimpleNamespace:
    return SimpleNamespace(
        evidence={"format": bindings.resolution_control.EVIDENCE_FORMAT},
        current_boot_id_sha256="c" * 64,
        prior_boot_id_sha256="b" * 64,
        prior_backend_pid=42,
    )


@contextlib.contextmanager
def patched_validation(manifest_sha256: str):
    upstream = make_upstream()
    semantic_names = (
        "_validate_request_and_binding",
        "_validate_host_identity",
        "_validate_result_and_phase",
        "_validate_terminal",
        "_validate_no_forbidden_commands",
    )
    with contextlib.ExitStack() as stack:
        stack.enter_context(
            mock.patch.object(
                bindings, "REQUIRED_PRIOR_MANIFEST_SHA256", manifest_sha256
            )
        )
        stack.enter_context(
            mock.patch.object(
                bindings.resolution_control,
                "validate_result_resolution_bindings",
                return_value=upstream,
            )
        )
        stack.enter_context(
            mock.patch.object(bindings.network_ready, "_require_committed_regular")
        )
        stack.enter_context(mock.patch.object(bindings, "_validate_source_target"))
        stack.enter_context(
            mock.patch.object(
                bindings.runtime_control, "_require_no_raw_operation_id"
            )
        )
        stack.enter_context(
            mock.patch.object(
                bindings,
                "_validate_request_and_binding",
                return_value="a" * 40,
            )
        )
        stack.enter_context(
            mock.patch.object(
                bindings,
                "_validate_host_identity",
                return_value=("d" * 64, 378),
            )
        )
        stack.enter_context(
            mock.patch.object(
                bindings,
                "_validate_result_and_phase",
                return_value={"outcome": "recovery-qualified"},
            )
        )
        for name in semantic_names[3:]:
            stack.enter_context(mock.patch.object(bindings, name))
        yield


def write_manifest(root: Path, names: tuple[str, ...]) -> str:
    root.mkdir(mode=0o700)
    lines = []
    for name in names:
        path = root / name
        path.write_text("{}\n", encoding="utf-8")
        os.chmod(path, 0o600)
        lines.append(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {name}\n")
    manifest = root / "files.sha256"
    manifest.write_text("".join(lines), encoding="ascii")
    os.chmod(manifest, 0o600)
    return hashlib.sha256(manifest.read_bytes()).hexdigest()


def observation(argv: list[str], payload: bytes) -> dict[str, object]:
    return {
        "argv": argv,
        "exit_code": 0,
        "stderr": stream(b""),
        "stdout": stream(payload),
        "timed_out": False,
    }


def stream(payload: bytes) -> dict[str, object]:
    return {
        "sha256": hashlib.sha256(payload).hexdigest(),
        "total_bytes": len(payload),
        "truncated": False,
    }


def terminal(request: SimpleNamespace, upstream: SimpleNamespace) -> dict[str, object]:
    return {
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": upstream.prior_backend_pid,
        "business_guest_action": "not-performed",
        "file_pull_invocations": 3,
        "file_push_invocations": 0,
        "format": bindings.resolution_control.EVIDENCE_FORMAT,
        "guest_exec_invocations": 0,
        "guest_probe_invocations": 0,
        "guest_recovery_outcome": "recovery-qualified",
        "identity_observation_count": 3,
        "inventory_probe_invocations": 0,
        "maintenance_resume_invocations": 0,
        "operation_id": "existing-receipt-hash-only",
        "outcome": "recovery-qualified",
        "phase_readback_invocations": 1,
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "prior_fresh_boot_recovery_preflight_attempt_id": (
            bindings.resolution_control.REQUIRED_PRIOR_ATTEMPT_ID
        ),
        "reason": "deferred-readback-recovery-qualified",
        "result_readback_invocations": 2,
        "result_resolution_attempt_id": bindings.REQUIRED_PRIOR_ATTEMPT_ID,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "artifacts-staged-preserved-no-resume",
    }


def write_json(path: Path, value: dict[str, object]) -> None:
    path.write_text(json.dumps(value, sort_keys=True) + "\n", encoding="utf-8")
    os.chmod(path, 0o600)


if __name__ == "__main__":
    unittest.main()

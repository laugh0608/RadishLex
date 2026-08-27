#!/usr/bin/env python3
from __future__ import annotations

import base64
import contextlib
import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_fresh_boot_resume_result_bindings as bindings
import test_l6_utm_install_artifacts_staged_fresh_boot_resume as resume_test


class FreshBootResumeResultBindingTests(unittest.TestCase):
    def test_manifest_binds_exactly_forty_seven_members(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_fresh_boot_resume_root,
                bindings.REQUIRED_ENTRY_NAMES,
            )
            request.prior_fresh_boot_resume_manifest_sha256 = manifest_sha256

            with patched_validation(manifest_sha256):
                result = bindings.validate_fresh_boot_resume_result_bindings(request)

            self.assertEqual(
                result.evidence["prior_fresh_boot_resume_entries_verified"], 47
            )
            self.assertEqual(result.guest_resume_outcome, "state-indeterminate")
            self.assertEqual(result.maintenance_resume_invocations, 1)
            self.assertEqual(result.postflight_invocations, 1)
            self.assertEqual(result.evidence["dpkg_mutation_executed"], "unknown")

    def test_manifest_order_drift_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            manifest_sha256 = write_manifest(
                request.prior_fresh_boot_resume_root,
                tuple(reversed(bindings.REQUIRED_ENTRY_NAMES)),
            )
            request.prior_fresh_boot_resume_manifest_sha256 = manifest_sha256
            with patched_validation(manifest_sha256), self.assertRaisesRegex(
                ValueError, "prior-fresh-boot-resume-entry-set-invalid"
            ):
                bindings.validate_fresh_boot_resume_result_bindings(request)

    def test_stable_indeterminate_terminal_and_phase_are_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = make_request(root)
            upstream = resume_test.make_binding(request)
            evidence = request.prior_fresh_boot_resume_root
            evidence.mkdir(mode=0o700)
            guest = bindings._expected_guest_terminal(request, upstream)
            payload = bindings.guest_driver.canonical_json(guest)
            driver_observation = observation(
                list(bindings.control.driver_argv(request, upstream)), b""
            )
            write_json(
                evidence / "guest-fresh-boot-resume-once.json",
                {
                    "format": bindings.control.EVIDENCE_FORMAT,
                    "observation": driver_observation,
                    "raw_output_persisted": False,
                },
            )
            write_payload(
                evidence / "guest-fresh-boot-resume-marker-readback.json",
                [
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    request.guest_fresh_boot_resume_marker_path,
                ],
                bindings.control.marker_bytes(request, upstream),
            )
            for index in (1, 2):
                write_payload(
                    evidence
                    / f"guest-fresh-boot-resume-result-readback-{index}.json",
                    [
                        "utmctl",
                        "file",
                        "pull",
                        request.target_uuid,
                        request.guest_fresh_boot_resume_terminal_path,
                    ],
                    payload,
                )
            write_json(evidence / "guest-fresh-boot-resume-result.json", guest)
            write_payload(
                evidence / "guest-fresh-boot-resume-phase-readback.json",
                [
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    request.guest_fresh_boot_resume_phase_path,
                ],
                bindings.guest_driver.canonical_json(
                    {
                        "format": bindings.guest_driver.PHASE_FORMAT,
                        "phase": "indeterminate",
                    }
                ),
            )

            value = bindings._validate_execution(request, upstream, evidence)
            self.assertEqual(value["outcome"], "state-indeterminate")
            self.assertEqual(value["maintenance_resume_invocations"], 1)
            self.assertEqual(value["postflight_invocations"], 1)

            drift = json.loads(
                (
                    evidence / "guest-fresh-boot-resume-result-readback-2.json"
                ).read_text()
            )
            drift["stdout"]["total_bytes"] += 1
            write_json(
                evidence / "guest-fresh-boot-resume-result-readback-2.json",
                drift,
            )
            with self.assertRaisesRegex(
                ValueError, "prior-fresh-boot-resume-result-2-stream-invalid"
            ):
                bindings._validate_execution(request, upstream, evidence)

    def test_host_terminal_requires_unknown_dpkg_and_exact_one_shots(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = make_request(root)
            upstream = resume_test.make_binding(request)
            evidence = request.prior_fresh_boot_resume_root
            evidence.mkdir(mode=0o700)
            guest = bindings._expected_guest_terminal(request, upstream)
            write_json(
                evidence / "terminal.json",
                terminal(request, upstream, guest),
            )

            bindings._validate_terminal(request, upstream, evidence, guest)

            value = json.loads((evidence / "terminal.json").read_text())
            value["maintenance_resume_invocations"] = 0
            write_json(evidence / "terminal.json", value)
            with self.assertRaisesRegex(
                ValueError, "prior-fresh-boot-resume-terminal-invalid"
            ):
                bindings._validate_terminal(request, upstream, evidence, guest)

    def test_command_inventory_rejects_unexpected_status(self) -> None:
        value = {
            "observation": {
                "argv": ["utmctl", "status", "synthetic"],
            }
        }
        self.assertEqual(
            bindings._all_argv(value), [["utmctl", "status", "synthetic"]]
        )


def make_request(root: Path) -> SimpleNamespace:
    base = resume_test.make_request(root)
    values = dict(base.__dict__)
    for name in (
        "guest_fresh_boot_resume_root",
        "guest_frozen_resume_driver_incoming",
        "guest_frozen_resume_driver_path",
        "guest_frozen_recovery_probe_incoming",
        "guest_frozen_recovery_probe_path",
        "guest_fresh_boot_resume_driver_incoming",
        "guest_fresh_boot_resume_driver_path",
        "guest_fresh_boot_resume_marker_path",
        "guest_fresh_boot_resume_phase_path",
        "guest_fresh_boot_resume_terminal_path",
    ):
        values[name] = getattr(base, name)
    values.update(
        expected_repository_head="a" * 40,
        prior_fresh_boot_resume_root=root / "prior-fresh-boot-resume",
        prior_fresh_boot_resume_manifest_sha256=(
            bindings.REQUIRED_PRIOR_MANIFEST_SHA256
        ),
        prior_fresh_boot_resume_attempt_id=bindings.REQUIRED_PRIOR_ATTEMPT_ID,
    )
    return SimpleNamespace(**values)


@contextlib.contextmanager
def patched_validation(manifest_sha256: str):
    upstream = resume_test.make_binding(make_request(Path("/tmp/radishlex-binding")))
    with contextlib.ExitStack() as stack:
        stack.enter_context(
            mock.patch.object(
                bindings, "REQUIRED_PRIOR_MANIFEST_SHA256", manifest_sha256
            )
        )
        stack.enter_context(
            mock.patch.object(
                bindings.control,
                "validate_fresh_boot_resume_bindings",
                return_value=upstream,
            )
        )
        stack.enter_context(
            mock.patch.object(bindings.network_ready, "_require_committed_regular")
        )
        stack.enter_context(
            mock.patch.object(bindings, "_validate_request_and_binding", return_value=bindings.REQUIRED_PRIOR_REPOSITORY_HEAD)
        )
        stack.enter_context(
            mock.patch.object(bindings.result_bindings, "_validate_source_target")
        )
        stack.enter_context(
            mock.patch.object(
                bindings,
                "_validate_host_identity",
                return_value=("d" * 64, 378),
            )
        )
        for name in (
            "_validate_readiness",
            "_validate_delivery",
            "_validate_terminal",
            "_validate_command_inventory",
        ):
            stack.enter_context(mock.patch.object(bindings, name))
        stack.enter_context(
            mock.patch.object(
                bindings,
                "_validate_execution",
                return_value={"outcome": "state-indeterminate"},
            )
        )
        stack.enter_context(
            mock.patch.object(
                bindings.runtime_control, "_require_no_raw_operation_id"
            )
        )
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
        "prefix_base64": base64.b64encode(payload).decode("ascii"),
        "prefix_utf8": payload.decode("utf-8", errors="replace"),
        "sha256": hashlib.sha256(payload).hexdigest(),
        "total_bytes": len(payload),
        "truncated": False,
    }


def write_payload(path: Path, argv: list[str], payload: bytes) -> None:
    write_json(path, observation(argv, payload))


def terminal(
    request: SimpleNamespace,
    upstream: bindings.control.FreshBootResumeBinding,
    guest: dict[str, object],
) -> dict[str, object]:
    return {
        "agent_readiness_invocations": 1,
        "automatic_cleanup": "not-performed",
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "backend_pid": upstream.prior_backend_pid,
        "driver_transport_exit_code": 0,
        "file_pull_invocations": 7,
        "file_push_invocations": 3,
        "format": bindings.control.EVIDENCE_FORMAT,
        "fresh_boot_resume_attempt_id": request.fresh_boot_resume_attempt_id,
        "guest_exec_invocations": 11,
        "guest_resume_outcome": guest["outcome"],
        "identity_observation_count": 3,
        "inventory_probe_invocations": 0,
        "maintenance_resume_invocations": 1,
        "operation_id": "reconstructed-secret-hash-only",
        "operation_id_sha256": bindings.guest_driver.EXPECTED_OPERATION_ID_SHA256,
        "outcome": "state-indeterminate",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "postflight_invocations": 1,
        "reason": f"guest-resume-state-indeterminate:{bindings.REQUIRED_REASON}",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "terminal_relevant_host_process_count": 0,
        "transaction": "state-indeterminate",
        "transport_exit_disambiguated_by_terminal": True,
    }


def write_json(path: Path, value: dict[str, object]) -> None:
    path.write_text(json.dumps(value, sort_keys=True) + "\n", encoding="utf-8")
    os.chmod(path, 0o600)


if __name__ == "__main__":
    unittest.main()

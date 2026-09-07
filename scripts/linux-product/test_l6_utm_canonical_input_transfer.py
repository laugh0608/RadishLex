#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import io
import json
import os
import tarfile
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_canonical_input_transfer as input_transfer
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer


NETWORK_EVIDENCE = (
    "format=radishlex-linux-l6-v4-network-ready-v1\n"
    "attempt_id=d75818f-v4-20260823-v2\n"
    "boot_id=12345678-1234-4234-8234-123456789abc\n"
    "outcome=passed\n"
    "reason=none\n"
    "active_interface_count=1\n"
    "active_interfaces=lo\n"
    "non_loopback_interface_count=0\n"
    "non_loopback_up_count=0\n"
    "ipv4_main_route_count=0\n"
    "ipv6_main_route_count=0\n"
).encode("ascii")


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


class LinuxL6CanonicalInputTransferTests(unittest.TestCase):
    def test_transfer_requires_exact_readbacks_and_one_atomic_installer(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request, bundle = make_request(Path(temporary))
            members = inspect_bundle(bundle)
            evidence = guest_evidence(request, members)
            runner = FakeRunner(
                successful_prefix(request, bundle)
                + [
                    expected(installer_argv(request)),
                    expected(evidence_pull_argv(request), stdout=evidence),
                    expected(evidence_pull_argv(request), stdout=evidence),
                ]
            )

            result = input_transfer.run_canonical_input_transfer(
                request,
                runner=runner,
                binding_validator=valid_binding,
                target_validator=valid_target,
            )

            self.assertEqual(result.outcome, "input-ready")
            self.assertEqual(result.exit_code, input_transfer.EXIT_INPUT_READY)
            self.assertEqual(result.bundle_push_invocations, 1)
            self.assertEqual(result.bundle_readback_invocations, 2)
            self.assertEqual(result.guest_installer_invocations, 1)
            self.assertEqual(runner.expected, [])
            commands = [" ".join(call) for call in runner.calls]
            for forbidden in (
                "utmctl list",
                "utmctl start",
                "utmctl status",
                "utmctl stop",
                "utmctl clone",
                "utmctl delete",
                "/usr/bin/dpkg",
                "radishlex-linux-maintenance",
                "radishlex-linux-l6-acceptance",
            ):
                self.assertFalse(
                    any(forbidden in command for command in commands),
                    forbidden,
                )
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["operation_id"], "not-generated")
            self.assertEqual(terminal["transaction"], "not-performed")
            self.assertEqual(terminal["automatic_stop"], "not-performed")
            self.assertEqual(terminal["automatic_retry"], "not-performed")
            assert_manifest_valid(self, request.output_root)

    def test_live_network_drift_rejects_before_guest_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request, bundle = make_request(Path(temporary))
            runner = FakeRunner(
                [
                    expected(input_transfer.PROCESS_COMMAND, stdout=ps_output()),
                    expected(
                        input_transfer.network_ready._lsof_argv(request),
                        stdout=lsof_output(request),
                    ),
                    expected(network_pull_argv(request), stdout=NETWORK_EVIDENCE),
                    expected(
                        network_pull_argv(request),
                        stdout=NETWORK_EVIDENCE + b"drift\n",
                    ),
                ]
            )

            result = input_transfer.run_canonical_input_transfer(
                request,
                runner=runner,
                binding_validator=valid_binding,
                target_validator=valid_target,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(result.bundle_push_invocations, 0)
            self.assertEqual(result.guest_installer_invocations, 0)
            self.assertFalse(
                any(call[:2] == ("utmctl", "exec") for call in runner.calls)
            )

    def test_bundle_readback_drift_fails_closed_without_installer(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request, bundle = make_request(Path(temporary))
            drifted = bytearray(bundle)
            drifted[0] ^= 1
            calls = successful_prefix(request, bundle, include_second_bundle=False)
            calls.append(
                expected(bundle_pull_argv(request), stdout=bytes(drifted))
            )
            runner = FakeRunner(calls)

            result = input_transfer.run_canonical_input_transfer(
                request,
                runner=runner,
                binding_validator=valid_binding,
                target_validator=valid_target,
            )

            self.assertEqual(result.outcome, "transfer-failed-closed")
            self.assertEqual(
                result.exit_code,
                input_transfer.EXIT_TRANSFER_FAILED_CLOSED,
            )
            self.assertEqual(result.bundle_push_invocations, 1)
            self.assertEqual(result.guest_installer_invocations, 0)
            self.assertEqual(runner.expected, [])

    def test_missing_evidence_after_installer_is_indeterminate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request, bundle = make_request(Path(temporary))
            runner = FakeRunner(
                successful_prefix(request, bundle)
                + [
                    expected(installer_argv(request)),
                    expected(evidence_pull_argv(request), exit_code=1),
                ]
            )

            result = input_transfer.run_canonical_input_transfer(
                request,
                runner=runner,
                binding_validator=valid_binding,
                target_validator=valid_target,
            )

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(
                result.exit_code, input_transfer.EXIT_STATE_INDETERMINATE
            )
            self.assertEqual(result.guest_installer_invocations, 1)
            self.assertEqual(runner.expected, [])

    def test_authorization_and_v2_manifest_are_exact(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request, _ = make_request(
                Path(temporary),
                authorized_exact_source_bundle_identity=False,
            )
            with self.assertRaisesRegex(
                input_transfer.CanonicalInputTransferError,
                "authorized-exact-source-bundle-identity-required",
            ):
                request.validate()

            drifted, _ = make_request(
                Path(temporary) / "drift",
                prior_network_manifest_sha256="b" * 64,
            )
            with self.assertRaisesRegex(
                input_transfer.CanonicalInputTransferError,
                "required-prior-network-manifest-mismatch",
            ):
                drifted.validate()

    def test_binding_verifies_v2_manifest_and_live_boot_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request, _ = make_request(Path(temporary))
            populate_prior_network_root(request)

            with binding_git_and_manifest_mocks(request):
                binding = input_transfer.validate_transfer_bindings(request)

            self.assertEqual(
                binding.evidence["prior_network_entries_verified"], 14
            )
            self.assertEqual(
                binding.evidence["prior_network_manifest_sha256"],
                input_transfer.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256,
            )
            self.assertEqual(
                binding.guest_evidence_path,
                "/var/tmp/radishlex-l6-v4-network-ready-"
                "d75818f-v4-20260823-v2/network.evidence",
            )
            self.assertEqual(binding.guest_evidence_bytes, NETWORK_EVIDENCE)

    def test_binding_rejects_v2_terminal_that_claims_business_input(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request, _ = make_request(Path(temporary))
            populate_prior_network_root(
                request, terminal_overrides={"business_input": "performed"}
            )

            with binding_git_and_manifest_mocks(request):
                with self.assertRaisesRegex(
                    input_transfer.CanonicalInputTransferError,
                    "prior-network-terminal-semantics-invalid",
                ):
                    input_transfer.validate_transfer_bindings(request)

    def test_archive_symlink_is_rejected_before_guest_commands(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_root = Path(temporary)
            invalid = canonical_bundle(
                symlink_path=guest_installer.CANONICAL_INVENTORY[-1]
            )
            request, _ = make_request(
                temporary_root, bundle_override=invalid
            )
            runner = FakeRunner([])

            result = input_transfer.run_canonical_input_transfer(
                request,
                runner=runner,
                binding_validator=valid_binding,
                target_validator=valid_target,
            )

            self.assertEqual(result.outcome, "precondition-rejected")
            self.assertEqual(runner.calls, [])

    def test_guest_extractor_preserves_private_root_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_root = Path(temporary)
            bundle = canonical_bundle(
                archive_uid=os.getuid(), archive_gid=os.getgid()
            )
            staging = temporary_root / "staging"
            staging.mkdir(mode=0o700)
            os.chmod(staging, 0o700)
            with io.BytesIO(bundle) as archive_file:
                expected_members = guest_installer.inspect_canonical_archive(
                    archive_file,
                    identity_uid=os.getuid(),
                    identity_gid=os.getgid(),
                )
            with io.BytesIO(bundle) as archive_file:
                actual_members = guest_installer.extract_canonical_archive(
                    archive_file,
                    staging,
                    identity_uid=os.getuid(),
                    identity_gid=os.getgid(),
                )
            self.assertEqual(actual_members, expected_members)
            guest_installer._verify_tree(
                staging,
                expected_members,
                identity_uid=os.getuid(),
                identity_gid=os.getgid(),
            )

    def test_guest_parent_creation_rejects_symlink_without_following_it(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_root = Path(temporary)
            staging = temporary_root / "staging"
            outside = temporary_root / "outside"
            staging.mkdir(mode=0o700)
            outside.mkdir(mode=0o755)
            os.chmod(staging, 0o700)
            os.chmod(outside, 0o755)
            (staging / "source").symlink_to(outside, target_is_directory=True)

            with self.assertRaisesRegex(
                guest_installer.GuestInputInstallError,
                "staging-directory-identity-invalid",
            ):
                guest_installer._create_private_parents(
                    staging / "source" / "artifacts",
                    staging,
                    identity_uid=os.getuid(),
                    identity_gid=os.getgid(),
                )

            self.assertEqual(outside.stat().st_mode & 0o777, 0o755)


def successful_prefix(
    request: input_transfer.CanonicalInputTransferRequest,
    bundle: bytes,
    *,
    include_second_bundle: bool = True,
) -> list[ExpectedCall]:
    installer = (
        request.repository_root / input_transfer.GUEST_INSTALLER_RELATIVE_PATH
    ).read_bytes()
    calls = [
        expected(input_transfer.PROCESS_COMMAND, stdout=ps_output()),
        expected(
            input_transfer.network_ready._lsof_argv(request),
            stdout=lsof_output(request),
        ),
        expected(network_pull_argv(request), stdout=NETWORK_EVIDENCE),
        expected(network_pull_argv(request), stdout=NETWORK_EVIDENCE),
        expected(
            (
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/mkdir",
                "-m",
                "0700",
                request.guest_root,
            )
        ),
        expected(
            (
                "utmctl",
                "file",
                "push",
                request.target_uuid,
                request.guest_installer_incoming,
            ),
            stdin_sha256=hashlib.sha256(installer).hexdigest(),
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
                request.guest_installer_incoming,
                request.guest_installer_path,
            )
        ),
        expected(
            (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                request.guest_installer_path,
            ),
            stdout=installer,
        ),
        expected(
            (
                "utmctl",
                "file",
                "push",
                request.target_uuid,
                request.guest_bundle_path,
            ),
            stdin_sha256=hashlib.sha256(bundle).hexdigest(),
        ),
        expected(bundle_pull_argv(request), stdout=bundle),
    ]
    if include_second_bundle:
        calls.append(expected(bundle_pull_argv(request), stdout=bundle))
    return calls


def installer_argv(
    request: input_transfer.CanonicalInputTransferRequest,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "exec",
        request.target_uuid,
        "--cmd",
        "/usr/bin/python3",
        request.guest_installer_path,
        "--attempt-id",
        request.attempt_id,
        "--transfer-root",
        request.guest_root,
        "--bundle-path",
        request.guest_bundle_path,
        "--expected-bundle-size",
        str(request.source_bundle_size),
        "--expected-bundle-sha256",
        request.source_bundle_sha256,
    )


def network_pull_argv(
    request: input_transfer.CanonicalInputTransferRequest,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        "/var/tmp/network-v2/network.evidence",
    )


def bundle_pull_argv(
    request: input_transfer.CanonicalInputTransferRequest,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        request.guest_bundle_path,
    )


def evidence_pull_argv(
    request: input_transfer.CanonicalInputTransferRequest,
) -> tuple[str, ...]:
    return (
        "utmctl",
        "file",
        "pull",
        request.target_uuid,
        request.guest_evidence_path,
    )


def make_request(
    temporary_root: Path,
    *,
    bundle_override: bytes | None = None,
    **overrides: object,
) -> tuple[input_transfer.CanonicalInputTransferRequest, bytes]:
    temporary_root.mkdir(parents=True, exist_ok=True)
    prior = temporary_root / "network-v2"
    prior.mkdir()
    bundle = bundle_override or canonical_bundle()
    source = temporary_root / "canonical-input.ustar"
    source.write_bytes(bundle)
    source.chmod(0o600)
    values: dict[str, object] = {
        "repository_root": Path(__file__).resolve().parents[2],
        "expected_repository_head": "a" * 40,
        "prior_network_root": prior,
        "prior_network_manifest_sha256": (
            input_transfer.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
        ),
        "source_bundle_path": source,
        "source_bundle_size": len(bundle),
        "source_bundle_sha256": hashlib.sha256(bundle).hexdigest(),
        "output_root": temporary_root / "input-evidence",
        "attempt_id": "d75818f-v4-input-20260823",
        "target_uuid": input_transfer.REQUIRED_TARGET_UUID,
        "target_name": input_transfer.REQUIRED_TARGET_NAME,
        "target_package_path": (
            input_transfer.transport_bindings.expected_target_package_path(
                input_transfer.REQUIRED_TARGET_NAME
            )
        ),
        "command_timeout_seconds": 60,
        "transfer_timeout_seconds": 600,
        "authorized_canonical_input_transfer": True,
        "authorized_exact_source_bundle_identity": True,
        "authorized_private_staging_and_atomic_switch": True,
        "authorized_no_operation_transaction_stop_or_retry": True,
    }
    values.update(overrides)
    return (
        input_transfer.CanonicalInputTransferRequest(  # type: ignore[arg-type]
            **values
        ),
        bundle,
    )


def populate_prior_network_root(
    request: input_transfer.CanonicalInputTransferRequest,
    *,
    terminal_overrides: dict[str, object] | None = None,
) -> None:
    root = request.prior_network_root
    os.chmod(root, 0o700)
    attempt_id = "d75818f-v4-20260823-v2"
    guest_root = f"/var/tmp/radishlex-l6-v4-network-ready-{attempt_id}"
    request_value = {
        "attempt_id": attempt_id,
        "expected_repository_head": (
            input_transfer.transfer_bindings.REQUIRED_PRIOR_NETWORK_REPOSITORY_HEAD
        ),
        "format": input_transfer.network_ready.EVIDENCE_FORMAT,
        "guest_root": guest_root,
        "prior_network_failure_manifest_sha256": (
            input_transfer.network_ready.REQUIRED_PRIOR_NETWORK_FAILURE_MANIFEST_SHA256
        ),
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
    }
    terminal: dict[str, object] = {
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "business_input": "not-performed",
        "file_pull_invocations": 3,
        "file_push_invocations": 1,
        "guest_exec_invocations": 3,
        "network_evidence_outcome": "passed",
        "network_script_command_exit_code": 0,
        "network_script_command_timed_out": False,
        "network_script_invocations": 1,
        "operation_id": "not-generated",
        "outcome": "network-ready",
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": "double-readback-loopback-only-main-routes-empty",
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
    }
    if terminal_overrides:
        terminal.update(terminal_overrides)
    observation = start_control.CommandObservation.from_bytes(
        (
            "utmctl",
            "file",
            "pull",
            request.target_uuid,
            f"{guest_root}/network.evidence",
        ),
        stdout=NETWORK_EVIDENCE,
    ).as_json()
    parsed = input_transfer.network_ready.parse_guest_network_evidence(
        NETWORK_EVIDENCE, SimpleNamespace(attempt_id=attempt_id)  # type: ignore[arg-type]
    )
    values = {
        "request.json": request_value,
        "binding-preflight.json": {"format": "synthetic"},
        "target-files-preflight.json": {"format": "synthetic"},
        "host-process-preflight.json": {"format": "synthetic"},
        "target-handles-preflight.json": {"format": "synthetic"},
        "guest-root-create.json": {"format": "synthetic"},
        "guest-script-push.json": {"format": "synthetic"},
        "guest-script-normalize.json": {"format": "synthetic"},
        "guest-script-readback.json": {"format": "synthetic"},
        "guest-network-script.json": {"format": "synthetic"},
        "guest-network-evidence-readback-1.json": observation,
        "guest-network-evidence-readback-2.json": observation,
        "guest-network-evidence.json": parsed,
        "terminal.json": terminal,
    }
    manifest_lines = []
    for name, value in values.items():
        payload = (
            json.dumps(value, ensure_ascii=True, indent=2, sort_keys=True)
            + "\n"
        ).encode("utf-8")
        path = root / name
        path.write_bytes(payload)
        path.chmod(0o600)
        manifest_lines.append(f"{hashlib.sha256(payload).hexdigest()}  {name}\n")
    manifest = root / "files.sha256"
    manifest.write_text("".join(manifest_lines), encoding="ascii")
    manifest.chmod(0o600)


def binding_git_and_manifest_mocks(
    request: input_transfer.CanonicalInputTransferRequest,
):
    original_sha256 = input_transfer.network_ready._sha256_file

    def git_result(repository_root: Path, arguments: tuple[str, ...]) -> bytes:
        del repository_root
        if arguments == ("rev-parse", "HEAD"):
            return (request.expected_repository_head + "\n").encode("ascii")
        if arguments == ("status", "--porcelain"):
            return b""
        raise AssertionError(arguments)

    def sha256_result(path: Path) -> str:
        if path == request.prior_network_root / "files.sha256":
            return input_transfer.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
        return original_sha256(path)

    return _CombinedPatches(
        mock.patch.object(start_control, "_run_git", side_effect=git_result),
        mock.patch.object(
            input_transfer.transfer_bindings.start_control,
            "_run_git",
            side_effect=git_result,
        ),
        mock.patch.object(
            input_transfer.network_ready,
            "_sha256_file",
            side_effect=sha256_result,
        ),
    )


class _CombinedPatches:
    def __init__(self, *patchers) -> None:
        self.patchers = patchers

    def __enter__(self):
        for patcher in self.patchers:
            patcher.start()
        return self

    def __exit__(self, exc_type, exc_value, traceback) -> None:
        for patcher in reversed(self.patchers):
            patcher.stop()


def canonical_bundle(
    *,
    archive_uid: int = 0,
    archive_gid: int = 0,
    symlink_path: str | None = None,
) -> bytes:
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w", format=tarfile.USTAR_FORMAT) as archive:
        for index, name in enumerate(guest_installer.CANONICAL_INVENTORY):
            content = f"synthetic-l6-input-{index}:{name}\n".encode("utf-8")
            member = tarfile.TarInfo(name)
            member.uid = archive_uid
            member.gid = archive_gid
            member.uname = "root"
            member.gname = "root"
            member.mode = 0o755 if name.startswith("radishlex-linux-") else 0o600
            member.mtime = 0
            if name == symlink_path:
                member.type = tarfile.SYMTYPE
                member.linkname = "../outside"
                member.size = 0
                archive.addfile(member)
            else:
                member.size = len(content)
                archive.addfile(member, io.BytesIO(content))
    return output.getvalue()


def inspect_bundle(bundle: bytes) -> tuple[guest_installer.ArchiveMember, ...]:
    with io.BytesIO(bundle) as source:
        return guest_installer.inspect_canonical_archive(source)


def guest_evidence(
    request: input_transfer.CanonicalInputTransferRequest,
    members: tuple[guest_installer.ArchiveMember, ...],
    *,
    outcome: str = "passed",
    final_switch: str = "performed",
    phase: str = "final-readback",
    reason: str = "none",
) -> bytes:
    value = {
        "attempt_id": request.attempt_id,
        "automatic_cleanup": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "bundle_sha256": request.source_bundle_sha256,
        "bundle_size": request.source_bundle_size,
        "final_input_root": str(guest_installer.FINAL_INPUT_ROOT),
        "final_switch": final_switch,
        "format": guest_installer.EVIDENCE_FORMAT,
        "inventory": [member.as_json() for member in members],
        "inventory_count": len(members),
        "operation_id": "not-generated",
        "outcome": outcome,
        "phase": phase,
        "reason": reason,
        "transaction": "not-performed",
    }
    return (
        json.dumps(value, ensure_ascii=True, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def valid_binding(
    request: input_transfer.CanonicalInputTransferRequest,
) -> input_transfer.NetworkBinding:
    return input_transfer.NetworkBinding(
        evidence={
            "format": input_transfer.EVIDENCE_FORMAT,
            "repository_head": request.expected_repository_head,
        },
        guest_evidence_path="/var/tmp/network-v2/network.evidence",
        guest_evidence_bytes=NETWORK_EVIDENCE,
    )


def valid_target(
    request: input_transfer.CanonicalInputTransferRequest,
) -> dict[str, object]:
    return {
        "format": input_transfer.EVIDENCE_FORMAT,
        "target_uuid": request.target_uuid,
    }


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


def ps_output() -> bytes:
    return b"1 0 0 launchd\n"


def lsof_output(
    request: input_transfer.CanonicalInputTransferRequest,
) -> bytes:
    data = request.target_package_path / "Data"
    return (
        "p2177\n"
        "cQEMULauncher\n"
        "f17\n"
        "tREG\n"
        f"n{data / 'efi_vars.fd'}\n"
        "f18\n"
        "tREG\n"
        f"n{data / input_transfer.network_ready.REQUIRED_QCOW2_NAME}\n"
    ).encode("utf-8")


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
        test.assertEqual(path.stat().st_mode & 0o777, 0o600)
        test.assertEqual(
            hashlib.sha256(path.read_bytes()).hexdigest(), expected_hash
        )


if __name__ == "__main__":
    unittest.main()

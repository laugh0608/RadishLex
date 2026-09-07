#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import BinaryIO, Protocol

import l6_utm_canonical_input_bindings as transfer_bindings
import l6_utm_guest_network_ready as network_ready
import l6_utm_launch_transport_bindings as transport_bindings
import l6_utm_launch_transport_v2 as launch_transport
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer


EVIDENCE_FORMAT = transfer_bindings.EVIDENCE_FORMAT
CONTROL_RELATIVE_PATH = transfer_bindings.CONTROL_RELATIVE_PATH
BINDINGS_RELATIVE_PATH = transfer_bindings.BINDINGS_RELATIVE_PATH
GUEST_INSTALLER_RELATIVE_PATH = transfer_bindings.GUEST_INSTALLER_RELATIVE_PATH
REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256 = (
    transfer_bindings.REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
)
REQUIRED_TARGET_UUID = transport_bindings.REQUIRED_TARGET_UUID
REQUIRED_TARGET_NAME = transport_bindings.REQUIRED_TARGET_NAME
PROCESS_COMMAND = launch_transport.PROCESS_COMMAND
EXIT_INPUT_READY = 0
EXIT_TRANSFER_FAILED_CLOSED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}")


class CanonicalInputTransferError(ValueError):
    pass


@dataclass(frozen=True)
class CanonicalInputTransferRequest:
    repository_root: Path
    expected_repository_head: str
    prior_network_root: Path
    prior_network_manifest_sha256: str
    source_bundle_path: Path
    source_bundle_size: int
    source_bundle_sha256: str
    output_root: Path
    attempt_id: str
    target_uuid: str
    target_name: str
    target_package_path: Path
    command_timeout_seconds: int
    transfer_timeout_seconds: int
    authorized_canonical_input_transfer: bool
    authorized_exact_source_bundle_identity: bool
    authorized_private_staging_and_atomic_switch: bool
    authorized_no_operation_transaction_stop_or_retry: bool

    @property
    def guest_root(self) -> str:
        return f"/var/tmp/radishlex-l6-v4-input-transfer-{self.attempt_id}"

    @property
    def guest_installer_incoming(self) -> str:
        return f"{self.guest_root}/install-canonical-input.incoming.py"

    @property
    def guest_installer_path(self) -> str:
        return f"{self.guest_root}/install-canonical-input.py"

    @property
    def guest_bundle_path(self) -> str:
        return f"{self.guest_root}/canonical-input.ustar.incoming"

    @property
    def guest_evidence_path(self) -> str:
        return f"{self.guest_root}/transfer.evidence.json"

    def validate(self) -> None:
        for path, label in (
            (self.repository_root, "repository-root"),
            (self.prior_network_root, "prior-network-root"),
            (self.source_bundle_path, "source-bundle-path"),
            (self.output_root, "output-root"),
            (self.target_package_path, "target-package-path"),
        ):
            if not path.is_absolute() or ".." in path.parts:
                raise CanonicalInputTransferError(
                    f"{label}-must-be-absolute-normalized"
                )
        for root, reason in (
            (self.repository_root, "repository"),
            (self.prior_network_root, "prior-network"),
            (self.source_bundle_path, "source-bundle"),
            (self.target_package_path, "target"),
        ):
            if _paths_overlap(self.output_root, root):
                raise CanonicalInputTransferError(
                    f"output-root-must-not-overlap-{reason}"
                )
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise CanonicalInputTransferError(
                "expected-repository-head-invalid"
            )
        if (
            self.prior_network_manifest_sha256
            != REQUIRED_PRIOR_NETWORK_MANIFEST_SHA256
        ):
            raise CanonicalInputTransferError(
                "required-prior-network-manifest-mismatch"
            )
        if not 1 <= self.source_bundle_size <= guest_installer.MAX_BUNDLE_BYTES:
            raise CanonicalInputTransferError("source-bundle-size-invalid")
        if not HEX_64.fullmatch(self.source_bundle_sha256):
            raise CanonicalInputTransferError("source-bundle-sha256-invalid")
        if not SAFE_ATTEMPT_ID.fullmatch(self.attempt_id):
            raise CanonicalInputTransferError("attempt-id-invalid")
        try:
            canonical_uuid = str(uuid.UUID(self.target_uuid)).upper()
        except ValueError as exc:
            raise CanonicalInputTransferError("target-uuid-invalid") from exc
        if canonical_uuid != self.target_uuid:
            raise CanonicalInputTransferError("target-uuid-not-canonical")
        if self.target_uuid != REQUIRED_TARGET_UUID:
            raise CanonicalInputTransferError("required-target-uuid-mismatch")
        if self.target_name != REQUIRED_TARGET_NAME:
            raise CanonicalInputTransferError("required-target-name-mismatch")
        if self.target_package_path != transport_bindings.expected_target_package_path(
            self.target_name
        ):
            raise CanonicalInputTransferError("target-package-path-mismatch")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise CanonicalInputTransferError(
                "command-timeout-seconds-out-of-range"
            )
        if not 1 <= self.transfer_timeout_seconds <= 900:
            raise CanonicalInputTransferError(
                "transfer-timeout-seconds-out-of-range"
            )
        if not self.authorized_canonical_input_transfer:
            raise CanonicalInputTransferError(
                "authorized-canonical-input-transfer-required"
            )
        if not self.authorized_exact_source_bundle_identity:
            raise CanonicalInputTransferError(
                "authorized-exact-source-bundle-identity-required"
            )
        if not self.authorized_private_staging_and_atomic_switch:
            raise CanonicalInputTransferError(
                "authorized-private-staging-and-atomic-switch-required"
            )
        if not self.authorized_no_operation_transaction_stop_or_retry:
            raise CanonicalInputTransferError(
                "authorized-no-operation-transaction-stop-or-retry-required"
            )

    def as_json(self) -> dict[str, object]:
        return {
            "attempt_id": self.attempt_id,
            "authorization": {
                "canonical_input_transfer": True,
                "exact_source_bundle_identity": True,
                "no_operation_transaction_stop_or_retry": True,
                "private_staging_and_atomic_switch": True,
            },
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_repository_head": self.expected_repository_head,
            "format": EVIDENCE_FORMAT,
            "guest_final_input_root": str(guest_installer.FINAL_INPUT_ROOT),
            "guest_root": self.guest_root,
            "prior_network_manifest_sha256": (
                self.prior_network_manifest_sha256
            ),
            "source_bundle_path_sha256": _sha256_text(
                str(self.source_bundle_path)
            ),
            "source_bundle_sha256": self.source_bundle_sha256,
            "source_bundle_size": self.source_bundle_size,
            "target_name": self.target_name,
            "target_package_path_sha256": _sha256_text(
                str(self.target_package_path)
            ),
            "target_uuid": self.target_uuid,
            "transfer_timeout_seconds": self.transfer_timeout_seconds,
        }


NetworkBinding = transfer_bindings.NetworkBinding


@dataclass(frozen=True)
class SourceBundle:
    file_object: BinaryIO
    descriptor: dict[str, object]
    members: tuple[guest_installer.ArchiveMember, ...]

    def as_json(self) -> dict[str, object]:
        return {
            "descriptor": self.descriptor,
            "format": EVIDENCE_FORMAT,
            "inventory": [member.as_json() for member in self.members],
            "inventory_count": len(self.members),
        }


@dataclass(frozen=True)
class CanonicalInputTransferResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    bundle_push_invocations: int
    bundle_readback_invocations: int
    guest_installer_invocations: int


class CommandRunner(Protocol):
    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file: BinaryIO | None = None,
    ) -> start_control.CommandObservation: ...


class SubprocessCommandRunner:
    def run(
        self,
        argv: tuple[str, ...],
        timeout_seconds: int,
        *,
        stdin_file: BinaryIO | None = None,
    ) -> start_control.CommandObservation:
        environment = os.environ.copy()
        environment["LC_ALL"] = "C"
        environment["LANG"] = "C"
        if stdin_file is not None:
            stdin_file.seek(0)
        with tempfile.TemporaryFile() as stdout_file, tempfile.TemporaryFile() as stderr_file:
            try:
                process = subprocess.Popen(
                    argv,
                    stdin=stdin_file if stdin_file is not None else subprocess.DEVNULL,
                    stdout=stdout_file,
                    stderr=stderr_file,
                    env=environment,
                    shell=False,
                )
            except OSError as exc:
                raise CanonicalInputTransferError(
                    f"command-unavailable:{argv[0]}:{exc.__class__.__name__}"
                ) from exc
            timed_out = False
            try:
                exit_code: int | None = process.wait(timeout=timeout_seconds)
            except subprocess.TimeoutExpired:
                timed_out = True
                process.kill()
                process.wait()
                exit_code = None
            return start_control.CommandObservation(
                argv=argv,
                exit_code=exit_code,
                timed_out=timed_out,
                stdout=start_control._capture_file(stdout_file),
                stderr=start_control._capture_file(stderr_file),
            )


def run_canonical_input_transfer(
    request: CanonicalInputTransferRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator=None,
    source_opener=None,
    target_validator=None,
) -> CanonicalInputTransferResult:
    request.validate()
    writer = start_control.EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_transfer_bindings
    open_source = source_opener or open_source_bundle
    validate_target = target_validator or network_ready.validate_target_files

    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    guest_mutation_started = False
    bundle_push_invocations = 0
    bundle_readback_invocations = 0
    file_pull_invocations = 0
    file_push_invocations = 0
    guest_exec_invocations = 0
    guest_installer_invocations = 0
    parsed_guest_evidence: dict[str, object] | None = None
    source_bundle: SourceBundle | None = None

    try:
        binding = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding.evidence)

        stage = "source-bundle-preflight"
        source_bundle = open_source(request)
        writer.write_json("source-bundle-preflight.json", source_bundle.as_json())

        stage = "target-files-preflight"
        writer.write_json(
            "target-files-preflight.json", validate_target(request)
        )

        stage = "host-process-preflight"
        process_observation = command_runner.run(
            PROCESS_COMMAND, request.command_timeout_seconds
        )
        network_ready.start_control._require_successful_observation(
            process_observation, "host-process-preflight"
        )
        processes = launch_transport.parse_relevant_processes(
            process_observation
        )
        if any(item["role"] == "utmctl" for item in processes):
            raise CanonicalInputTransferError(
                "utmctl-process-active-before-input-transfer"
            )
        writer.write_json(
            "host-process-preflight.json",
            {
                "format": EVIDENCE_FORMAT,
                "observation": network_ready._observation_metadata(
                    process_observation
                ),
                "relevant_process_count": len(processes),
            },
        )

        stage = "target-handles-preflight"
        lsof_observation = command_runner.run(
            network_ready._lsof_argv(request),
            request.command_timeout_seconds,
        )
        handles = network_ready.parse_target_handles(lsof_observation, request)
        writer.write_json(
            "target-handles-preflight.json",
            {
                "format": EVIDENCE_FORMAT,
                "observation": network_ready._observation_metadata(
                    lsof_observation
                ),
                **handles,
            },
        )

        for index in (1, 2):
            stage = f"network-evidence-live-readback-{index}"
            file_pull_invocations += 1
            observation = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    binding.guest_evidence_path,
                ),
                request.command_timeout_seconds,
            )
            writer.write_json(
                f"network-evidence-live-readback-{index}.json",
                observation.as_json(),
            )
            _require_exact_small_readback(
                observation,
                binding.guest_evidence_bytes,
                stage,
            )

        installer_bytes = _read_guest_installer(request.repository_root)
        stage = "guest-root-create"
        guest_exec_invocations += 1
        guest_mutation_started = True
        setup_observation = command_runner.run(
            (
                "utmctl",
                "exec",
                request.target_uuid,
                "--cmd",
                "/bin/mkdir",
                "-m",
                "0700",
                request.guest_root,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json("guest-root-create.json", setup_observation.as_json())
        _require_successful_small_observation(
            setup_observation, "guest-root-create"
        )

        stage = "guest-installer-push"
        file_push_invocations += 1
        with (request.repository_root / GUEST_INSTALLER_RELATIVE_PATH).open(
            "rb"
        ) as installer_source:
            installer_push = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "push",
                    request.target_uuid,
                    request.guest_installer_incoming,
                ),
                request.command_timeout_seconds,
                stdin_file=installer_source,
            )
        writer.write_json(
            "guest-installer-push.json", installer_push.as_json()
        )
        _require_successful_small_observation(
            installer_push, "guest-installer-push"
        )

        stage = "guest-installer-normalize"
        guest_exec_invocations += 1
        installer_normalize = command_runner.run(
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
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "guest-installer-normalize.json", installer_normalize.as_json()
        )
        _require_successful_small_observation(
            installer_normalize, "guest-installer-normalize"
        )

        stage = "guest-installer-readback"
        file_pull_invocations += 1
        installer_readback = command_runner.run(
            (
                "utmctl",
                "file",
                "pull",
                request.target_uuid,
                request.guest_installer_path,
            ),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "guest-installer-readback.json", installer_readback.as_json()
        )
        _require_exact_small_readback(
            installer_readback,
            installer_bytes,
            "guest-installer-readback",
        )

        stage = "source-bundle-push"
        bundle_push_invocations = 1
        file_push_invocations += 1
        source_bundle.file_object.seek(0)
        bundle_push = command_runner.run(
            (
                "utmctl",
                "file",
                "push",
                request.target_uuid,
                request.guest_bundle_path,
            ),
            request.transfer_timeout_seconds,
            stdin_file=source_bundle.file_object,
        )
        writer.write_json("source-bundle-push.json", bundle_push.as_json())
        _require_successful_small_observation(
            bundle_push, "source-bundle-push"
        )

        for index in (1, 2):
            stage = f"source-bundle-readback-{index}"
            bundle_readback_invocations += 1
            file_pull_invocations += 1
            observation = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    request.guest_bundle_path,
                ),
                request.transfer_timeout_seconds,
            )
            writer.write_json(
                f"source-bundle-readback-{index}.json",
                {
                    "format": EVIDENCE_FORMAT,
                    "observation": network_ready._observation_metadata(
                        observation
                    ),
                },
            )
            _require_exact_bundle_readback(observation, request, stage)

        stage = "guest-installer"
        guest_exec_invocations += 1
        guest_installer_invocations = 1
        installer_observation = command_runner.run(
            (
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
            ),
            request.transfer_timeout_seconds,
        )
        writer.write_json(
            "guest-installer.json", installer_observation.as_json()
        )
        if installer_observation.timed_out:
            raise CanonicalInputTransferError("guest-installer-timed-out")
        if installer_observation.stdout.truncated or installer_observation.stderr.truncated:
            raise CanonicalInputTransferError(
                "guest-installer-output-truncated"
            )

        evidence_readbacks: list[start_control.CommandObservation] = []
        for index in (1, 2):
            stage = f"guest-transfer-evidence-readback-{index}"
            file_pull_invocations += 1
            observation = command_runner.run(
                (
                    "utmctl",
                    "file",
                    "pull",
                    request.target_uuid,
                    request.guest_evidence_path,
                ),
                request.command_timeout_seconds,
            )
            writer.write_json(
                f"guest-transfer-evidence-readback-{index}.json",
                observation.as_json(),
            )
            _require_successful_small_observation(observation, stage)
            if observation.stdout.total_bytes == 0:
                raise CanonicalInputTransferError(f"{stage}-empty")
            evidence_readbacks.append(observation)
        if (
            evidence_readbacks[0].stdout.total_bytes
            != evidence_readbacks[1].stdout.total_bytes
            or evidence_readbacks[0].stdout.sha256
            != evidence_readbacks[1].stdout.sha256
            or evidence_readbacks[0].stdout.prefix
            != evidence_readbacks[1].stdout.prefix
        ):
            raise CanonicalInputTransferError(
                "guest-transfer-evidence-readback-drift"
            )
        parsed_guest_evidence = parse_guest_transfer_evidence(
            evidence_readbacks[0].stdout.prefix,
            request,
            source_bundle.members,
        )
        writer.write_json(
            "guest-transfer-evidence.json", parsed_guest_evidence
        )
        if parsed_guest_evidence["outcome"] == "passed":
            outcome = "input-ready"
            exit_code = EXIT_INPUT_READY
            reason = "canonical-double-readback-and-atomic-switch-passed"
        elif parsed_guest_evidence["final_switch"] == "not-performed":
            outcome = "transfer-failed-closed"
            exit_code = EXIT_TRANSFER_FAILED_CLOSED
            reason = (
                "guest-transfer-failed:"
                f"{parsed_guest_evidence['phase']}:"
                f"{parsed_guest_evidence['reason']}"
            )
        else:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE
            reason = "guest-transfer-failed-after-final-switch"
    except (
        CanonicalInputTransferError,
        guest_installer.GuestInputInstallError,
        network_ready.NetworkReadyError,
        start_control.StartControlError,
        transport_bindings.BindingError,
        OSError,
        tarfile.TarError,
    ) as exc:
        reason = f"{stage}:{exc}"
        if guest_installer_invocations == 1:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE
        elif guest_mutation_started:
            outcome = "transfer-failed-closed"
            exit_code = EXIT_TRANSFER_FAILED_CLOSED
    finally:
        if source_bundle is not None:
            try:
                source_postflight = revalidate_open_source_bundle(
                    request, source_bundle
                )
                writer.write_json(
                    "source-bundle-postflight.json", source_postflight
                )
            except (
                CanonicalInputTransferError,
                OSError,
                guest_installer.GuestInputInstallError,
                tarfile.TarError,
            ) as exc:
                reason = f"source-bundle-postflight:{exc}"
                if guest_installer_invocations == 1:
                    outcome = "state-indeterminate"
                    exit_code = EXIT_STATE_INDETERMINATE
                elif guest_mutation_started:
                    outcome = "transfer-failed-closed"
                    exit_code = EXIT_TRANSFER_FAILED_CLOSED
                else:
                    outcome = "precondition-rejected"
                    exit_code = EXIT_PRECONDITION_REJECTED
            source_bundle.file_object.close()

    terminal = {
        "automatic_quit": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "bundle_push_invocations": bundle_push_invocations,
        "bundle_readback_invocations": bundle_readback_invocations,
        "file_pull_invocations": file_pull_invocations,
        "file_push_invocations": file_push_invocations,
        "format": EVIDENCE_FORMAT,
        "guest_exec_invocations": guest_exec_invocations,
        "guest_installer_invocations": guest_installer_invocations,
        "guest_terminal_outcome": (
            parsed_guest_evidence.get("outcome")
            if parsed_guest_evidence
            else None
        ),
        "input_root": str(guest_installer.FINAL_INPUT_ROOT),
        "operation_id": "not-generated",
        "outcome": outcome,
        "plain_utmctl_list": "not-performed",
        "plain_utmctl_start": "not-performed",
        "plain_utmctl_status": "not-performed",
        "reason": reason,
        "source_bundle_sha256": request.source_bundle_sha256,
        "source_bundle_size": request.source_bundle_size,
        "target_name": request.target_name,
        "target_uuid": request.target_uuid,
        "transaction": "not-performed",
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return CanonicalInputTransferResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        bundle_push_invocations=bundle_push_invocations,
        bundle_readback_invocations=bundle_readback_invocations,
        guest_installer_invocations=guest_installer_invocations,
    )


def validate_transfer_bindings(
    request: CanonicalInputTransferRequest,
) -> NetworkBinding:
    if Path(__file__).absolute() != request.repository_root / CONTROL_RELATIVE_PATH:
        raise CanonicalInputTransferError("executed-control-path-mismatch")
    for module_path, relative_path, label in (
        (
            Path(transfer_bindings.__file__).absolute(),
            BINDINGS_RELATIVE_PATH,
            "bindings",
        ),
        (
            Path(guest_installer.__file__).absolute(),
            GUEST_INSTALLER_RELATIVE_PATH,
            "guest-installer",
        ),
    ):
        if module_path != request.repository_root / relative_path:
            raise CanonicalInputTransferError(
                f"executed-{label}-path-mismatch"
            )
    try:
        return transfer_bindings.validate_transfer_bindings(request)
    except ValueError as exc:
        raise CanonicalInputTransferError(str(exc)) from exc


def open_source_bundle(request: CanonicalInputTransferRequest) -> SourceBundle:
    path = request.source_bundle_path
    try:
        path_info = path.lstat()
    except OSError as exc:
        raise CanonicalInputTransferError("source-bundle-unavailable") from exc
    if (
        not stat.S_ISREG(path_info.st_mode)
        or stat.S_ISLNK(path_info.st_mode)
        or stat.S_IMODE(path_info.st_mode) != 0o600
        or path_info.st_uid != os.getuid()
        or path_info.st_gid != os.getgid()
        or path_info.st_nlink != 1
        or path_info.st_size != request.source_bundle_size
    ):
        raise CanonicalInputTransferError("source-bundle-identity-invalid")
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(path, flags)
    file_object = os.fdopen(descriptor, "rb", closefd=True)
    try:
        descriptor_info = os.fstat(file_object.fileno())
        if (
            descriptor_info.st_dev != path_info.st_dev
            or descriptor_info.st_ino != path_info.st_ino
            or descriptor_info.st_size != path_info.st_size
            or stat.S_IMODE(descriptor_info.st_mode) != 0o600
            or descriptor_info.st_uid != os.getuid()
            or descriptor_info.st_gid != os.getgid()
            or descriptor_info.st_nlink != 1
        ):
            raise CanonicalInputTransferError(
                "source-bundle-descriptor-drift"
            )
        digest = _sha256_open_file(file_object)
        if digest != request.source_bundle_sha256:
            raise CanonicalInputTransferError(
                "source-bundle-sha256-mismatch"
            )
        members = guest_installer.inspect_canonical_archive(file_object)
        descriptor_evidence = {
            "device": descriptor_info.st_dev,
            "inode": descriptor_info.st_ino,
            "mode": "0600",
            "owner_gid": descriptor_info.st_gid,
            "owner_uid": descriptor_info.st_uid,
            "sha256": digest,
            "size": descriptor_info.st_size,
        }
        file_object.seek(0)
        return SourceBundle(file_object, descriptor_evidence, members)
    except Exception:
        file_object.close()
        raise


def revalidate_open_source_bundle(
    request: CanonicalInputTransferRequest, source: SourceBundle
) -> dict[str, object]:
    info = os.fstat(source.file_object.fileno())
    descriptor = source.descriptor
    if (
        info.st_dev != descriptor["device"]
        or info.st_ino != descriptor["inode"]
        or info.st_size != request.source_bundle_size
        or stat.S_IMODE(info.st_mode) != 0o600
        or info.st_uid != descriptor["owner_uid"]
        or info.st_gid != descriptor["owner_gid"]
        or info.st_nlink != 1
    ):
        raise CanonicalInputTransferError(
            "source-bundle-postflight-descriptor-drift"
        )
    digest = _sha256_open_file(source.file_object)
    if digest != request.source_bundle_sha256:
        raise CanonicalInputTransferError(
            "source-bundle-postflight-sha256-drift"
        )
    members = guest_installer.inspect_canonical_archive(source.file_object)
    if members != source.members:
        raise CanonicalInputTransferError(
            "source-bundle-postflight-inventory-drift"
        )
    source.file_object.seek(0)
    return {
        "descriptor_unchanged": True,
        "format": EVIDENCE_FORMAT,
        "inventory_unchanged": True,
        "sha256": digest,
        "size": info.st_size,
    }


def parse_guest_transfer_evidence(
    payload: bytes,
    request: CanonicalInputTransferRequest,
    expected_members: tuple[guest_installer.ArchiveMember, ...],
) -> dict[str, object]:
    try:
        text = payload.decode("utf-8")
        value = json.loads(text)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise CanonicalInputTransferError(
            "guest-transfer-evidence-json-invalid"
        ) from exc
    if (
        not text.endswith("\n")
        or "\r" in text
        or "\x00" in text
        or not isinstance(value, dict)
    ):
        raise CanonicalInputTransferError(
            "guest-transfer-evidence-shape-invalid"
        )
    expected_keys = {
        "attempt_id",
        "automatic_cleanup",
        "automatic_retry",
        "automatic_stop",
        "bundle_sha256",
        "bundle_size",
        "final_input_root",
        "final_switch",
        "format",
        "inventory",
        "inventory_count",
        "operation_id",
        "outcome",
        "phase",
        "reason",
        "transaction",
    }
    if set(value) != expected_keys:
        raise CanonicalInputTransferError(
            "guest-transfer-evidence-keys-invalid"
        )
    fixed = {
        "attempt_id": request.attempt_id,
        "automatic_cleanup": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "bundle_sha256": request.source_bundle_sha256,
        "bundle_size": request.source_bundle_size,
        "final_input_root": str(guest_installer.FINAL_INPUT_ROOT),
        "format": guest_installer.EVIDENCE_FORMAT,
        "inventory_count": len(expected_members),
        "operation_id": "not-generated",
        "transaction": "not-performed",
    }
    if any(value.get(key) != expected for key, expected in fixed.items()):
        raise CanonicalInputTransferError(
            "guest-transfer-evidence-identity-invalid"
        )
    if value.get("inventory") != [
        member.as_json() for member in expected_members
    ]:
        raise CanonicalInputTransferError(
            "guest-transfer-evidence-inventory-invalid"
        )
    outcome = value.get("outcome")
    final_switch = value.get("final_switch")
    phase = value.get("phase")
    reason = value.get("reason")
    if outcome == "passed":
        if (
            final_switch != "performed"
            or phase != "final-readback"
            or reason != "none"
        ):
            raise CanonicalInputTransferError(
                "guest-transfer-passed-semantics-invalid"
            )
    elif outcome == "failed":
        if final_switch not in ("not-performed", "performed") or not isinstance(
            reason, str
        ) or reason in ("", "none"):
            raise CanonicalInputTransferError(
                "guest-transfer-failed-semantics-invalid"
            )
    else:
        raise CanonicalInputTransferError(
            "guest-transfer-evidence-outcome-invalid"
        )
    return value


def _read_guest_installer(repository_root: Path) -> bytes:
    path = repository_root / GUEST_INSTALLER_RELATIVE_PATH
    network_ready._require_committed_regular(path, "guest-installer")
    payload = path.read_bytes()
    if not payload or len(payload) > 64 * 1024:
        raise CanonicalInputTransferError("guest-installer-size-invalid")
    return payload


def _require_successful_small_observation(
    observation: start_control.CommandObservation, label: str
) -> None:
    if observation.timed_out:
        raise CanonicalInputTransferError(f"{label}-timed-out")
    if observation.exit_code != 0:
        raise CanonicalInputTransferError(
            f"{label}-exit-{observation.exit_code}"
        )
    if observation.stdout.truncated or observation.stderr.truncated:
        raise CanonicalInputTransferError(f"{label}-output-truncated")
    if observation.stderr.total_bytes != 0:
        raise CanonicalInputTransferError(f"{label}-stderr-not-empty")


def _require_exact_small_readback(
    observation: start_control.CommandObservation,
    expected: bytes,
    label: str,
) -> None:
    _require_successful_small_observation(observation, label)
    if (
        observation.stdout.total_bytes != len(expected)
        or observation.stdout.sha256 != hashlib.sha256(expected).hexdigest()
        or observation.stdout.prefix != expected
    ):
        raise CanonicalInputTransferError(f"{label}-content-mismatch")


def _require_exact_bundle_readback(
    observation: start_control.CommandObservation,
    request: CanonicalInputTransferRequest,
    label: str,
) -> None:
    if observation.timed_out:
        raise CanonicalInputTransferError(f"{label}-timed-out")
    if observation.exit_code != 0:
        raise CanonicalInputTransferError(
            f"{label}-exit-{observation.exit_code}"
        )
    if observation.stderr.truncated or observation.stderr.total_bytes != 0:
        raise CanonicalInputTransferError(f"{label}-stderr-invalid")
    if (
        observation.stdout.total_bytes != request.source_bundle_size
        or observation.stdout.sha256 != request.source_bundle_sha256
    ):
        raise CanonicalInputTransferError(f"{label}-content-mismatch")


def _sha256_open_file(file_object: BinaryIO) -> str:
    file_object.seek(0)
    digest = hashlib.sha256()
    while chunk := file_object.read(1024 * 1024):
        digest.update(chunk)
    file_object.seek(0)
    return digest.hexdigest()


def _sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _path_is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def _paths_overlap(first: Path, second: Path) -> bool:
    return _path_is_within(first, second) or _path_is_within(second, first)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Transfer one explicitly identified canonical L6 input USTAR "
            "to the v4 guest with bytewise readback and an atomic no-replace "
            "input-root switch."
        )
    )
    parser.add_argument("command", choices=("transfer",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--prior-network-root", type=Path, required=True)
    parser.add_argument(
        "--prior-network-manifest-sha256", required=True
    )
    parser.add_argument("--source-bundle-path", type=Path, required=True)
    parser.add_argument("--source-bundle-size", type=int, required=True)
    parser.add_argument("--source-bundle-sha256", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--target-uuid", required=True)
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--target-package-path", type=Path, required=True)
    parser.add_argument("--command-timeout-seconds", type=int, default=60)
    parser.add_argument("--transfer-timeout-seconds", type=int, default=600)
    parser.add_argument(
        "--authorized-canonical-input-transfer", action="store_true"
    )
    parser.add_argument(
        "--authorized-exact-source-bundle-identity", action="store_true"
    )
    parser.add_argument(
        "--authorized-private-staging-and-atomic-switch",
        action="store_true",
    )
    parser.add_argument(
        "--authorized-no-operation-transaction-stop-or-retry",
        action="store_true",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = CanonicalInputTransferRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        prior_network_root=args.prior_network_root,
        prior_network_manifest_sha256=args.prior_network_manifest_sha256,
        source_bundle_path=args.source_bundle_path,
        source_bundle_size=args.source_bundle_size,
        source_bundle_sha256=args.source_bundle_sha256,
        output_root=args.output_root,
        attempt_id=args.attempt_id,
        target_uuid=args.target_uuid,
        target_name=args.target_name,
        target_package_path=args.target_package_path,
        command_timeout_seconds=args.command_timeout_seconds,
        transfer_timeout_seconds=args.transfer_timeout_seconds,
        authorized_canonical_input_transfer=(
            args.authorized_canonical_input_transfer
        ),
        authorized_exact_source_bundle_identity=(
            args.authorized_exact_source_bundle_identity
        ),
        authorized_private_staging_and_atomic_switch=(
            args.authorized_private_staging_and_atomic_switch
        ),
        authorized_no_operation_transaction_stop_or_retry=(
            args.authorized_no_operation_transaction_stop_or_retry
        ),
    )
    try:
        result = run_canonical_input_transfer(request)
    except (
        CanonicalInputTransferError,
        guest_installer.GuestInputInstallError,
        network_ready.NetworkReadyError,
        start_control.StartControlError,
        transport_bindings.BindingError,
        OSError,
        tarfile.TarError,
    ) as exc:
        print(f"l6_utm_canonical_input_transfer_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"canonical_input_transfer_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())

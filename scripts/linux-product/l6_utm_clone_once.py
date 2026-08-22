#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import BinaryIO, Callable, Protocol, Sequence


EVIDENCE_FORMAT = "radishlex-linux-l6-utm-clone-once-v1"
CONTROL_RELATIVE_PATH = Path("scripts/linux-product/l6_utm_clone_once.py")
MAX_CAPTURE_BYTES = 64 * 1024
EXIT_CREATED = 0
EXIT_FAILED_CLOSED_ABSENT = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}")


class CloneControlError(ValueError):
    pass


@dataclass(frozen=True)
class CapturedOutput:
    prefix: bytes
    total_bytes: int
    sha256: str
    truncated: bool

    @classmethod
    def from_bytes(cls, value: bytes) -> CapturedOutput:
        return cls(
            prefix=value[:MAX_CAPTURE_BYTES],
            total_bytes=len(value),
            sha256=hashlib.sha256(value).hexdigest(),
            truncated=len(value) > MAX_CAPTURE_BYTES,
        )

    def as_json(self) -> dict[str, object]:
        return {
            "prefix_base64": base64.b64encode(self.prefix).decode("ascii"),
            "prefix_utf8": self.prefix.decode("utf-8", errors="replace"),
            "sha256": self.sha256,
            "total_bytes": self.total_bytes,
            "truncated": self.truncated,
        }


@dataclass(frozen=True)
class CommandObservation:
    argv: tuple[str, ...]
    exit_code: int | None
    timed_out: bool
    stdout: CapturedOutput
    stderr: CapturedOutput

    @classmethod
    def from_bytes(
        cls,
        argv: Sequence[str],
        *,
        exit_code: int | None = 0,
        timed_out: bool = False,
        stdout: bytes = b"",
        stderr: bytes = b"",
    ) -> CommandObservation:
        return cls(
            argv=tuple(argv),
            exit_code=exit_code,
            timed_out=timed_out,
            stdout=CapturedOutput.from_bytes(stdout),
            stderr=CapturedOutput.from_bytes(stderr),
        )

    def as_json(self) -> dict[str, object]:
        return {
            "argv": list(self.argv),
            "exit_code": self.exit_code,
            "stderr": self.stderr.as_json(),
            "stdout": self.stdout.as_json(),
            "timed_out": self.timed_out,
        }


class CommandRunner(Protocol):
    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> CommandObservation: ...


class SubprocessCommandRunner:
    def run(
        self, argv: tuple[str, ...], timeout_seconds: int
    ) -> CommandObservation:
        environment = os.environ.copy()
        environment["LC_ALL"] = "C"
        environment["LANG"] = "C"
        with tempfile.TemporaryFile() as stdout_file, tempfile.TemporaryFile() as stderr_file:
            try:
                process = subprocess.Popen(
                    argv,
                    stdin=subprocess.DEVNULL,
                    stdout=stdout_file,
                    stderr=stderr_file,
                    env=environment,
                    shell=False,
                )
            except OSError as exc:
                raise CloneControlError(
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

            return CommandObservation(
                argv=argv,
                exit_code=exit_code,
                timed_out=timed_out,
                stdout=_capture_file(stdout_file),
                stderr=_capture_file(stderr_file),
            )


@dataclass(frozen=True)
class CloneRequest:
    repository_root: Path
    expected_repository_head: str
    prior_failure_root: Path
    prior_failure_manifest_sha256: str
    output_root: Path
    attempt_id: str
    source_uuid: str
    source_name: str
    target_name: str
    target_package_path: Path
    expected_vm_count: int
    expected_preclone_inventory_sha256: str
    clone_timeout_seconds: int
    command_timeout_seconds: int
    authorized_l6_vm_clone: bool
    authorized_no_automatic_retry_delete_or_start: bool

    def validate(self) -> None:
        for path, label in (
            (self.repository_root, "repository-root"),
            (self.prior_failure_root, "prior-failure-root"),
            (self.output_root, "output-root"),
            (self.target_package_path, "target-package-path"),
        ):
            if not path.is_absolute():
                raise CloneControlError(f"{label}-must-be-absolute")
            if ".." in path.parts:
                raise CloneControlError(f"{label}-must-be-normalized")
        if _path_is_within(self.output_root, self.repository_root):
            raise CloneControlError("output-root-must-be-outside-repository")
        if _path_is_within(self.output_root, self.prior_failure_root):
            raise CloneControlError("output-root-must-not-modify-prior-failure")
        if _paths_overlap(self.output_root, self.target_package_path):
            raise CloneControlError("output-root-and-target-package-must-not-overlap")
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise CloneControlError("expected-repository-head-invalid")
        if not HEX_64.fullmatch(self.prior_failure_manifest_sha256):
            raise CloneControlError("prior-failure-manifest-sha256-invalid")
        if not HEX_64.fullmatch(self.expected_preclone_inventory_sha256):
            raise CloneControlError("expected-preclone-inventory-sha256-invalid")
        if not SAFE_ATTEMPT_ID.fullmatch(self.attempt_id):
            raise CloneControlError("attempt-id-invalid")
        _validate_uuid(self.source_uuid, "source-uuid")
        _validate_vm_name(self.source_name, "source-name")
        _validate_vm_name(self.target_name, "target-name")
        if self.source_name == self.target_name:
            raise CloneControlError("source-and-target-name-must-differ")
        if self.target_package_path.name != f"{self.target_name}.utm":
            raise CloneControlError("target-package-name-mismatch")
        if not 1 <= self.expected_vm_count <= 128:
            raise CloneControlError("expected-vm-count-out-of-range")
        if not 1 <= self.clone_timeout_seconds <= 300:
            raise CloneControlError("clone-timeout-seconds-out-of-range")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise CloneControlError("command-timeout-seconds-out-of-range")
        if not self.authorized_l6_vm_clone:
            raise CloneControlError("authorized-l6-vm-clone-required")
        if not self.authorized_no_automatic_retry_delete_or_start:
            raise CloneControlError(
                "authorized-no-automatic-retry-delete-or-start-required"
            )

    def as_json(self) -> dict[str, object]:
        return {
            "attempt_id": self.attempt_id,
            "authorization": {
                "l6_vm_clone": True,
                "no_automatic_retry_delete_or_start": True,
            },
            "clone_timeout_seconds": self.clone_timeout_seconds,
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_preclone_inventory_sha256": (
                self.expected_preclone_inventory_sha256
            ),
            "expected_repository_head": self.expected_repository_head,
            "expected_vm_count": self.expected_vm_count,
            "format": EVIDENCE_FORMAT,
            "prior_failure_manifest_sha256": (
                self.prior_failure_manifest_sha256
            ),
            "source_name": self.source_name,
            "source_uuid": self.source_uuid,
            "target_name": self.target_name,
            "target_package_name": self.target_package_path.name,
            "target_package_path_sha256": _sha256_text(
                str(self.target_package_path)
            ),
        }


@dataclass(frozen=True)
class RegisteredVm:
    uuid: str
    status: str
    name: str


@dataclass(frozen=True)
class CloneResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    clone_invocations: int


class EvidenceWriter:
    def __init__(self, root: Path):
        self.root = root
        self._files: list[str] = []

    @classmethod
    def create(cls, root: Path) -> EvidenceWriter:
        parent = root.parent
        try:
            parent_stat = parent.lstat()
        except OSError as exc:
            raise CloneControlError("output-parent-unavailable") from exc
        if not stat.S_ISDIR(parent_stat.st_mode) or stat.S_ISLNK(
            parent_stat.st_mode
        ):
            raise CloneControlError("output-parent-must-be-directory")
        try:
            os.mkdir(root, 0o700)
        except FileExistsError as exc:
            raise CloneControlError("output-root-must-be-absent") from exc
        os.chmod(root, 0o700)
        _fsync_directory(parent)
        return cls(root)

    def write_json(self, name: str, value: dict[str, object]) -> None:
        if not re.fullmatch(r"[a-z0-9][a-z0-9.-]*\.json", name):
            raise CloneControlError("evidence-name-invalid")
        payload = (
            json.dumps(value, ensure_ascii=True, indent=2, sort_keys=True) + "\n"
        ).encode("utf-8")
        self._write_exclusive(name, payload)

    def write_manifest(self) -> str:
        lines = []
        for name in self._files:
            digest = _sha256_file(self.root / name)
            lines.append(f"{digest}  {name}\n")
        self._write_exclusive("files.sha256", "".join(lines).encode("ascii"))
        _fsync_directory(self.root)
        return _sha256_file(self.root / "files.sha256")

    def _write_exclusive(self, name: str, payload: bytes) -> None:
        path = self.root / name
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL
        if hasattr(os, "O_NOFOLLOW"):
            flags |= os.O_NOFOLLOW
        descriptor = os.open(path, flags, 0o600)
        try:
            with os.fdopen(descriptor, "wb", closefd=False) as output:
                output.write(payload)
                output.flush()
                os.fsync(output.fileno())
        finally:
            os.close(descriptor)
        os.chmod(path, 0o600)
        self._files.append(name)


BindingValidator = Callable[[CloneRequest], dict[str, object]]


def run_clone_once(
    request: CloneRequest,
    *,
    runner: CommandRunner | None = None,
    binding_validator: BindingValidator | None = None,
) -> CloneResult:
    request.validate()
    writer = EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_clone_bindings

    clone_invocations = 0
    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    clone_observation: CommandObservation | None = None
    terminal_package_state = "not-observed"

    try:
        binding_result = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding_result)

        stage = "target-package-preclone"
        preclone_package = observe_target_package(request.target_package_path)
        writer.write_json("target-package-preclone.json", preclone_package)
        if preclone_package["state"] != "absent":
            raise CloneControlError("target-package-not-absent-before-clone")

        stage = "utmctl-list-preclone"
        preclone_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json("utmctl-list-preclone.json", preclone_list.as_json())
        preclone_vms = parse_utmctl_list(preclone_list)
        validate_preclone_inventory(preclone_vms, request)

        stage = "utmctl-clone"
        clone_invocations = 1
        clone_observation = command_runner.run(
            ("utmctl", "clone", request.source_uuid, request.target_name),
            request.clone_timeout_seconds,
        )
        writer.write_json("utmctl-clone.json", clone_observation.as_json())

        stage = "utmctl-list-terminal"
        terminal_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json("utmctl-list-terminal.json", terminal_list.as_json())
        terminal_vms = parse_utmctl_list(terminal_list)

        stage = "target-package-terminal"
        terminal_package = observe_target_package(request.target_package_path)
        terminal_package_state = str(terminal_package["state"])
        writer.write_json("target-package-terminal.json", terminal_package)

        if clone_created(
            clone_observation,
            preclone_vms,
            terminal_vms,
            terminal_package_state,
            request,
        ):
            outcome = "created"
            exit_code = EXIT_CREATED
            reason = "command-registry-and-package-confirmed"
        elif clone_failed_closed_absent(
            clone_observation,
            preclone_vms,
            terminal_vms,
            terminal_package_state,
        ):
            outcome = "failed-closed-absent"
            exit_code = EXIT_FAILED_CLOSED_ABSENT
            reason = "command-finished-and-no-registry-or-package-change"
        else:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE
            reason = "clone-postconditions-not-conclusive"
    except CloneControlError as exc:
        reason = f"{stage}:{exc}"
        if clone_invocations == 1:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE

    terminal = {
        "automatic_delete": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_start": "not-performed",
        "clone_command_exit_code": (
            clone_observation.exit_code if clone_observation else None
        ),
        "clone_command_timed_out": (
            clone_observation.timed_out if clone_observation else False
        ),
        "clone_invocations": clone_invocations,
        "format": EVIDENCE_FORMAT,
        "guest_exec": "not-performed",
        "input_transfer": "not-performed",
        "operation_id": "not-generated",
        "outcome": outcome,
        "reason": reason,
        "target_name": request.target_name,
        "target_package_state": terminal_package_state,
        "transaction": "not-performed",
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return CloneResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        clone_invocations=clone_invocations,
    )


def validate_clone_bindings(request: CloneRequest) -> dict[str, object]:
    repository_head = _run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if repository_head != request.expected_repository_head:
        raise CloneControlError("repository-head-drift")
    repository_status = _run_git(
        request.repository_root, ("status", "--porcelain")
    )
    if repository_status:
        raise CloneControlError("repository-not-clean")
    control_sha256 = _validate_control_identity(request.repository_root)

    manifest_path = request.prior_failure_root / "files.sha256"
    if _sha256_file(manifest_path) != request.prior_failure_manifest_sha256:
        raise CloneControlError("prior-failure-manifest-drift")
    entries = _verify_sha256_manifest(
        request.prior_failure_root, manifest_path
    )
    return {
        "control_sha256": control_sha256,
        "expected_preclone_inventory_sha256": (
            request.expected_preclone_inventory_sha256
        ),
        "format": EVIDENCE_FORMAT,
        "prior_failure_entries_verified": entries,
        "prior_failure_manifest_sha256": request.prior_failure_manifest_sha256,
        "repository_clean": True,
        "repository_head": repository_head,
    }


def parse_utmctl_list(
    observation: CommandObservation,
) -> tuple[RegisteredVm, ...]:
    _require_successful_observation(observation, "utmctl-list")
    try:
        text = observation.stdout.prefix.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise CloneControlError("utmctl-list-not-utf8") from exc
    lines = text.splitlines()
    if not lines or lines[0].split() != ["UUID", "Status", "Name"]:
        raise CloneControlError("utmctl-list-header-invalid")
    registered: list[RegisteredVm] = []
    seen: set[str] = set()
    for line in lines[1:]:
        fields = line.split(maxsplit=2)
        if len(fields) != 3:
            raise CloneControlError("utmctl-list-row-invalid")
        vm_uuid, vm_status, vm_name = fields
        _validate_uuid(vm_uuid, "utmctl-list-uuid")
        if vm_uuid in seen:
            raise CloneControlError("utmctl-list-uuid-duplicate")
        _validate_vm_name(vm_name, "utmctl-list-name")
        if not vm_status:
            raise CloneControlError("utmctl-list-status-empty")
        seen.add(vm_uuid)
        registered.append(RegisteredVm(vm_uuid, vm_status, vm_name))
    return tuple(registered)


def canonical_inventory_sha256(registered: tuple[RegisteredVm, ...]) -> str:
    payload = json.dumps(
        [
            {"name": item.name, "status": item.status, "uuid": item.uuid}
            for item in sorted(registered, key=lambda item: item.uuid)
        ],
        ensure_ascii=True,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def validate_preclone_inventory(
    registered: tuple[RegisteredVm, ...], request: CloneRequest
) -> None:
    if len(registered) != request.expected_vm_count:
        raise CloneControlError("preclone-vm-count-mismatch")
    if any(item.status != "stopped" for item in registered):
        raise CloneControlError("preclone-vm-not-all-stopped")
    source = next(
        (item for item in registered if item.uuid == request.source_uuid), None
    )
    if source is None or source.name != request.source_name:
        raise CloneControlError("preclone-source-identity-mismatch")
    if sum(item.name == request.source_name for item in registered) != 1:
        raise CloneControlError("preclone-source-name-not-unique")
    if any(item.name == request.target_name for item in registered):
        raise CloneControlError("preclone-target-name-already-registered")
    if (
        canonical_inventory_sha256(registered)
        != request.expected_preclone_inventory_sha256
    ):
        raise CloneControlError("preclone-inventory-sha256-mismatch")


def clone_created(
    observation: CommandObservation,
    preclone: tuple[RegisteredVm, ...],
    terminal: tuple[RegisteredVm, ...],
    package_state: str,
    request: CloneRequest,
) -> bool:
    if (
        observation.timed_out
        or observation.exit_code != 0
        or observation.stderr.total_bytes != 0
        or package_state != "directory"
        or len(terminal) != len(preclone) + 1
    ):
        return False
    baseline = {item.uuid: item for item in preclone}
    for vm_uuid, item in baseline.items():
        terminal_item = next(
            (candidate for candidate in terminal if candidate.uuid == vm_uuid), None
        )
        if terminal_item != item:
            return False
    new_items = [item for item in terminal if item.uuid not in baseline]
    if len(new_items) != 1:
        return False
    created = new_items[0]
    if created.name != request.target_name or created.status != "stopped":
        return False
    if sum(item.name == request.target_name for item in terminal) != 1:
        return False
    return all(item.status == "stopped" for item in terminal)


def clone_failed_closed_absent(
    observation: CommandObservation,
    preclone: tuple[RegisteredVm, ...],
    terminal: tuple[RegisteredVm, ...],
    package_state: str,
) -> bool:
    return (
        not observation.timed_out
        and observation.exit_code is not None
        and package_state == "absent"
        and canonical_inventory_sha256(preclone)
        == canonical_inventory_sha256(terminal)
    )


def observe_target_package(path: Path) -> dict[str, object]:
    parent = path.parent
    try:
        parent_stat = parent.lstat()
    except OSError as exc:
        raise CloneControlError("target-package-parent-unavailable") from exc
    if not stat.S_ISDIR(parent_stat.st_mode) or stat.S_ISLNK(
        parent_stat.st_mode
    ):
        raise CloneControlError("target-package-parent-invalid")
    common = {
        "package_name": path.name,
        "package_path_sha256": _sha256_text(str(path)),
    }
    try:
        package_stat = path.lstat()
    except FileNotFoundError:
        return {**common, "state": "absent"}
    except OSError as exc:
        raise CloneControlError("target-package-unavailable") from exc
    if stat.S_ISLNK(package_stat.st_mode):
        state = "symlink"
    elif stat.S_ISDIR(package_stat.st_mode):
        state = "directory"
    else:
        state = "other"
    return {
        **common,
        "gid": package_stat.st_gid,
        "mode": f"{stat.S_IMODE(package_stat.st_mode):04o}",
        "state": state,
        "uid": package_stat.st_uid,
    }


def _validate_uuid(value: str, label: str) -> None:
    try:
        canonical_uuid = str(uuid.UUID(value)).upper()
    except ValueError as exc:
        raise CloneControlError(f"{label}-invalid") from exc
    if canonical_uuid != value:
        raise CloneControlError(f"{label}-must-be-uppercase-canonical")


def _validate_vm_name(value: str, label: str) -> None:
    if (
        not value
        or len(value.encode("utf-8")) > 160
        or any(character in "\x00\r\n/" for character in value)
    ):
        raise CloneControlError(f"{label}-invalid")


def _require_successful_observation(
    observation: CommandObservation, label: str
) -> None:
    if observation.timed_out:
        raise CloneControlError(f"{label}-timed-out")
    if observation.exit_code != 0:
        raise CloneControlError(f"{label}-exit-{observation.exit_code}")
    if observation.stdout.truncated or observation.stderr.truncated:
        raise CloneControlError(f"{label}-output-truncated")
    if observation.stderr.total_bytes != 0:
        raise CloneControlError(f"{label}-stderr-not-empty")


def _capture_file(file_object: BinaryIO) -> CapturedOutput:
    file_object.seek(0, os.SEEK_END)
    total_bytes = file_object.tell()
    file_object.seek(0)
    digest = hashlib.sha256()
    prefix = bytearray()
    while True:
        chunk = file_object.read(1024 * 1024)
        if not chunk:
            break
        digest.update(chunk)
        if len(prefix) < MAX_CAPTURE_BYTES:
            prefix.extend(chunk[: MAX_CAPTURE_BYTES - len(prefix)])
    return CapturedOutput(
        prefix=bytes(prefix),
        total_bytes=total_bytes,
        sha256=digest.hexdigest(),
        truncated=total_bytes > MAX_CAPTURE_BYTES,
    )


def _run_git(repository_root: Path, arguments: tuple[str, ...]) -> bytes:
    try:
        repository_stat = repository_root.lstat()
    except OSError as exc:
        raise CloneControlError("repository-root-unavailable") from exc
    if not stat.S_ISDIR(repository_stat.st_mode) or stat.S_ISLNK(
        repository_stat.st_mode
    ):
        raise CloneControlError("repository-root-invalid")
    observation = SubprocessCommandRunner().run(
        ("/usr/bin/git", "-C", str(repository_root), *arguments), 30
    )
    _require_successful_observation(observation, "git")
    return observation.stdout.prefix


def _validate_control_identity(repository_root: Path) -> str:
    expected_path = repository_root / CONTROL_RELATIVE_PATH
    invoked_path = Path(__file__).absolute()
    if invoked_path != expected_path:
        raise CloneControlError("executed-control-path-mismatch")
    try:
        control_stat = expected_path.lstat()
    except OSError as exc:
        raise CloneControlError("executed-control-unavailable") from exc
    if (
        not stat.S_ISREG(control_stat.st_mode)
        or stat.S_ISLNK(control_stat.st_mode)
        or control_stat.st_nlink != 1
        or stat.S_IMODE(control_stat.st_mode) & 0o022
    ):
        raise CloneControlError("executed-control-identity-invalid")
    return _sha256_file(expected_path)


def _verify_sha256_manifest(root: Path, manifest: Path) -> int:
    try:
        root_stat = root.lstat()
        manifest_stat = manifest.lstat()
    except OSError as exc:
        raise CloneControlError("prior-failure-evidence-unavailable") from exc
    if (
        not stat.S_ISDIR(root_stat.st_mode)
        or stat.S_ISLNK(root_stat.st_mode)
        or stat.S_IMODE(root_stat.st_mode) != 0o700
    ):
        raise CloneControlError("prior-failure-root-invalid")
    if (
        not stat.S_ISREG(manifest_stat.st_mode)
        or stat.S_ISLNK(manifest_stat.st_mode)
        or stat.S_IMODE(manifest_stat.st_mode) != 0o600
        or manifest_stat.st_uid != root_stat.st_uid
        or manifest_stat.st_nlink != 1
    ):
        raise CloneControlError("prior-failure-manifest-identity-invalid")
    try:
        lines = manifest.read_text(encoding="ascii").splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        raise CloneControlError("prior-failure-manifest-unreadable") from exc
    if not lines:
        raise CloneControlError("prior-failure-manifest-empty")
    seen: set[str] = set()
    for line in lines:
        if len(line) < 67 or line[64:66] != "  ":
            raise CloneControlError("prior-failure-manifest-line-invalid")
        expected_hash = line[:64]
        name = line[66:]
        if not HEX_64.fullmatch(expected_hash):
            raise CloneControlError("prior-failure-entry-hash-invalid")
        if (
            not name
            or name in (".", "..")
            or name.startswith("/")
            or "\\" in name
            or "\x00" in name
            or any(part in ("", ".", "..") for part in name.split("/"))
            or name in seen
        ):
            raise CloneControlError("prior-failure-entry-name-invalid")
        path = root / name
        try:
            item_stat = path.lstat()
        except OSError as exc:
            raise CloneControlError("prior-failure-entry-unavailable") from exc
        if (
            not stat.S_ISREG(item_stat.st_mode)
            or stat.S_ISLNK(item_stat.st_mode)
            or stat.S_IMODE(item_stat.st_mode) != 0o600
            or item_stat.st_uid != root_stat.st_uid
            or item_stat.st_nlink != 1
        ):
            raise CloneControlError("prior-failure-entry-identity-invalid")
        if _sha256_file(path) != expected_hash:
            raise CloneControlError("prior-failure-entry-drift")
        seen.add(name)
    return len(seen)


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            while chunk := source.read(1024 * 1024):
                digest.update(chunk)
    except OSError as exc:
        raise CloneControlError(f"sha256-file-unreadable:{path.name}") from exc
    return digest.hexdigest()


def _sha256_text(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


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
            "Run exactly one authorized UTM clone command and classify the "
            "result from durable command, registry, and package evidence."
        )
    )
    parser.add_argument("command", choices=("clone-once",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--prior-failure-root", type=Path, required=True)
    parser.add_argument("--prior-failure-manifest-sha256", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--source-uuid", required=True)
    parser.add_argument("--source-name", required=True)
    parser.add_argument("--target-name", required=True)
    parser.add_argument("--target-package-path", type=Path, required=True)
    parser.add_argument("--expected-vm-count", type=int, required=True)
    parser.add_argument("--expected-preclone-inventory-sha256", required=True)
    parser.add_argument("--clone-timeout-seconds", type=int, default=120)
    parser.add_argument("--command-timeout-seconds", type=int, default=15)
    parser.add_argument("--authorized-l6-vm-clone", action="store_true")
    parser.add_argument(
        "--authorized-no-automatic-retry-delete-or-start",
        action="store_true",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = CloneRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        prior_failure_root=args.prior_failure_root,
        prior_failure_manifest_sha256=args.prior_failure_manifest_sha256,
        output_root=args.output_root,
        attempt_id=args.attempt_id,
        source_uuid=args.source_uuid,
        source_name=args.source_name,
        target_name=args.target_name,
        target_package_path=args.target_package_path,
        expected_vm_count=args.expected_vm_count,
        expected_preclone_inventory_sha256=(
            args.expected_preclone_inventory_sha256
        ),
        clone_timeout_seconds=args.clone_timeout_seconds,
        command_timeout_seconds=args.command_timeout_seconds,
        authorized_l6_vm_clone=args.authorized_l6_vm_clone,
        authorized_no_automatic_retry_delete_or_start=(
            args.authorized_no_automatic_retry_delete_or_start
        ),
    )
    try:
        result = run_clone_once(request)
    except CloneControlError as exc:
        print(f"l6_utm_clone_once_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"clone_once_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())

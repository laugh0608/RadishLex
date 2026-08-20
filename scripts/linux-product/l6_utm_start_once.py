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
import time
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import BinaryIO, Callable, Protocol, Sequence


EVIDENCE_FORMAT = "radishlex-linux-l6-utm-start-once-v1"
MAX_CAPTURE_BYTES = 64 * 1024
EXIT_STARTED = 0
EXIT_FAILED_CLOSED_STOPPED = 10
EXIT_PRECONDITION_REJECTED = 11
EXIT_STATE_INDETERMINATE = 12
HEX_40 = re.compile(r"[0-9a-f]{40}")
HEX_64 = re.compile(r"[0-9a-f]{64}")
SAFE_ATTEMPT_ID = re.compile(r"[a-z0-9][a-z0-9-]{0,63}")


class StartControlError(ValueError):
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
                raise StartControlError(
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
class StartRequest:
    repository_root: Path
    expected_repository_head: str
    prior_failure_root: Path
    prior_failure_manifest_sha256: str
    output_root: Path
    attempt_id: str
    clone_uuid: str
    clone_name: str
    expected_vm_count: int
    poll_attempts: int
    poll_interval_seconds: int
    start_timeout_seconds: int
    command_timeout_seconds: int
    authorized_l6_vm_start: bool
    authorized_no_automatic_stop_or_retry: bool

    def validate(self) -> None:
        for path, label in (
            (self.repository_root, "repository-root"),
            (self.prior_failure_root, "prior-failure-root"),
            (self.output_root, "output-root"),
        ):
            if not path.is_absolute():
                raise StartControlError(f"{label}-must-be-absolute")
            if ".." in path.parts:
                raise StartControlError(f"{label}-must-be-normalized")
        if _path_is_within(self.output_root, self.repository_root):
            raise StartControlError("output-root-must-be-outside-repository")
        if _path_is_within(self.output_root, self.prior_failure_root):
            raise StartControlError("output-root-must-not-modify-prior-failure")
        if not HEX_40.fullmatch(self.expected_repository_head):
            raise StartControlError("expected-repository-head-invalid")
        if not HEX_64.fullmatch(self.prior_failure_manifest_sha256):
            raise StartControlError("prior-failure-manifest-sha256-invalid")
        if not SAFE_ATTEMPT_ID.fullmatch(self.attempt_id):
            raise StartControlError("attempt-id-invalid")
        try:
            parsed_uuid = uuid.UUID(self.clone_uuid)
        except ValueError as exc:
            raise StartControlError("clone-uuid-invalid") from exc
        if str(parsed_uuid).upper() != self.clone_uuid:
            raise StartControlError("clone-uuid-must-be-uppercase-canonical")
        if (
            not self.clone_name
            or len(self.clone_name.encode("utf-8")) > 160
            or any(character in "\x00\r\n" for character in self.clone_name)
        ):
            raise StartControlError("clone-name-invalid")
        if not 1 <= self.expected_vm_count <= 128:
            raise StartControlError("expected-vm-count-out-of-range")
        if not 1 <= self.poll_attempts <= 300:
            raise StartControlError("poll-attempts-out-of-range")
        if not 1 <= self.poll_interval_seconds <= 10:
            raise StartControlError("poll-interval-seconds-out-of-range")
        if not 1 <= self.start_timeout_seconds <= 300:
            raise StartControlError("start-timeout-seconds-out-of-range")
        if not 1 <= self.command_timeout_seconds <= 60:
            raise StartControlError("command-timeout-seconds-out-of-range")
        if not self.authorized_l6_vm_start:
            raise StartControlError("authorized-l6-vm-start-required")
        if not self.authorized_no_automatic_stop_or_retry:
            raise StartControlError(
                "authorized-no-automatic-stop-or-retry-required"
            )

    def as_json(self) -> dict[str, object]:
        return {
            "attempt_id": self.attempt_id,
            "authorization": {
                "l6_vm_start": True,
                "no_automatic_stop_or_retry": True,
            },
            "clone_name": self.clone_name,
            "clone_uuid": self.clone_uuid,
            "command_timeout_seconds": self.command_timeout_seconds,
            "expected_repository_head": self.expected_repository_head,
            "expected_vm_count": self.expected_vm_count,
            "format": EVIDENCE_FORMAT,
            "poll_attempts": self.poll_attempts,
            "poll_interval_seconds": self.poll_interval_seconds,
            "prior_failure_manifest_sha256": self.prior_failure_manifest_sha256,
            "start_timeout_seconds": self.start_timeout_seconds,
        }


@dataclass(frozen=True)
class RegisteredVm:
    uuid: str
    status: str
    name: str


@dataclass(frozen=True)
class StartResult:
    outcome: str
    exit_code: int
    evidence_root: Path
    manifest_sha256: str
    start_invocations: int
    status_poll_count: int


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
            raise StartControlError("output-parent-unavailable") from exc
        if not stat.S_ISDIR(parent_stat.st_mode) or stat.S_ISLNK(
            parent_stat.st_mode
        ):
            raise StartControlError("output-parent-must-be-directory")
        try:
            os.mkdir(root, 0o700)
        except FileExistsError as exc:
            raise StartControlError("output-root-must-be-absent") from exc
        os.chmod(root, 0o700)
        _fsync_directory(parent)
        return cls(root)

    def write_json(self, name: str, value: dict[str, object]) -> None:
        if not re.fullmatch(r"[a-z0-9][a-z0-9.-]*\.json", name):
            raise StartControlError("evidence-name-invalid")
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


BindingValidator = Callable[[StartRequest], dict[str, object]]
Sleeper = Callable[[float], None]


def run_start_once(
    request: StartRequest,
    *,
    runner: CommandRunner | None = None,
    sleeper: Sleeper = time.sleep,
    binding_validator: BindingValidator | None = None,
) -> StartResult:
    request.validate()
    writer = EvidenceWriter.create(request.output_root)
    writer.write_json("request.json", request.as_json())
    command_runner = runner or SubprocessCommandRunner()
    validate_bindings = binding_validator or validate_start_bindings

    start_invocations = 0
    status_poll_count = 0
    observed_started = False
    stage = "binding-preflight"
    reason = "not-run"
    outcome = "precondition-rejected"
    exit_code = EXIT_PRECONDITION_REJECTED
    start_observation: CommandObservation | None = None

    try:
        binding_result = validate_bindings(request)
        writer.write_json("binding-preflight.json", binding_result)

        stage = "utmctl-list-prestart"
        prestart_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json("utmctl-list-prestart.json", prestart_list.as_json())
        prestart_vms = parse_utmctl_list(prestart_list)
        require_inventory(
            prestart_vms,
            request,
            target_status="stopped",
            other_status="stopped",
        )

        stage = "utmctl-status-prestart"
        prestart_status = command_runner.run(
            ("utmctl", "status", request.clone_uuid),
            request.command_timeout_seconds,
        )
        writer.write_json(
            "utmctl-status-prestart.json", prestart_status.as_json()
        )
        if parse_utmctl_status(prestart_status) != "stopped":
            raise StartControlError("target-not-stopped-before-start")

        stage = "utmctl-start"
        start_invocations = 1
        start_observation = command_runner.run(
            ("utmctl", "start", request.clone_uuid),
            request.start_timeout_seconds,
        )
        writer.write_json("utmctl-start.json", start_observation.as_json())

        stage = "utmctl-status-poll"
        for attempt in range(1, request.poll_attempts + 1):
            status_observation = command_runner.run(
                ("utmctl", "status", request.clone_uuid),
                request.command_timeout_seconds,
            )
            status_poll_count = attempt
            writer.write_json(
                f"utmctl-status-poll-{attempt:03d}.json",
                status_observation.as_json(),
            )
            try:
                observed_status = parse_utmctl_status(status_observation)
            except StartControlError:
                observed_status = None
            if observed_status == "started":
                observed_started = True
                break
            if attempt < request.poll_attempts:
                sleeper(request.poll_interval_seconds)

        stage = "utmctl-list-terminal"
        terminal_list = command_runner.run(
            ("utmctl", "list"), request.command_timeout_seconds
        )
        writer.write_json("utmctl-list-terminal.json", terminal_list.as_json())
        terminal_vms = parse_utmctl_list(terminal_list)

        if observed_started:
            require_inventory(
                terminal_vms,
                request,
                target_status="started",
                other_status="stopped",
            )
            outcome = "started-observed"
            exit_code = EXIT_STARTED
            reason = "status-and-inventory-started"
        elif inventory_matches(
            terminal_vms,
            request,
            target_status="stopped",
            other_status="stopped",
        ):
            outcome = "failed-closed-stopped"
            exit_code = EXIT_FAILED_CLOSED_STOPPED
            reason = "started-not-observed-and-all-vms-stopped"
        else:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE
            reason = "started-not-observed-and-terminal-inventory-not-stopped"
    except StartControlError as exc:
        reason = f"{stage}:{exc}"
        if start_invocations == 1:
            outcome = "state-indeterminate"
            exit_code = EXIT_STATE_INDETERMINATE

    terminal = {
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "clone_name": request.clone_name,
        "clone_uuid": request.clone_uuid,
        "format": EVIDENCE_FORMAT,
        "guest_exec": "not-performed",
        "input_transfer": "not-performed",
        "operation_id": "not-generated",
        "outcome": outcome,
        "reason": reason,
        "start_command_exit_code": (
            start_observation.exit_code if start_observation else None
        ),
        "start_command_timed_out": (
            start_observation.timed_out if start_observation else False
        ),
        "start_invocations": start_invocations,
        "status_poll_count": status_poll_count,
        "transaction": "not-performed",
    }
    writer.write_json("terminal.json", terminal)
    manifest_sha256 = writer.write_manifest()
    return StartResult(
        outcome=outcome,
        exit_code=exit_code,
        evidence_root=request.output_root,
        manifest_sha256=manifest_sha256,
        start_invocations=start_invocations,
        status_poll_count=status_poll_count,
    )


def validate_start_bindings(request: StartRequest) -> dict[str, object]:
    repository_head = _run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if repository_head != request.expected_repository_head:
        raise StartControlError("repository-head-drift")
    repository_status = _run_git(
        request.repository_root, ("status", "--porcelain")
    )
    if repository_status:
        raise StartControlError("repository-not-clean")

    manifest_path = request.prior_failure_root / "files.sha256"
    if _sha256_file(manifest_path) != request.prior_failure_manifest_sha256:
        raise StartControlError("prior-failure-manifest-drift")
    entries = _verify_sha256_manifest(
        request.prior_failure_root, manifest_path
    )
    return {
        "format": EVIDENCE_FORMAT,
        "prior_failure_entries_verified": entries,
        "prior_failure_manifest_sha256": request.prior_failure_manifest_sha256,
        "repository_clean": True,
        "repository_head": repository_head,
    }


def parse_utmctl_status(observation: CommandObservation) -> str:
    _require_successful_observation(observation, "utmctl-status")
    value = observation.stdout.prefix
    if value.endswith(b"\n"):
        value = value[:-1]
    if value not in (b"started", b"stopped"):
        raise StartControlError("utmctl-status-not-canonical")
    return value.decode("ascii")


def parse_utmctl_list(
    observation: CommandObservation,
) -> tuple[RegisteredVm, ...]:
    _require_successful_observation(observation, "utmctl-list")
    try:
        text = observation.stdout.prefix.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise StartControlError("utmctl-list-not-utf8") from exc
    lines = text.splitlines()
    if not lines or lines[0].split() != ["UUID", "Status", "Name"]:
        raise StartControlError("utmctl-list-header-invalid")
    registered: list[RegisteredVm] = []
    seen: set[str] = set()
    for line in lines[1:]:
        fields = line.split(maxsplit=2)
        if len(fields) != 3:
            raise StartControlError("utmctl-list-row-invalid")
        vm_uuid, vm_status, vm_name = fields
        try:
            canonical_uuid = str(uuid.UUID(vm_uuid)).upper()
        except ValueError as exc:
            raise StartControlError("utmctl-list-uuid-invalid") from exc
        if canonical_uuid != vm_uuid or vm_uuid in seen:
            raise StartControlError("utmctl-list-uuid-noncanonical-or-duplicate")
        if not vm_status or not vm_name:
            raise StartControlError("utmctl-list-row-empty")
        seen.add(vm_uuid)
        registered.append(RegisteredVm(vm_uuid, vm_status, vm_name))
    return tuple(registered)


def require_inventory(
    registered: tuple[RegisteredVm, ...],
    request: StartRequest,
    *,
    target_status: str,
    other_status: str,
) -> None:
    if not inventory_matches(
        registered,
        request,
        target_status=target_status,
        other_status=other_status,
    ):
        raise StartControlError(
            f"inventory-mismatch:{target_status}:{other_status}"
        )


def inventory_matches(
    registered: tuple[RegisteredVm, ...],
    request: StartRequest,
    *,
    target_status: str,
    other_status: str,
) -> bool:
    if len(registered) != request.expected_vm_count:
        return False
    target = next(
        (item for item in registered if item.uuid == request.clone_uuid), None
    )
    if (
        target is None
        or target.name != request.clone_name
        or target.status != target_status
    ):
        return False
    return all(
        item.status == other_status
        for item in registered
        if item.uuid != request.clone_uuid
    )


def _require_successful_observation(
    observation: CommandObservation, label: str
) -> None:
    if observation.timed_out:
        raise StartControlError(f"{label}-timed-out")
    if observation.exit_code != 0:
        raise StartControlError(f"{label}-exit-{observation.exit_code}")
    if observation.stdout.truncated or observation.stderr.truncated:
        raise StartControlError(f"{label}-output-truncated")
    if observation.stderr.total_bytes != 0:
        raise StartControlError(f"{label}-stderr-not-empty")


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
        raise StartControlError("repository-root-unavailable") from exc
    if not stat.S_ISDIR(repository_stat.st_mode) or stat.S_ISLNK(
        repository_stat.st_mode
    ):
        raise StartControlError("repository-root-invalid")
    observation = SubprocessCommandRunner().run(
        ("/usr/bin/git", "-C", str(repository_root), *arguments), 30
    )
    _require_successful_observation(observation, "git")
    return observation.stdout.prefix


def _verify_sha256_manifest(root: Path, manifest: Path) -> int:
    try:
        root_stat = root.lstat()
        manifest_stat = manifest.lstat()
    except OSError as exc:
        raise StartControlError("prior-failure-evidence-unavailable") from exc
    if (
        not stat.S_ISDIR(root_stat.st_mode)
        or stat.S_ISLNK(root_stat.st_mode)
        or stat.S_IMODE(root_stat.st_mode) != 0o700
    ):
        raise StartControlError("prior-failure-root-invalid")
    if (
        not stat.S_ISREG(manifest_stat.st_mode)
        or stat.S_ISLNK(manifest_stat.st_mode)
        or stat.S_IMODE(manifest_stat.st_mode) != 0o600
        or manifest_stat.st_uid != root_stat.st_uid
        or manifest_stat.st_nlink != 1
    ):
        raise StartControlError("prior-failure-manifest-identity-invalid")
    try:
        lines = manifest.read_text(encoding="ascii").splitlines()
    except (OSError, UnicodeDecodeError) as exc:
        raise StartControlError("prior-failure-manifest-unreadable") from exc
    if not lines:
        raise StartControlError("prior-failure-manifest-empty")
    seen: set[str] = set()
    for line in lines:
        if len(line) < 67 or line[64:66] != "  ":
            raise StartControlError("prior-failure-manifest-line-invalid")
        expected_hash = line[:64]
        name = line[66:]
        if not HEX_64.fullmatch(expected_hash):
            raise StartControlError("prior-failure-entry-hash-invalid")
        if (
            not name
            or name in (".", "..")
            or name.startswith("/")
            or "\\" in name
            or "\x00" in name
            or any(part in ("", ".", "..") for part in name.split("/"))
            or name in seen
        ):
            raise StartControlError("prior-failure-entry-name-invalid")
        path = root / name
        try:
            item_stat = path.lstat()
        except OSError as exc:
            raise StartControlError("prior-failure-entry-unavailable") from exc
        if (
            not stat.S_ISREG(item_stat.st_mode)
            or stat.S_ISLNK(item_stat.st_mode)
            or stat.S_IMODE(item_stat.st_mode) != 0o600
            or item_stat.st_uid != root_stat.st_uid
            or item_stat.st_nlink != 1
        ):
            raise StartControlError("prior-failure-entry-identity-invalid")
        if _sha256_file(path) != expected_hash:
            raise StartControlError("prior-failure-entry-drift")
        seen.add(name)
    return len(seen)


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            while chunk := source.read(1024 * 1024):
                digest.update(chunk)
    except OSError as exc:
        raise StartControlError(f"sha256-file-unreadable:{path.name}") from exc
    return digest.hexdigest()


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


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run exactly one authorized UTM start command and durably capture "
            "its bounded diagnostics and every status observation."
        )
    )
    parser.add_argument("command", choices=("start-once",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--prior-failure-root", type=Path, required=True)
    parser.add_argument("--prior-failure-manifest-sha256", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--attempt-id", required=True)
    parser.add_argument("--clone-uuid", required=True)
    parser.add_argument("--clone-name", required=True)
    parser.add_argument("--expected-vm-count", type=int, required=True)
    parser.add_argument("--poll-attempts", type=int, default=60)
    parser.add_argument("--poll-interval-seconds", type=int, default=1)
    parser.add_argument("--start-timeout-seconds", type=int, default=90)
    parser.add_argument("--command-timeout-seconds", type=int, default=15)
    parser.add_argument("--authorized-l6-vm-start", action="store_true")
    parser.add_argument(
        "--authorized-no-automatic-stop-or-retry", action="store_true"
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = StartRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        prior_failure_root=args.prior_failure_root,
        prior_failure_manifest_sha256=args.prior_failure_manifest_sha256,
        output_root=args.output_root,
        attempt_id=args.attempt_id,
        clone_uuid=args.clone_uuid,
        clone_name=args.clone_name,
        expected_vm_count=args.expected_vm_count,
        poll_attempts=args.poll_attempts,
        poll_interval_seconds=args.poll_interval_seconds,
        start_timeout_seconds=args.start_timeout_seconds,
        command_timeout_seconds=args.command_timeout_seconds,
        authorized_l6_vm_start=args.authorized_l6_vm_start,
        authorized_no_automatic_stop_or_retry=(
            args.authorized_no_automatic_stop_or_retry
        ),
    )
    try:
        result = run_start_once(request)
    except StartControlError as exc:
        print(f"l6_utm_start_once_error={exc}", file=sys.stderr)
        return EXIT_PRECONDITION_REJECTED
    print(f"start_once_outcome={result.outcome}")
    print(f"evidence_root={result.evidence_root}")
    print(f"files_sha256={result.manifest_sha256}")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())

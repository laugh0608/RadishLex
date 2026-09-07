#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable


EVIDENCE_FORMAT = (
    "radishlex-linux-l6-v4-canonical-input-negative-preflight-v1"
)
MAX_SMALL_FILE_BYTES = 1024 * 1024
EXIT_PASSED = 0
EXIT_REJECTED = 10
EXIT_INDETERMINATE = 12
FINAL_INPUT_ROOT = Path("/var/tmp/radishlex-l6-inputs")
EXPECTED_DPKG_STATUS_SHA256 = (
    "2c31c35c262b2b2761055fa12a55361f3d47ebfa1923dbb3cc3691499d2ce572"
)
EXPECTED_DPKG_CONFIG_SHA256 = (
    "fead43b89af3ea5691c48f32d7fe1ba0f7ab229fb5d230f612d76fe8e6f5a015"
)
EXPECTED_DEPENDENCY_SHA256 = (
    "28fe10654a7f09b18f7f49fb32025f710fd7adf0587b2ddd1ed70bcdd6aa6aac"
)
EXPECTED_STARTUP_OUTPUT = "0:1:4:15:0|error-absent"
DEPENDENCY_PACKAGES = (
    "libatk1.0-0t64",
    "libc6",
    "libcairo-gobject2",
    "libcairo2",
    "libepoxy0",
    "libfcitx5core7",
    "libfcitx5utils2",
    "libgcc-s1",
    "libgdk-pixbuf-2.0-0",
    "libglib2.0-0t64",
    "libgtk-3-0t64",
    "libharfbuzz0b",
    "libpango-1.0-0",
    "libpangocairo-1.0-0",
    "libstdc++6",
    "zlib1g",
    "fcitx5",
    "librime1t64",
    "fonts-dejavu-core",
    "fonts-noto-cjk",
)


class NegativePreflightProbeError(ValueError):
    pass


@dataclass(frozen=True)
class InputMember:
    path: str
    mode: int
    size: int
    sha256: str


INPUT_MEMBERS = (
    InputMember(
        "build-environment.json",
        0o644,
        514,
        "c5fb0d2cd960cc3d4e9879f2c484721947cf5bdfa05a8d904a804dbcbdb396de",
    ),
    InputMember(
        "case.sh",
        0o600,
        33485,
        "30e7f0e0dc7202d32592c5b1e003d995d8063a67c278e82469eb39d33e54588d",
    ),
    InputMember(
        "radishlex-linux-l6-acceptance",
        0o755,
        1981296,
        "5ca804e65941e4ba975fe758f65f61ad2c6adcd479a64e67d61c3e2e9aa3c7a6",
    ),
    InputMember(
        "radishlex-linux-maintenance",
        0o755,
        1787048,
        "422a5080b30fea3e4fa114674a9ec4b2636149779361e9628142bcc14c7457f3",
    ),
    InputMember(
        "release-pair.evidence.json",
        0o644,
        4470,
        "c74fac1217fd0716a424e1b53c6c3f042131346bd566b66114d4cca342d49849",
    ),
    InputMember(
        "snapshot-identity.evidence.txt",
        0o600,
        2030,
        "7f656ad3065a87e52d0abd4e87a70a6638945e9759a48ee9b23441d7c1396d7b",
    ),
    InputMember(
        "source/artifacts/radishlex_26.7.1+38-1_arm64.deb",
        0o644,
        40806592,
        "09ed122804b11767b8ac7cd69c323c1f6eef511fd6ae7284d75756fb60569bec",
    ),
    InputMember(
        "source/artifacts/radishlex_26.7.1+38-1_arm64.deb.evidence.json",
        0o644,
        2376,
        "fe3d6297c08dccd8cacba13d50aa44dbb1c94b0ca8c2ab4df3a5c605466fcf94",
    ),
    InputMember(
        "startup.py",
        0o600,
        1675,
        "0dcaef674b0e0eb43b2ff9588131e1a15a162c304500348f817bca22d54d8cb2",
    ),
    InputMember(
        "target-startup/usr/lib/aarch64-linux-gnu/radishlex/manager/lib/"
        "libradishlex_ime_ffi.so",
        0o600,
        7383672,
        "193145352256ba9921a61988368005c213423132f8fb3ae2635f58b366adf27d",
    ),
    InputMember(
        "target/artifacts/radishlex_26.7.1+38-2_arm64.deb",
        0o644,
        40806592,
        "4dd0054051612654940a5973cc8a465be9b598a40a6fc036c7b263088aa5dcec",
    ),
    InputMember(
        "target/artifacts/radishlex_26.7.1+38-2_arm64.deb.evidence.json",
        0o644,
        2376,
        "9e646c86c33bc5027b82c8af0b659df7515722f66ffa92be09420f9974f63f17",
    ),
)


def canonical_json(value: dict[str, object]) -> bytes:
    return (
        json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"
    ).encode("utf-8")


def expected_marker_bytes(attempt_id: str, probe_sha256: str) -> bytes:
    return canonical_json(
        {
            "format": EVIDENCE_FORMAT,
            "preflight_attempt_id": attempt_id,
            "probe_sha256": probe_sha256,
        }
    )


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(path, flags)
    try:
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            raise NegativePreflightProbeError(f"not-regular:{path}")
        while True:
            chunk = os.read(fd, 1024 * 1024)
            if not chunk:
                break
            digest.update(chunk)
    finally:
        os.close(fd)
    return digest.hexdigest()


def require_private_directory(path: Path) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISDIR(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != 0o700
        or metadata.st_uid != 0
        or metadata.st_gid != 0
        or os.listxattr(path, follow_symlinks=False)
    ):
        raise NegativePreflightProbeError(f"directory-identity:{path}")


def require_regular_identity(path: Path, member: InputMember) -> None:
    metadata = path.lstat()
    if (
        not stat.S_ISREG(metadata.st_mode)
        or stat.S_ISLNK(metadata.st_mode)
        or stat.S_IMODE(metadata.st_mode) != member.mode
        or metadata.st_uid != 0
        or metadata.st_gid != 0
        or metadata.st_nlink != 1
        or metadata.st_size != member.size
        or os.listxattr(path, follow_symlinks=False)
        or sha256_file(path) != member.sha256
    ):
        raise NegativePreflightProbeError(f"input-member-identity:{member.path}")


def validate_input_inventory(
    input_root: Path, members: tuple[InputMember, ...] = INPUT_MEMBERS
) -> None:
    require_private_directory(input_root)
    observed_files: list[str] = []
    for current, directories, files in os.walk(input_root, followlinks=False):
        current_path = Path(current)
        require_private_directory(current_path)
        for name in directories:
            require_private_directory(current_path / name)
        for name in files:
            path = current_path / name
            if path.is_symlink():
                raise NegativePreflightProbeError("input-symlink")
            observed_files.append(path.relative_to(input_root).as_posix())
    expected_paths = tuple(item.path for item in members)
    if tuple(sorted(observed_files, key=lambda item: item.encode("utf-8"))) != (
        expected_paths
    ):
        raise NegativePreflightProbeError("input-inventory-drift")
    for member in members:
        require_regular_identity(input_root / member.path, member)


def validate_release_pair(input_root: Path) -> None:
    try:
        pair = json.loads(
            (input_root / "release-pair.evidence.json").read_text(
                encoding="utf-8"
            )
        )
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise NegativePreflightProbeError("release-pair-json") from exc
    source = pair.get("releases", {}).get("source", {})
    target = pair.get("releases", {}).get("target", {})
    if (
        pair.get("evidence_format")
        != "radishlex-linux-l6-release-pair-evidence-v1"
        or pair.get("matrix_profile") != "debian13-arm64-ephemeral-v1"
        or source.get("package_version") != "26.7.1+38-1"
        or target.get("package_version") != "26.7.1+38-2"
        or source.get("package", {}).get("sha256") != INPUT_MEMBERS[6].sha256
        or target.get("package", {}).get("sha256") != INPUT_MEMBERS[10].sha256
        or source.get("dependencies") != target.get("dependencies")
        or source.get("dependencies", {}).get("count") != 20
    ):
        raise NegativePreflightProbeError("release-pair-semantics")


def parse_dpkg_status(payload: str) -> dict[str, dict[str, str]]:
    packages: dict[str, dict[str, str]] = {}
    for paragraph in payload.split("\n\n"):
        fields: dict[str, str] = {}
        for line in paragraph.splitlines():
            if not line or line[0].isspace() or ": " not in line:
                continue
            key, value = line.split(": ", 1)
            fields[key] = value
        name = fields.get("Package")
        if name:
            if name in packages:
                raise NegativePreflightProbeError("dpkg-status-duplicate-package")
            packages[name] = fields
    return packages


def validate_package_and_dependencies() -> tuple[str, int]:
    status_path = Path("/var/lib/dpkg/status")
    if sha256_file(status_path) != EXPECTED_DPKG_STATUS_SHA256:
        raise NegativePreflightProbeError("dpkg-status-drift")
    if sha256_file(Path("/etc/dpkg/dpkg.cfg")) != EXPECTED_DPKG_CONFIG_SHA256:
        raise NegativePreflightProbeError("dpkg-config-drift")
    packages = parse_dpkg_status(status_path.read_text(encoding="utf-8"))
    if "radishlex" in packages:
        raise NegativePreflightProbeError("package-present")
    lines: list[str] = []
    for name in DEPENDENCY_PACKAGES:
        fields = packages.get(name)
        if (
            fields is None
            or fields.get("Status") != "install ok installed"
            or not fields.get("Architecture")
            or not fields.get("Version")
        ):
            raise NegativePreflightProbeError(f"dependency-invalid:{name}")
        lines.append(f"{name}|{fields['Architecture']}|{fields['Version']}")
    digest = hashlib.sha256(("\n".join(lines) + "\n").encode()).hexdigest()
    if digest != EXPECTED_DEPENDENCY_SHA256:
        raise NegativePreflightProbeError("dependency-digest-drift")
    if any(Path("/var/lib/dpkg/updates").iterdir()):
        raise NegativePreflightProbeError("dpkg-updates-present")
    if any(Path("/var/lib/dpkg/info").glob("radishlex*")):
        raise NegativePreflightProbeError("dpkg-info-present")
    font_owners = (
        (
            Path("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
            Path("/var/lib/dpkg/info/fonts-dejavu-core.list"),
        ),
        (
            Path("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
            Path("/var/lib/dpkg/info/fonts-noto-cjk.list"),
        ),
    )
    for font, owner_list in font_owners:
        if not font.is_file() or font.is_symlink():
            raise NegativePreflightProbeError(f"font-invalid:{font}")
        owned = owner_list.read_text(encoding="utf-8").splitlines()
        if str(font) not in owned:
            raise NegativePreflightProbeError(f"font-owner-invalid:{font}")
    dpkg_log = Path("/var/log/dpkg.log")
    return sha256_file(dpkg_log), dpkg_log.stat().st_size


def validate_negative_state() -> None:
    absent = (
        Path("/var/lib/radishlex/install-v1"),
        Path("/var/tmp/radishlex-l6-evidence"),
        Path("/run/lock/radishlex-install-v1.lock"),
        Path(
            "/run/radishlex-l6-crash-install-artifacts-staged/"
            "operation-id.secret"
        ),
    )
    if any(path.exists() or path.is_symlink() for path in absent):
        raise NegativePreflightProbeError("negative-state-present")


def validate_xdg_absent() -> None:
    paths = (
        Path("/home/radishlex-l6/.local/share/radishlex"),
        Path("/home/radishlex-l6/.config/radishlex"),
        Path("/home/radishlex-l6/.local/state/radishlex"),
        Path("/home/radishlex-l6/.cache/radishlex"),
        Path("/home/radishlex-l6/.config/fcitx5/profile"),
    )
    if any(path.exists() or path.is_symlink() for path in paths):
        raise NegativePreflightProbeError("user-xdg-present")


def validate_processes(proc_root: Path = Path("/proc")) -> None:
    forbidden_mappings = (
        b"/usr/lib/aarch64-linux-gnu/radishlex/manager",
        b"/usr/lib/aarch64-linux-gnu/fcitx5/radishlex.so",
        b"/usr/lib/aarch64-linux-gnu/fcitx5/libradishlex_ime_ffi.so",
        b"/var/tmp/radishlex-l6-inputs/radishlex-linux-maintenance",
        b"/var/tmp/radishlex-l6-inputs/radishlex-linux-l6-acceptance",
    )
    forbidden_commands = (
        b"radishlex-linux-maintenance",
        b"radishlex-linux-l6-acceptance",
        b"/var/tmp/radishlex-l6-inputs/case.sh",
    )
    for process in proc_root.iterdir():
        if not process.name.isdigit() or int(process.name) == os.getpid():
            continue
        try:
            comm = (process / "comm").read_text(encoding="utf-8").strip()
            cmdline = (process / "cmdline").read_bytes()
            maps = (process / "maps").read_bytes()
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
        if comm in {"dpkg", "dpkg-deb", "fcitx5", "radishlex_manager"}:
            raise NegativePreflightProbeError(f"forbidden-process:{comm}")
        if any(item in cmdline for item in forbidden_commands):
            raise NegativePreflightProbeError("forbidden-command")
        if any(item in maps for item in forbidden_mappings):
            raise NegativePreflightProbeError("forbidden-mapping")


Command = Callable[[tuple[str, ...]], bytes]


def run_read_only(argv: tuple[str, ...]) -> bytes:
    environment = {
        "LC_ALL": "C",
        "PATH": "/usr/sbin:/usr/bin:/sbin:/bin",
    }
    completed = subprocess.run(
        argv,
        check=False,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=30,
        env=environment,
        cwd="/",
    )
    if completed.returncode != 0 or completed.stderr:
        raise NegativePreflightProbeError(f"command-failed:{argv[0]}")
    return completed.stdout


def validate_network(command: Command = run_read_only) -> None:
    interfaces = sorted(path.name for path in Path("/sys/class/net").iterdir())
    if interfaces != ["lo"]:
        raise NegativePreflightProbeError("network-interface-drift")
    if command(("/usr/sbin/ip", "-4", "route", "show", "table", "main")):
        raise NegativePreflightProbeError("ipv4-main-route-present")
    if command(("/usr/sbin/ip", "-6", "route", "show", "table", "main")):
        raise NegativePreflightProbeError("ipv6-main-route-present")
    addresses = command(
        ("/usr/sbin/ip", "-brief", "address", "show", "lo")
    ).decode("ascii").split()
    if addresses[2:] != ["127.0.0.1/8", "::1/128"]:
        raise NegativePreflightProbeError("loopback-address-drift")


def run_startup(input_root: Path, role: str, executable: str) -> str:
    startup = input_root / "startup.py"
    ffi = input_root / INPUT_MEMBERS[9].path
    output = run_read_only(
        (
            "/usr/bin/python3",
            "-B",
            str(startup),
            str(ffi),
            role,
            executable,
        )
    )
    try:
        value = output.decode("utf-8").strip()
    except UnicodeDecodeError as exc:
        raise NegativePreflightProbeError("startup-output-invalid") from exc
    if value != EXPECTED_STARTUP_OUTPUT:
        raise NegativePreflightProbeError("startup-decision-drift")
    return value


def write_exclusive(path: Path, payload: bytes) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0)
    fd = os.open(path, flags, 0o600)
    try:
        os.fchmod(fd, 0o600)
        os.fchown(fd, 0, 0)
        written = 0
        while written < len(payload):
            written += os.write(fd, payload[written:])
        os.fsync(fd)
    finally:
        os.close(fd)


def sync_directory(path: Path) -> None:
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def write_phase(root: Path, phase: str) -> None:
    incoming = root / ".phase.incoming.json"
    destination = root / "phase.json"
    write_exclusive(
        incoming,
        canonical_json({"format": EVIDENCE_FORMAT, "phase": phase}),
    )
    os.replace(incoming, destination)
    sync_directory(root)


def publish_terminal_evidence(path: Path, payload: bytes) -> None:
    incoming = path.parent / f".{path.name}.incoming"
    write_exclusive(incoming, payload)
    os.link(incoming, path, follow_symlinks=False)
    os.unlink(incoming)
    sync_directory(path.parent)


def current_boot_id_sha256() -> str:
    boot_id = Path("/proc/sys/kernel/random/boot_id").read_text(
        encoding="ascii"
    ).strip()
    return hashlib.sha256(boot_id.encode("ascii")).hexdigest()


def base_evidence(args: argparse.Namespace) -> dict[str, object]:
    return {
        "acceptance_invocations": 0,
        "automatic_cleanup": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_stop": "not-performed",
        "boot_id_sha256": "not-observed",
        "case_invocations": 0,
        "checkpoint_evidence": "not-observed",
        "dependency_count": 0,
        "dependency_identity": "not-observed",
        "dpkg_config_sha256": "not-observed",
        "dpkg_invocations": 0,
        "dpkg_log_sha256": "not-observed",
        "dpkg_log_size": 0,
        "dpkg_status_sha256": "not-observed",
        "fcitx_startup": "not-observed",
        "final_input_root": str(FINAL_INPUT_ROOT),
        "font_identity": "not-observed",
        "format": EVIDENCE_FORMAT,
        "guard": "not-observed",
        "input_inventory_count": 0,
        "input_inventory_identity": "not-observed",
        "input_root_identity": "not-observed",
        "maintenance_invocations": 0,
        "manager_startup": "not-observed",
        "network": "not-observed",
        "operation_id": "not-generated",
        "outcome": "predicate_failed",
        "package": "not-observed",
        "phase": "bootstrap",
        "preflight_attempt_id": args.preflight_attempt_id,
        "product_processes": "not-observed",
        "reason": "not-run",
        "receipt_terminal": "not-observed",
        "release_pair_identity": "not-observed",
        "resolution_attempt_id": args.resolution_attempt_id,
        "startup_reason": "not-observed",
        "state_root": "not-observed",
        "transaction": "not-performed",
        "transfer_attempt_id": args.transfer_attempt_id,
        "user_xdg": "not-observed",
    }


def execute_probe(args: argparse.Namespace) -> int:
    root = Path(args.preflight_root)
    probe = Path(args.probe_path)
    marker = root / "attempt.marker.json"
    phase_path = root / "phase.json"
    evidence_path = root / "negative-preflight.evidence.json"
    evidence = base_evidence(args)
    marker_created = False
    try:
        if os.geteuid() != 0 or os.getegid() != 0:
            raise NegativePreflightProbeError("root-required")
        require_private_directory(root)
        probe_member = InputMember(
            probe.name,
            0o600,
            args.expected_probe_size,
            args.expected_probe_sha256,
        )
        require_regular_identity(probe, probe_member)
        if set(path.name for path in root.iterdir()) != {probe.name}:
            raise NegativePreflightProbeError("preflight-root-not-create-new")
        write_exclusive(
            marker,
            expected_marker_bytes(
                args.preflight_attempt_id, args.expected_probe_sha256
            ),
        )
        sync_directory(root)
        marker_created = True

        evidence["phase"] = "boot-binding"
        write_phase(root, evidence["phase"])
        boot_id_sha256 = current_boot_id_sha256()
        if boot_id_sha256 != args.expected_boot_id_sha256:
            raise NegativePreflightProbeError("boot-id-drift")
        evidence["boot_id_sha256"] = boot_id_sha256

        evidence["phase"] = "input-inventory"
        write_phase(root, evidence["phase"])
        validate_input_inventory(FINAL_INPUT_ROOT)
        evidence["input_root_identity"] = "private-directory"
        evidence["input_inventory_identity"] = "matched"
        evidence["input_inventory_count"] = len(INPUT_MEMBERS)

        evidence["phase"] = "release-pair"
        write_phase(root, evidence["phase"])
        validate_release_pair(FINAL_INPUT_ROOT)
        evidence["release_pair_identity"] = "matched"

        evidence["phase"] = "negative-state"
        write_phase(root, evidence["phase"])
        validate_negative_state()
        evidence["package"] = "not-installed"
        evidence["state_root"] = "absent"
        evidence["checkpoint_evidence"] = "absent"
        evidence["guard"] = "absent"

        evidence["phase"] = "package-dependencies"
        write_phase(root, evidence["phase"])
        dpkg_log_sha256, dpkg_log_size = validate_package_and_dependencies()
        evidence["dependency_count"] = len(DEPENDENCY_PACKAGES)
        evidence["dependency_identity"] = "matched"
        evidence["dpkg_config_sha256"] = EXPECTED_DPKG_CONFIG_SHA256
        evidence["dpkg_log_sha256"] = dpkg_log_sha256
        evidence["dpkg_log_size"] = dpkg_log_size
        evidence["dpkg_status_sha256"] = EXPECTED_DPKG_STATUS_SHA256
        evidence["font_identity"] = "dejavu+noto-cjk|matched"

        evidence["phase"] = "xdg-process-network"
        write_phase(root, evidence["phase"])
        validate_xdg_absent()
        validate_processes()
        validate_network()
        evidence["user_xdg"] = "absent"
        evidence["product_processes"] = "absent"
        evidence["network"] = "loopback-only-main-routes-empty"

        evidence["phase"] = "startup"
        write_phase(root, evidence["phase"])
        evidence["manager_startup"] = run_startup(
            FINAL_INPUT_ROOT,
            "1",
            "/usr/lib/aarch64-linux-gnu/radishlex/manager/radishlex_manager",
        )
        evidence["fcitx_startup"] = run_startup(
            FINAL_INPUT_ROOT,
            "2",
            "/usr/lib/aarch64-linux-gnu/fcitx5/radishlex.so",
        )
        evidence["startup_reason"] = "ReceiptMissing"
        evidence["receipt_terminal"] = "absent"

        evidence["phase"] = "postflight"
        write_phase(root, evidence["phase"])
        validate_negative_state()
        validate_xdg_absent()
        validate_processes()
        validate_network()
        evidence["phase"] = "complete"
        write_phase(root, evidence["phase"])
        evidence["outcome"] = "passed"
        evidence["reason"] = "none"
        exit_code = EXIT_PASSED
    except (NegativePreflightProbeError, OSError, UnicodeError, ValueError) as exc:
        evidence["outcome"] = "predicate_failed"
        evidence["reason"] = f"{type(exc).__name__}:{exc}".replace("\n", " ")
        exit_code = EXIT_REJECTED

    if not marker_created:
        return EXIT_INDETERMINATE
    try:
        if not phase_path.exists():
            return EXIT_INDETERMINATE
        publish_terminal_evidence(evidence_path, canonical_json(evidence))
    except OSError:
        return EXIT_INDETERMINATE
    return exit_code


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run the create-new, read-only L6 canonical input negative "
            "preflight without invoking case, maintenance, acceptance, or dpkg."
        )
    )
    parser.add_argument("--preflight-attempt-id", required=True)
    parser.add_argument("--transfer-attempt-id", required=True)
    parser.add_argument("--resolution-attempt-id", required=True)
    parser.add_argument("--preflight-root", required=True)
    parser.add_argument("--probe-path", required=True)
    parser.add_argument("--expected-probe-size", type=int, required=True)
    parser.add_argument("--expected-probe-sha256", required=True)
    parser.add_argument("--expected-boot-id-sha256", required=True)
    return parser.parse_args(argv)


def main() -> int:
    try:
        return execute_probe(parse_args())
    except (NegativePreflightProbeError, OSError, ValueError) as exc:
        print(f"l6_v4_negative_preflight_probe_error={exc}", file=sys.stderr)
        return EXIT_INDETERMINATE


if __name__ == "__main__":
    raise SystemExit(main())

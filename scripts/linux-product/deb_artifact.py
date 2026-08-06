#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import re
import stat
import tarfile
import tempfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

import product_metadata
import rootfs as rootfs_contract


ARTIFACT_CONTRACT_PATH = (
    product_metadata.PACKAGE_ROOT / "debian/artifact.json"
)
ARTIFACT_CONTRACT_KEYS = {
    "format_version",
    "evidence_format_version",
    "distribution_identity",
    "archive_format",
    "tar_format",
    "compression",
    "source_date_epoch",
    "package_filename",
    "evidence_filename",
    "control_members",
    "installed_size_model",
}
EVIDENCE_KEYS = {
    "format_version",
    "distribution_identity",
    "archive",
    "package",
    "control",
    "product_manifest",
}
CONTROL_FIELDS = (
    "Package",
    "Version",
    "Architecture",
    "Maintainer",
    "Section",
    "Priority",
    "Installed-Size",
    "Depends",
    "Homepage",
    "Description",
)
DEPENDENCY_PATTERN = re.compile(
    r"[a-z0-9][a-z0-9+.-]*(?::[a-z0-9][a-z0-9-]*)?"
    r"(?: \((?:>=|<=|=|<<|>>) [A-Za-z0-9.+:~_-]+\))?"
)
MAX_PACKAGE_SIZE = 512 * 1024 * 1024
MAX_EVIDENCE_SIZE = 1024 * 1024
MAX_DEPENDENCY_EVIDENCE_SIZE = 64 * 1024
MAX_TAR_MEMBERS = 10_000
MAX_TAR_MEMBER_SIZE = 256 * 1024 * 1024


class DebianArtifactError(RuntimeError):
    pass


@dataclass(frozen=True)
class DebianArtifactContract:
    format_version: int
    evidence_format_version: int
    distribution_identity: str
    archive_format: str
    tar_format: str
    compression: str
    source_date_epoch: int
    package_filename: str
    evidence_filename: str
    control_members: tuple[str, ...]
    installed_size_model: str

    @classmethod
    def load(
        cls,
        metadata: product_metadata.LinuxProductMetadata,
        path: Path = ARTIFACT_CONTRACT_PATH,
    ) -> "DebianArtifactContract":
        value = product_metadata.load_json_object(
            path, "Linux Debian artifact contract"
        )
        if set(value) != ARTIFACT_CONTRACT_KEYS:
            raise DebianArtifactError(
                "Debian artifact contract fields do not match format v1"
            )
        control_members = value["control_members"]
        if not isinstance(control_members, list) or not all(
            isinstance(item, str) for item in control_members
        ):
            raise DebianArtifactError("control_members must be a string list")
        contract = cls(
            **{
                **value,
                "control_members": tuple(control_members),
            }
        )
        contract.validate(metadata)
        return contract

    def validate(self, metadata: product_metadata.LinuxProductMetadata) -> None:
        exact = {
            "format_version": 1,
            "evidence_format_version": 1,
            "distribution_identity": metadata.distribution_identity,
            "archive_format": "debian-binary-2.0-ar-v1",
            "tar_format": "ustar",
            "compression": "none",
            "source_date_epoch": 0,
            "package_filename": (
                f"{metadata.package_name}_{metadata.package_version}_"
                f"{metadata.debian_architecture}.deb"
            ),
            "evidence_filename": (
                f"{metadata.package_name}_{metadata.package_version}_"
                f"{metadata.debian_architecture}.deb.evidence.json"
            ),
            "control_members": ("control", "md5sums"),
            "installed_size_model": "sum-file-ceil-kib-v1",
        }
        for field, expected in exact.items():
            if getattr(self, field) != expected:
                raise DebianArtifactError(
                    f"{field} differs from the Debian artifact identity"
                )


@dataclass(frozen=True)
class ArMember:
    name: str
    timestamp: int
    uid: int
    gid: int
    mode: int
    data: bytes


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def md5_bytes(value: bytes) -> str:
    # Debian md5sums is a compatibility inventory. Product identity uses SHA-256.
    return hashlib.md5(value, usedforsecurity=False).hexdigest()


def canonical_json_bytes(value: dict[str, Any]) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    ).encode("utf-8")


def require_canonical_file(path: Path, label: str) -> Path:
    try:
        return rootfs_contract.require_canonical_input(
            path, label, directory=False
        )
    except rootfs_contract.LinuxRootfsError as exc:
        raise DebianArtifactError(str(exc)) from exc


def require_output_directory(path: Path) -> Path:
    try:
        path = rootfs_contract.require_canonical_input(
            path, "Debian artifact output directory", directory=True
        )
    except rootfs_contract.LinuxRootfsError as exc:
        raise DebianArtifactError(str(exc)) from exc
    if stat.S_IMODE(path.stat().st_mode) != 0o755:
        raise DebianArtifactError(
            "Debian artifact output directory must use mode 0755"
        )
    if any(path.iterdir()):
        raise DebianArtifactError("Debian artifact output directory must be empty")
    return path


def dependency_package_name(value: str) -> str:
    return value.split(" ", 1)[0].split(":", 1)[0]


def parse_dependency_list(value: str, label: str) -> tuple[str, ...]:
    if not value or value != value.strip() or "\n" in value or "${" in value:
        raise DebianArtifactError(f"{label} is not a resolved dependency list")
    dependencies = tuple(part.strip() for part in value.split(","))
    if any(
        not part or DEPENDENCY_PATTERN.fullmatch(part) is None
        for part in dependencies
    ):
        raise DebianArtifactError(f"{label} contains an unsupported dependency")
    names = [dependency_package_name(part) for part in dependencies]
    if len(set(names)) != len(names):
        raise DebianArtifactError(f"{label} contains a duplicate package")
    return dependencies


def load_shlibs_dependencies(path: Path) -> tuple[str, ...]:
    path = require_canonical_file(path, "dpkg-shlibdeps evidence")
    if path.stat().st_size > MAX_DEPENDENCY_EVIDENCE_SIZE:
        raise DebianArtifactError("dpkg-shlibdeps evidence exceeds the size limit")
    try:
        value = path.read_text(encoding="utf-8")
    except Exception as exc:
        raise DebianArtifactError(
            f"cannot read dpkg-shlibdeps evidence: {exc}"
        ) from exc
    prefix = "shlibs:Depends="
    if not value.endswith("\n") or value.count("\n") != 1 or not value.startswith(prefix):
        raise DebianArtifactError(
            "dpkg-shlibdeps evidence must contain one canonical shlibs:Depends line"
        )
    dependencies = parse_dependency_list(
        value[len(prefix) : -1], "dpkg-shlibdeps evidence"
    )
    return tuple(sorted(dependencies, key=dependency_package_name))


def fixed_dependencies(
    metadata: product_metadata.LinuxProductMetadata,
) -> tuple[str, ...]:
    substitutions = ("${shlibs:Depends}", "${misc:Depends}")
    if metadata.hard_dependencies[:2] != substitutions:
        raise DebianArtifactError("Linux hard dependency substitutions have drifted")
    return metadata.hard_dependencies[2:]


def binary_dependencies(
    metadata: product_metadata.LinuxProductMetadata,
    shlibs_dependencies: tuple[str, ...],
) -> tuple[str, ...]:
    shlibs_names = {dependency_package_name(item) for item in shlibs_dependencies}
    fixed = fixed_dependencies(metadata)
    overlap = shlibs_names & {dependency_package_name(item) for item in fixed}
    if overlap:
        raise DebianArtifactError(
            "dpkg-shlibdeps evidence overlaps fixed product dependencies: "
            + ", ".join(sorted(overlap))
        )
    return (*shlibs_dependencies, *fixed)


def installed_size_kib(rootfs: Path) -> int:
    sizes = [path.stat().st_size for path in rootfs.rglob("*") if path.is_file()]
    return max(1, sum((size + 1023) // 1024 for size in sizes))


def render_binary_control(
    metadata: product_metadata.LinuxProductMetadata,
    shlibs_dependencies: tuple[str, ...],
    installed_size: int,
) -> bytes:
    if installed_size <= 0:
        raise DebianArtifactError("Installed-Size must be positive")
    dependencies = binary_dependencies(metadata, shlibs_dependencies)
    template = product_metadata.render_control(metadata)
    lines: list[str] = []
    for line in template.splitlines():
        if line.startswith("Depends: "):
            line = "Depends: " + ", ".join(dependencies)
        lines.append(line)
        if line == "Priority: optional":
            lines.append(f"Installed-Size: {installed_size}")
    rendered = "\n".join(lines) + "\n"
    if "${" in rendered or re.search(r"@[A-Z_]+@", rendered) is not None:
        raise DebianArtifactError("binary control contains an unresolved token")
    parse_control(rendered.encode("utf-8"), metadata)
    return rendered.encode("utf-8")


def parse_control(
    value: bytes,
    metadata: product_metadata.LinuxProductMetadata,
) -> dict[str, str]:
    try:
        text = value.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise DebianArtifactError("binary control is not UTF-8") from exc
    if not text.endswith("\n") or "\n\n" in text:
        raise DebianArtifactError("binary control must be one normalized paragraph")
    fields: dict[str, str] = {}
    current = ""
    for line in text.splitlines():
        if line.startswith(" "):
            if current != "Description":
                raise DebianArtifactError("binary control has an invalid continuation")
            fields[current] += "\n" + line[1:]
            continue
        if ": " not in line:
            raise DebianArtifactError("binary control has an invalid field")
        current, field_value = line.split(": ", 1)
        if current in fields or not field_value:
            raise DebianArtifactError("binary control has a duplicate or empty field")
        fields[current] = field_value
    if tuple(fields) != CONTROL_FIELDS:
        raise DebianArtifactError("binary control fields or order differ from contract")
    exact = {
        "Package": metadata.package_name,
        "Version": metadata.package_version,
        "Architecture": metadata.debian_architecture,
        "Maintainer": "RadishLex <laugh0608@foxmail.com>",
        "Section": "utils",
        "Priority": "optional",
        "Homepage": "https://github.com/laugh0608/RadishLex",
        "Description": (
            "Local-first Chinese input system for Fcitx5\n"
            "RadishLex combines a native Fcitx5 addon with a local management "
            "application.\n"
            "This template describes the Debian 13 ARM64 local acceptance "
            "carrier only."
        ),
    }
    for field, expected in exact.items():
        if fields[field] != expected:
            raise DebianArtifactError(f"binary control {field} differs from contract")
    if not fields["Installed-Size"].isdigit() or int(fields["Installed-Size"]) <= 0:
        raise DebianArtifactError("binary control Installed-Size is invalid")
    parse_dependency_list(fields["Depends"], "binary control Depends")
    return fields


def tar_info(name: str, mode: int, size: int, directory: bool) -> tarfile.TarInfo:
    info = tarfile.TarInfo(name)
    info.type = tarfile.DIRTYPE if directory else tarfile.REGTYPE
    info.mode = mode
    info.uid = 0
    info.gid = 0
    info.uname = "root"
    info.gname = "root"
    info.mtime = 0
    info.size = 0 if directory else size
    return info


def build_tar(entries: list[tuple[str, int, bytes | None]]) -> bytes:
    output = io.BytesIO()
    try:
        with tarfile.open(
            fileobj=output,
            mode="w",
            format=tarfile.USTAR_FORMAT,
            dereference=False,
        ) as archive:
            for name, mode, content in entries:
                info = tar_info(name, mode, len(content or b""), content is None)
                archive.addfile(
                    info,
                    None if content is None else io.BytesIO(content),
                )
    except (OSError, tarfile.TarError, ValueError) as exc:
        raise DebianArtifactError(f"cannot create canonical USTAR archive: {exc}") from exc
    return output.getvalue()


def build_data_tar(rootfs: Path) -> tuple[bytes, bytes]:
    entries: list[tuple[str, int, bytes | None]] = []
    md5_records: list[str] = []
    for path in sorted(rootfs.rglob("*"), key=lambda item: item.relative_to(rootfs).as_posix()):
        relative = path.relative_to(rootfs).as_posix()
        name = f"./{relative}"
        mode = stat.S_IMODE(path.stat().st_mode)
        if path.is_dir():
            entries.append((name, mode, None))
            continue
        content = path.read_bytes()
        entries.append((name, mode, content))
        md5_records.append(f"{md5_bytes(content)}  {relative}")
    md5sums = ("\n".join(md5_records) + "\n").encode("utf-8")
    return build_tar(entries), md5sums


def build_control_tar(control: bytes, md5sums: bytes) -> bytes:
    return build_tar(
        [
            ("./control", 0o644, control),
            ("./md5sums", 0o644, md5sums),
        ]
    )


def ar_header(name: str, size: int) -> bytes:
    stored_name = f"{name}/"
    if len(stored_name) > 16 or size < 0 or size >= 10_000_000_000:
        raise DebianArtifactError("Debian ar member name or size is unsupported")
    header = (
        f"{stored_name:<16}{0:<12}{0:<6}{0:<6}{0o100644:<8o}{size:<10}`\n"
    ).encode("ascii")
    if len(header) != 60:
        raise DebianArtifactError("Debian ar header is not canonical")
    return header


def build_ar(members: list[tuple[str, bytes]]) -> bytes:
    output = bytearray(b"!<arch>\n")
    for name, content in members:
        output.extend(ar_header(name, len(content)))
        output.extend(content)
        if len(content) % 2:
            output.extend(b"\n")
    return bytes(output)


def parse_ar(value: bytes) -> list[ArMember]:
    if not value.startswith(b"!<arch>\n"):
        raise DebianArtifactError("Debian artifact has an invalid ar magic")
    offset = 8
    members: list[ArMember] = []
    while offset < len(value):
        if len(value) - offset < 60:
            raise DebianArtifactError("Debian artifact has a truncated ar header")
        header = value[offset : offset + 60]
        offset += 60
        if header[58:] != b"`\n":
            raise DebianArtifactError("Debian artifact has an invalid ar trailer")
        try:
            stored_name = header[:16].decode("ascii").rstrip()
            timestamp = int(header[16:28].decode("ascii").strip())
            uid = int(header[28:34].decode("ascii").strip())
            gid = int(header[34:40].decode("ascii").strip())
            mode = int(header[40:48].decode("ascii").strip(), 8)
            size = int(header[48:58].decode("ascii").strip())
        except (UnicodeDecodeError, ValueError) as exc:
            raise DebianArtifactError("Debian artifact ar header is malformed") from exc
        if not stored_name.endswith("/") or stored_name.startswith(("/", "#1/")):
            raise DebianArtifactError("Debian artifact uses an unsupported ar name")
        name = stored_name[:-1]
        if timestamp != 0 or uid != 0 or gid != 0 or mode != 0o100644:
            raise DebianArtifactError("Debian artifact ar identity is not canonical")
        end = offset + size
        if end > len(value):
            raise DebianArtifactError("Debian artifact has a truncated ar member")
        content = value[offset:end]
        offset = end
        if size % 2:
            if offset >= len(value) or value[offset : offset + 1] != b"\n":
                raise DebianArtifactError("Debian artifact ar padding is invalid")
            offset += 1
        members.append(ArMember(name, timestamp, uid, gid, mode, content))
    return members


def normalized_tar_relative(name: str) -> Path:
    if not name.startswith("./") or name.endswith("/"):
        raise DebianArtifactError("Debian tar path is not canonical")
    value = name[2:]
    pure = PurePosixPath(value)
    if (
        not value
        or pure.is_absolute()
        or pure.as_posix() != value
        or any(part in ("", ".", "..") for part in pure.parts)
    ):
        raise DebianArtifactError("Debian tar path escapes the package root")
    return Path(*pure.parts)


def read_tar(value: bytes, label: str) -> list[tuple[tarfile.TarInfo, bytes | None]]:
    try:
        with tarfile.open(fileobj=io.BytesIO(value), mode="r:") as archive:
            members = archive.getmembers()
            if len(members) > MAX_TAR_MEMBERS:
                raise DebianArtifactError(f"{label} exceeds the member limit")
            result: list[tuple[tarfile.TarInfo, bytes | None]] = []
            names: set[str] = set()
            for member in members:
                normalized_tar_relative(member.name)
                if member.name in names:
                    raise DebianArtifactError(f"{label} contains a duplicate path")
                names.add(member.name)
                if not (member.isdir() or member.isreg()):
                    raise DebianArtifactError(f"{label} contains a link or special node")
                if member.size < 0 or member.size > MAX_TAR_MEMBER_SIZE:
                    raise DebianArtifactError(f"{label} member exceeds the size limit")
                if (
                    member.uid != 0
                    or member.gid != 0
                    or member.uname != "root"
                    or member.gname != "root"
                    or member.mtime != 0
                    or member.pax_headers
                    or member.linkname
                ):
                    raise DebianArtifactError(f"{label} member identity is not canonical")
                content = None
                if member.isreg():
                    extracted = archive.extractfile(member)
                    if extracted is None:
                        raise DebianArtifactError(f"cannot read {label} member")
                    content = extracted.read()
                    if len(content) != member.size:
                        raise DebianArtifactError(f"{label} member size is inconsistent")
                result.append((member, content))
            return result
    except DebianArtifactError:
        raise
    except (OSError, tarfile.TarError) as exc:
        raise DebianArtifactError(f"cannot parse {label}: {exc}") from exc


def extract_data_tar(value: bytes, destination: Path) -> None:
    destination.chmod(0o755)
    for member, content in read_tar(value, "data.tar"):
        relative = normalized_tar_relative(member.name)
        target = destination / relative
        if member.isdir():
            if target.exists():
                raise DebianArtifactError("data.tar directory appears more than once")
            if not target.parent.is_dir():
                raise DebianArtifactError("data.tar directory parent is missing")
            target.mkdir()
            target.chmod(member.mode)
            continue
        if not target.parent.is_dir():
            raise DebianArtifactError("data.tar file parent is missing")
        with target.open("xb") as output:
            output.write(content or b"")
        target.chmod(member.mode)


def control_tar_values(value: bytes) -> tuple[bytes, bytes]:
    members = read_tar(value, "control.tar")
    if [member.name for member, _ in members] != ["./control", "./md5sums"]:
        raise DebianArtifactError("control.tar inventory or order differs from contract")
    for member, _ in members:
        if member.mode != 0o644:
            raise DebianArtifactError("control.tar file mode differs from 0644")
    return members[0][1] or b"", members[1][1] or b""


def artifact_evidence(
    contract: DebianArtifactContract,
    metadata: product_metadata.LinuxProductMetadata,
    package_bytes: bytes,
    members: list[tuple[str, bytes]],
    control: bytes,
    fields: dict[str, str],
    md5sums: bytes,
    product_manifest: bytes,
) -> dict[str, Any]:
    dependencies = parse_dependency_list(fields["Depends"], "binary control Depends")
    return {
        "format_version": contract.evidence_format_version,
        "distribution_identity": metadata.distribution_identity,
        "archive": {
            "compression": contract.compression,
            "format": contract.archive_format,
            "members": [
                {
                    "name": name,
                    "sha256": sha256_bytes(content),
                    "size": len(content),
                }
                for name, content in members
            ],
            "source_date_epoch": contract.source_date_epoch,
            "tar_format": contract.tar_format,
        },
        "package": {
            "filename": contract.package_filename,
            "sha256": sha256_bytes(package_bytes),
            "size": len(package_bytes),
        },
        "control": {
            "architecture": fields["Architecture"],
            "depends": list(dependencies),
            "installed_size_kib": int(fields["Installed-Size"]),
            "md5sums_sha256": sha256_bytes(md5sums),
            "package": fields["Package"],
            "sha256": sha256_bytes(control),
            "version": fields["Version"],
        },
        "product_manifest": {
            "format_version": metadata.product_manifest_format_version,
            "path": "/usr/share/radishlex/product-manifest.json",
            "sha256": sha256_bytes(product_manifest),
        },
    }


def write_artifacts(
    output_directory: Path,
    contract: DebianArtifactContract,
    package_bytes: bytes,
    evidence_bytes: bytes,
) -> tuple[Path, Path]:
    package_path = output_directory / contract.package_filename
    evidence_path = output_directory / contract.evidence_filename
    staged_package = output_directory / f".{contract.package_filename}.tmp"
    staged_evidence = output_directory / f".{contract.evidence_filename}.tmp"
    try:
        for path, value in (
            (staged_package, package_bytes),
            (staged_evidence, evidence_bytes),
        ):
            descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(descriptor, "wb") as output:
                output.write(value)
                output.flush()
                os.fsync(output.fileno())
            path.chmod(0o644)
        staged_package.rename(package_path)
        staged_evidence.rename(evidence_path)
        directory_descriptor = os.open(output_directory, os.O_RDONLY)
        try:
            os.fsync(directory_descriptor)
        finally:
            os.close(directory_descriptor)
    except Exception:
        for path in (staged_package, staged_evidence, package_path, evidence_path):
            try:
                path.unlink()
            except FileNotFoundError:
                pass
        raise
    return package_path, evidence_path


def build(
    product_rootfs: Path,
    shlibs_dependencies_path: Path,
    output_directory: Path,
) -> tuple[Path, Path]:
    metadata, layout = product_metadata.validate_source_contract()
    contract = DebianArtifactContract.load(metadata)
    try:
        product_rootfs = rootfs_contract.require_canonical_input(
            product_rootfs, "Linux product rootfs", directory=True
        )
        rootfs_contract.verify(product_rootfs)
    except rootfs_contract.LinuxRootfsError as exc:
        raise DebianArtifactError(str(exc)) from exc
    output_directory = require_output_directory(output_directory)
    shlibs_dependencies = load_shlibs_dependencies(shlibs_dependencies_path)
    data_tar, md5sums = build_data_tar(product_rootfs)
    installed_size = installed_size_kib(product_rootfs)
    control = render_binary_control(metadata, shlibs_dependencies, installed_size)
    control_tar = build_control_tar(control, md5sums)
    members = [
        ("debian-binary", b"2.0\n"),
        ("control.tar", control_tar),
        ("data.tar", data_tar),
    ]
    package_bytes = build_ar(members)
    manifest_path = rootfs_contract.target_in_rootfs(
        product_rootfs, layout["paths"]["product_manifest"]
    )
    fields = parse_control(control, metadata)
    evidence = artifact_evidence(
        contract,
        metadata,
        package_bytes,
        members,
        control,
        fields,
        md5sums,
        manifest_path.read_bytes(),
    )
    package_path, evidence_path = write_artifacts(
        output_directory,
        contract,
        package_bytes,
        canonical_json_bytes(evidence),
    )
    verify(package_path, evidence_path)
    return package_path, evidence_path


def verify(package_path: Path, evidence_path: Path) -> dict[str, Any]:
    metadata, layout = product_metadata.validate_source_contract()
    contract = DebianArtifactContract.load(metadata)
    package_path = require_canonical_file(package_path, "Debian package artifact")
    evidence_path = require_canonical_file(evidence_path, "Debian artifact evidence")
    if package_path.name != contract.package_filename:
        raise DebianArtifactError("Debian package filename differs from identity")
    if evidence_path.name != contract.evidence_filename:
        raise DebianArtifactError("Debian evidence filename differs from identity")
    if stat.S_IMODE(package_path.stat().st_mode) != 0o644:
        raise DebianArtifactError("Debian package artifact must use mode 0644")
    if stat.S_IMODE(evidence_path.stat().st_mode) != 0o644:
        raise DebianArtifactError("Debian artifact evidence must use mode 0644")
    if package_path.stat().st_size > MAX_PACKAGE_SIZE:
        raise DebianArtifactError("Debian package artifact exceeds the size limit")
    if evidence_path.stat().st_size > MAX_EVIDENCE_SIZE:
        raise DebianArtifactError("Debian artifact evidence exceeds the size limit")
    package_bytes = package_path.read_bytes()
    ar_members = parse_ar(package_bytes)
    expected_names = ["debian-binary", "control.tar", "data.tar"]
    if [member.name for member in ar_members] != expected_names:
        raise DebianArtifactError("Debian ar member inventory or order is invalid")
    if ar_members[0].data != b"2.0\n":
        raise DebianArtifactError("Debian binary format marker is invalid")
    control, md5sums = control_tar_values(ar_members[1].data)
    fields = parse_control(control, metadata)
    dependencies = parse_dependency_list(fields["Depends"], "binary control Depends")
    fixed = fixed_dependencies(metadata)
    if len(dependencies) <= len(fixed) or dependencies[-len(fixed) :] != fixed:
        raise DebianArtifactError("binary control fixed dependencies have drifted")
    shlibs_dependencies = dependencies[: -len(fixed)]
    if tuple(sorted(shlibs_dependencies, key=dependency_package_name)) != shlibs_dependencies:
        raise DebianArtifactError("binary control shlibs dependencies are not canonical")

    temporary_root = Path(tempfile.gettempdir()).resolve()
    with tempfile.TemporaryDirectory(
        prefix="radishlex-deb-verify.", dir=temporary_root
    ) as temporary:
        extracted_rootfs = Path(temporary) / "rootfs"
        extracted_rootfs.mkdir(mode=0o755)
        extract_data_tar(ar_members[2].data, extracted_rootfs)
        try:
            rootfs_contract.verify(extracted_rootfs)
        except rootfs_contract.LinuxRootfsError as exc:
            raise DebianArtifactError(str(exc)) from exc
        rebuilt_data, rebuilt_md5sums = build_data_tar(extracted_rootfs)
        if rebuilt_md5sums != md5sums:
            raise DebianArtifactError("Debian md5sums differs from the payload")
        installed_size = installed_size_kib(extracted_rootfs)
        if int(fields["Installed-Size"]) != installed_size:
            raise DebianArtifactError("Debian Installed-Size differs from payload")
        rebuilt_control = render_binary_control(
            metadata, shlibs_dependencies, installed_size
        )
        rebuilt_control_tar = build_control_tar(rebuilt_control, rebuilt_md5sums)
        rebuilt_members = [
            ("debian-binary", b"2.0\n"),
            ("control.tar", rebuilt_control_tar),
            ("data.tar", rebuilt_data),
        ]
        if build_ar(rebuilt_members) != package_bytes:
            raise DebianArtifactError("Debian artifact bytes are not canonical")
        manifest_path = rootfs_contract.target_in_rootfs(
            extracted_rootfs, layout["paths"]["product_manifest"]
        )
        expected_evidence = artifact_evidence(
            contract,
            metadata,
            package_bytes,
            rebuilt_members,
            rebuilt_control,
            fields,
            rebuilt_md5sums,
            manifest_path.read_bytes(),
        )

    try:
        actual_evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
    except Exception as exc:
        raise DebianArtifactError(f"invalid Debian artifact evidence: {exc}") from exc
    if not isinstance(actual_evidence, dict) or set(actual_evidence) != EVIDENCE_KEYS:
        raise DebianArtifactError("Debian artifact evidence fields differ from format v1")
    if actual_evidence != expected_evidence:
        raise DebianArtifactError("Debian artifact evidence does not match package")
    if evidence_path.read_bytes() != canonical_json_bytes(actual_evidence):
        raise DebianArtifactError("Debian artifact evidence is not canonical JSON")
    return actual_evidence


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build or verify a deterministic RadishLex Debian artifact."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    build_parser = subparsers.add_parser("build")
    build_parser.add_argument("--rootfs", type=Path, required=True)
    build_parser.add_argument("--shlibs-depends", type=Path, required=True)
    build_parser.add_argument("--output-dir", type=Path, required=True)
    verify_parser = subparsers.add_parser("verify")
    verify_parser.add_argument("--package", type=Path, required=True)
    verify_parser.add_argument("--evidence", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.command == "build":
            package_path, evidence_path = build(
                args.rootfs, args.shlibs_depends, args.output_dir
            )
            print(package_path)
            print(evidence_path)
        else:
            verify(args.package, args.evidence)
    except (
        DebianArtifactError,
        product_metadata.LinuxProductMetadataError,
        product_metadata.product_data.ProductDataError,
    ) as exc:
        raise SystemExit(str(exc)) from exc
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

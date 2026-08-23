#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import stat
import subprocess
import sys
import tarfile
from dataclasses import dataclass
from pathlib import Path
from typing import BinaryIO

import deb_artifact
import l6_guest_case_contract as guest_case_contract
import l6_release_pair
import l6_utm_guest_network_ready as network_ready
import l6_utm_start_once as start_control
import l6_v4_canonical_input_install as guest_installer
from l6_maintenance_refresh_io import (
    L6MaintenanceRefreshError,
    fsync_directory,
    rename_directory_no_replace,
    require_absent_output,
    write_exclusive_file,
)


EVIDENCE_FORMAT = "radishlex-linux-l6-v4-canonical-input-bundle-v1"
REPO_ROOT = Path(__file__).resolve().parents[2]
GENERATOR_RELATIVE_PATH = Path(
    "scripts/linux-product/l6_v4_canonical_input_bundle.py"
)
HANDOFF_ROOT = Path(
    "/Users/luobo/VirtualMachines/RadishLex-L6-Handoff-d75818f/"
    "release-pair-d75818f"
)
CASE_TEMPLATE_ROOT = Path(
    "/Users/luobo/VirtualMachines/"
    "RadishLex-L6-Crash-Install-Prepared-d75818f-v3-Input-Preflight-Ready"
)
CLONE_PREPARED_ROOT = Path(
    "/Users/luobo/VirtualMachines/"
    "RadishLex-L6-Crash-Install-Artifacts-Staged-d75818f-v4-Clone-Prepared"
)
HANDOFF_RECORD_SHA256 = (
    "c74fac1217fd0716a424e1b53c6c3f042131346bd566b66114d4cca342d49849"
)
CASE_TEMPLATE_MANIFEST_SHA256 = (
    "0b703d1060be9736bc92d88bab660dc61a50e2a5800f44c49c8c84c1e6455027"
)
CLONE_PREPARED_MANIFEST_SHA256 = (
    "c40c55d9fa2c62d5ceabd6894612a31d4ea9eb18666d808602d515d5f867144b"
)
CASE_TEMPLATE_SHA256 = (
    "ddfa7face6b2fa530668f103361476111c12b563fcd9bb13da15dd65c2f98c7e"
)
STARTUP_TEMPLATE_SHA256 = (
    "0dcaef674b0e0eb43b2ff9588131e1a15a162c304500348f817bca22d54d8cb2"
)
RENDERED_CASE_SHA256 = (
    "30e7f0e0dc7202d32592c5b1e003d995d8063a67c278e82469eb39d33e54588d"
)
STARTUP_FFI_SHA256 = (
    "193145352256ba9921a61988368005c213423132f8fb3ae2635f58b366adf27d"
)
TARGET_FFI_PACKAGE_PATH = (
    "./usr/lib/aarch64-linux-gnu/radishlex/manager/lib/"
    "libradishlex_ime_ffi.so"
)
TARGET_FFI_ARCHIVE_PATH = guest_case_contract.STARTUP_FFI_PATH
TARGET_UUID = "50B75F88-493D-42C0-A1DC-054DEC478038"
TARGET_NAME = (
    "RadishLex-Debian13-ARM64-L6-d75818f-"
    "crash-install-artifacts-staged-v4"
)
SOURCE_PACKAGE = "radishlex_26.7.1+38-1_arm64.deb"
TARGET_PACKAGE = "radishlex_26.7.1+38-2_arm64.deb"
SOURCE_IDENTITIES = {
    "build-environment.json": (0o644, 514, "c5fb0d2cd960cc3d4e9879f2c484721947cf5bdfa05a8d904a804dbcbdb396de"),
    "radishlex-linux-l6-acceptance": (0o755, 1_981_296, "5ca804e65941e4ba975fe758f65f61ad2c6adcd479a64e67d61c3e2e9aa3c7a6"),
    "radishlex-linux-maintenance": (0o755, 1_787_048, "422a5080b30fea3e4fa114674a9ec4b2636149779361e9628142bcc14c7457f3"),
    "release-pair.evidence.json": (0o644, 4_470, HANDOFF_RECORD_SHA256),
    f"source/artifacts/{SOURCE_PACKAGE}": (0o644, 40_806_592, "09ed122804b11767b8ac7cd69c323c1f6eef511fd6ae7284d75756fb60569bec"),
    f"source/artifacts/{SOURCE_PACKAGE}.evidence.json": (0o644, 2_376, "fe3d6297c08dccd8cacba13d50aa44dbb1c94b0ca8c2ab4df3a5c605466fcf94"),
    f"target/artifacts/{TARGET_PACKAGE}": (0o644, 40_806_592, "4dd0054051612654940a5973cc8a465be9b598a40a6fc036c7b263088aa5dcec"),
    f"target/artifacts/{TARGET_PACKAGE}.evidence.json": (0o644, 2_376, "9e646c86c33bc5027b82c8af0b659df7515722f66ffa92be09420f9974f63f17"),
}
CASE_REPLACEMENTS = (
    (b"install_prepared", b"install_artifacts_staged", 14),
    (b"install-prepared", b"install-artifacts-staged", 35),
    (
        b'.checkpoint == "prepared"',
        b'.checkpoint == "artifacts_staged"',
        1,
    ),
    (b'.state == "prepared"', b'.state == "artifacts_staged"', 1),
    (
        b"receipt=install|not_applicable|prepared|chain-1",
        b"receipt=install|not_applicable|artifacts_staged|chain-1",
        1,
    ),
)
CLONE_PREPARED_FIELDS = {
    "clone_manifest_sha256": "f76d1943e4af0f21f2fa9f93481b4d7e77112fb9f14bb64400f71f9457cff6e2",
    "control_script_sha256": "9afe32b7bb605d99d5c986e719bbb239c12fb7afc63fae6e93e885aa843b73e5",
    "repository_head": "e4ca1bd0fbad38d1813643a31a606ffd1b3c0fd9",
    "source_config_sha256": "06d91dfe81672ab56bf0607fddf9836cf05647904c5df12399801a81637a1568",
    "source_efi_sha256": "0b797641e80ea0314017006730332811a44d0170dd334910fb52088b5b521418",
    "source_qcow2_sha256": "4967234b09963df4f6bcd6d44f906c66d18e782092e727a0a220381b580e4b18",
    "target_config_sha256": "2de7280bbf906a8e899662c8686cb108755c83263150f5bd455490b050536195",
    "target_efi_sha256": "0b797641e80ea0314017006730332811a44d0170dd334910fb52088b5b521418",
    "target_name": TARGET_NAME,
    "target_network": "[]",
    "target_qcow2_sha256": "4967234b09963df4f6bcd6d44f906c66d18e782092e727a0a220381b580e4b18",
    "target_qcow2_second_sha256": "4967234b09963df4f6bcd6d44f906c66d18e782092e727a0a220381b580e4b18",
    "target_uuid": TARGET_UUID,
}


class CanonicalInputBundleError(ValueError):
    pass


@dataclass(frozen=True)
class CanonicalInputBundleRequest:
    repository_root: Path
    expected_repository_head: str
    output_root: Path
    authorized_host_input_bundle_generation: bool
    authorized_exact_frozen_sources: bool
    authorized_create_new_freeze: bool
    authorized_no_guest_vm_or_system_mutation: bool

    @property
    def staging_root(self) -> Path:
        return self.output_root.parent / f".{self.output_root.name}.incoming"

    def validate(self) -> None:
        for path, label in (
            (self.repository_root, "repository-root"),
            (self.output_root, "output-root"),
        ):
            if not path.is_absolute() or ".." in path.parts:
                raise CanonicalInputBundleError(
                    f"{label}-must-be-absolute-normalized"
                )
        if self.repository_root.resolve() != REPO_ROOT:
            raise CanonicalInputBundleError("repository-root-mismatch")
        if not start_control.HEX_40.fullmatch(
            self.expected_repository_head
        ):
            raise CanonicalInputBundleError("expected-repository-head-invalid")
        for source, label in (
            (self.repository_root, "repository"),
            (HANDOFF_ROOT, "handoff"),
            (CASE_TEMPLATE_ROOT, "case-template"),
            (CLONE_PREPARED_ROOT, "clone-prepared"),
        ):
            if _paths_overlap(self.output_root, source):
                raise CanonicalInputBundleError(
                    f"output-root-must-not-overlap-{label}"
                )
        if not self.authorized_host_input_bundle_generation:
            raise CanonicalInputBundleError(
                "authorized-host-input-bundle-generation-required"
            )
        if not self.authorized_exact_frozen_sources:
            raise CanonicalInputBundleError(
                "authorized-exact-frozen-sources-required"
            )
        if not self.authorized_create_new_freeze:
            raise CanonicalInputBundleError(
                "authorized-create-new-freeze-required"
            )
        if not self.authorized_no_guest_vm_or_system_mutation:
            raise CanonicalInputBundleError(
                "authorized-no-guest-vm-or-system-mutation-required"
            )


@dataclass
class OpenedSource:
    path: Path
    mode: int
    size: int
    sha256: str
    file_object: BinaryIO
    device: int
    inode: int

    def verify(self) -> None:
        info = os.fstat(self.file_object.fileno())
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_IMODE(info.st_mode) != self.mode
            or info.st_uid != os.getuid()
            or info.st_gid != os.getgid()
            or info.st_nlink != 1
            or info.st_size != self.size
            or info.st_dev != self.device
            or info.st_ino != self.inode
        ):
            raise CanonicalInputBundleError(
                f"source-file-identity-drift:{self.path}"
            )
        self.file_object.seek(0)
        digest = hashlib.sha256()
        while chunk := self.file_object.read(1024 * 1024):
            digest.update(chunk)
        if digest.hexdigest() != self.sha256:
            raise CanonicalInputBundleError(
                f"source-file-sha256-drift:{self.path}"
            )
        self.file_object.seek(0)

    def read_bytes(self) -> bytes:
        self.verify()
        data = self.file_object.read()
        self.file_object.seek(0)
        return data

    def close(self) -> None:
        self.file_object.close()


@dataclass(frozen=True)
class ArchivePayload:
    path: str
    mode: int
    size: int
    sha256: str
    source: OpenedSource | bytes

    def as_json(self) -> dict[str, object]:
        return {
            "mode": f"{self.mode:04o}",
            "path": self.path,
            "sha256": self.sha256,
            "size": self.size,
        }


@dataclass(frozen=True)
class CanonicalInputBundleResult:
    output_root: Path
    bundle_path: Path
    bundle_size: int
    bundle_sha256: str
    manifest_sha256: str
    inventory: tuple[dict[str, object], ...]


def render_case(template: bytes) -> bytes:
    if hashlib.sha256(template).hexdigest() != CASE_TEMPLATE_SHA256:
        raise CanonicalInputBundleError("case-template-sha256-mismatch")
    rendered = template
    for old, new, count in CASE_REPLACEMENTS:
        if rendered.count(old) != count:
            raise CanonicalInputBundleError(
                f"case-template-replacement-count-mismatch:{old!r}"
            )
        rendered = rendered.replace(old, new)
    if (
        b"install_prepared" in rendered
        or b"install-prepared" in rendered
        or hashlib.sha256(rendered).hexdigest() != RENDERED_CASE_SHA256
    ):
        raise CanonicalInputBundleError("rendered-case-contract-mismatch")
    dispatch = rendered.rsplit(b'case "$1" in\n', 1)
    if len(dispatch) != 2 or dispatch[1] != (
        b"  preflight) preflight ;;\n"
        b"  crash) crash ;;\n"
        b"  inspect-crash) inspect_crash ;;\n"
        b"  *) fail command ;;\n"
        b"esac\n"
    ):
        raise CanonicalInputBundleError("rendered-case-dispatch-mismatch")
    return rendered


def render_snapshot_identity(
    *, repository_head: str, generator_sha256: str
) -> bytes:
    fields = (
        (
            "format",
            "radishlex-linux-l6-install-artifacts-staged-"
            "snapshot-identity-d75818f-v4",
        ),
        ("bundle_generation_repository_head", repository_head),
        ("bundle_generator_sha256", generator_sha256),
        (
            "clone_prepared_repository_head",
            CLONE_PREPARED_FIELDS["repository_head"],
        ),
        (
            "clone_materialization_control_sha256",
            CLONE_PREPARED_FIELDS["control_script_sha256"],
        ),
        (
            "clone_once_manifest_sha256",
            CLONE_PREPARED_FIELDS["clone_manifest_sha256"],
        ),
        ("clone_prepared_manifest_sha256", CLONE_PREPARED_MANIFEST_SHA256),
        ("clone_uuid", TARGET_UUID),
        ("clone_name", TARGET_NAME),
        ("registration_source_uuid", "E671DB9C-5E2C-447B-9425-8D91D2CFD465"),
        (
            "registration_source",
            "stopped-reinstall-terminal|Network=[]|config-equal-except-name-uuid",
        ),
        ("disk_source", "Debian13-ARM64-DependencyFrozen|read-only"),
        ("source_config_sha256", CLONE_PREPARED_FIELDS["source_config_sha256"]),
        ("source_efi_sha256", CLONE_PREPARED_FIELDS["source_efi_sha256"]),
        ("source_qcow2_sha256", CLONE_PREPARED_FIELDS["source_qcow2_sha256"]),
        ("handoff_record_sha256", HANDOFF_RECORD_SHA256),
        ("clone_config_sha256", CLONE_PREPARED_FIELDS["target_config_sha256"]),
        ("clone_config_size", "3004"),
        ("clone_efi_sha256", CLONE_PREPARED_FIELDS["target_efi_sha256"]),
        ("clone_efi_size", "655360"),
        ("clone_qcow2_sha256", CLONE_PREPARED_FIELDS["target_qcow2_sha256"]),
        ("clone_qcow2_size", "10084679680"),
        ("network_collection", "[]"),
        ("disk_copy", "apfs-clonefile"),
        (
            "registered_vms_sha256",
            "0fda6e958cb106c9599e35457a02e1217f42bab63c840819e5495e11820b16c3",
        ),
        ("registered_vm_count", "21"),
        ("registered_vms", "all-stopped"),
        ("source_registration_and_clone_disk_handles", "0"),
        ("clone_start_command", "not-issued-at-clone-prepared-freeze"),
        ("input_transfer", "not-started-at-clone-prepared-freeze"),
        ("operation_id", "not-generated"),
        ("acceptance_maintenance_dpkg", "not-run"),
        ("snapshot_identity", "passed"),
    )
    return "".join(f"{key}={value}\n" for key, value in fields).encode(
        "ascii"
    )


def build_canonical_archive(
    output_path: Path, payloads: tuple[ArchivePayload, ...]
) -> None:
    if tuple(payload.path for payload in payloads) != guest_installer.CANONICAL_INVENTORY:
        raise CanonicalInputBundleError("archive-inventory-contract-mismatch")
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    descriptor = os.open(output_path, flags, 0o600)
    try:
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "wb", closefd=False) as output:
            with tarfile.open(
                fileobj=output, mode="w:", format=tarfile.USTAR_FORMAT
            ) as archive:
                for payload in payloads:
                    member = tarfile.TarInfo(payload.path)
                    member.mode = payload.mode
                    member.uid = 0
                    member.gid = 0
                    member.uname = "root"
                    member.gname = "root"
                    member.mtime = 0
                    member.size = payload.size
                    if isinstance(payload.source, bytes):
                        source: BinaryIO = io.BytesIO(payload.source)
                    else:
                        payload.source.verify()
                        source = payload.source.file_object
                    archive.addfile(member, source)
                output.flush()
                os.fsync(output.fileno())
    finally:
        os.close(descriptor)


def inspect_bundle(path: Path) -> tuple[guest_installer.ArchiveMember, ...]:
    with path.open("rb") as archive:
        return guest_installer.inspect_canonical_archive(
            archive, identity_uid=0, identity_gid=0
        )


def build_bundle(request: CanonicalInputBundleRequest) -> CanonicalInputBundleResult:
    request.validate()
    _require_clean_committed_repository(request)
    _require_directory(HANDOFF_ROOT, 0o755, "handoff-root")
    _require_directory(CASE_TEMPLATE_ROOT, 0o700, "case-template-root")
    _require_directory(CLONE_PREPARED_ROOT, 0o700, "clone-prepared-root")
    _verify_manifest_root(
        CASE_TEMPLATE_ROOT,
        CASE_TEMPLATE_MANIFEST_SHA256,
        expected_entries=64,
        label="case-template",
    )
    _verify_manifest_root(
        CLONE_PREPARED_ROOT,
        CLONE_PREPARED_MANIFEST_SHA256,
        expected_entries=7,
        label="clone-prepared",
    )
    _verify_clone_prepared_contract(CLONE_PREPARED_ROOT)
    _verify_handoff(HANDOFF_ROOT)

    sources: list[OpenedSource] = []
    try:
        handoff_sources: dict[str, OpenedSource] = {}
        for relative_path, identity in SOURCE_IDENTITIES.items():
            source = _open_exact_source(
                HANDOFF_ROOT / relative_path, *identity
            )
            sources.append(source)
            handoff_sources[relative_path] = source
        case_template = _open_exact_source(
            CASE_TEMPLATE_ROOT / "input-case.sh",
            0o600,
            33_069,
            CASE_TEMPLATE_SHA256,
        )
        startup = _open_exact_source(
            CASE_TEMPLATE_ROOT / "startup.py",
            0o600,
            1_675,
            STARTUP_TEMPLATE_SHA256,
        )
        sources.extend((case_template, startup))
        rendered_case = render_case(case_template.read_bytes())
        generator_sha256 = network_ready._sha256_file(
            request.repository_root / GENERATOR_RELATIVE_PATH
        )
        snapshot_identity = render_snapshot_identity(
            repository_head=request.expected_repository_head,
            generator_sha256=generator_sha256,
        )
        target_ffi = _extract_target_startup_ffi(
            handoff_sources[
                f"target/artifacts/{TARGET_PACKAGE}"
            ].read_bytes()
        )
        payloads = _archive_payloads(
            handoff_sources,
            rendered_case=rendered_case,
            snapshot_identity=snapshot_identity,
            startup=startup,
            target_ffi=target_ffi,
        )

        try:
            require_absent_output(request.output_root, "output root")
            require_absent_output(request.staging_root, "staging root")
        except L6MaintenanceRefreshError as exc:
            raise CanonicalInputBundleError(str(exc)) from exc
        request.staging_root.mkdir(mode=0o700)
        os.chmod(request.staging_root, 0o700)
        fsync_directory(request.output_root.parent)

        request_payload = {
            "authorization": {
                "create_new_freeze": True,
                "exact_frozen_sources": True,
                "host_input_bundle_generation": True,
                "no_guest_vm_or_system_mutation": True,
            },
            "case_template_manifest_sha256": CASE_TEMPLATE_MANIFEST_SHA256,
            "clone_prepared_manifest_sha256": CLONE_PREPARED_MANIFEST_SHA256,
            "expected_repository_head": request.expected_repository_head,
            "format": EVIDENCE_FORMAT,
            "handoff_record_sha256": HANDOFF_RECORD_SHA256,
            "output_root": str(request.output_root),
            "source_roots": {
                "case_template": str(CASE_TEMPLATE_ROOT),
                "clone_prepared": str(CLONE_PREPARED_ROOT),
                "handoff": str(HANDOFF_ROOT),
            },
            "target_name": TARGET_NAME,
            "target_uuid": TARGET_UUID,
        }
        write_exclusive_file(
            request.staging_root / "request.json",
            _canonical_json_bytes(request_payload),
            0o600,
        )
        write_exclusive_file(
            request.staging_root / "case.sh", rendered_case, 0o600
        )
        write_exclusive_file(
            request.staging_root / "snapshot-identity.evidence.txt",
            snapshot_identity,
            0o600,
        )
        _check_shell_syntax(request.staging_root / "case.sh")
        bundle_path = request.staging_root / "canonical-input.ustar"
        build_canonical_archive(bundle_path, payloads)
        observed = inspect_bundle(bundle_path)
        expected_inventory = tuple(payload.as_json() for payload in payloads)
        if tuple(member.as_json() for member in observed) != expected_inventory:
            raise CanonicalInputBundleError("archive-readback-identity-mismatch")
        write_exclusive_file(
            request.staging_root / "bundle-inventory.json",
            _canonical_json_bytes(
                {
                    "format": EVIDENCE_FORMAT,
                    "inventory": list(expected_inventory),
                    "inventory_count": len(expected_inventory),
                    "order": guest_case_contract.INPUT_INVENTORY_ORDER,
                }
            ),
            0o600,
        )
        bundle_size = bundle_path.stat().st_size
        bundle_sha256 = _sha256_file(bundle_path)
        terminal = {
            "automatic_retry": "not-performed",
            "automatic_stop": "not-performed",
            "bundle_path": str(request.output_root / bundle_path.name),
            "bundle_sha256": bundle_sha256,
            "bundle_size": bundle_size,
            "format": EVIDENCE_FORMAT,
            "guest_entry": "not-performed",
            "input_transfer": "not-performed",
            "operation_id": "not-generated",
            "outcome": "frozen",
            "system_mutation": "not-performed",
            "transaction": "not-performed",
            "utmctl_invocations": 0,
            "vm_start_stop": "not-performed",
        }
        write_exclusive_file(
            request.staging_root / "terminal.json",
            _canonical_json_bytes(terminal),
            0o600,
        )
        for source in sources:
            source.verify()
        manifest_bytes = _manifest_bytes(request.staging_root)
        write_exclusive_file(
            request.staging_root / "files.sha256", manifest_bytes, 0o600
        )
        fsync_directory(request.staging_root)
        try:
            rename_directory_no_replace(
                request.staging_root, request.output_root
            )
        except L6MaintenanceRefreshError as exc:
            raise CanonicalInputBundleError(str(exc)) from exc
        fsync_directory(request.output_root.parent)
        manifest_sha256 = _verify_frozen_output(request.output_root)
        return CanonicalInputBundleResult(
            output_root=request.output_root,
            bundle_path=request.output_root / "canonical-input.ustar",
            bundle_size=bundle_size,
            bundle_sha256=bundle_sha256,
            manifest_sha256=manifest_sha256,
            inventory=expected_inventory,
        )
    finally:
        for source in sources:
            source.close()


def _archive_payloads(
    handoff_sources: dict[str, OpenedSource],
    *,
    rendered_case: bytes,
    snapshot_identity: bytes,
    startup: OpenedSource,
    target_ffi: bytes,
) -> tuple[ArchivePayload, ...]:
    generated = {
        "case.sh": (0o600, rendered_case),
        "snapshot-identity.evidence.txt": (0o600, snapshot_identity),
        "startup.py": (0o600, startup),
        TARGET_FFI_ARCHIVE_PATH: (0o600, target_ffi),
    }
    payloads: list[ArchivePayload] = []
    for path in guest_case_contract.canonical_input_inventory(
        SOURCE_PACKAGE, TARGET_PACKAGE
    ):
        generated_value = generated.get(path)
        if generated_value is not None:
            mode, value = generated_value
            if isinstance(value, OpenedSource):
                payloads.append(
                    ArchivePayload(path, mode, value.size, value.sha256, value)
                )
            else:
                payloads.append(
                    ArchivePayload(
                        path,
                        mode,
                        len(value),
                        hashlib.sha256(value).hexdigest(),
                        value,
                    )
                )
            continue
        source = handoff_sources.get(path)
        if source is None:
            raise CanonicalInputBundleError(
                f"archive-source-missing:{path}"
            )
        payloads.append(
            ArchivePayload(path, source.mode, source.size, source.sha256, source)
        )
    return tuple(payloads)


def _extract_target_startup_ffi(package: bytes) -> bytes:
    members = deb_artifact.parse_ar(package)
    if [member.name for member in members] != [
        "debian-binary",
        "control.tar",
        "data.tar",
    ]:
        raise CanonicalInputBundleError("target-package-ar-inventory-mismatch")
    data_tar = members[2].data
    matches = [
        (member, data)
        for member, data in deb_artifact.read_tar(data_tar, "data.tar")
        if member.name == TARGET_FFI_PACKAGE_PATH
    ]
    if len(matches) != 1 or matches[0][1] is None:
        raise CanonicalInputBundleError("target-startup-ffi-inventory-mismatch")
    member, data = matches[0]
    if (
        member.mode != 0o644
        or len(data) != 7_383_672
        or hashlib.sha256(data).hexdigest() != STARTUP_FFI_SHA256
    ):
        raise CanonicalInputBundleError("target-startup-ffi-identity-mismatch")
    return data


def _verify_handoff(root: Path) -> None:
    evidence_path = root / "release-pair.evidence.json"
    if _sha256_file(evidence_path) != HANDOFF_RECORD_SHA256:
        raise CanonicalInputBundleError("handoff-record-sha256-mismatch")
    contract = l6_release_pair.load_contract()
    record = l6_release_pair.load_json(
        evidence_path, "L6 release pair evidence", canonical=True
    )
    l6_release_pair.verify_published_record(evidence_path, record, contract)


def _verify_clone_prepared_contract(root: Path) -> None:
    preflight = _parse_evidence(root / "preflight.evidence.txt")
    postverify = _parse_evidence(root / "postverify.evidence.txt")
    for label, evidence in (("preflight", preflight), ("postverify", postverify)):
        for key, expected in CLONE_PREPARED_FIELDS.items():
            if key == "target_qcow2_sha256" and label == "preflight":
                actual = evidence.get("source_qcow2_sha256")
            elif key == "target_qcow2_second_sha256" and label == "preflight":
                continue
            elif key == "target_efi_sha256" and label == "preflight":
                actual = evidence.get("source_efi_sha256")
            else:
                actual = evidence.get(key)
            if actual != expected:
                raise CanonicalInputBundleError(
                    f"clone-prepared-{label}-field-mismatch:{key}"
                )
    exact_post = {
        "automatic_delete": "not-performed",
        "automatic_retry": "not-performed",
        "automatic_start": "not-performed",
        "guest_exec": "not-performed",
        "input_transfer": "not-performed",
        "operation_id": "not-generated",
        "postverify": "passed",
        "registered_vm_count": "21",
        "registered_vms": "all-stopped",
        "replacement_count": "2",
        "source_registration_target_handles": "0",
        "transaction": "not-performed",
    }
    for key, expected in exact_post.items():
        if postverify.get(key) != expected:
            raise CanonicalInputBundleError(
                f"clone-prepared-postverify-field-mismatch:{key}"
            )


def _require_clean_committed_repository(
    request: CanonicalInputBundleRequest,
) -> None:
    head = start_control._run_git(
        request.repository_root, ("rev-parse", "HEAD")
    ).decode("ascii").strip()
    if head != request.expected_repository_head:
        raise CanonicalInputBundleError("repository-head-drift")
    if start_control._run_git(
        request.repository_root, ("status", "--porcelain")
    ):
        raise CanonicalInputBundleError("repository-not-clean")
    for relative, label in (
        (GENERATOR_RELATIVE_PATH, "generator"),
        (
            Path("scripts/linux-product/l6_guest_case_contract.py"),
            "guest-case-contract",
        ),
        (
            Path("scripts/linux-product/l6_v4_canonical_input_install.py"),
            "guest-installer",
        ),
        (Path("scripts/linux-product/deb_artifact.py"), "deb-artifact"),
    ):
        network_ready._require_committed_regular(
            request.repository_root / relative, label
        )


def _open_exact_source(
    path: Path, mode: int, size: int, sha256: str
) -> OpenedSource:
    flags = os.O_RDONLY | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags)
    except OSError as exc:
        raise CanonicalInputBundleError(
            f"cannot-open-source-file:{path}"
        ) from exc
    file_object = os.fdopen(descriptor, "rb")
    info = os.fstat(descriptor)
    opened = OpenedSource(
        path=path,
        mode=mode,
        size=size,
        sha256=sha256,
        file_object=file_object,
        device=info.st_dev,
        inode=info.st_ino,
    )
    try:
        opened.verify()
    except Exception:
        opened.close()
        raise
    return opened


def _require_directory(path: Path, mode: int, label: str) -> None:
    try:
        info = path.lstat()
    except OSError as exc:
        raise CanonicalInputBundleError(f"{label}-unavailable") from exc
    if (
        not stat.S_ISDIR(info.st_mode)
        or stat.S_ISLNK(info.st_mode)
        or stat.S_IMODE(info.st_mode) != mode
        or info.st_uid != os.getuid()
        or info.st_gid != os.getgid()
    ):
        raise CanonicalInputBundleError(f"{label}-identity-mismatch")


def _verify_manifest_root(
    root: Path,
    manifest_sha256: str,
    *,
    expected_entries: int,
    label: str,
) -> None:
    manifest = root / "files.sha256"
    info = manifest.lstat()
    if (
        not stat.S_ISREG(info.st_mode)
        or stat.S_IMODE(info.st_mode) != 0o600
        or info.st_uid != os.getuid()
        or info.st_gid != os.getgid()
        or info.st_nlink != 1
        or _sha256_file(manifest) != manifest_sha256
    ):
        raise CanonicalInputBundleError(f"{label}-manifest-identity-mismatch")
    if start_control._verify_sha256_manifest(root, manifest) != expected_entries:
        raise CanonicalInputBundleError(f"{label}-manifest-entry-count-mismatch")


def _parse_evidence(path: Path) -> dict[str, str]:
    try:
        raw = path.read_bytes()
        text = raw.decode("ascii")
    except (OSError, UnicodeDecodeError) as exc:
        raise CanonicalInputBundleError("clone-prepared-evidence-invalid") from exc
    if not text.endswith("\n") or "\r" in text or "\x00" in text:
        raise CanonicalInputBundleError("clone-prepared-evidence-not-canonical")
    fields: dict[str, str] = {}
    for line in text.splitlines():
        if line.count("=") != 1:
            raise CanonicalInputBundleError("clone-prepared-evidence-line-invalid")
        key, value = line.split("=", 1)
        if not key or key in fields:
            raise CanonicalInputBundleError("clone-prepared-evidence-field-invalid")
        fields[key] = value
    return fields


def _check_shell_syntax(path: Path) -> None:
    try:
        subprocess.run(
            ["/bin/sh", "-n", str(path)],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            env={"LC_ALL": "C", "PATH": "/usr/bin:/bin"},
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        raise CanonicalInputBundleError("rendered-case-shell-syntax-invalid") from exc


def _manifest_bytes(root: Path) -> bytes:
    lines = []
    for path in sorted(root.iterdir(), key=lambda item: item.name.encode("utf-8")):
        if path.name == "files.sha256":
            continue
        info = path.lstat()
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_uid != os.getuid()
            or info.st_gid != os.getgid()
            or info.st_nlink != 1
        ):
            raise CanonicalInputBundleError("freeze-file-identity-mismatch")
        lines.append(f"{_sha256_file(path)}  {path.name}\n")
    return "".join(lines).encode("ascii")


def _verify_frozen_output(root: Path) -> str:
    _require_directory(root, 0o700, "frozen-output-root")
    manifest = root / "files.sha256"
    expected_entries = 6
    if start_control._verify_sha256_manifest(root, manifest) != expected_entries:
        raise CanonicalInputBundleError("frozen-output-manifest-entry-count-mismatch")
    if _manifest_bytes(root) != manifest.read_bytes():
        raise CanonicalInputBundleError("frozen-output-manifest-readback-mismatch")
    for path in root.iterdir():
        info = path.lstat()
        if (
            not stat.S_ISREG(info.st_mode)
            or stat.S_IMODE(info.st_mode) != 0o600
            or info.st_uid != os.getuid()
            or info.st_gid != os.getgid()
            or info.st_nlink != 1
        ):
            raise CanonicalInputBundleError("frozen-output-file-identity-mismatch")
    inspect_bundle(root / "canonical-input.ustar")
    return _sha256_file(manifest)


def _canonical_json_bytes(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode("utf-8")


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _paths_overlap(left: Path, right: Path) -> bool:
    left = left.absolute()
    right = right.absolute()
    return left == right or left in right.parents or right in left.parents


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Build and create-new freeze the exact v4 install_artifacts_staged "
            "canonical host input bundle without entering the guest."
        )
    )
    parser.add_argument("command", choices=("build",))
    parser.add_argument("--repository-root", type=Path, required=True)
    parser.add_argument("--expected-repository-head", required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument(
        "--authorized-host-input-bundle-generation", action="store_true"
    )
    parser.add_argument("--authorized-exact-frozen-sources", action="store_true")
    parser.add_argument("--authorized-create-new-freeze", action="store_true")
    parser.add_argument(
        "--authorized-no-guest-vm-or-system-mutation", action="store_true"
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    request = CanonicalInputBundleRequest(
        repository_root=args.repository_root,
        expected_repository_head=args.expected_repository_head,
        output_root=args.output_root,
        authorized_host_input_bundle_generation=(
            args.authorized_host_input_bundle_generation
        ),
        authorized_exact_frozen_sources=args.authorized_exact_frozen_sources,
        authorized_create_new_freeze=args.authorized_create_new_freeze,
        authorized_no_guest_vm_or_system_mutation=(
            args.authorized_no_guest_vm_or_system_mutation
        ),
    )
    try:
        result = build_bundle(request)
    except (
        CanonicalInputBundleError,
        deb_artifact.DebianArtifactError,
        l6_release_pair.L6ReleasePairError,
        OSError,
    ) as exc:
        print(f"canonical_input_bundle_error={exc}", file=sys.stderr)
        return 1
    print("canonical_input_bundle_outcome=frozen")
    print(f"bundle_path={result.bundle_path}")
    print(f"bundle_size={result.bundle_size}")
    print(f"bundle_sha256={result.bundle_sha256}")
    print(f"manifest_sha256={result.manifest_sha256}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

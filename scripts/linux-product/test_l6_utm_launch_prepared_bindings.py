#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import plistlib
import tempfile
import unittest
import uuid
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_launch_transport_bindings as transport_bindings


TARGET_UUID = transport_bindings.REQUIRED_TARGET_UUID
TARGET_NAME = transport_bindings.REQUIRED_TARGET_NAME
V7_PEERS = tuple(
    (str(uuid.UUID(int=index)).upper(), f"Synthetic-Frozen-{index:02d}")
    for index in range(1, 20)
) + (
    (
        transport_bindings.FROZEN_V7_TARGET_UUID,
        transport_bindings.FROZEN_V7_TARGET_NAME,
    ),
)


class LinuxL6UtmLaunchPreparedBindingsTests(unittest.TestCase):
    def test_prepared_binding_requires_exact_semantics_and_v7_inventory(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_root = Path(temporary)
            prepared_root, manifest_sha256, _ = create_prepared_evidence(
                temporary_root
            )
            request = prepared_request(
                prepared_root, manifest_sha256, temporary_root
            )

            result = transport_bindings.validate_prepared_evidence(
                request, v7_identity()
            )

            self.assertEqual(result["prior_prepared_entries_verified"], 7)
            self.assertEqual(result["prepared_inventory_count"], 21)
            self.assertEqual(result["prior_prepared_outcome"], "passed")

    def test_prepared_binding_rejects_self_consistent_forbidden_action(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_root = Path(temporary)
            prepared_root, manifest_sha256, _ = create_prepared_evidence(
                temporary_root,
                postverify_overrides={"automatic_start": "performed"},
            )
            request = prepared_request(
                prepared_root, manifest_sha256, temporary_root
            )

            with self.assertRaisesRegex(
                transport_bindings.BindingError,
                "prior-prepared-postverify-automatic-start",
            ):
                transport_bindings.validate_prepared_evidence(
                    request, v7_identity()
                )

    def test_live_target_identity_hashes_opened_regular_files(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            temporary_root = Path(temporary)
            package_path, prepared_binding = create_live_target_package(
                temporary_root
            )
            request = prepared_request(
                temporary_root / "unused-prepared",
                "f" * 64,
                temporary_root,
                package_path=package_path,
            )
            with mock.patch.object(
                transport_bindings.prepared_bindings,
                "expected_target_package_path",
                return_value=package_path,
            ):
                identity = transport_bindings.validate_live_target_identity(
                    request, prepared_binding
                )
                self.assertEqual(
                    identity["target_qcow2"]["sha256"],
                    prepared_binding["prepared_target_qcow2_sha256"],
                )
                self.assertEqual(identity["target_package"]["mode"], "0755")
                self.assertEqual(identity["target_data"]["mode"], "0755")
                qcow2 = (
                    package_path
                    / "Data"
                    / transport_bindings.TARGET_DISK_IMAGE_NAME
                )
                qcow2.write_bytes(b"synthetic-qcow2-drift")
                with self.assertRaisesRegex(
                    transport_bindings.BindingError,
                    "target-qcow2-sha256-mismatch",
                ):
                    transport_bindings.validate_live_target_identity(
                        request, prepared_binding
                    )


def prepared_request(
    prepared_root: Path,
    manifest_sha256: str,
    temporary_root: Path,
    *,
    package_path: Path | None = None,
) -> SimpleNamespace:
    return SimpleNamespace(
        prior_prepared_root=prepared_root,
        prior_prepared_manifest_sha256=manifest_sha256,
        target_package_path=(
            package_path or temporary_root / f"{TARGET_NAME}.utm"
        ),
        target_uuid=TARGET_UUID,
        target_name=TARGET_NAME,
        expected_vm_count=21,
    )


def v7_identity() -> dict[str, object]:
    return {
        "baseline_inventory": [
            {"name": name, "status": "stopped", "uuid": vm_uuid}
            for vm_uuid, name in V7_PEERS
        ]
    }


def create_prepared_evidence(
    temporary_root: Path,
    *,
    postverify_overrides: dict[str, str] | None = None,
) -> tuple[Path, str, dict[str, str]]:
    root = temporary_root / "bound-prepared"
    root.mkdir(mode=0o700)
    config_payload = target_config_payload()
    config_sha256 = hashlib.sha256(config_payload).hexdigest()
    efi_sha256 = hashlib.sha256(b"synthetic-efi").hexdigest()
    qcow2_sha256 = hashlib.sha256(b"synthetic-qcow2").hexdigest()
    control_payload = b"#!/bin/sh\nexit 0\n"
    control_sha256 = hashlib.sha256(control_payload).hexdigest()
    inventory = V7_PEERS + ((TARGET_UUID, TARGET_NAME),)
    registered_payload = (
        "UUID Status Name\n"
        + "".join(
            f"{vm_uuid} stopped {name}\n" for vm_uuid, name in inventory
        )
    ).encode("utf-8")
    registered_sha256 = hashlib.sha256(registered_payload).hexdigest()
    source_config_sha256 = "3" * 64
    preflight = {
        "format": transport_bindings.PREPARED_PREFLIGHT_FORMAT,
        "repository_head": transport_bindings.REQUIRED_PREPARED_REPOSITORY_HEAD,
        "control_script_sha256": control_sha256,
        "clone_manifest_sha256": (
            transport_bindings.REQUIRED_CLONE_MANIFEST_SHA256
        ),
        "registered_vms_sha256": registered_sha256,
        "registered_vm_count": "21",
        "registered_vms": "all-stopped",
        "target_uuid": TARGET_UUID,
        "target_name": TARGET_NAME,
        "target_config_sha256": config_sha256,
        "target_efi_before_sha256": "1" * 64,
        "target_qcow2_before_sha256": "2" * 64,
        "source_config_sha256": source_config_sha256,
        "source_efi_sha256": efi_sha256,
        "source_qcow2_sha256": qcow2_sha256,
        "source_registration_target_handles": "0",
        "target_network": "[]",
        "materialization_authorized": "true",
        "no_automatic_start_retry_delete_authorized": "true",
        "preflight": "passed",
    }
    postverify = {
        "format": transport_bindings.PREPARED_EVIDENCE_FORMAT,
        "repository_head": transport_bindings.REQUIRED_PREPARED_REPOSITORY_HEAD,
        "control_script_sha256": control_sha256,
        "clone_manifest_sha256": (
            transport_bindings.REQUIRED_CLONE_MANIFEST_SHA256
        ),
        "target_uuid": TARGET_UUID,
        "target_name": TARGET_NAME,
        "target_config_sha256": config_sha256,
        "target_efi_sha256": efi_sha256,
        "target_qcow2_sha256": qcow2_sha256,
        "target_qcow2_second_sha256": qcow2_sha256,
        "source_config_sha256": source_config_sha256,
        "source_efi_sha256": efi_sha256,
        "source_qcow2_sha256": qcow2_sha256,
        "registered_vms_sha256": registered_sha256,
        "registered_vm_count": "21",
        "registered_vms": "all-stopped",
        "target_network": "[]",
        "source_registration_target_handles": "0",
        "replacement_count": "2",
        "materialization_authorized": "true",
        "no_automatic_start_retry_delete_authorized": "true",
        "automatic_retry": "not-performed",
        "automatic_delete": "not-performed",
        "automatic_start": "not-performed",
        "guest_exec": "not-performed",
        "input_transfer": "not-performed",
        "operation_id": "not-generated",
        "transaction": "not-performed",
        "postverify": "passed",
    }
    postverify.update(postverify_overrides or {})
    clone_check = "".join(
        f"{name}: OK\n"
        for name in (
            "request.json",
            "binding-preflight.json",
            "target-package-preclone.json",
            "utmctl-list-preclone.json",
            "utmctl-clone.json",
            "utmctl-list-terminal.json",
            "target-package-terminal.json",
            "terminal.json",
        )
    ).encode("ascii")
    values = {
        "clone-once-manifest-check.evidence.txt": clone_check,
        "materialize-control.sh": control_payload,
        "postverify.evidence.txt": key_value_payload(postverify),
        "preflight.evidence.txt": key_value_payload(preflight),
        "registered-vms.postmaterialize.evidence.txt": registered_payload,
        "registered-vms.prematerialize.evidence.txt": registered_payload,
        "target-config.plist": config_payload,
    }
    manifest_lines = []
    for name in sorted(values):
        path = root / name
        path.write_bytes(values[name])
        path.chmod(0o600)
        manifest_lines.append(
            f"{hashlib.sha256(values[name]).hexdigest()}  {name}\n"
        )
    manifest = root / "files.sha256"
    manifest.write_text("".join(manifest_lines), encoding="ascii")
    manifest.chmod(0o600)
    return (
        root,
        hashlib.sha256(manifest.read_bytes()).hexdigest(),
        {
            "config_sha256": config_sha256,
            "efi_sha256": efi_sha256,
            "qcow2_sha256": qcow2_sha256,
        },
    )


def create_live_target_package(
    temporary_root: Path,
) -> tuple[Path, dict[str, object]]:
    package_path = temporary_root / f"{TARGET_NAME}.utm"
    data_path = package_path / "Data"
    data_path.mkdir(parents=True)
    package_path.chmod(0o755)
    data_path.chmod(0o755)
    config_payload = target_config_payload()
    efi_payload = b"synthetic-efi"
    qcow2_payload = b"synthetic-qcow2"
    (package_path / "config.plist").write_bytes(config_payload)
    (data_path / "efi_vars.fd").write_bytes(efi_payload)
    (data_path / transport_bindings.TARGET_DISK_IMAGE_NAME).write_bytes(
        qcow2_payload
    )
    return (
        package_path,
        {
            "prepared_target_config_sha256": hashlib.sha256(
                config_payload
            ).hexdigest(),
            "prepared_target_efi_sha256": hashlib.sha256(
                efi_payload
            ).hexdigest(),
            "prepared_target_qcow2_sha256": hashlib.sha256(
                qcow2_payload
            ).hexdigest(),
        },
    )


def target_config_payload() -> bytes:
    return plistlib.dumps(
        {
            "Backend": "QEMU",
            "ConfigurationVersion": 4,
            "Drive": [
                {
                    "Identifier": transport_bindings.TARGET_DISK_IDENTIFIER,
                    "ImageName": transport_bindings.TARGET_DISK_IMAGE_NAME,
                    "ImageType": "Disk",
                    "Interface": "VirtIO",
                    "ReadOnly": False,
                }
            ],
            "Information": {"Name": TARGET_NAME, "UUID": TARGET_UUID},
            "Network": [],
            "System": {"Architecture": "aarch64", "Target": "virt"},
        },
        fmt=plistlib.FMT_XML,
        sort_keys=True,
    )


def key_value_payload(value: dict[str, str]) -> bytes:
    return "".join(f"{key}={item}\n" for key, item in value.items()).encode(
        "ascii"
    )


if __name__ == "__main__":
    unittest.main()

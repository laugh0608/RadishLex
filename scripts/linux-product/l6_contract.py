#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

from l6_guest_case_contract import (
    BUILD_ENVIRONMENT_FILENAME,
    GUEST_AGENT_TRUST_POLICY,
    INPUT_INVENTORY_ORDER,
    LinuxL6GuestCaseContractError,
    validate_crash_checkpoint_matrix,
)
from product_metadata import (
    PACKAGE_ROOT,
    LinuxProductMetadata,
    LinuxProductMetadataError,
)


MATRIX_PATH = PACKAGE_ROOT / "l6-matrix.json"


class LinuxL6ContractError(ValueError):
    pass


def canonical_json_bytes(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, indent=2, separators=(",", ": ")) + "\n"
    ).encode("utf-8")


def expected_matrix(metadata: LinuxProductMetadata) -> dict[str, Any]:
    return {
        "format_version": 1,
        "profile": "debian13-arm64-ephemeral-v1",
        "product": {
            "package_name": metadata.package_name,
            "distribution_identity": metadata.distribution_identity,
            "runtime_layout": metadata.runtime_layout,
            "data_layout": metadata.data_layout,
        },
        "guest": {
            "os_id": "debian",
            "version_id": metadata.debian_release,
            "version_codename": metadata.debian_codename,
            "architecture": metadata.debian_architecture,
            "multiarch_tuple": metadata.multiarch_tuple,
            "acceptance_user": "radishlex-l6",
            "acceptance_home": "/home/radishlex-l6",
            "initial_package_state": "not-installed",
            "initial_state_root": "absent",
            "clean_snapshot_required": True,
            "p04_guest_reuse": False,
        },
        "release_pair": {
            "strategy": "successive-debian-revisions-from-distinct-commits-v1",
            "source_role": "source",
            "target_role": "target",
            "product_version_equal": True,
            "build_number_equal": True,
            "data_contract_equal": True,
            "package_hash_distinct": True,
            "target_version_relation": "greater-than-source",
        },
        "operations": expected_operations(),
        "crash_scenarios": expected_crash_scenarios(),
        "probes": expected_probes(),
        "execution": {
            "operation_id_format": "lowercase-hex-32",
            "checkpoint_mechanism": "acceptance-checkpoint-process-group-stop-v1",
            "artifact_input_root": "/var/tmp/radishlex-l6-inputs",
            "input_inventory_order": INPUT_INVENTORY_ORDER,
            "build_environment_filename": BUILD_ENVIRONMENT_FILENAME,
            "guest_agent_trust_policy": GUEST_AGENT_TRUST_POLICY,
            "evidence_root": "/var/tmp/radishlex-l6-evidence",
            "state_root": "/var/lib/radishlex/install-v1",
            "one_mutation_per_authorization": True,
            "network_policy": "offline-after-dependency-freeze",
            "cleanup_policy": "preserve-until-separate-authorization",
            "evidence_format": "canonical-json-with-artifact-hashes-v1",
        },
    }


def expected_operations() -> list[dict[str, Any]]:
    return [
        operation(1, "install_source", "install", "absent", None, "source", "source"),
        operation(2, "upgrade_target", "upgrade", "source", "source", "target", "target"),
        operation(3, "repair_target", "repair", "target", None, "target", "target"),
        operation(4, "rollback_source", "rollback", "target", "target", "source", "source"),
        operation(5, "remove_source", "remove", "source", "source", None, "absent"),
        operation(6, "reinstall_target", "install", "absent", None, "target", "target"),
    ]


def operation(
    step: int,
    identifier: str,
    kind: str,
    installed_before: str,
    source: str | None,
    target: str | None,
    installed_after: str,
) -> dict[str, Any]:
    return {
        "step": step,
        "id": identifier,
        "kind": kind,
        "installed_before": installed_before,
        "source": source,
        "target": target,
        "installed_after": installed_after,
        "terminal": "completed",
    }


def expected_crash_scenarios() -> list[dict[str, Any]]:
    return [
        crash("install_prepared", "install_source", "prepared", "process_group_terminated", "completed"),
        crash(
            "install_artifacts_staged",
            "install_source",
            "artifacts_staged",
            "process_group_terminated",
            "completed",
        ),
        crash("upgrade_quiesced", "upgrade_target", "quiesced", "process_group_terminated", "completed"),
        crash(
            "upgrade_before_dpkg",
            "upgrade_target",
            "package_mutating_before_dpkg",
            "process_group_terminated",
            "completed",
        ),
        crash(
            "upgrade_after_dpkg",
            "upgrade_target",
            "target_applied_before_proof",
            "process_group_terminated",
            "completed",
        ),
        crash(
            "upgrade_rollback_required",
            "upgrade_target",
            "rollback_required",
            "target_validation_rejected_then_process_group_terminated",
            "rolled_back",
        ),
        crash(
            "upgrade_before_source_restore",
            "upgrade_target",
            "source_restoring_before_dpkg",
            "target_validation_rejected_then_process_group_terminated",
            "rolled_back",
        ),
        crash(
            "upgrade_after_source_restore",
            "upgrade_target",
            "source_applied_before_proof",
            "target_validation_rejected_then_process_group_terminated",
            "rolled_back",
        ),
    ]


def crash(
    identifier: str,
    operation_id: str,
    checkpoint: str,
    fault: str,
    expected_terminal: str,
) -> dict[str, Any]:
    return {
        "id": identifier,
        "operation": operation_id,
        "checkpoint": checkpoint,
        "fault": fault,
        "expected_terminal": expected_terminal,
        "restore_snapshot_after": True,
    }


def expected_probes() -> dict[str, Any]:
    return {
        "dependency_relationship": "all-evidence-control-depends-v1",
        "font_roles": [
            {
                "package": "fonts-dejavu-core",
                "family": "DejaVu Sans",
                "sample": "RadishLex ABI v9 12345",
                "owner_required": True,
            },
            {
                "package": "fonts-noto-cjk",
                "family": "Noto Sans CJK SC",
                "sample": "萝卜词核中文输入",
                "owner_required": True,
            },
        ],
        "startup_components": ["manager", "fcitx-addon"],
        "startup_cases": [
            "installed-allowed-product",
            "fresh-absent-failed-closed",
            "nonterminal-maintenance-required",
            "removed-failed-closed",
            "wrong-sibling-failed-closed",
            "preload-interposition-failed-closed",
        ],
        "xdg_paths": [
            "/home/radishlex-l6/.local/share/radishlex",
            "/home/radishlex-l6/.config/radishlex",
            "/home/radishlex-l6/.local/state/radishlex",
            "/home/radishlex-l6/.cache/radishlex",
            "/home/radishlex-l6/.config/fcitx5/profile",
        ],
        "xdg_fingerprint": "metadata-size-sha256-without-content-export-v1",
        "system_observations": [
            "os-release",
            "dpkg-status",
            "dpkg-audit",
            "product-inventory",
            "receipt-summary",
            "proc-maps",
            "font-family-glyph-owner",
        ],
    }


def load_matrix(path: Path = MATRIX_PATH) -> dict[str, Any]:
    try:
        if path.is_symlink() or not path.is_file():
            raise LinuxL6ContractError("L6 matrix must be a regular file")
        raw = path.read_bytes()
        value = json.loads(raw.decode("utf-8"))
    except LinuxL6ContractError:
        raise
    except Exception as exc:
        raise LinuxL6ContractError(f"cannot load L6 matrix: {exc}") from exc
    if not isinstance(value, dict):
        raise LinuxL6ContractError("L6 matrix root must be an object")
    if raw != canonical_json_bytes(value):
        raise LinuxL6ContractError("L6 matrix must use canonical JSON formatting")
    return value


def validate_matrix(path: Path = MATRIX_PATH) -> dict[str, Any]:
    try:
        metadata = LinuxProductMetadata.load(PACKAGE_ROOT / "product.json")
    except LinuxProductMetadataError as exc:
        raise LinuxL6ContractError(str(exc)) from exc
    actual = load_matrix(path)
    expected = expected_matrix(metadata)
    if actual != expected:
        raise LinuxL6ContractError("L6 matrix differs from the Debian 13 ARM64 contract")
    validate_semantics(actual)
    return actual


def validate_semantics(matrix: dict[str, Any]) -> None:
    operations = matrix["operations"]
    installed = "absent"
    operation_ids: set[str] = set()
    for index, item in enumerate(operations, start=1):
        if item["step"] != index or item["id"] in operation_ids:
            raise LinuxL6ContractError("L6 operation steps or identities are invalid")
        operation_ids.add(item["id"])
        if item["installed_before"] != installed:
            raise LinuxL6ContractError("L6 operation chain is discontinuous")
        installed = item["installed_after"]
    if installed != "target":
        raise LinuxL6ContractError("L6 primary sequence must finish on the target release")

    checkpoints: set[str] = set()
    for scenario in matrix["crash_scenarios"]:
        if scenario["operation"] not in operation_ids:
            raise LinuxL6ContractError("L6 crash scenario references an unknown operation")
        if scenario["checkpoint"] in checkpoints:
            raise LinuxL6ContractError("L6 crash checkpoints must be unique")
        checkpoints.add(scenario["checkpoint"])
        if not scenario["restore_snapshot_after"]:
            raise LinuxL6ContractError("every L6 crash scenario must restore its snapshot")
    try:
        validate_crash_checkpoint_matrix(matrix["crash_scenarios"])
    except LinuxL6GuestCaseContractError as exc:
        raise LinuxL6ContractError(str(exc)) from exc

    xdg_paths = matrix["probes"]["xdg_paths"]
    acceptance_home = matrix["guest"]["acceptance_home"] + "/"
    if any(not path.startswith(acceptance_home) for path in xdg_paths):
        raise LinuxL6ContractError("L6 XDG probes must stay in the acceptance user home")
    if matrix["guest"]["p04_guest_reuse"]:
        raise LinuxL6ContractError("the frozen P04 guest cannot be reused for L6")
    if not matrix["execution"]["one_mutation_per_authorization"]:
        raise LinuxL6ContractError("L6 requires one system mutation per authorization")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate the RadishLex Linux L6 acceptance matrix."
    )
    parser.add_argument("command", choices=("validate",))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.command == "validate":
        validate_matrix()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

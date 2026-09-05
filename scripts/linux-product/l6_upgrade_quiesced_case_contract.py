#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from dataclasses import asdict
from pathlib import Path
from typing import Any

import l6_guest_case_contract as guest_case_contract
import l6_v4_canonical_input_bundle as canonical_input_bundle


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
CONTRACT_PATH = Path("packaging/linux/l6-upgrade-quiesced-case.json")
MATRIX_PATH = Path("packaging/linux/l6-matrix.json")
RELEASE_PAIR_PATH = Path("packaging/linux/l6-release-pair.json")


class UpgradeQuiescedCaseContractError(ValueError):
    pass


EXPECTED_START_PACKAGE_SHA256 = (
    "09ed122804b11767b8ac7cd69c323c1f6eef511fd6ae7284d75756fb60569bec"
)

EXPECTED_START = {
    "all_registered_vms_stopped_precondition": True,
    "classification": (
        "local-operator-recovery-point-not-canonical-session-evidence"
    ),
    "clone_policy": "immutable-source-new-independent-target",
    "config_sha256": (
        "5111741c54a49068dbbacfd131891b0990a531001769db21942eb88ac6767d2b"
    ),
    "efi_sha256": (
        "365b5a170dca95bdf07c0e5e940fafd4580a353e43c141abede71c95f91261bc"
    ),
    "identity": "S2-source-data-512e8ab-0c2cefd6",
    "local_evidence_sha256": (
        "d164de0f5e6e88afbfa76ccd1c7d321d7064bbfd3ee1e275d3d4842d0dfa5c74"
    ),
    "prior_crash_clone_reuse": False,
    "qcow2_sha256": (
        "0c2cefd6b63420adf143e1f1d6e4e70f59841bf1ba56cb54e86d2e7a226f3eea"
    ),
    "registered_with_utm": False,
    "relative_path": "RadishLex-L6-Snapshots/S2-source-data-512e8ab",
    "role": "S2-source-data",
    "single_vm_runtime_limit": 1,
    "source_artifact_evidence_sha256": (
        "fe3d6297c08dccd8cacba13d50aa44dbb1c94b0ca8c2ab4df3a5c605466fcf94"
    ),
    "source_dpkg_status_sha256": (
        "33c4973d4bcccc1932de35b2b326c61140037ee46613c7a925f2cbdbd3d5cff1"
    ),
    "source_operation_chain_length": 1,
    "source_package_sha256": EXPECTED_START_PACKAGE_SHA256,
    "source_package_version": "26.7.1+38-1",
    "source_receipt_sha256": (
        "e58b144e2d148a7bc37790045c27ef6746308cfdea2e4e0924aa9ed9afa63ab8"
    ),
    "source_open_handles_required": 0,
    "target_clone_must_be_absent": True,
    "xdg_fingerprint_sha256": (
        "f3df287fa0f1da1d5f1fb3169fe607a84ad7fb9b60aab1308ba2eeb410d0b86b"
    ),
}

EXPECTED_AUTHORIZATION_SEQUENCE = [
    {
        "id": "clone",
        "next_state": "independent-clone-stopped-unstarted",
        "ordinal": 1,
        "single_authorized_effect": (
            "create-independent-clone-from-immutable-s2"
        ),
    },
    {
        "id": "start",
        "next_state": "independent-clone-started-no-business-input",
        "ordinal": 2,
        "single_authorized_effect": "start-only-the-independent-clone",
    },
    {
        "id": "input-preflight",
        "next_state": (
            "offline-source-installed-inputs-bound-preflight-passed"
        ),
        "ordinal": 3,
        "single_authorized_effect": (
            "publish-exact-inputs-and-run-readonly-preflight"
        ),
    },
    {
        "id": "crash",
        "next_state": "upgrade-quiesced-process-group-terminated",
        "ordinal": 4,
        "single_authorized_effect": (
            "run-once-to-quiesced-and-terminate-worker-process-group"
        ),
    },
    {
        "id": "resume",
        "next_state": "upgrade-completed-postflight-passed",
        "ordinal": 5,
        "single_authorized_effect": (
            "resume-once-with-fresh-relationship-and-quiescence-proofs"
        ),
    },
    {
        "id": "terminal-stop",
        "next_state": "independent-clone-stopped-terminal-frozen",
        "ordinal": 6,
        "single_authorized_effect": "request-one-graceful-terminal-stop",
    },
]

EXPECTED_CLONE_FRONT_DOOR = {
    "control_sequence": [
        "clone-once-from-dedicated-registration-shell",
        "materialize-s2-efi-and-qcow2-once",
    ],
    "failure_policy": {
        "automatic_delete": False,
        "automatic_retry": False,
        "automatic_rollback": False,
        "automatic_start": False,
        "partial_target_is_state_indeterminate": True,
    },
    "materialization": {
        "config_policy": "registration-shell-except-name-and-uuid",
        "copy_primitive": "apfs-clonefile",
        "members": [
            "Data/efi_vars.fd",
            "Data/FFF05A20-E829-493C-8F40-B40884425A3F.qcow2",
        ],
        "replacement_count": 2,
        "source_config_is_not_copied": True,
    },
    "preclone_baseline": {
        "all_registered_vms_stopped": True,
        "delete_batch_id": "fifth-batch-v1",
        "delete_evidence_relative_path": (
            "RadishLex-L6-Asset-Retirement-Delete-20260901-"
            "Fifth-Batch-v1"
        ),
        "delete_manifest_sha256": (
            "a65abab201530518d0ccfec6de60a0c09d156197fce7a30950b97a605181b106"
        ),
        "delete_repository_head": (
            "da15e97087bd9ff793c0c04e878bb086b62b9eb2"
        ),
        "deleted_packages_must_remain_absent": True,
        "inventory_sha256": (
            "dc91dd99d10b01399886e592343973a9518dba1ebb8dda61b9299c8c99630fe8"
        ),
        "managed_members": [
            {
                "name": "RadishLex-Debian13-ARM64",
                "uuid": "E0168AA6-AFDA-4C81-9327-590DAC48C49B",
            },
            {
                "name": "Debian13-ARM64",
                "uuid": "755199B1-1C18-4441-8A1E-D423C8DE0022",
            },
            {
                "name": "Debian13-ARM64-CleanBase",
                "uuid": "21197987-AEBB-46E6-ABDC-B9762F5C0CE4",
            },
            {
                "name": "RadishLex-L6-PairBuilder-2fa1b8c-v2",
                "uuid": "E2F5624C-B0F9-4102-B149-5F72C1851D49",
            },
            {
                "name": (
                    "RadishLex-L6-Registration-Shell-d75818f-"
                    "Upgrade-Quiesced-v1"
                ),
                "uuid": "0BAA7355-A55A-463E-97FE-82A785E252A7",
            },
        ],
        "registered_vm_count": 5,
    },
    "predecessor_failure": {
        "attempt_id": "d75818f-upgrade-quiesced-clone-20260902-v1",
        "clone_invocations": 0,
        "evidence_relative_path": (
            "RadishLex-L6-Crash-Upgrade-Quiesced-d75818f-"
            "Clone-Once-v1"
        ),
        "foreign_vm_count": 2,
        "inventory_sha256": (
            "738b950ad2ba78f03273cc519a6521a6f8ccae4bdd6848cc173355e1f9b905cc"
        ),
        "manifest_sha256": (
            "79901be8bb19c18b71eef775c4ef11267ec580e294a8fa64cb65146611f5265f"
        ),
        "outcome": "precondition-rejected",
        "reason": "utmctl-list-preclone:preclone-vm-count-mismatch",
        "registered_vm_count": 7,
        "replacement_count": 0,
        "repository_head": (
            "b3e8491d9bcc16082e91e085f50631064307fe18"
        ),
        "started_foreign_vm_count": 1,
    },
    "live_inventory_policy": {
        "all_registered_vms_must_be_stopped": True,
        "foreign_overlay_allowed": True,
        "foreign_overlay_must_remain_unchanged": True,
        "managed_vm_count": 5,
        "target_is_only_allowed_addition": True,
    },
    "partial_recovery": {
        "materialization": {
            "authorization_is_separate_from_move": True,
            "copy_invocations": 2,
            "evidence_relative_path": (
                "RadishLex-L6-Crash-Upgrade-Quiesced-d75818f-"
                "Clone-Partial-Materialize-v1"
            ),
            "handle_rounds": 5,
            "inventory_queries": 2,
            "attempt_id": (
                "d75818f-upgrade-quiesced-clone-partial-"
                "materialize-20260905-v1"
            ),
            "replacement_count": 2,
            "source_s2_must_remain_unchanged": True,
            "target_config_must_remain_unchanged": True,
        },
        "move": {
            "adopt_attempt_id": (
                "d75818f-upgrade-quiesced-clone-partial-"
                "move-adopt-20260905-v1"
            ),
            "adopt_evidence_relative_path": (
                "RadishLex-L6-Crash-Upgrade-Quiesced-d75818f-"
                "Clone-Partial-Move-Adopt-v1"
            ),
            "adopt_inventory_queries": 2,
            "adopt_is_read_only": True,
            "destination_location": "operator-asset-root",
            "external_action_is_separately_authorized": True,
            "prepare_inventory_queries": 1,
            "prepare_attempt_id": (
                "d75818f-upgrade-quiesced-clone-partial-"
                "move-prepare-20260905-v1"
            ),
            "prepare_evidence_relative_path": (
                "RadishLex-L6-Crash-Upgrade-Quiesced-d75818f-"
                "Clone-Partial-Move-Prepare-v1"
            ),
            "primitive": "utm-native-ui",
            "source_location": "utm-documents-root",
        },
        "partial_clone": {
            "attempt_id": (
                "d75818f-upgrade-quiesced-clone-20260905-v2"
            ),
            "clone_invocations": 1,
            "control_sha256": (
                "64e895aa253be8c24d2921103ef53bbd550ff17c5397d4d84a7671a3d3df411a"
            ),
            "default_package": {
                "config_sha256": (
                    "0bac8867d9c1522059f91716324b03f3032373d761a8861b4ba05b521d190a8f"
                ),
                "efi_sha256": (
                    "7b0a7f26192011e6e98c770694269b40f8b70620ca58fc4973a232fb223600d5"
                ),
                "qcow2_name": (
                    "EF62CC7C-DF55-4A83-8633-F4630CF3B234.qcow2"
                ),
                "qcow2_sha256": (
                    "ae44c4d0b6b789f232932cc0dfbda31dcd54edfb6849ed97e3a4a0c228c2b94b"
                ),
            },
            "evidence_relative_path": (
                "RadishLex-L6-Crash-Upgrade-Quiesced-d75818f-"
                "Clone-Once-v2"
            ),
            "foreign_vm_count": 2,
            "manifest_sha256": (
                "0fa2cd297be9d0739485c3e9d0d4e47d3193ed5c9ef6f1527f6522c61f3084ff"
            ),
            "outcome": "state-indeterminate",
            "postclone_inventory_sha256": (
                "4fdf86920dbf7c569e18ba7b97f3aca4527def3eb3f63186ecc0cb421eb412a0"
            ),
            "postclone_managed_vm_count": 6,
            "postclone_registered_vm_count": 8,
            "preclone_inventory_sha256": (
                "d91030846f2c9b4118fe91a6e43437d663317d79cd529710b3678b4f43da9bb1"
            ),
            "preclone_registered_vm_count": 7,
            "reason": (
                "target-package-postclone:"
                "clone-postconditions-indeterminate"
            ),
            "replacement_count": 0,
            "repository_head": (
                "2811ef399c5308a281d5ca512b3a557968f55aa9"
            ),
            "target_name": (
                "RadishLex-Debian13-ARM64-L6-d75818f-"
                "upgrade-quiesced"
            ),
            "target_uuid": "2672A88A-91D6-4677-9793-9BC83B46224A",
        },
    },
    "registration_shell": {
        "dedicated_case_evidence_required": True,
        "location_policy": {
            "authoritative_root": "operator-asset-root",
            "default_storage_partial_is_clone_source": False,
            "move_before_configuration_update": True,
            "move_primitive": "utm-native-ui",
        },
        "may_reuse_frozen_crash_clone": False,
        "may_reuse_frozen_transaction_terminal": False,
        "network": [],
        "purpose": "registration-only-no-guest-state-reuse",
    },
}


def load_repository_contract(
    repository_root: Path = REPOSITORY_ROOT,
) -> tuple[dict[str, Any], dict[str, Any], dict[str, Any]]:
    return (
        _read_strict_json(repository_root / CONTRACT_PATH),
        _read_strict_json(repository_root / MATRIX_PATH),
        _read_strict_json(repository_root / RELEASE_PAIR_PATH),
    )


def validate_upgrade_quiesced_case_contract(
    contract: dict[str, Any],
    matrix: dict[str, Any],
    release_pair: dict[str, Any],
) -> None:
    _require_exact_keys(
        contract,
        {
            "authorization_sequence",
            "clone_front_door",
            "checkpoint",
            "format_version",
            "matrix",
            "profile",
            "release_pair",
            "start",
        },
        "contract",
    )
    if (
        contract["format_version"] != 1
        or contract["profile"]
        != "debian13-arm64-upgrade-quiesced-crash-v1"
    ):
        raise UpgradeQuiescedCaseContractError("contract identity drift")

    _validate_matrix(contract["matrix"], matrix)
    _validate_start(contract["start"])
    if contract["clone_front_door"] != EXPECTED_CLONE_FRONT_DOOR:
        raise UpgradeQuiescedCaseContractError(
            "clone front door must keep a dedicated shell and exact S2 materialization"
        )
    _validate_release_pair(
        contract["release_pair"], contract["start"], matrix, release_pair
    )
    _validate_checkpoint(contract["checkpoint"])
    if contract["authorization_sequence"] != EXPECTED_AUTHORIZATION_SEQUENCE:
        raise UpgradeQuiescedCaseContractError(
            "authorization sequence must keep six separately authorized phases"
        )


def validate_repository_contract(
    repository_root: Path = REPOSITORY_ROOT,
) -> None:
    validate_upgrade_quiesced_case_contract(
        *load_repository_contract(repository_root)
    )


def _validate_matrix(contract_matrix: Any, matrix: dict[str, Any]) -> None:
    expected = {
        "checkpoint": "quiesced",
        "expected_terminal": "completed",
        "fault": "process_group_terminated",
        "operation": "upgrade_target",
        "restore_snapshot_after": True,
        "scenario": "upgrade_quiesced",
    }
    if contract_matrix != expected:
        raise UpgradeQuiescedCaseContractError("case matrix identity drift")
    scenarios = matrix.get("crash_scenarios")
    if not isinstance(scenarios, list):
        raise UpgradeQuiescedCaseContractError("matrix crash scenarios invalid")
    scenario = next(
        (
            item
            for item in scenarios
            if isinstance(item, dict) and item.get("id") == "upgrade_quiesced"
        ),
        None,
    )
    matrix_expected = {
        "checkpoint": expected["checkpoint"],
        "expected_terminal": expected["expected_terminal"],
        "fault": expected["fault"],
        "id": expected["scenario"],
        "operation": expected["operation"],
        "restore_snapshot_after": expected["restore_snapshot_after"],
    }
    if scenario != matrix_expected:
        raise UpgradeQuiescedCaseContractError(
            "upgrade quiesced case differs from L6 matrix"
        )


def _validate_start(start: Any) -> None:
    if start != EXPECTED_START:
        raise UpgradeQuiescedCaseContractError(
            "upgrade quiesced must start from the exact immutable S2 identity"
        )


def _validate_release_pair(
    case_pair: Any,
    start: Any,
    matrix: dict[str, Any],
    release_pair: dict[str, Any],
) -> None:
    if not isinstance(case_pair, dict):
        raise UpgradeQuiescedCaseContractError("case release pair invalid")
    _require_exact_keys(
        case_pair,
        {"handoff_record_sha256", "relationship", "source", "target"},
        "case release pair",
    )
    source = _require_mapping(case_pair["source"], "case source")
    target = _require_mapping(case_pair["target"], "case target")
    relationship = _require_mapping(
        case_pair["relationship"], "case relationship"
    )

    anchor = _require_mapping(
        _require_mapping(release_pair.get("source"), "release source").get(
            "chain_anchor"
        ),
        "release source chain anchor",
    )
    source_package = _require_mapping(anchor.get("package"), "source package")
    source_evidence = _require_mapping(
        anchor.get("artifact_evidence"), "source artifact evidence"
    )
    expected_source = {
        "artifact_evidence_filename": source_evidence.get("filename"),
        "artifact_evidence_sha256": source_evidence.get("sha256"),
        "artifact_evidence_size": source_evidence.get("size"),
        "package_filename": source_package.get("filename"),
        "package_sha256": source_package.get("sha256"),
        "package_size": source_package.get("size"),
        "package_version": _require_mapping(
            release_pair.get("source"), "release source"
        ).get("package_version"),
        "repository_commit": _require_mapping(
            release_pair.get("source"), "release source"
        ).get("repository_commit"),
        "role": "source",
    }
    if source != expected_source:
        raise UpgradeQuiescedCaseContractError(
            "case source differs from the committed chain anchor"
        )

    expected_target = {
        "artifact_evidence_filename": (
            "radishlex_26.7.1+38-2_arm64.deb.evidence.json"
        ),
        "artifact_evidence_sha256": (
            "9e646c86c33bc5027b82c8af0b659df7515722f66ffa92be09420f9974f63f17"
        ),
        "artifact_evidence_size": 2376,
        "package_filename": "radishlex_26.7.1+38-2_arm64.deb",
        "package_sha256": (
            "4dd0054051612654940a5973cc8a465be9b598a40a6fc036c7b263088aa5dcec"
        ),
        "package_size": 40806592,
        "package_version": _require_mapping(
            release_pair.get("target"), "release target"
        ).get("package_version"),
        "repository_commit": "d75818f764758bfc4d90102ed2a6b3208d1debcf",
        "role": "target",
    }
    if target != expected_target:
        raise UpgradeQuiescedCaseContractError(
            "case target differs from the frozen d75818f handoff"
        )

    _validate_canonical_input_identity(source, "source")
    _validate_canonical_input_identity(target, "target")
    if (
        case_pair["handoff_record_sha256"]
        != canonical_input_bundle.HANDOFF_RECORD_SHA256
    ):
        raise UpgradeQuiescedCaseContractError(
            "case handoff record differs from canonical input"
        )

    expected_relationship = {
        "data_contract_equal": True,
        "installed_after": "target",
        "installed_before": "source",
        "operation_kind": "upgrade",
        "required_staged_slots": ["source", "target"],
        "version_relation": "target_newer",
    }
    matrix_pair = _require_mapping(
        matrix.get("release_pair"), "matrix release pair"
    )
    start_identity = _require_mapping(start, "case start")
    if (
        relationship != expected_relationship
        or matrix_pair.get("data_contract_equal") is not True
        or matrix_pair.get("target_version_relation")
        != "greater-than-source"
        or source["package_sha256"]
        != start_identity.get("source_package_sha256")
        or source["package_version"]
        != start_identity.get("source_package_version")
        or source["artifact_evidence_sha256"]
        != start_identity.get("source_artifact_evidence_sha256")
    ):
        raise UpgradeQuiescedCaseContractError(
            "source-to-target upgrade relationship is inconsistent"
        )

def _validate_canonical_input_identity(
    artifact: dict[str, Any], role: str
) -> None:
    package_path = f"{role}/artifacts/{artifact['package_filename']}"
    evidence_path = (
        f"{role}/artifacts/{artifact['artifact_evidence_filename']}"
    )
    package = canonical_input_bundle.SOURCE_IDENTITIES.get(package_path)
    evidence = canonical_input_bundle.SOURCE_IDENTITIES.get(evidence_path)
    if package != (
        0o644,
        artifact["package_size"],
        artifact["package_sha256"],
    ) or evidence != (
        0o644,
        artifact["artifact_evidence_size"],
        artifact["artifact_evidence_sha256"],
    ):
        raise UpgradeQuiescedCaseContractError(
            f"{role} differs from the frozen canonical input identity"
        )


def _validate_checkpoint(checkpoint: Any) -> None:
    expectation = guest_case_contract.crash_checkpoint_expectation(
        "upgrade_quiesced"
    )
    expected = {
        "dpkg_child": expectation.dpkg_child,
        "dpkg_log": expectation.dpkg_log,
        "dpkg_status": expectation.dpkg_status,
        "expected_terminal": expectation.expected_terminal,
        "guard": asdict(expectation.guard),
        "network": expectation.network,
        "package_state": expectation.package_state,
        "process_group": expectation.process_group,
        "process_group_member_count": expectation.process_group_member_count,
        "product_processes": expectation.product_processes,
        "receipt": _json_compatible(asdict(expectation.receipt)),
        "resume": {
            "steps": list(expectation.resume_steps),
            "target_apply_total": 1,
        },
        "startup_case": expectation.startup_case,
        "xdg_fingerprint": expectation.xdg,
    }
    if checkpoint != expected:
        raise UpgradeQuiescedCaseContractError(
            "checkpoint or resume semantics differ from the guest-case contract"
        )


def _json_compatible(value: Any) -> Any:
    if isinstance(value, tuple):
        return [_json_compatible(item) for item in value]
    if isinstance(value, dict):
        return {key: _json_compatible(item) for key, item in value.items()}
    return value


def _read_strict_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=_pairs)
    except (OSError, UnicodeError, json.JSONDecodeError) as exc:
        raise UpgradeQuiescedCaseContractError(
            f"cannot read canonical JSON: {path}"
        ) from exc
    if not isinstance(value, dict):
        raise UpgradeQuiescedCaseContractError(f"JSON root must be an object: {path}")
    return value


def _pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise UpgradeQuiescedCaseContractError(
                f"duplicate JSON key: {key}"
            )
        value[key] = item
    return value


def _require_mapping(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise UpgradeQuiescedCaseContractError(f"{label} must be an object")
    return value


def _require_exact_keys(
    value: dict[str, Any], expected: set[str], label: str
) -> None:
    if set(value) != expected:
        raise UpgradeQuiescedCaseContractError(f"{label} fields drift")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Validate the repository-only Linux L6 upgrade-quiesced case contract."
        )
    )
    parser.add_argument("command", choices=("validate",))
    return parser.parse_args()


def main() -> int:
    parse_args()
    validate_repository_contract()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
from __future__ import annotations

import argparse
from dataclasses import dataclass


BUILD_ENVIRONMENT_FILENAME = "build-environment.json"
INPUT_INVENTORY_ORDER = "utf8-bytewise-relative-path-v1"
GUEST_AGENT_TRUST_POLICY = "result-and-evidence-file-readback-v1"
STARTUP_FFI_PATH = (
    "target-startup/usr/lib/aarch64-linux-gnu/radishlex/manager/lib/"
    "libradishlex_ime_ffi.so"
)
MAX_READBACK_BYTES = 1024 * 1024


class LinuxL6GuestCaseContractError(ValueError):
    pass


@dataclass(frozen=True)
class TrustedGuestReadback:
    result: str
    evidence: str


@dataclass(frozen=True)
class GuestAgentObservation:
    observer_exit_code: int | None
    observer_stdout: bytes
    observer_stderr: bytes
    result_file: bytes | None
    evidence_file: bytes | None

    def require_file_readback(self) -> TrustedGuestReadback:
        return TrustedGuestReadback(
            result=_canonical_readback_text(self.result_file, "result"),
            evidence=_canonical_readback_text(self.evidence_file, "evidence"),
        )


@dataclass(frozen=True)
class StartupExpectation:
    decision: str
    reason: str
    receipt_terminal: str | None
    wire_output: str


@dataclass(frozen=True)
class CrashReceiptExpectation:
    operation_kind: str
    version_relation: str
    state: str
    operation_chain_length: int
    required_staged_slots: tuple[str, ...]
    source_artifact_present: bool
    target_artifact_present: bool
    staged_slots: tuple[str, ...]
    target_proof_present: bool
    source_proof_present: bool
    failure_code: str | None
    manual_recovery_required: bool


@dataclass(frozen=True)
class CrashGuardExpectation:
    owner: str
    mode: str
    size_bytes: int
    link_count: int
    advisory_lock: str


@dataclass(frozen=True)
class CrashCheckpointExpectation:
    matrix_operation: str
    checkpoint: str
    fault: str
    receipt: CrashReceiptExpectation
    package_state: str
    dpkg_status: str
    dpkg_log: str
    guard: CrashGuardExpectation
    startup_case: str
    process_group: str
    process_group_member_count: int
    dpkg_child: str
    xdg: str
    product_processes: str
    network: str
    resume_steps: tuple[str, ...]
    expected_terminal: str


STARTUP_EXPECTATIONS = {
    "fresh-absent": StartupExpectation(
        decision="FailedClosed",
        reason="ReceiptMissing",
        receipt_terminal=None,
        wire_output="0:1:4:15:0|error-absent",
    ),
    "removed-terminal": StartupExpectation(
        decision="FailedClosed",
        reason="RemovedProgram",
        receipt_terminal="completed",
        wire_output="0:1:4:24:6|error-absent",
    ),
    "active-guard": StartupExpectation(
        decision="MaintenanceRequired",
        reason="ActiveGuard",
        receipt_terminal=None,
        wire_output="0:1:3:10:0|error-absent",
    ),
}


CRASH_CHECKPOINT_EXPECTATIONS = {
    "install_artifacts_staged": CrashCheckpointExpectation(
        matrix_operation="install_source",
        checkpoint="artifacts_staged",
        fault="process_group_terminated",
        receipt=CrashReceiptExpectation(
            operation_kind="install",
            version_relation="not_applicable",
            state="artifacts_staged",
            operation_chain_length=1,
            required_staged_slots=("target",),
            source_artifact_present=False,
            target_artifact_present=True,
            staged_slots=("target",),
            target_proof_present=False,
            source_proof_present=False,
            failure_code=None,
            manual_recovery_required=False,
        ),
        package_state="not_installed",
        dpkg_status="unchanged_from_preflight",
        dpkg_log="unchanged_from_preflight",
        guard=CrashGuardExpectation(
            owner="root:root",
            mode="0600",
            size_bytes=0,
            link_count=1,
            advisory_lock="unlocked_after_worker_exit",
        ),
        startup_case="active-guard",
        process_group="terminated",
        process_group_member_count=0,
        dpkg_child="absent",
        xdg="unchanged_from_preflight",
        product_processes="unchanged_from_preflight",
        network="unchanged_from_preflight",
        resume_steps=(
            "validate_staged_relationship",
            "prove_target_quiescence",
            "apply_target_once",
            "verify_target",
            "complete",
        ),
        expected_terminal="completed",
    ),
    "upgrade_quiesced": CrashCheckpointExpectation(
        matrix_operation="upgrade_target",
        checkpoint="quiesced",
        fault="process_group_terminated",
        receipt=CrashReceiptExpectation(
            operation_kind="upgrade",
            version_relation="target_newer",
            state="quiesced",
            operation_chain_length=2,
            required_staged_slots=("source", "target"),
            source_artifact_present=True,
            target_artifact_present=True,
            staged_slots=("source", "target"),
            target_proof_present=False,
            source_proof_present=False,
            failure_code=None,
            manual_recovery_required=False,
        ),
        package_state="source_installed",
        dpkg_status="unchanged_from_preflight",
        dpkg_log="unchanged_from_preflight",
        guard=CrashGuardExpectation(
            owner="root:root",
            mode="0600",
            size_bytes=0,
            link_count=1,
            advisory_lock="unlocked_after_worker_exit",
        ),
        startup_case="active-guard",
        process_group="terminated",
        process_group_member_count=0,
        dpkg_child="absent",
        xdg="unchanged_from_s2",
        product_processes="unchanged_from_preflight",
        network="unchanged_from_preflight",
        resume_steps=(
            "validate_staged_relationship",
            "prove_target_quiescence",
            "apply_target_once",
            "verify_target",
            "complete",
        ),
        expected_terminal="completed",
    ),
}


def canonical_input_inventory(
    source_package: str, target_package: str
) -> tuple[str, ...]:
    source_package = _package_basename(source_package, "source package")
    target_package = _package_basename(target_package, "target package")
    if source_package == target_package:
        raise LinuxL6GuestCaseContractError(
            "source and target package names must differ"
        )

    paths = (
        BUILD_ENVIRONMENT_FILENAME,
        "case.sh",
        "radishlex-linux-l6-acceptance",
        "radishlex-linux-maintenance",
        "release-pair.evidence.json",
        "snapshot-identity.evidence.txt",
        f"source/artifacts/{source_package}",
        f"source/artifacts/{source_package}.evidence.json",
        "startup.py",
        STARTUP_FFI_PATH,
        f"target/artifacts/{target_package}",
        f"target/artifacts/{target_package}.evidence.json",
    )
    if len(paths) != len(set(paths)):
        raise LinuxL6GuestCaseContractError("input inventory contains duplicates")
    return tuple(sorted(paths, key=lambda item: item.encode("utf-8")))


def startup_expectation(case: str) -> StartupExpectation:
    try:
        return STARTUP_EXPECTATIONS[case]
    except KeyError as exc:
        raise LinuxL6GuestCaseContractError(
            f"unknown L6 guest startup case: {case}"
        ) from exc


def crash_checkpoint_expectation(scenario: str) -> CrashCheckpointExpectation:
    try:
        return CRASH_CHECKPOINT_EXPECTATIONS[scenario]
    except KeyError as exc:
        raise LinuxL6GuestCaseContractError(
            f"unknown L6 guest crash checkpoint case: {scenario}"
        ) from exc


def validate_crash_checkpoint_matrix(
    matrix_scenarios: list[dict[str, object]],
) -> None:
    for scenario, expectation in CRASH_CHECKPOINT_EXPECTATIONS.items():
        matrix_scenario = next(
            (item for item in matrix_scenarios if item.get("id") == scenario),
            None,
        )
        if matrix_scenario is None or (
            matrix_scenario.get("operation") != expectation.matrix_operation
            or matrix_scenario.get("checkpoint") != expectation.checkpoint
            or matrix_scenario.get("fault") != expectation.fault
            or matrix_scenario.get("expected_terminal")
            != expectation.expected_terminal
            or matrix_scenario.get("restore_snapshot_after") is not True
        ):
            raise LinuxL6GuestCaseContractError(
                f"L6 guest crash checkpoint case differs from the matrix: {scenario}"
            )


def validate_guest_case_contract() -> None:
    inventory = canonical_input_inventory(
        "radishlex_26.7.1+38-1_arm64.deb",
        "radishlex_26.7.1+38-2_arm64.deb",
    )
    if inventory != tuple(sorted(inventory, key=lambda item: item.encode("utf-8"))):
        raise LinuxL6GuestCaseContractError(
            "input inventory must use canonical UTF-8 byte ordering"
        )
    if inventory.count(BUILD_ENVIRONMENT_FILENAME) != 1:
        raise LinuxL6GuestCaseContractError(
            "input inventory must contain one build environment file"
        )
    if "build-environment.evidence.json" in inventory:
        raise LinuxL6GuestCaseContractError(
            "legacy build environment alias is forbidden"
        )
    if inventory.index(STARTUP_FFI_PATH) > inventory.index(
        "target/artifacts/radishlex_26.7.1+38-2_arm64.deb"
    ):
        raise LinuxL6GuestCaseContractError(
            "target-startup must precede target artifacts in canonical inventory"
        )

    fresh = startup_expectation("fresh-absent")
    removed = startup_expectation("removed-terminal")
    if fresh == removed or fresh.receipt_terminal is not None:
        raise LinuxL6GuestCaseContractError(
            "fresh absent and removed terminal startup states must remain distinct"
        )

    for scenario, expectation in CRASH_CHECKPOINT_EXPECTATIONS.items():
        active_guard = startup_expectation(expectation.startup_case)
        if (
            expectation.receipt.state != expectation.checkpoint
            or expectation.receipt.required_staged_slots
            != expectation.receipt.staged_slots
            or expectation.process_group != "terminated"
            or expectation.process_group_member_count != 0
            or expectation.dpkg_child != "absent"
            or active_guard.decision != "MaintenanceRequired"
            or active_guard.reason != "ActiveGuard"
            or active_guard.receipt_terminal is not None
        ):
            raise LinuxL6GuestCaseContractError(
                f"L6 guest crash state is inconsistent: {scenario}"
            )

    staged = crash_checkpoint_expectation("install_artifacts_staged")
    if staged.package_state != "not_installed":
        raise LinuxL6GuestCaseContractError(
            "install artifacts-staged package state is inconsistent"
        )

    quiesced = crash_checkpoint_expectation("upgrade_quiesced")
    if (
        quiesced.receipt.operation_kind != "upgrade"
        or quiesced.receipt.version_relation != "target_newer"
        or quiesced.receipt.operation_chain_length != 2
        or not quiesced.receipt.source_artifact_present
        or not quiesced.receipt.target_artifact_present
        or quiesced.receipt.target_proof_present
        or quiesced.receipt.source_proof_present
        or quiesced.package_state != "source_installed"
        or quiesced.xdg != "unchanged_from_s2"
    ):
        raise LinuxL6GuestCaseContractError(
            "upgrade quiesced crash state is inconsistent"
        )


def _package_basename(value: str, label: str) -> str:
    if (
        not value
        or value in (".", "..")
        or "/" in value
        or "\\" in value
        or "\x00" in value
        or not value.isascii()
        or any(
            not (character.isalnum() or character in "._+~%:-")
            for character in value
        )
        or not value.endswith("_arm64.deb")
    ):
        raise LinuxL6GuestCaseContractError(
            f"{label} must be a safe Debian ARM64 package basename"
        )
    return value


def _canonical_readback_text(value: bytes | None, label: str) -> str:
    if value is None or not value or len(value) > MAX_READBACK_BYTES:
        raise LinuxL6GuestCaseContractError(
            f"guest {label} file readback must be present and bounded"
        )
    try:
        text = value.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise LinuxL6GuestCaseContractError(
            f"guest {label} file readback must be UTF-8"
        ) from exc
    if (
        text.startswith("\ufeff")
        or "\x00" in text
        or "\r" in text
        or not text.endswith("\n")
    ):
        raise LinuxL6GuestCaseContractError(
            f"guest {label} file readback must use canonical LF text"
        )
    return text


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate the repository-only Linux L6 guest-case contract."
    )
    parser.add_argument("command", choices=("validate",))
    return parser.parse_args()


def main() -> int:
    parse_args()
    validate_guest_case_contract()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


class LinuxL6ControllerContractError(ValueError):
    pass


@dataclass(frozen=True)
class ControllerSources:
    workspace: str
    product_cargo: str
    product_lib: str
    product_host: str
    product_main: str
    checkpoint: str
    acceptance_cargo: str
    acceptance_lib: str
    acceptance_main: str
    acceptance_command: str
    acceptance_controller: str
    acceptance_evidence: str
    acceptance_process: str
    acceptance_scenario: str

    @classmethod
    def load(cls, root: Path = REPO_ROOT) -> ControllerSources:
        paths = {
            "workspace": "Cargo.toml",
            "product_cargo": "platforms/linux-product/Cargo.toml",
            "product_lib": "platforms/linux-product/src/lib.rs",
            "product_host": "platforms/linux-product/src/system/host.rs",
            "product_main": "platforms/linux-product/src/bin/radishlex-linux-maintenance.rs",
            "checkpoint": "platforms/linux-product/src/checkpoint.rs",
            "acceptance_cargo": "platforms/linux-l6-acceptance/Cargo.toml",
            "acceptance_lib": "platforms/linux-l6-acceptance/src/lib.rs",
            "acceptance_main": (
                "platforms/linux-l6-acceptance/src/bin/"
                "radishlex-linux-l6-acceptance.rs"
            ),
            "acceptance_command": "platforms/linux-l6-acceptance/src/command.rs",
            "acceptance_controller": "platforms/linux-l6-acceptance/src/controller.rs",
            "acceptance_evidence": "platforms/linux-l6-acceptance/src/evidence.rs",
            "acceptance_process": "platforms/linux-l6-acceptance/src/process.rs",
            "acceptance_scenario": "platforms/linux-l6-acceptance/src/scenario.rs",
        }
        values: dict[str, str] = {}
        for name, relative in paths.items():
            path = root / relative
            try:
                values[name] = path.read_text(encoding="utf-8")
            except OSError as exc:
                raise LinuxL6ControllerContractError(
                    f"cannot read L6 controller source: {relative}"
                ) from exc
        return cls(**values)


def validate_controller_contract(
    sources: ControllerSources | None = None,
) -> ControllerSources:
    sources = sources or ControllerSources.load()
    require(
        sources.workspace,
        '"platforms/linux-l6-acceptance"',
        "acceptance crate must be a workspace member",
    )
    for token in (
        "[features]",
        "default = []",
        "l6-acceptance-checkpoints = []",
    ):
        require(
            sources.product_cargo,
            token,
            "production checkpoint feature contract is incomplete",
        )
    require(
        sources.acceptance_cargo,
        'features = ["l6-acceptance-checkpoints"]',
        "acceptance crate must compile the isolated checkpoint surface",
    )
    for token in (
        '#[cfg(feature = "l6-acceptance-checkpoints")]',
        "run_linux_maintenance_with_l6_checkpoints",
        "LinuxL6CheckpointSink",
    ):
        require(
            sources.product_lib,
            token,
            "checkpoint API must remain behind the compile feature",
        )
    for forbidden in (
        "authorized-l6-crash",
        "L6CrashScenario",
        "run_linux_maintenance_with_l6_checkpoints",
        "RADISHLEX_L6",
    ):
        forbid(
            sources.product_main,
            forbidden,
            "production maintenance CLI must not contain L6 activation",
        )
    require(
        sources.product_host,
        "DisabledLinuxL6Checkpoints",
        "production host must bind the disabled checkpoint sink",
    )
    require(
        sources.product_host,
        '#[cfg(feature = "l6-acceptance-checkpoints")]\n'
        "pub fn run_linux_maintenance_with_l6_checkpoints",
        "acceptance host entry must be compile-gated",
    )

    checkpoints = (
        "prepared",
        "artifacts_staged",
        "quiesced",
        "package_mutating_before_dpkg",
        "target_applied_before_proof",
        "rollback_required",
        "source_restoring_before_dpkg",
        "source_applied_before_proof",
    )
    checkpoint_variants = (
        "Prepared",
        "ArtifactsStaged",
        "Quiesced",
        "PackageMutatingBeforeDpkg",
        "TargetAppliedBeforeProof",
        "RollbackRequired",
        "SourceRestoringBeforeDpkg",
        "SourceAppliedBeforeProof",
    )
    scenarios = (
        "install_prepared",
        "install_artifacts_staged",
        "upgrade_quiesced",
        "upgrade_before_dpkg",
        "upgrade_after_dpkg",
        "upgrade_rollback_required",
        "upgrade_before_source_restore",
        "upgrade_after_source_restore",
    )
    for checkpoint, variant in zip(checkpoints, checkpoint_variants, strict=True):
        require(
            sources.checkpoint,
            f'"{checkpoint}"',
            f"missing compile checkpoint: {checkpoint}",
        )
        require(
            sources.acceptance_scenario,
            f"LinuxL6Checkpoint::{variant}",
            f"scenario mapping is missing checkpoint: {checkpoint}",
        )
    for scenario in scenarios:
        require(
            sources.acceptance_scenario,
            f'"{scenario}"',
            f"missing L6 crash scenario: {scenario}",
        )

    for source in (
        sources.acceptance_lib,
        sources.acceptance_main,
        sources.acceptance_command,
        sources.acceptance_controller,
        sources.acceptance_evidence,
        sources.acceptance_process,
        sources.acceptance_scenario,
    ):
        for forbidden in (
            "std::env::var(",
            "std::env::var_os(",
            "RADISHLEX_L6_",
            "--state-root",
            "--evidence-root",
            "--dpkg-program",
            "--checkpoint-path",
            'Command::new("/bin/sh")',
            'Command::new("/bin/bash")',
        ):
            forbid(
                source,
                forbidden,
                "acceptance capability must not use environment/path/shell overrides",
            )

    process_requirements = (
        '.process_group(0)',
        'const KILL_PROGRAM: &str = "/usr/bin/kill"',
        'Command::new(KILL_PROGRAM)',
        '.env_clear()',
        'fs::read_dir("/proc")',
        "prove_process_group_empty",
        "dpkg_member_count",
        "inherited-pipe-v1",
    )
    combined_process = (
        sources.acceptance_process
        + sources.acceptance_controller
        + sources.acceptance_evidence
    )
    for token in process_requirements:
        require(
            combined_process,
            token,
            "controller process-group or checkpoint proof is incomplete",
        )
    forbid(
        sources.acceptance_process,
        "receipt.json",
        "controller must not poll the receipt to locate a checkpoint",
    )
    controller_order = (
        "wait_for_checkpoint",
        "terminate_process_group",
        "wait_for_worker",
        "prove_process_group_empty",
        "CheckpointEvidenceEnvelopeV1::new",
    )
    positions = [sources.acceptance_controller.find(token) for token in controller_order]
    if any(position < 0 for position in positions) or positions != sorted(positions):
        raise LinuxL6ControllerContractError(
            "controller must notify, terminate, wait, prove empty, then form evidence"
        )

    for token in (
        '"radishlex-linux-l6-checkpoint-evidence-v1"',
        '"radishlex-linux-l6-acceptance-v1"',
        "operation_id_sha256",
        "canonical_bytes",
        "deny_unknown_fields",
        'dpkg_child: "absent"',
        "process_group_member_count: 0",
    ):
        require(
            sources.acceptance_evidence,
            token,
            "versioned canonical redacted evidence contract is incomplete",
        )
    for forbidden in (
        "operation_id: String",
        "pid: ",
        "proc_maps",
        "dpkg_stdout",
        "dpkg_stderr",
        "user_data: ",
    ):
        forbid(
            sources.acceptance_evidence,
            forbidden,
            "evidence envelope contains a forbidden raw field",
        )
    return sources


def require(source: str, token: str, message: str) -> None:
    if token not in source:
        raise LinuxL6ControllerContractError(message)


def forbid(source: str, token: str, message: str) -> None:
    if token in source:
        raise LinuxL6ControllerContractError(message)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate the compile-isolated Linux L6 controller boundary."
    )
    parser.add_argument("command", choices=("validate",))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if args.command == "validate":
        validate_controller_contract()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

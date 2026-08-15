#!/usr/bin/env python3
from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


class L6MaintenanceRefreshContractError(ValueError):
    pass


@dataclass(frozen=True)
class MaintenanceRefreshSources:
    builder: str
    tool: str
    io: str
    environment: str
    contract: str

    @classmethod
    def load(cls, root: Path = REPO_ROOT) -> MaintenanceRefreshSources:
        paths = {
            "builder": "scripts/build-linux-l6-maintenance-refresh.sh",
            "tool": "scripts/linux-product/l6_maintenance_refresh.py",
            "io": "scripts/linux-product/l6_maintenance_refresh_io.py",
            "environment": "scripts/linux-product/l6_maintenance_refresh_environment.py",
            "contract": "packaging/linux/l6-maintenance-refresh.json",
        }
        values: dict[str, str] = {}
        for name, relative in paths.items():
            try:
                values[name] = (root / relative).read_text(encoding="utf-8")
            except OSError as exc:
                raise L6MaintenanceRefreshContractError(
                    f"cannot read maintenance refresh source: {relative}"
                ) from exc
        return cls(**values)


def require(source: str, token: str, message: str) -> None:
    if token not in source:
        raise L6MaintenanceRefreshContractError(message)


def forbid(source: str, token: str, message: str) -> None:
    if token in source:
        raise L6MaintenanceRefreshContractError(message)


def validate_maintenance_refresh_contract(
    sources: MaintenanceRefreshSources | None = None,
) -> MaintenanceRefreshSources:
    sources = sources or MaintenanceRefreshSources.load()
    for token in (
        "--base-record ABSOLUTE_FILE --target-package ABSOLUTE_FILE --target-artifact-evidence ABSOLUTE_FILE --refresh-root ABSOLUTE_PATH --output ABSENT_ABSOLUTE_PATH",
        '"$(uname -s)" != "Linux"',
        '"$(uname -m)" != "aarch64"',
        'python3 "${refresh_tool}" validate-contract',
        'python3 "${refresh_tool}" validate-refresh',
        'python3 "${refresh_tool}" stage-base',
        '--base-record "${base_record_input}"',
        '--target-package "${target_package_input}"',
        '--target-artifact-evidence "${target_artifact_evidence_input}"',
        '--refresh-root "${refresh_root}"',
        'export CARGO_NET_OFFLINE=true',
        'CARGO_TARGET_DIR="${refresh_root}/target"',
        '--locked --release --no-default-features',
        '-p radishlex-linux-product-install',
        '--bin radishlex-linux-maintenance',
        '--bin radishlex-linux-artifact-verifier',
        '"${artifact_verifier}"',
        '--package "${target_package}"',
        '--evidence "${target_artifact_evidence}"',
        'python3 "${refresh_tool}" environment',
        'python3 "${refresh_tool}" record',
        'python3 "${refresh_tool}" verify',
        'python3 "${refresh_tool}" publish',
        '--staging "${staging}"',
        '--output "${canonical_output}"',
    ):
        require(
            sources.builder,
            token,
            f"maintenance refresh builder contract is incomplete: {token}",
        )
    ordered = (
        'python3 "${refresh_tool}" stage-base',
        '--bin radishlex-linux-maintenance',
        '"${artifact_verifier}"',
        'python3 "${refresh_tool}" record',
        'python3 "${refresh_tool}" verify',
        'python3 "${refresh_tool}" publish',
    )
    positions = [sources.builder.find(token) for token in ordered]
    if any(position < 0 for position in positions) or positions != sorted(positions):
        raise L6MaintenanceRefreshContractError(
            "maintenance refresh must stage the frozen base, build, verify, record, and publish in order"
        )
    for forbidden in (
        "radishlex-linux-l6-acceptance",
        "build-linux-deb-artifact.sh",
        "build-manager-linux-product.sh",
        "build-linux-product-addon-stage.sh",
        "rootfs.py",
        "flutter ",
        "cmake ",
        "--authorized-system-mutation",
        "--preserve-user-data",
        "--force-",
        "/usr/bin/dpkg",
        "apt-get",
        "apt install",
        "docker ",
        "limactl",
        "multipass",
        "qemu-system",
        "utmctl",
        "systemctl",
        "receipt.json",
        "/var/lib/dpkg/status",
    ):
        forbid(
            sources.builder,
            forbidden,
            "maintenance refresh builder must not rebuild a package, activate acceptance, or mutate a guest",
        )

    for token in (
        "import l6_release_pair",
        'BASE_TARGET_COMMIT = "80e49ced45b316eaa801f913c659142528a32c08"',
        'REPAIR_FIX_COMMIT = "b0197f5a0f0a88e53cf4b6bf862252a4374197de"',
        'BASE_RECORD_SHA256 = "cda70afa89b3f0ee05235b95dcc346eaeea9805c4b87af9d451ce1900f00659b"',
        'BASE_PACKAGE_SHA256 = "b211d9406825515b2ba1c473b5f98069de00fa709505e2ba3ed3cb9b8af7d09c"',
        '"2a1132c6fb27d4ca2e5e4bbd76864e3e753749c2bb43cae82287635b1c2d0e1b"',
        '"b060c2403424560e9ac0c11f890838894d19d153a89541575ade9afd87fe7d81"',
        "def verify_refresh_root",
        "require_git_ancestor(root, REPAIR_FIX_COMMIT, refresh_commit)",
        '"maintenance refresh must not change target product metadata"',
        "l6_release_pair.validate_record(record, pair_contract)",
        '"base target package identity differs from artifact evidence"',
        '"maintenance refresh executable must differ from the frozen base executable"',
        'l6_release_pair.parse_aarch64_elf(',
        '"radishlex-linux-l6-maintenance-refresh-evidence-v1"',
        "rename_directory_no_replace(staging, output)",
        "fsync_directory(output.parent)",
        "verify_published_record(",
        '"operation_id"',
        '"proc_maps"',
    ):
        require(
            sources.tool,
            token,
            "maintenance refresh identity, staging, or evidence verifier is incomplete",
        )
    for token in (
        "class L6MaintenanceRefreshError",
        "def require_absent_output",
        "def write_exclusive_file",
        "def fsync_directory",
        "def rename_directory_no_replace",
        "renameat2",
        "RENAME_NOREPLACE",
        "renameatx_np",
        "RENAME_EXCL",
        "os.O_EXCL",
        "os.fsync",
    ):
        require(
            sources.io,
            token,
            "maintenance refresh exclusive I/O contract is incomplete",
        )
    for token in (
        "def collect_environment",
        "def validate_environment",
        '"debian13-arm64-maintenance-refresh-build-v1"',
        '"codename": "trixie"',
        '"cargo", "git", "rustc"',
    ):
        require(
            sources.environment,
            token,
            "maintenance refresh build environment contract is incomplete",
        )
    for forbidden in (
        "RADISHLEX_L6_",
        "--state-root",
        "--evidence-root",
        "--dpkg-program",
        "--checkpoint-path",
        "--force-",
    ):
        forbid(
            sources.tool,
            forbidden,
            "maintenance refresh verifier must not add runtime acceptance activation or overrides",
        )

    for token in (
        '"profile": "debian13-arm64-maintenance-refresh-v1"',
        '"base_artifact_policy": "frozen-release-pair-target-v1"',
        '"package_rebuild": false',
        '"refresh_clean_root": true',
        '"sha256": "cda70afa89b3f0ee05235b95dcc346eaeea9805c4b87af9d451ce1900f00659b"',
        '"sha256": "b211d9406825515b2ba1c473b5f98069de00fa709505e2ba3ed3cb9b8af7d09c"',
        '"sha256": "2a1132c6fb27d4ca2e5e4bbd76864e3e753749c2bb43cae82287635b1c2d0e1b"',
        '"production_maintenance_sha256": "b060c2403424560e9ac0c11f890838894d19d153a89541575ade9afd87fe7d81"',
        '"repository_commit": "80e49ced45b316eaa801f913c659142528a32c08"',
        '"required_ancestor_commit": "b0197f5a0f0a88e53cf4b6bf862252a4374197de"',
        '"role": "production-maintenance"',
    ):
        require(
            sources.contract,
            token,
            "maintenance refresh committed identity is incomplete",
        )
    for forbidden in (
        "acceptance-controller",
        "radishlex-linux-l6-acceptance",
        '"package_rebuild": true',
    ):
        forbid(
            sources.contract,
            forbidden,
            "maintenance refresh contract must remain production-only and package-preserving",
        )
    return sources


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate the repository-only L6 maintenance refresh contract."
    )
    parser.add_argument("command", choices=("validate",))
    return parser.parse_args()


def main() -> int:
    parse_args()
    try:
        validate_maintenance_refresh_contract()
    except L6MaintenanceRefreshContractError as exc:
        raise SystemExit(str(exc)) from exc
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

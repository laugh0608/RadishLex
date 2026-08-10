#!/usr/bin/env python3
from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


class L6ReleasePairContractError(ValueError):
    pass


@dataclass(frozen=True)
class ReleasePairSources:
    builder: str
    tool: str
    contract: str

    @classmethod
    def load(cls, root: Path = REPO_ROOT) -> ReleasePairSources:
        paths = {
            "builder": "scripts/build-linux-l6-release-pair.sh",
            "tool": "scripts/linux-product/l6_release_pair.py",
            "contract": "packaging/linux/l6-release-pair.json",
        }
        values: dict[str, str] = {}
        for name, relative in paths.items():
            try:
                values[name] = (root / relative).read_text(encoding="utf-8")
            except OSError as exc:
                raise L6ReleasePairContractError(
                    f"cannot read L6 release pair source: {relative}"
                ) from exc
        return cls(**values)


def require(source: str, token: str, message: str) -> None:
    if token not in source:
        raise L6ReleasePairContractError(message)


def forbid(source: str, token: str, message: str) -> None:
    if token in source:
        raise L6ReleasePairContractError(message)


def validate_release_pair_contract(
    sources: ReleasePairSources | None = None,
) -> ReleasePairSources:
    sources = sources or ReleasePairSources.load()
    for token in (
        '--source-root ABSOLUTE_PATH --target-root ABSOLUTE_PATH --output ABSENT_ABSOLUTE_PATH',
        '"$(uname -s)" != "Linux"',
        '"$(uname -m)" != "aarch64"',
        'if [[ "${source_root}" == "${target_root}" ]]',
        'python3 "${pair_tool}" validate-roots',
        'export CARGO_NET_OFFLINE=true',
        'CARGO_TARGET_DIR="${root}/target"',
        'CARGO_TARGET_DIR="${target_root}/target"',
        'if [[ "${role}" == "source" ]]',
        '"CPLUS_INCLUDE_PATH=${root}/crates/ime-ffi/include"',
        '-u CPATH',
        '-u C_INCLUDE_PATH',
        '-u CPLUS_INCLUDE_PATH',
        '-u OBJC_INCLUDE_PATH',
        '-u RUSTC_WRAPPER',
        'build_release source "${source_root}"',
        'build_release target "${target_root}"',
        'build-manager-linux-product.sh" --system-product',
        'build-linux-product-addon-stage.sh"',
        'rootfs.py" assemble',
        'check-linux-product-layout.sh"',
        'build-linux-deb-artifact.sh"',
        '--locked --release --no-default-features',
        '-p radishlex-linux-product-install',
        '--bin radishlex-linux-artifact-verifier',
        '-p radishlex-linux-l6-acceptance',
        'verify_release_artifact source "${source_package}"',
        'verify_release_artifact target "${target_package}"',
        'python3 "${pair_tool}" record',
        'python3 "${pair_tool}" verify',
        'mv "${staging}" "${canonical_output}"',
    ):
        require(
            sources.builder,
            token,
            f"release pair builder contract is incomplete: {token}",
        )
    ordered = (
        'build_release source "${source_root}"',
        'build_release target "${target_root}"',
        '-p radishlex-linux-product-install',
        '-p radishlex-linux-l6-acceptance',
        'verify_release_artifact source "${source_package}"',
        'verify_release_artifact target "${target_package}"',
        'python3 "${pair_tool}" record',
        'python3 "${pair_tool}" verify',
        'mv "${staging}" "${canonical_output}"',
    )
    positions = [sources.builder.find(token) for token in ordered]
    if any(position < 0 for position in positions) or positions != sorted(positions):
        raise L6ReleasePairContractError(
            "release pair must build two packages, freeze binaries, verify, then publish"
        )
    for forbidden in (
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
        "systemctl",
        "receipt.json",
        "/var/lib/dpkg/status",
    ):
        forbid(
            sources.builder,
            forbidden,
            "release pair builder must not mutate a guest, package database, or receipt",
        )

    for token in (
        'SOURCE_COMMIT = "55351f21536d6dca3f90ab053c2a81e2b9bea354"',
        '"source and target require separate clean roots"',
        'CLEAN_OUTPUT_RELATIVES',
        '"target commit must differ from source"',
        '"target commit is not a descendant of source"',
        "require_absent_build_outputs=False",
        '"source and target Debian revisions are not adjacent"',
        'verify_debian_artifact',
        'parse_aarch64_elf',
        'production executable contains acceptance capability',
        'acceptance executable lacks compile identity markers',
        'canonical_json_bytes',
        'radishlex-linux-l6-release-pair-evidence-v1',
        'operation_id',
        'proc_maps',
    ):
        require(sources.tool, token, "release pair identity/evidence verifier is incomplete")
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
            "release pair verifier must not add runtime acceptance activation or overrides",
        )

    for token in (
        '"profile": "debian13-arm64-release-pair-v1"',
        '"separate_clean_roots": true',
        '"repository_commit": "55351f21536d6dca3f90ab053c2a81e2b9bea354"',
        '"package_version": "26.7.1+38-1"',
        '"package_version": "26.7.1+38-2"',
        '"build_identity": "radishlex-linux-maintenance-production-v1"',
        '"build_identity": "radishlex-linux-l6-acceptance-v1"',
    ):
        require(sources.contract, token, "release pair JSON identity is incomplete")
    return sources


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate the repository-only L6 release pair contract."
    )
    parser.add_argument("command", choices=("validate",))
    return parser.parse_args()


def main() -> int:
    parse_args()
    try:
        validate_release_pair_contract()
    except L6ReleasePairContractError as exc:
        raise SystemExit(str(exc)) from exc
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

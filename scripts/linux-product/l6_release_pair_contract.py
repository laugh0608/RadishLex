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
    anchor: str
    builder: str
    tool: str
    contract: str

    @classmethod
    def load(cls, root: Path = REPO_ROOT) -> ReleasePairSources:
        paths = {
            "anchor": "scripts/linux-product/l6_source_anchor.py",
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
        '--source-package ABSOLUTE_FILE --source-artifact-evidence ABSOLUTE_FILE --target-root ABSOLUTE_PATH --output ABSENT_ABSOLUTE_PATH',
        '"$(uname -s)" != "Linux"',
        '"$(uname -m)" != "aarch64"',
        'python3 "${pair_tool}" validate-target',
        'python3 "${pair_tool}" stage-source',
        '--source-package "${source_package_input}"',
        '--source-artifact-evidence "${source_artifact_evidence_input}"',
        '--output-dir "${staging}/source/artifacts"',
        'export CARGO_NET_OFFLINE=true',
        'CARGO_TARGET_DIR="${root}/target"',
        'CARGO_TARGET_DIR="${target_root}/target"',
        '-u CPATH',
        '-u C_INCLUDE_PATH',
        '-u CPLUS_INCLUDE_PATH',
        '-u OBJC_INCLUDE_PATH',
        '-u RUSTC_WRAPPER',
        'build_target_release "${target_root}"',
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
        'python3 "${pair_tool}" stage-source',
        'build_target_release "${target_root}"',
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
    for forbidden in ("source_root=", "${source_root}", "--source-root"):
        forbid(
            sources.builder,
            forbidden,
            "release pair builder must not accept or rebuild a source root",
        )

    for token in (
        "from l6_source_anchor import SourceAnchorStageError, stage_exact_source_anchor",
        'SOURCE_COMMIT = "55351f21536d6dca3f90ab053c2a81e2b9bea354"',
        '"09ed122804b11767b8ac7cd69c323c1f6eef511fd6ae7284d75756fb60569bec"',
        '"fe3d6297c08dccd8cacba13d50aa44dbb1c94b0ca8c2ab4df3a5c605466fcf94"',
        'CLEAN_OUTPUT_RELATIVES',
        '"target commit must differ from source"',
        '"target commit is not a descendant of source"',
        "require_absent_build_outputs=False",
        'stage_source_anchor',
        '"source artifact differs from prior-terminal chain anchor"',
        '"source evidence differs from prior-terminal chain anchor"',
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
    for token in (
        "class SourceAnchorStageError",
        "def read_exact_file",
        "def stage_exact_source_anchor",
        '"source package identity differs from artifact evidence"',
        "os.O_EXCL",
        "os.fsync",
        "canonical_json_bytes",
    ):
        require(
            sources.anchor,
            token,
            "prior-terminal source anchor staging contract is incomplete",
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
            "release pair verifier must not add runtime acceptance activation or overrides",
        )

    for token in (
        '"profile": "debian13-arm64-release-pair-v1"',
        '"source_artifact_policy": "prior-terminal-installed-artifact-v1"',
        '"source_rebuild": false',
        '"target_clean_root": true',
        '"repository_commit": "55351f21536d6dca3f90ab053c2a81e2b9bea354"',
        '"sha256": "09ed122804b11767b8ac7cd69c323c1f6eef511fd6ae7284d75756fb60569bec"',
        '"sha256": "fe3d6297c08dccd8cacba13d50aa44dbb1c94b0ca8c2ab4df3a5c605466fcf94"',
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

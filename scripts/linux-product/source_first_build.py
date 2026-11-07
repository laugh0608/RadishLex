#!/usr/bin/env python3
"""Source-first single artifact assembly and reviewed target-only pair orchestration."""
from __future__ import annotations

import argparse
import os
import platform
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import l6_release_pair as elf_tools
import source_first as admission
import source_first_contract as contract
from source_first_io import (MAX_ELF, MAX_JSON, MAX_PACKAGE, FileSnapshot, SourceFirstError,
                             canonical, check_tree, digest, load_json, new_private_root,
                             parse_json, publish, read_file, real_directory, stat_identity,
                             write_exclusive)
from source_first_process import BuildCommands
from source_first_stage import read_source_inputs, stage_source


SOURCE_ENVIRONMENT = "radishlex-linux-source-first-build-environment-v1"


def inspect_environment(repository: Path, *, deadline: float | None = None) -> tuple[dict[str, Any], FileSnapshot]:
    # The caller provisions isolation before this read-only probe; no namespace/network changes.
    if not sys.platform.startswith("linux") or platform.machine() not in {"aarch64", "arm64"}:
        raise SourceFirstError("build environment requires Debian 13 ARM64 Linux")
    if {path.name for path in Path("/sys/class/net").iterdir()} != {"lo"}:
        raise SourceFirstError("offline build requires a pre-provisioned loopback-only network")
    flutter = shutil.which("flutter")
    if flutter is None:
        raise SourceFirstError("Flutter must already be provisioned")
    sdk = Path(flutter).resolve().parents[1]
    if not (sdk / "bin/cache/artifacts/engine/linux-arm64-release").is_dir():
        raise SourceFirstError("offline Flutter ARM64 release engine cache is absent")
    config = repository / "apps/radishlex-manager/.dart_tool/package_config.json"
    raw, snapshot = read_file(config, mode=0o644)
    parse_json(raw, require_canonical=False)
    release = {}
    for line in Path("/etc/os-release").read_text().splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            release[key] = value.strip('"')
    operating_system = {"id": release.get("ID"), "version_id": release.get("VERSION_ID"),
                        "codename": release.get("VERSION_CODENAME")}
    contract.exact(operating_system, {"id": "debian", "version_id": "13", "codename": "trixie"},
                   "build operating system")
    versions = {}
    deadline = deadline if deadline is not None else time.monotonic() + 90
    environment = {key: value for key, value in os.environ.items()
                   if key in {"PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "PUB_CACHE", "FLUTTER_ROOT"}}
    environment.update(CARGO_NET_OFFLINE="true", RUSTUP_AUTO_INSTALL="0", LC_ALL="C")
    for label, arguments in (
            ("cargo", ["cargo", "--version"]), ("cmake", ["cmake", "--version"]),
            ("dpkg_shlibdeps", ["dpkg-shlibdeps", "--version"]),
            ("flutter", ["flutter", "--version", "--machine"]),
            ("git", ["/usr/bin/git", "--version"]), ("rustc", ["rustc", "--version"])):
        try:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise SourceFirstError("environment probe budget exhausted")
            result = subprocess.run(arguments, cwd=repository, check=True, stdin=subprocess.DEVNULL,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                    timeout=min(15, remaining), env=environment)
        except (OSError, subprocess.SubprocessError) as exc:
            raise SourceFirstError(f"frozen tool probe failed once: {label}") from exc
        if len(result.stdout) > 4096:
            raise SourceFirstError("tool version output exceeds 4 KiB")
        version = (parse_json(result.stdout, require_canonical=False)["frameworkVersion"]
                   if label == "flutter" else result.stdout.decode().splitlines()[0])
        versions[label] = elf_tools.safe_version(version, label)
    toolchain = {"architecture": "arm64", "format_version": 1, "operating_system": operating_system,
                 "profile": "debian13-arm64-release-pair-build-v1", "tools": versions}
    elf_tools.validate_environment(toolchain)
    value = {"format": SOURCE_ENVIRONMENT, "network": "loopback-only",
             "toolchain": toolchain, "pub_config_sha256": snapshot.sha256}
    return value, snapshot


def build_environment(work: Path) -> dict[str, str]:
    permitted = {"PATH", "HOME", "CARGO_HOME", "RUSTUP_HOME", "PUB_CACHE", "FLUTTER_ROOT"}
    environment = {key: value for key, value in os.environ.items() if key in permitted}
    environment.update({"LC_ALL": "C", "LANG": "C", "PYTHONDONTWRITEBYTECODE": "1",
                        "CARGO_NET_OFFLINE": "true", "CARGO_INCREMENTAL": "0",
                        "CARGO_BUILD_JOBS": "1", "RUST_TEST_THREADS": "1",
                        "CMAKE_BUILD_PARALLEL_LEVEL": "1", "GOMAXPROCS": "1",
                        "RADISHLEX_PRESERVE_BUILD_FAILURES": "1", "TMPDIR": str(work / "temporary")})
    environment["RUSTUP_AUTO_INSTALL"] = "0"
    if "PATH" not in environment:
        raise SourceFirstError("explicit tool PATH is required")
    return environment


def require_output(path: Path, repository: Path, work: Path | None = None) -> None:
    real_directory(path.parent)
    if (not path.is_absolute() or path.exists() or path.is_symlink() or
            path == repository or repository in path.parents or
            (work is not None and (path == work or work in path.parents or path in work.parents))):
        raise SourceFirstError("work/publication must be absent real paths outside the checkout")


def unchanged_checkout(repository: Path, commit: str, candidate: dict[str, Any],
                       target: bool) -> None:
    contract.exact(admission.git(repository, "rev-parse", "HEAD").decode().strip(), commit, "build HEAD")
    if admission.git(repository, "status", "--porcelain=v1", "--untracked-files=all",
                     "--ignore-submodules=none"):
        raise SourceFirstError("source changed during build")
    expected = candidate["source_metadata"]
    if target:
        expected = dict(expected, debian_revision="3", package_version="26.7.1+40-3")
    actual = parse_json((repository / admission.METADATA_PATH).read_bytes(), require_canonical=False)
    contract.exact(actual, expected, "build metadata")
    for path in contract.LOCKS + contract.RECIPE_PATHS:
        contract.exact((repository / path).read_bytes(), admission.blob(repository, commit, path),
                       f"build committed input {path}")


def copy_cargo_executable(source: Path, destination: Path) -> FileSnapshot:
    real_directory(source.parent)
    original = source.lstat()
    identity = stat_identity(original)
    alias = None
    if original.st_nlink == 2:
        deps = source.parent / "deps"
        real_directory(deps)
        name = re.escape(source.name.replace("-", "_"))
        alias = next((entry for entry in deps.iterdir()
                      if re.fullmatch(name + r"-[0-9a-f]{16}", entry.name)
                      and stat_identity(entry.lstat()) == identity), None)
        if alias is None:
            raise SourceFirstError(f"Cargo executable hardlink is outside its expected deps path: {source}")

    links = 2 if alias is not None else 1
    raw, before = read_file(source, mode=0o755, limit=MAX_ELF, links=links)
    if before.identity != identity:
        raise SourceFirstError(f"Cargo executable changed before copy: {source}")
    copied = write_exclusive(destination, raw, 0o755)
    read_file(source, mode=0o755, limit=MAX_ELF, previous=before, links=links)
    if alias is not None:
        real_directory(alias.parent)
        if stat_identity(alias.lstat()) != identity:
            raise SourceFirstError(f"Cargo executable hardlink changed during copy: {alias}")
    return copied


def executable_record(path: Path, commit: str, role: str) -> dict[str, Any]:
    _, before = read_file(path, mode=0o755, limit=MAX_ELF)
    elf = elf_tools.parse_aarch64_elf(
        path, "acceptance-controller" if role == "acceptance" else "production-maintenance")
    read_file(path, mode=0o755, limit=MAX_ELF, previous=before)
    return {"file": before.record(), "commit": commit, "elf": elf,
            "feature_profile": "compile-isolated-l6-acceptance-checkpoints"
            if role == "acceptance" else "no-default-features"}


def assemble_artifact(repository: Path, work: Path, handoff: Path, commit: str,
                      commands: BuildCommands, *, target: bool) -> tuple[dict[str, Any], dict[str, FileSnapshot]]:
    manager = repository / "apps/radishlex-manager/build/linux/arm64/release/bundle"
    addon, rootfs, artifacts = work / "addon", work / "rootfs", handoff / "artifacts"
    new_private_root(artifacts)
    commands.run([str(repository / "scripts/check-linux-product-metadata.sh")], repository, "metadata")
    commands.run([str(repository / "scripts/build-manager-linux-product.sh"), "--system-product-offline"],
                 repository, "manager")
    commands.run([str(repository / "scripts/build-linux-product-addon-stage.sh"),
                  "--ffi-library", str(manager / "lib/libradishlex_ime_ffi.so"),
                  "--output", str(addon)], repository, "addon")
    rootfs_tool = str(repository / "scripts/linux-product/rootfs.py")
    commands.run([sys.executable, rootfs_tool, "assemble", "--manager-bundle", str(manager),
                  "--addon-stage", str(addon), "--output", str(rootfs)], repository, "rootfs")
    commands.run([sys.executable, rootfs_tool, "verify", "--rootfs", str(rootfs)],
                 repository, "rootfs-verify")
    commands.run([str(repository / "scripts/check-linux-product-layout.sh"),
                  "--manager-bundle", str(manager), "--addon-stage", str(addon)], repository, "payload")
    # The existing Debian builder requires 0755; the outer handoff remains private.
    artifacts.chmod(0o755)
    commands.run([str(repository / "scripts/build-linux-deb-artifact.sh"),
                  "--rootfs", str(rootfs), "--output-dir", str(artifacts)], repository, "artifact")
    artifacts.chmod(0o700)
    cargo_home = Path(commands.environment.get("CARGO_HOME",
                                               str(Path(commands.environment.get("HOME", "")) / ".cargo")))
    if not cargo_home.is_absolute():
        raise SourceFirstError("absolute pre-provisioned Cargo home is required")
    commands.environment["CARGO_TARGET_DIR"] = str(repository / "target")
    commands.environment["CARGO_ENCODED_RUSTFLAGS"] = (
        f"--remap-path-prefix={repository}=/usr/src/radishlex\x1f"
        f"--remap-path-prefix={cargo_home}=/usr/src/cargo")
    cargo = ["cargo", "build", "--manifest-path", str(repository / "Cargo.toml"),
             "--locked", "--offline", "--release"]
    commands.run(cargo + ["--no-default-features", "-p", "radishlex-linux-product-install",
                          "--bin", "radishlex-linux-maintenance",
                          "--bin", "radishlex-linux-artifact-verifier"], repository, "production")
    roles = {"production": "radishlex-linux-maintenance", "verifier": "radishlex-linux-artifact-verifier"}
    if target:
        commands.run(cargo + ["-p", "radishlex-linux-l6-acceptance",
                              "--bin", "radishlex-linux-l6-acceptance"], repository, "acceptance")
        roles["acceptance"] = "radishlex-linux-l6-acceptance"
    files: dict[str, FileSnapshot] = {}
    executables = {}
    for role, binary in roles.items():
        files[binary] = copy_cargo_executable(repository / "target/release" / binary, handoff / binary)
        executables[role] = executable_record(handoff / binary, commit, role)
    if len({value["file"]["sha256"] for value in executables.values()}) != len(roles):
        raise SourceFirstError("build executables are not distinct")
    version = "26.7.1+40-3" if target else "26.7.1+40-2"
    package = artifacts / f"radishlex_{version}_arm64.deb"
    evidence = package.with_name(package.name + ".evidence.json")
    _, package_snapshot = read_file(package, mode=0o644, limit=MAX_PACKAGE)
    evidence_raw, evidence_snapshot = read_file(evidence, mode=0o644)
    value = parse_json(evidence_raw)
    contract.exact(value.get("package"), package_snapshot.record(), "built artifact package")
    control, manifest = value.get("control"), value.get("product_manifest")
    if not isinstance(control, dict) or not isinstance(manifest, dict):
        raise SourceFirstError("artifact control/manifest are absent")
    contract.exact(control.get("version"), version, "built artifact version")
    contract.exact(control.get("architecture"), "arm64", "built artifact architecture")
    dependencies = control.get("depends")
    if (not isinstance(dependencies, list) or not dependencies or
            not all(isinstance(item, str) and item and "\n" not in item for item in dependencies)):
        raise SourceFirstError("built dependencies are invalid")
    contract.hex_value(manifest.get("sha256"))
    result = commands.run([str(handoff / roles["verifier"]), "--package", str(package),
                           "--evidence", str(evidence)], repository, "actual-artifact")
    contract.exact(result, b"artifact_verification=ok\n", "actual verifier result")
    read_file(package, mode=0o644, limit=MAX_PACKAGE, previous=package_snapshot)
    read_file(evidence, mode=0o644, previous=evidence_snapshot)
    files["artifacts/" + package.name] = package_snapshot
    files["artifacts/" + evidence.name] = evidence_snapshot
    check_tree(artifacts, {package.name: package_snapshot, evidence.name: evidence_snapshot}, mode=0o644)
    return {"package": package_snapshot.record(), "artifact_evidence": evidence_snapshot.record(),
            "manifest_sha256": manifest["sha256"],
            "dependencies_sha256": digest(("\n".join(dependencies) + "\n").encode()),
            "executables": executables}, files


def seal(handoff: Path, output: Path, files: dict[str, FileSnapshot]) -> Path:
    inventory = {name: dict(item.record(), mode=item.identity[4] & 0o777)
                 for name, item in sorted(files.items())}
    files["inventory.json"] = write_exclusive(handoff / "inventory.json", canonical(inventory))
    return publish(handoff, output, files, mode=None)


def failure(work: Path, phase: str, error: BaseException) -> None:
    write_exclusive(work / "failure.json", canonical(
        {"phase": phase, "error_type": type(error).__name__, "result": "incomplete-preserved"}))


def build(repository: Path, work: Path, output: Path, environment_file: Path, budget: int,
          *, review_commit: str | None = None, seed_root: Path | None = None,
          review_path: Path | None = None, source_package: Path | None = None,
          source_evidence: Path | None = None) -> Path:
    if type(budget) is not int or not 1 <= budget <= 7200:
        raise SourceFirstError("explicit build budget must be 1..7200 seconds")
    deadline = time.monotonic() + budget
    target = review_commit is not None
    commit = admission.clean_head(repository)
    candidate = parse_json(admission.blob(repository, commit, admission.CANDIDATE_PATH))
    contract.validate_candidate(candidate)
    lineage = baseline = None
    source_snapshots: tuple[FileSnapshot, FileSnapshot] | None = None
    seed_snapshots: dict[str, FileSnapshot] = {}
    review_snapshot = None
    if target:
        if None in (seed_root, review_path, source_package, source_evidence):
            raise SourceFirstError("target requires the reviewed seed and exact source inputs")
        lineage = admission.pair_preflight(repository, review_commit, seed_root, review_path,
                                            source_package, source_evidence)
        baseline = parse_json(admission.blob(repository, review_commit, admission.BASELINE_PATH))
        _, _, package_before, evidence_before = read_source_inputs(
            source_package, source_evidence, baseline["source"])
        source_snapshots = (package_before, evidence_before)
        _, seed_snapshots, _ = admission.verify_seed(seed_root, candidate)
        _, review_snapshot = load_json(review_path)
    else:
        contract.exact(admission.committed_json(repository, commit, admission.METADATA_PATH),
                       candidate["source_metadata"], "source assembly metadata")
    require_output(work, repository)
    require_output(output, repository, work)
    if work.parent.stat().st_dev != output.parent.stat().st_dev:
        raise SourceFirstError("work and publication must share a filesystem")
    expected_environment, expected_snapshot = load_json(environment_file)
    actual_environment, pub_config = inspect_environment(repository, deadline=deadline)
    contract.exact(actual_environment, expected_environment, "frozen build environment")
    if time.monotonic() >= deadline:
        raise SourceFirstError("preflight exhausted the build budget")
    # All admission checks precede the first output or build command.
    new_private_root(work)
    phase = "initialize"
    try:
        for name in ("logs", "temporary", "handoff"):
            new_private_root(work / name)
        handoff = work / "handoff"
        commands = BuildCommands(work / "logs", budget, build_environment(work))
        commands.deadline = deadline
        write_exclusive(work / "request.json", canonical(
            {"commit": commit, "kind": "target" if target else "source", "budget_seconds": budget,
             "environment_sha256": expected_snapshot.sha256, "output": str(output)}))
        files: dict[str, FileSnapshot] = {}
        if target:
            phase = "exact-source-copy"
            copied = stage_source(source_package, source_evidence, handoff / "source", baseline["source"])
            files.update({"source/" + name: item for name, item in copied.items()})
        phase = "target-artifact" if target else "source-artifact"
        artifact, built_files = assemble_artifact(repository, work, handoff, commit, commands, target=target)
        files.update(built_files)
        phase = "record-and-revalidate"
        files["build-environment.json"] = write_exclusive(handoff / "build-environment.json",
                                                         canonical(actual_environment))
        if target:
            source = baseline["source"]
            for item in source_snapshots:
                read_file(item.path, limit=MAX_PACKAGE if item.path == source_package else MAX_JSON,
                          mode=0o644, previous=item)
            staged_package = handoff / "source" / source_package.name
            staged_evidence = handoff / "source" / source_evidence.name
            result = commands.run([str(handoff / "radishlex-linux-artifact-verifier"),
                                   "--package", str(staged_package), "--evidence", str(staged_evidence)],
                                  repository, "actual-source")
            contract.exact(result, b"artifact_verification=ok\n", "target verifier/source result")
            pair = {"format": contract.PAIR_FORMAT, "profile": contract.PROFILE,
                    "matrix_profile": contract.MATRIX, **{key: lineage[key] for key in
                    ("source_commit", "baseline_commit", "target_commit", "baseline_sha256", "seed_sha256")},
                    "review_sha256": baseline["review_sha256"], "source": source,
                    "target": {key: artifact[key] for key in
                               ("package", "artifact_evidence", "manifest_sha256", "dependencies_sha256")},
                    "executables": artifact["executables"],
                    "environment_sha256": expected_snapshot.sha256}
            contract.validate_pair(pair, baseline, review_commit, commit, lineage["baseline_sha256"])
            files["pair.evidence.json"] = write_exclusive(handoff / "pair.evidence.json", canonical(pair))
            # Revalidate private evidence without requiring the now-present expected build outputs absent.
            seed, seed_files, _ = admission.verify_seed(seed_root, candidate)
            for name, item in seed_snapshots.items():
                contract.exact(seed_files[name].identity, item.identity, "seed identity across build")
            contract.validate_baseline(baseline, candidate, seed, seed_files["seed.evidence.json"].sha256)
            read_file(review_path, previous=review_snapshot)
        else:
            source = {**artifact, "commit": commit,
                      "metadata_sha256": digest(canonical(candidate["source_metadata"])),
                      "locks": {path: digest(admission.blob(repository, commit, path)) for path in contract.LOCKS},
                      "recipe_sha256": digest(canonical(
                          {path: digest(admission.blob(repository, commit, path)) for path in contract.RECIPE_PATHS})),
                      "environment_sha256": expected_snapshot.sha256}
            contract.validate_source(source, candidate)
            files["source.json"] = write_exclusive(handoff / "source.json", canonical(source))
        unchanged_checkout(repository, commit, candidate, target)
        read_file(environment_file, previous=expected_snapshot)
        read_file(pub_config.path, mode=0o644, previous=pub_config)
        if time.monotonic() >= commands.deadline:
            raise SourceFirstError("build deadline exceeded before publication")
        phase = "publish"
        result = seal(handoff, output, files)
        if time.monotonic() >= commands.deadline:
            raise SourceFirstError("build deadline exceeded during publication; output retained")
        return result
    except BaseException as exc:
        try:
            failure(work, phase, exc)
        except BaseException as diagnostic:
            raise exc from diagnostic
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    if sys.argv[1:2] == ["inspect-environment"]:
        parser.add_argument("kind", choices=("inspect-environment",))
        parser.add_argument("--output", required=True, type=Path)
        args = parser.parse_args()
        try:
            require_output(args.output, admission.REPO_ROOT)
            value, _ = inspect_environment(admission.REPO_ROOT)
            write_exclusive(args.output, canonical(value))
            print(f"Unreviewed local build environment: {args.output}")
            return 0
        except (SourceFirstError, OSError, elf_tools.L6ReleasePairError) as exc:
            print(f"source-first environment probe rejected: {exc}", file=sys.stderr)
            return 1
    parser.add_argument("kind", choices=("source", "target"))
    parser.add_argument("--work", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--environment-file", required=True, type=Path)
    parser.add_argument("--budget-seconds", required=True, type=int)
    parser.add_argument("--allow-build-process-timeout-cleanup", action="store_true")
    parser.add_argument("--review-commit")
    parser.add_argument("--seed-root", type=Path)
    parser.add_argument("--review", type=Path)
    parser.add_argument("--source-package", type=Path)
    parser.add_argument("--source-artifact-evidence", type=Path)
    args = parser.parse_args()
    if not args.allow_build_process_timeout_cleanup:
        parser.error("this invocation requires explicit authorization for build process timeout cleanup")
    if (args.kind == "target") != (args.review_commit is not None):
        parser.error("only target assembly requires --review-commit")
    if args.kind == "source" and any(value is not None for value in
                                     (args.seed_root, args.review, args.source_package,
                                      args.source_artifact_evidence)):
        parser.error("source assembly rejects target/seed inputs")
    try:
        result = build(admission.REPO_ROOT, args.work, args.output, args.environment_file,
                       args.budget_seconds, review_commit=args.review_commit, seed_root=args.seed_root,
                       review_path=args.review, source_package=args.source_package,
                       source_evidence=args.source_artifact_evidence)
        print(f"Source-first {args.kind} handoff: {result}")
        return 0
    except (SourceFirstError, OSError, elf_tools.L6ReleasePairError) as exc:
        print(f"source-first assembly rejected; retain work and any published output: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

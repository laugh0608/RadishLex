#!/usr/bin/env python3
"""Read-only source-first seed and reviewed-pair admission. Never builds or installs."""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path
from typing import Any

import source_first_contract as contract
from source_first_stage import read_source_inputs
from source_first_io import (MAX_JSON, FileSnapshot, SourceFirstError, canonical,
                             check_tree, digest, load_json, parse_json, read_file,
                             real_directory)


REPO_ROOT = Path(__file__).resolve().parents[2]
CANDIDATE_PATH = "packaging/linux/source-first-candidate.json"
BASELINE_PATH = "packaging/linux/source-first-baseline.json"
LEGACY_CASE = "40-2-to-40-3"
FROZEN_CASE = "40-3-to-40-4"
CASE_PATHS = {
    LEGACY_CASE: (CANDIDATE_PATH, BASELINE_PATH),
    FROZEN_CASE: ("packaging/linux/source-first-40-4-candidate.json",
                  "packaging/linux/source-first-40-4-baseline.json"),
}
METADATA_PATH = "packaging/linux/product.json"
ARTIFACT_PATH = "packaging/linux/debian/artifact.json"


def case_candidate(repository: Path, case: str, commit: str | None = None) -> dict[str, Any]:
    candidate_path, _ = CASE_PATHS[case]
    if commit is None:
        candidate, _ = load_json(repository / candidate_path, mode=0o644)
    else:
        candidate = parse_json(blob(repository, commit, candidate_path))
    contract.validate_candidate(candidate)
    expected = contract.FROZEN_CANDIDATE_FORMAT if case == FROZEN_CASE else contract.CANDIDATE_FORMAT
    contract.exact(candidate["format"], expected, "candidate for selected case")
    return candidate


def git(root: Path, *args: str) -> bytes:
    try:
        result = subprocess.run(["/usr/bin/git", "-C", str(root), *args],
                                check=True, stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                timeout=15, env={"PATH": "/usr/bin:/bin", "LC_ALL": "C",
                                                 "GIT_OPTIONAL_LOCKS": "0"})
    except (OSError, subprocess.SubprocessError) as exc:
        raise SourceFirstError(f"Git admission failed: {args[0]}") from exc
    return result.stdout


def blob(root: Path, commit: str, path: str, *, limit: int = MAX_JSON) -> bytes:
    contract.hex_value(commit, 40)
    name = f"{commit}:{path}"
    size = git(root, "cat-file", "-s", name).strip()
    if not size.isdigit() or not 0 < int(size) <= limit:
        raise SourceFirstError(f"committed input exceeds limit: {path}")
    raw = git(root, "show", name)
    if len(raw) != int(size):
        raise SourceFirstError("committed input size differs")
    return raw


def committed_json(root: Path, commit: str, path: str) -> dict[str, Any]:
    return parse_json(blob(root, commit, path), require_canonical=False)


def clean_head(root: Path) -> str:
    real_directory(root)
    if git(root, "rev-parse", "--show-toplevel").decode().strip() != str(root):
        raise SourceFirstError("must use the real repository root")
    if git(root, "status", "--porcelain=v1", "--untracked-files=all", "--ignore-submodules=none"):
        raise SourceFirstError("tracked/untracked/submodule changes block build admission")
    from l6_release_pair import CLEAN_OUTPUT_RELATIVES
    for relative in CLEAN_OUTPUT_RELATIVES:
        if relative == Path("apps/radishlex-manager/.dart_tool"):
            # Offline pub configuration is a pinned input, not a prior product build.
            continue
        path = root / relative
        if path.exists() or path.is_symlink():
            raise SourceFirstError(f"prior build output blocks admission: {relative}")
    commit = git(root, "rev-parse", "HEAD").decode().strip()
    contract.hex_value(commit, 40)
    return commit


def strict_ancestor(root: Path, ancestor: str, descendant: str) -> None:
    contract.hex_value(ancestor, 40)
    contract.hex_value(descendant, 40)
    if ancestor == descendant:
        raise SourceFirstError("commit roles must be distinct")
    git(root, "merge-base", "--is-ancestor", ancestor, descendant)


def source_build_inputs(root: Path, commit: str) -> dict[str, Any]:
    metadata = committed_json(root, commit, METADATA_PATH)
    recipe = {path: digest(blob(root, commit, path)) for path in contract.RECIPE_PATHS}
    return {"metadata_sha256": digest(canonical(metadata)), "recipe_sha256": digest(canonical(recipe)),
            "locks": {path: digest(blob(root, commit, path)) for path in contract.LOCKS}}


def source_git_inputs(root: Path, source: dict[str, Any],
                       candidate: dict[str, Any]) -> None:
    commit = source["commit"]
    metadata = committed_json(root, commit, METADATA_PATH)
    contract.exact(metadata, candidate["source_metadata"], "S committed metadata")
    for key, expected in source_build_inputs(root, commit).items():
        contract.exact(source[key], expected, f"S {key}")
    if candidate["format"] == contract.FROZEN_CANDIDATE_FORMAT:
        # S was built as the old T. Its historical declaration describes that
        # earlier pair. The new declaration pins the original T and its bytes.
        contract.exact(commit, candidate["frozen_handoff"]["commit"], "frozen source commit")
        contract.validate_source(source, candidate)
        original = case_candidate(root, LEGACY_CASE, commit)
        contract.validate_metadata_pair(original["source_metadata"], metadata, original)
    else:
        contract.exact(parse_json(blob(root, commit, CANDIDATE_PATH)), candidate, "S candidate")


def inspect_frozen_source(repository: Path, handoff: Path, candidate: dict[str, Any]) -> dict[str, Any]:
    """Project the archived target without creating a seed, baseline or package."""
    contract.validate_candidate(candidate)
    contract.exact(candidate["format"], contract.FROZEN_CANDIDATE_FORMAT, "frozen source case")
    anchor = candidate["frozen_handoff"]
    inventory, snapshot = load_json(handoff / "inventory.json")
    contract.exact(snapshot.sha256, anchor["inventory_sha256"], "frozen inventory")
    files = {"inventory.json": snapshot}
    for relative, item in inventory.items():
        expected = {key: item[key] for key in ("filename", "size", "sha256")}
        _, files[relative] = read_file(handoff / relative, mode=item["mode"],
                                       limit=item["size"], expected=expected)
    pair, pair_file = load_json(handoff / "pair.evidence.json")
    contract.exact(pair_file.sha256, anchor["pair_sha256"], "frozen pair")
    commit = anchor["commit"]
    baseline_commit = pair["baseline_commit"]
    strict_ancestor(repository, pair["source_commit"], baseline_commit)
    strict_ancestor(repository, baseline_commit, commit)
    baseline_bytes = blob(repository, baseline_commit, BASELINE_PATH)
    contract.exact(blob(repository, commit, BASELINE_PATH), baseline_bytes, "original T retains R")
    contract.validate_pair(pair, parse_json(baseline_bytes), baseline_commit, commit,
                           digest(baseline_bytes))
    target = pair["target"]
    for field in ("package", "artifact_evidence"):
        entry = target[field]
        contract.exact(files["artifacts/" + entry["filename"]].record(), entry, f"original {field}")
    for entry in pair["executables"].values():
        binary = entry["file"]
        contract.exact(files[binary["filename"]].record(), binary, "original executable")
    contract.exact(files["build-environment.json"].sha256, pair["environment_sha256"],
                   "original build environment")
    source = dict(target, commit=commit, **source_build_inputs(repository, commit),
                  environment_sha256=pair["environment_sha256"],
                  executables={k: pair["executables"][k] for k in ("production", "verifier")})
    source_git_inputs(repository, source, candidate)
    read_source_inputs(handoff / "artifacts" / source["package"]["filename"],
                       handoff / "artifacts" / source["artifact_evidence"]["filename"], source,
                       candidate["source_metadata"]["package_version"])
    check_tree(handoff, files, mode=None)
    return source


def verify_seed(root: Path, candidate: dict[str, Any]) -> tuple[
        dict[str, Any], dict[str, FileSnapshot], dict[str, dict[str, Any]]]:
    real_directory(root, 0o700)
    seed, snapshot = load_json(root / "seed.evidence.json")
    contract.validate_seed(seed, candidate)
    files = {"seed.evidence.json": snapshot}
    observations: dict[str, dict[str, Any]] = {}
    for attachment in seed["attachments"]:
        raw, item = read_file(root / attachment["path"], expected=attachment["file"])
        observation = parse_json(raw)
        contract.validate_observation(observation, attachment["role"], seed, candidate)
        files[attachment["path"]] = item
        observations[attachment["role"]] = observation
    contract.exact(observations["host-readback"]["facts"]["result_sha256"],
                   observations["guest-result"]["facts"]["result_sha256"], "guest/host result")
    contract.exact(observations["host-readback"]["facts"]["guest_inventory_sha256"],
                   observations["host-readback"]["facts"]["host_inventory_sha256"],
                   "guest/host inventory")
    check_tree(root, files, mode=0o600)
    return seed, files, observations


def pair_preflight(repository: Path, review_commit: str, seed_root: Path,
                   review_path: Path, source_package: Path, source_evidence: Path, *,
                   case: str = LEGACY_CASE) -> dict[str, Any]:
    """Only a committed R baseline can admit T; no external baseline argument exists."""
    target_commit = clean_head(repository)
    candidate = case_candidate(repository, case, target_commit)
    candidate_path, baseline_path = CASE_PATHS[case]
    if case == FROZEN_CASE:
        strict_ancestor(repository, candidate["required_target_ancestor"], target_commit)
    baseline_raw = blob(repository, review_commit, baseline_path)
    baseline = parse_json(baseline_raw)
    seed, files, observations = verify_seed(seed_root, candidate)
    contract.validate_baseline(baseline, candidate, seed, files["seed.evidence.json"].sha256)
    source = baseline["source"]
    strict_ancestor(repository, source["commit"], review_commit)
    strict_ancestor(repository, review_commit, target_commit)
    contract.exact(blob(repository, target_commit, baseline_path), baseline_raw,
                   "T preserves R baseline bytes")
    contract.exact(blob(repository, review_commit, candidate_path), canonical(candidate),
                   "R candidate bytes")
    source_git_inputs(repository, source, candidate)
    _, _, package_snapshot, evidence_snapshot = read_source_inputs(
        source_package, source_evidence, source, candidate["source_metadata"]["package_version"])
    source_metadata = committed_json(repository, source["commit"], METADATA_PATH)
    contract.exact(committed_json(repository, review_commit, METADATA_PATH), source_metadata,
                   "R source metadata")
    contract.validate_metadata_pair(source_metadata,
                                   committed_json(repository, target_commit, METADATA_PATH),
                                   candidate)
    for commit in (review_commit, target_commit):
        for path in contract.LOCKS:
            contract.exact(digest(blob(repository, commit, path)), source["locks"][path],
                           f"R/T lock {path}")
    source_artifact = committed_json(repository, source["commit"], ARTIFACT_PATH)
    contract.exact(committed_json(repository, review_commit, ARTIFACT_PATH), source_artifact,
                   "R artifact configuration")
    target_name = f"radishlex_{candidate['target_version']}_arm64.deb"
    target_artifact = dict(source_artifact, package_filename=target_name,
                           evidence_filename=target_name + ".evidence.json")
    contract.exact(committed_json(repository, target_commit, ARTIFACT_PATH), target_artifact,
                   "T artifact configuration")
    review, review_snapshot = load_json(review_path)
    contract.exact(review_snapshot.sha256, baseline["review_sha256"], "independent review bytes")
    contract.validate_review(review, baseline)
    contract.exact(observations["receipt"]["facts"]["receipt_sha256"],
                   baseline["receipt_sha256"], "reviewed receipt")
    contract.exact(observations["guest-result"]["facts"]["result_sha256"],
                   baseline["guest_result_sha256"], "reviewed guest result")
    # Re-read every private input and HEAD; preflight has no writing/building callback.
    check_tree(seed_root, files, mode=0o600)
    read_file(review_path, previous=review_snapshot)
    read_file(source_package, limit=contract.MAX_PACKAGE, mode=0o644, previous=package_snapshot)
    read_file(source_evidence, mode=0o644, previous=evidence_snapshot)
    contract.exact(clean_head(repository), target_commit, "HEAD across admission")
    return {"source_commit": source["commit"], "baseline_commit": review_commit,
            "target_commit": target_commit, "baseline_sha256": digest(baseline_raw),
            "seed_sha256": baseline["seed_sha256"]}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", choices=CASE_PATHS, default=LEGACY_CASE)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("validate-candidate")
    frozen_parser = sub.add_parser("inspect-frozen-source")
    frozen_parser.add_argument("--handoff-root", required=True, type=Path)
    seed_parser = sub.add_parser("verify-seed")
    seed_parser.add_argument("--seed-root", required=True, type=Path)
    pair_parser = sub.add_parser("preflight-pair")
    pair_parser.add_argument("--review-commit", required=True)
    pair_parser.add_argument("--seed-root", required=True, type=Path)
    pair_parser.add_argument("--review", required=True, type=Path)
    pair_parser.add_argument("--source-package", required=True, type=Path)
    pair_parser.add_argument("--source-artifact-evidence", required=True, type=Path)
    args = parser.parse_args()
    try:
        candidate = case_candidate(REPO_ROOT, args.case)
        if args.command == "inspect-frozen-source":
            print(canonical(inspect_frozen_source(REPO_ROOT, args.handoff_root, candidate)).decode(), end="")
        elif args.command == "verify-seed":
            verify_seed(args.seed_root, candidate)
            print("Portable seed bytes and summaries verified; raw-material review is still required.")
        elif args.command == "preflight-pair":
            result = pair_preflight(REPO_ROOT, args.review_commit, args.seed_root, args.review,
                                    args.source_package, args.source_artifact_evidence, case=args.case)
            print(canonical(result).decode(), end="")
        else:
            metadata = parse_json((REPO_ROOT / METADATA_PATH).read_bytes(), require_canonical=False)
            contract.validate_candidate_metadata(metadata, candidate)
            print("Source-first candidate only; no reviewed baseline or target admission.")
        return 0
    except (SourceFirstError, OSError) as exc:
        print(f"source-first admission rejected: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

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
METADATA_PATH = "packaging/linux/product.json"
ARTIFACT_PATH = "packaging/linux/debian/artifact.json"


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


def source_git_inputs(root: Path, source: dict[str, Any],
                       candidate: dict[str, Any]) -> None:
    commit = source["commit"]
    metadata = committed_json(root, commit, METADATA_PATH)
    contract.exact(metadata, candidate["source_metadata"], "S committed metadata")
    contract.exact(digest(canonical(metadata)), source["metadata_sha256"], "S metadata digest")
    for path in contract.LOCKS:
        contract.exact(digest(blob(root, commit, path)), source["locks"][path], f"S lock {path}")
    recipe = {path: digest(blob(root, commit, path)) for path in contract.RECIPE_PATHS}
    contract.exact(digest(canonical(recipe)), source["recipe_sha256"], "S build recipe")
    committed_candidate = parse_json(blob(root, commit, CANDIDATE_PATH))
    contract.exact(committed_candidate, candidate, "S candidate")


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
                   review_path: Path, source_package: Path, source_evidence: Path) -> dict[str, Any]:
    """Only a committed R baseline can admit T; no external baseline argument exists."""
    target_commit = clean_head(repository)
    candidate = parse_json(blob(repository, target_commit, CANDIDATE_PATH))
    contract.validate_candidate(candidate)
    baseline_raw = blob(repository, review_commit, BASELINE_PATH)
    baseline = parse_json(baseline_raw)
    seed, files, observations = verify_seed(seed_root, candidate)
    contract.validate_baseline(baseline, candidate, seed, files["seed.evidence.json"].sha256)
    source = baseline["source"]
    strict_ancestor(repository, source["commit"], review_commit)
    strict_ancestor(repository, review_commit, target_commit)
    contract.exact(blob(repository, target_commit, BASELINE_PATH), baseline_raw,
                   "T preserves R baseline bytes")
    contract.exact(blob(repository, review_commit, CANDIDATE_PATH), canonical(candidate),
                   "R candidate bytes")
    source_git_inputs(repository, source, candidate)
    _, _, package_snapshot, evidence_snapshot = read_source_inputs(
        source_package, source_evidence, source)
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
    target_artifact = dict(source_artifact,
                           package_filename="radishlex_26.7.1+40-3_arm64.deb",
                           evidence_filename="radishlex_26.7.1+40-3_arm64.deb.evidence.json")
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
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("validate-candidate")
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
        candidate, _ = load_json(REPO_ROOT / CANDIDATE_PATH, mode=0o644)
        contract.validate_candidate(candidate)
        if args.command == "verify-seed":
            verify_seed(args.seed_root, candidate)
            print("Portable seed bytes and summaries verified; raw-material review is still required.")
        elif args.command == "preflight-pair":
            result = pair_preflight(REPO_ROOT, args.review_commit, args.seed_root, args.review,
                                    args.source_package, args.source_artifact_evidence)
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

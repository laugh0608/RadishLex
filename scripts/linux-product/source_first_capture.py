#!/usr/bin/env python3
"""Collect an explicit private seed capture into a portable envelope; never install or query a VM."""
from __future__ import annotations

import argparse
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import source_first as admission
import source_first_contract as contract
from source_first_build import executable_record, failure, require_output
from source_first_io import (MAX_ELF, MAX_JSON, MAX_PACKAGE, FileSnapshot, SourceFirstError,
                             canonical, check_tree, digest, load_json, new_private_root,
                             parse_json, publish, read_file, relative_path, write_exclusive)
from source_first_stage import read_source_inputs


CAPTURE_FORMAT = "radishlex-linux-source-private-capture-v1"
REPORT_FORMAT = "radishlex-linux-source-capture-report-v1"
MATERIAL_FIELDS = {
    "xdg": {"before_sha256", "after_sha256"},
    "guest-result": {"result_sha256"},
    "host-readback": {"result_sha256", "guest_inventory_sha256", "host_inventory_sha256"},
    "receipt": {"receipt_sha256", "root_staging_sha256"},
}


def verify_receipt(verifier: Path, receipt: Path, package: Path, evidence: Path,
                   budget: float) -> dict[str, Any]:
    try:
        result = subprocess.run(
            [str(verifier), "--seed-receipt", str(receipt), "--package", str(package),
             "--evidence", str(evidence)], check=True, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=max(0.001, budget),
            env={"PATH": "/usr/bin:/bin", "LC_ALL": "C"}, close_fds=True)
    except (OSError, subprocess.SubprocessError) as exc:
        raise SourceFirstError("typed source receipt verifier failed once") from exc
    return parse_json(result.stdout)


def capture_inputs(root: Path) -> tuple[dict[str, Any], dict[str, FileSnapshot],
                                         dict[str, dict[str, Any]]]:
    capture, snapshot = load_json(root / "capture.json")
    contract.fields(capture, {"format", "profile", "source", "binding", "roles"}, "private capture")
    contract.exact(capture["format"], CAPTURE_FORMAT, "capture format")
    contract.exact(capture["profile"], contract.PROFILE, "capture profile")
    contract.fields(capture["roles"], contract.ROLES, "capture roles")
    files = {"capture.json": snapshot}
    reports: dict[str, dict[str, Any]] = {}
    total = snapshot.size
    for role, entry in capture["roles"].items():
        contract.fields(entry, {"report", "materials"}, "capture entry")
        materials = entry["materials"]
        contract.fields(materials, MATERIAL_FIELDS.get(role, set()), "capture materials")
        references = {"report": entry["report"], **materials}
        for key, reference in references.items():
            contract.fields(reference, {"path", "file"}, "capture reference")
            path = relative_path(reference["path"])
            if path in files:
                raise SourceFirstError("capture paths must be unique")
            limit = 64 * 1024 if key == "receipt_sha256" else MAX_JSON
            contract.identity(reference["file"], limit=limit, filename=Path(path).name)
            raw, item = read_file(root / path, expected=reference["file"], limit=limit)
            files[path] = item
            total += len(raw)
            # Raw receipt has its own compact canonical encoding, owned by Rust.
            if key != "receipt_sha256":
                value = parse_json(raw)
                if key == "report":
                    contract.fields(value, {"format", "role", "binding", "facts"}, "capture report")
                    contract.exact(value["format"], REPORT_FORMAT, "capture report format")
                    contract.exact(value["role"], role, "capture report role")
                    contract.exact(value["binding"], capture["binding"], "capture report binding")
                    reports[role] = value
        if len(files) > 512 or total > 16 * 1024 * 1024:
            raise SourceFirstError("private structured capture exceeds resource limit")
    check_tree(root, files, mode=0o600)
    return capture, files, reports


def collect(capture_root: Path, package: Path, evidence: Path, verifier: Path,
            work: Path, output: Path, budget: int, candidate: dict[str, Any]) -> Path:
    if type(budget) is not int or not 1 <= budget <= 300:
        raise SourceFirstError("explicit collection budget must be 1..300 seconds")
    deadline = time.monotonic() + budget
    contract.validate_candidate(candidate)
    capture, snapshots, reports = capture_inputs(capture_root)
    source, binding = capture["source"], capture["binding"]
    contract.validate_source(source, candidate)
    contract.validate_binding(binding, source)
    admission.source_git_inputs(admission.REPO_ROOT, source, candidate)
    _, _, package_before, evidence_before = read_source_inputs(package, evidence, source)
    _, verifier_before = read_file(verifier, mode=0o755, limit=MAX_ELF,
                                   expected=source["executables"]["verifier"]["file"])
    contract.exact(executable_record(verifier, source["commit"], "verifier"),
                   source["executables"]["verifier"], "S actual verifier identity")
    require_output(work, capture_root)
    require_output(output, capture_root, work)
    if work.parent.stat().st_dev != output.parent.stat().st_dev:
        raise SourceFirstError("capture work and output must share a filesystem")
    new_private_root(work)
    phase = "typed-receipt"
    try:
        handoff = work / "handoff"
        new_private_root(handoff)
        receipt_entry = capture["roles"]["receipt"]
        receipt_path = capture_root / receipt_entry["materials"]["receipt_sha256"]["path"]
        verified = verify_receipt(verifier, receipt_path, package, evidence, deadline - time.monotonic())
        contract.fields(verified, {"facts", "receipt_sha256", "operation_sha256", "root_identity_sha256"},
                        "typed receipt result")
        contract.exact(verified["operation_sha256"], binding["operation_sha256"], "receipt operation")
        receipt_reference = receipt_entry["materials"]["receipt_sha256"]
        contract.exact(verified["receipt_sha256"], receipt_reference["file"]["sha256"], "raw receipt")
        root_reference = receipt_entry["materials"]["root_staging_sha256"]
        root_report, _ = load_json(capture_root / root_reference["path"])
        contract.fields(root_report, {"root_identity_sha256", "binding", "staging_inventory_sha256"},
                        "root/staging capture")
        contract.exact(root_report["binding"], binding, "root/staging binding")
        contract.exact(root_report["root_identity_sha256"], verified["root_identity_sha256"],
                       "receipt/root physical identity")
        contract.hex_value(root_report["staging_inventory_sha256"])
        phase = "portable-collection"
        files: dict[str, FileSnapshot] = {}
        seed = {"format": contract.SEED_FORMAT, "profile": contract.PROFILE,
                "source": source, "binding": binding, "attachments": []}
        for role in sorted(contract.ROLES):
            entry = capture["roles"][role]
            facts = reports[role]["facts"]
            if role == "receipt":
                contract.exact(facts, verified["facts"], "capture receipt and typed facts")
            if not isinstance(facts, dict):
                raise SourceFirstError("capture facts must be an object")
            materials = entry["materials"]
            derived = ({key: reference["file"]["sha256"] for key, reference in materials.items()}
                       if role in MATERIAL_FIELDS else
                       {"observation_sha256": entry["report"]["file"]["sha256"]})
            if set(facts) & set(derived):
                raise SourceFirstError("capture must not supply derived proof digests")
            observation = {"format": contract.OBSERVATION_FORMAT, "role": role,
                           "binding": binding, "facts": {**facts, **derived}}
            contract.validate_observation(observation, role, seed, candidate)
            name = role + ".json"
            item = write_exclusive(handoff / name, canonical(observation))
            files[name] = item
            seed["attachments"].append({"role": role, "path": name, "file": item.record()})
        contract.validate_seed(seed, candidate)
        files["seed.evidence.json"] = write_exclusive(handoff / "seed.evidence.json", canonical(seed))
        # This reuses the closed guest/host cross-checks; no success values are synthesized.
        admission.verify_seed(handoff, candidate)
        check_tree(capture_root, snapshots, mode=0o600)
        read_file(package, mode=0o644, limit=MAX_PACKAGE, previous=package_before)
        read_file(evidence, mode=0o644, previous=evidence_before)
        read_file(verifier, mode=0o755, limit=MAX_ELF, previous=verifier_before)
        if time.monotonic() >= deadline:
            raise SourceFirstError("collection deadline exceeded")
        phase = "publish"
        result = publish(handoff, output, files, mode=0o600)
        if time.monotonic() >= deadline:
            raise SourceFirstError("collection deadline exceeded during publication; output retained")
        write_exclusive(work / "result.json", canonical(
            {"format": "radishlex-linux-source-collection-result-v1",
             "seed_sha256": files["seed.evidence.json"].sha256, "raw_material_review": "required",
             "private_inventory_sha256": digest(canonical(
                 {name: item.record() for name, item in sorted(snapshots.items())}))}))
        return result
    except BaseException as exc:
        try:
            failure(work, phase, exc)
        except BaseException as diagnostic:
            raise exc from diagnostic
        raise


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("capture-root", "source-package", "source-artifact-evidence", "verifier", "work", "output"):
        parser.add_argument("--" + name, required=True, type=Path)
    parser.add_argument("--budget-seconds", required=True, type=int)
    args = parser.parse_args()
    try:
        candidate, _ = load_json(admission.REPO_ROOT / admission.CANDIDATE_PATH, mode=0o644)
        result = collect(args.capture_root, args.source_package, args.source_artifact_evidence,
                         args.verifier, args.work, args.output, args.budget_seconds, candidate)
        print(f"Portable source seed collected: {result}; raw-material review remains required.")
        return 0
    except (SourceFirstError, OSError) as exc:
        print(f"source seed collection rejected; preserve capture/work/output: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

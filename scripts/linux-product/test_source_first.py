#!/usr/bin/env python3
"""V1/V2 synthetic admission tests; every case remains under one registered new root."""
from __future__ import annotations

import argparse
import copy
import json
import os
import signal
import subprocess
import sys
import time
import unittest
from pathlib import Path
from unittest.mock import patch

import source_first as admission
import source_first_contract as contract
import source_first_io as storage
import source_first_stage as stage


ROOT: Path
CANDIDATE = json.loads((admission.REPO_ROOT / admission.CANDIDATE_PATH).read_bytes())


def sha(label: str) -> str:
    return storage.digest(label.encode())


def put(path: Path, value: object, mode: int = 0o600) -> dict[str, object]:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    data = value if isinstance(value, bytes) else storage.canonical(value)
    snapshot = storage.write_exclusive(path, data, mode)
    return snapshot.record()


class Fixture:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.candidate = copy.deepcopy(CANDIDATE)
        self.package = root / "radishlex_26.7.1+40-2_arm64.deb"
        package = put(self.package, b"!<arch>\nsynthetic-source-only\n", 0o644)
        artifact = {"package": package,
                    "control": {"version": "26.7.1+40-2", "architecture": "arm64",
                                "depends": ["libc6 (>= 2.41)", "fonts-noto-cjk"]},
                    "product_manifest": {"sha256": sha("source manifest")}}
        self.evidence = root / (self.package.name + ".evidence.json")
        evidence = put(self.evidence, artifact, 0o644)
        elf = {"class": "ELF64", "machine": "AArch64",
               "interpreter": "/lib/ld-linux-aarch64.so.1", "type": "dynamic-or-executable"}
        self.source = {
            "commit": "1" * 40, "metadata_sha256": sha("unused"),
            "locks": {path: sha(path) for path in contract.LOCKS},
            "recipe_sha256": sha("recipe"), "environment_sha256": sha("environment"),
            "package": package, "artifact_evidence": evidence,
            "manifest_sha256": artifact["product_manifest"]["sha256"],
            "dependencies_sha256": sha("libc6 (>= 2.41)\nfonts-noto-cjk\n"),
            "executables": {
                role: {"file": {"filename": binary, "size": 200, "sha256": sha(role)},
                       "commit": "1" * 40, "feature_profile": "no-default-features", "elf": elf}
                for role, binary in (("production", "radishlex-linux-maintenance"),
                                     ("verifier", "radishlex-linux-artifact-verifier"))}}
        self.source["metadata_sha256"] = storage.digest(
            storage.canonical(self.candidate["source_metadata"]))
        self.source["locks"][contract.LOCKS[2]] = self.candidate[
            "source_metadata"]["rime_data_lock_sha256"]

    def make_seed(self) -> None:
        self.seed_root = self.root / "seed"
        storage.new_private_root(self.seed_root)
        binding = {"attempt": "synthetic-seed-v1", "guest_sha256": sha("guest"),
                   "boot_sha256": sha("boot"), "operation_sha256": sha("operation"),
                   "source_sha256": storage.digest(storage.canonical(self.source)),
                   "executor_sha256": self.source["executables"]["production"]["file"]["sha256"]}
        artifact = contract.artifact_identity(self.source, self.candidate)
        facts = {
            "preflight": {"package": "not-installed", "state_root": "absent", "programs": "absent",
                          "xdg": "absent", "profile": "absent", "unknown_entries": 0,
                          "guard": "absent", "processes": 0, "observation_sha256": sha("preflight")},
            "receipt": {"operation_kind": "install", "version_relation": "not_applicable",
                        "state": "completed", "chain_length": 1,
                        "initial_package": {"state": "not_installed", "artifact": None},
                        "source_artifact": None, "source_proof": None, "target_artifact": artifact,
                        "target_proof": {"state": "installed", "artifact": artifact},
                        "failure_code": None, "failure_after_state": None,
                        "manual_recovery_required": False, "receipt_sha256": sha("receipt"),
                        "root_staging_sha256": sha("root staging")},
            "postflight": {"package": "installed", "artifact": artifact, "audit": "clean",
                           "inventory": "matched", "manifest": "matched", "md5": "matched",
                           "sha256": "matched", "root_staging_receipt": "matched",
                           "observation_sha256": sha("postflight")},
            "startup": {"manager": "AllowedProduct", "fcitx": "AllowedProduct",
                        "artifact": artifact, "observation_sha256": sha("startup")},
            "quiescence": {"business_processes": 0, "package_mutations": 0,
                           "guard": "normal-terminal", "guard_probe_writes": 0,
                           "observation_sha256": sha("quiescence")},
            "dependencies": {"dependencies_sha256": self.source["dependencies_sha256"],
                             "versions": "matched", "font_family": "matched",
                             "font_glyph": "matched", "font_owner": "matched",
                             "observation_sha256": sha("dependencies")},
            "network": {"policy": "isolated", "offline": "verified",
                        "observation_sha256": sha("network")},
            "xdg": {"before": "absent", "after": "absent", "profile_before": "absent",
                    "profile_after": "absent", "writes": 0,
                    "before_sha256": sha("empty xdg"), "after_sha256": sha("empty xdg")},
            "guest-result": {"status": "completed", "artifact": artifact, "result_sha256": sha("result")},
            "host-readback": {"agent_result": "matched", "fixed_files": "matched",
                              "inventory": "closed", "result_sha256": sha("result"),
                              "guest_inventory_sha256": sha("inventory"),
                              "host_inventory_sha256": sha("inventory")},
            "host-stop": {"running_guests_before": 1, "target": "stopped",
                          "other_running_guests_after": 0, "method": "normal-shutdown",
                          "host_cross_check": "matched", "observation_sha256": sha("stop")},
            "input-stability": {"before_after": "identity-and-hash-matched",
                                "package": self.source["package"],
                                "artifact_evidence": self.source["artifact_evidence"],
                                "observation_sha256": sha("stable")},
        }
        self.observations = {}
        attachments = []
        for role in sorted(contract.ROLES):
            value = {"format": contract.OBSERVATION_FORMAT, "role": role,
                     "binding": binding, "facts": facts[role]}
            path = role + ".json"
            file = put(self.seed_root / path, value)
            attachments.append({"role": role, "path": path, "file": file})
            self.observations[role] = value
        self.seed = {"format": contract.SEED_FORMAT, "profile": contract.PROFILE,
                     "source": self.source, "binding": binding, "attachments": attachments}
        self.seed_identity = put(self.seed_root / "seed.evidence.json", self.seed)

    def make_baseline(self) -> None:
        inventory = storage.digest(storage.canonical(self.seed["attachments"]))
        self.review = {
            "format": contract.REVIEW_FORMAT, "profile": contract.PROFILE,
            "seed_sha256": self.seed_identity["sha256"], "source_commit": self.source["commit"],
            "binding": self.seed["binding"], "inventory_sha256": inventory,
            "receipt_sha256": sha("receipt"),
            "checks": {role: "raw-material-reviewed" for role in sorted(contract.ROLES)}}
        self.review_path = self.root / "independent-review.json"
        review = put(self.review_path, self.review)
        self.baseline = {
            "format": contract.BASELINE_FORMAT, "profile": contract.PROFILE,
            "candidate_sha256": storage.digest(storage.canonical(self.candidate)),
            "source": self.source, "seed_sha256": self.seed_identity["sha256"],
            "binding": self.seed["binding"], "review_sha256": review["sha256"],
            "inventory_sha256": inventory, "receipt_sha256": sha("receipt"),
            "guest_result_sha256": sha("result"),
            "target_revision": "3", "target_version": "26.7.1+40-3"}

    def command(self, *args: str) -> str:
        result = subprocess.run(
            ["/usr/bin/git", "-C", str(self.repo), "-c", "user.name=Synthetic Fixture",
             "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false",
             "-c", "core.hooksPath=/dev/null", *args],
            check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15,
            env={"PATH": "/usr/bin:/bin", "LC_ALL": "C", "GIT_CONFIG_NOSYSTEM": "1",
                 "GIT_CONFIG_GLOBAL": "/dev/null"})
        return result.stdout.decode().strip()

    def commit(self, message: str) -> str:
        self.command("add", ".")
        self.command("commit", "-m", message)
        return self.command("rev-parse", "HEAD")

    def repository(self) -> None:
        self.repo = self.root / "repository"
        self.repo.mkdir(mode=0o700)
        self.command("init", "--initial-branch=dev")
        paths = set(contract.LOCKS + contract.RECIPE_PATHS) | {
            admission.METADATA_PATH, admission.CANDIDATE_PATH}
        for path in paths:
            put(self.repo / path, (admission.REPO_ROOT / path).read_bytes(), 0o644)
        # Synthetic S stays source revision even when this suite is later run at real T.
        (self.repo / admission.METADATA_PATH).write_bytes(
            storage.canonical(self.candidate["source_metadata"]))
        artifact_path = self.repo / admission.ARTIFACT_PATH
        artifact = json.loads(artifact_path.read_bytes())
        artifact.update(package_filename=self.package.name,
                        evidence_filename=self.evidence.name)
        artifact_path.write_bytes(storage.canonical(artifact))
        self.s = self.commit("synthetic S")
        self.source["commit"] = self.s
        for executable in self.source["executables"].values():
            executable["commit"] = self.s
        for path in contract.LOCKS:
            self.source["locks"][path] = storage.digest((self.repo / path).read_bytes())
        recipe = {path: storage.digest((self.repo / path).read_bytes()) for path in contract.RECIPE_PATHS}
        self.source["recipe_sha256"] = storage.digest(storage.canonical(recipe))
        self.make_seed()
        self.make_baseline()
        put(self.repo / admission.BASELINE_PATH, self.baseline, 0o644)
        self.r = self.commit("synthetic R reviewed anchor")
        metadata = dict(self.candidate["source_metadata"],
                        debian_revision="3", package_version="26.7.1+40-3")
        (self.repo / admission.METADATA_PATH).write_bytes(storage.canonical(metadata))
        artifact_path = self.repo / admission.ARTIFACT_PATH
        artifact = json.loads(artifact_path.read_bytes())
        artifact.update(package_filename="radishlex_26.7.1+40-3_arm64.deb",
                        evidence_filename="radishlex_26.7.1+40-3_arm64.deb.evidence.json")
        artifact_path.write_bytes(storage.canonical(artifact))
        self.t = self.commit("synthetic T")


class BudgetExpired(KeyboardInterrupt):
    pass


def budget_expired(signum, frame) -> None:
    raise BudgetExpired("V1/V2 exceeded 300 seconds")


class SourceFirstTests(unittest.TestCase):
    def setUp(self) -> None:
        self.root = ROOT / self._testMethodName
        storage.new_private_root(self.root)
        self.fixture = Fixture(self.root)

    def reject(self, call, *args, **kwargs) -> None:
        with self.assertRaises((storage.SourceFirstError, OSError, RuntimeError)):
            call(*args, **kwargs)

    def test_candidate_cannot_be_baseline(self) -> None:
        f = self.fixture
        f.make_seed()
        self.reject(contract.validate_baseline, f.candidate, f.candidate, f.seed,
                    f.seed_identity["sha256"])

    def test_static_candidate_gate_accepts_source_and_target_without_anchor(self) -> None:
        candidate = self.fixture.candidate
        source = candidate["source_metadata"]
        contract.validate_candidate_metadata(source, candidate)
        target = dict(source, debian_revision="3", package_version="26.7.1+40-3")
        contract.validate_candidate_metadata(target, candidate)
        self.reject(contract.validate_candidate_metadata,
                    dict(target, debian_revision="4"), candidate)
        # Static format validation is not a reviewed anchor and cannot grant pair admission.
        self.fixture.make_seed()
        self.reject(contract.validate_baseline, candidate, candidate, self.fixture.seed,
                    self.fixture.seed_identity["sha256"])
        # Exercise the fixture and real admission path from a target-revision checkout.
        template = self.root / "target-template"
        storage.new_private_root(template)
        paths = set(contract.LOCKS + contract.RECIPE_PATHS) | {
            admission.METADATA_PATH, admission.CANDIDATE_PATH}
        for path in paths:
            put(template / path, (admission.REPO_ROOT / path).read_bytes(), 0o644)
        (template / admission.METADATA_PATH).write_bytes(storage.canonical(target))
        artifact_path = template / admission.ARTIFACT_PATH
        artifact = json.loads(artifact_path.read_bytes())
        artifact.update(package_filename="radishlex_26.7.1+40-3_arm64.deb",
                        evidence_filename="radishlex_26.7.1+40-3_arm64.deb.evidence.json")
        artifact_path.write_bytes(storage.canonical(artifact))
        # The first seed is retained; use a separate fixture for this independent chain.
        chain_root = self.root / "target-chain"
        storage.new_private_root(chain_root)
        fixture = Fixture(chain_root)
        with patch.object(admission, "REPO_ROOT", template):
            fixture.repository()
        result = admission.pair_preflight(
            fixture.repo, fixture.r, fixture.seed_root, fixture.review_path,
            fixture.package, fixture.evidence)
        self.assertEqual(result["source_commit"], fixture.s)

    def test_valid_seed_and_baseline_summaries(self) -> None:
        f = self.fixture
        f.make_seed()
        f.make_baseline()
        seed, files, _ = admission.verify_seed(f.seed_root, f.candidate)
        contract.validate_baseline(f.baseline, f.candidate, seed, files["seed.evidence.json"].sha256)
        contract.validate_review(f.review, f.baseline)

    def test_unknown_fields_profiles_and_zero_digests(self) -> None:
        f = self.fixture
        f.make_seed()
        for key, value in (("unknown", 1), ("profile", contract.MATRIX),
                           ("format", contract.BASELINE_FORMAT)):
            bad = copy.deepcopy(f.seed)
            bad[key] = value
            self.reject(contract.validate_seed, bad, f.candidate)
        bad = copy.deepcopy(f.seed)
        bad["binding"]["guest_sha256"] = "0" * 64
        self.reject(contract.validate_seed, bad, f.candidate)

    def test_noncanonical_duplicate_bom_and_oversized_json(self) -> None:
        for raw in (b'{"a":1}', b'{"a":1,"a":2}', b'\xef\xbb\xbf{}\n',
                    b" " * (storage.MAX_JSON + 1), b'{"a":NaN}', b"[]\n"):
            self.reject(storage.parse_json, raw)

    def test_unsafe_attachment_paths_counts_and_sizes(self) -> None:
        f = self.fixture
        f.make_seed()
        for path in ("/tmp/x", "../x", "a/../x", "a//x", "./x", "a" * 181, "x\\y"):
            bad = copy.deepcopy(f.seed)
            bad["attachments"][0]["path"] = path
            self.reject(contract.validate_seed, bad, f.candidate)
        for action in ("missing", "duplicate", "oversize"):
            bad = copy.deepcopy(f.seed)
            if action == "missing":
                bad["attachments"].pop()
            elif action == "duplicate":
                bad["attachments"][0] = bad["attachments"][1]
            else:
                bad["attachments"][0]["file"]["size"] = storage.MAX_JSON + 1
            self.reject(contract.validate_seed, bad, f.candidate)

    def test_missing_terminal_cli_only_and_incomplete_postflight(self) -> None:
        f = self.fixture
        f.make_seed()
        for role, key, value in (("receipt", "state", "prepared"),
                                 ("receipt", "chain_length", 2),
                                 ("receipt", "target_proof", None),
                                 ("receipt", "manual_recovery_required", True),
                                 ("postflight", "audit", "unknown"),
                                 ("startup", "manager", "unknown"),
                                 ("host-stop", "method", "forced"),
                                 ("host-readback", "fixed_files", "unknown")):
            bad = copy.deepcopy(f.observations[role])
            bad["facts"][key] = value
            self.reject(contract.validate_observation, bad, role, f.seed, f.candidate)
        cli = {"exit_code": 0}
        self.reject(contract.validate_observation, cli, "receipt", f.seed, f.candidate)

    def test_observation_guest_boot_operation_and_executor_binding(self) -> None:
        f = self.fixture
        f.make_seed()
        for key in contract.BINDING_KEYS:
            bad = copy.deepcopy(f.observations["receipt"])
            bad["binding"][key] = "another-attempt-v1" if key == "attempt" else sha("other " + key)
            self.reject(contract.validate_observation, bad, "receipt", f.seed, f.candidate)
        for key in ("commit", "feature_profile"):
            bad = copy.deepcopy(f.seed)
            bad["source"]["executables"]["production"][key] = (
                "2" * 40 if key == "commit" else "acceptance")
            self.reject(contract.validate_seed, bad, f.candidate)

    def test_bool_cannot_replace_count_or_size(self) -> None:
        f = self.fixture
        f.make_seed()
        bad = copy.deepcopy(f.observations["receipt"])
        bad["facts"]["chain_length"] = True
        self.reject(contract.validate_observation, bad, "receipt", f.seed, f.candidate)
        bad = copy.deepcopy(f.seed)
        bad["source"]["package"]["size"] = True
        self.reject(contract.validate_seed, bad, f.candidate)

    def test_receipt_artifact_and_source_proof_binding(self) -> None:
        f = self.fixture
        f.make_seed()
        for key in ("target_artifact", "target_proof", "source_artifact", "source_proof"):
            bad = copy.deepcopy(f.observations["receipt"])
            bad["facts"][key] = {"package_version": "26.7.1+38-1"}
            self.reject(contract.validate_observation, bad, "receipt", f.seed, f.candidate)

    def test_valid_git_lineage_readonly_and_no_build(self) -> None:
        f = self.fixture
        f.repository()
        with patch("source_first_stage.stage_source", side_effect=AssertionError("mutation")):
            result = admission.pair_preflight(f.repo, f.r, f.seed_root, f.review_path, f.package, f.evidence)
        self.assertEqual(result["source_commit"], f.s)
        self.assertEqual(result["target_commit"], f.t)
        self.assertEqual(f.command("status", "--porcelain"), "")
        self.assertFalse((f.repo / "target").exists())

    def test_git_roles_equal_reversed_or_unrelated(self) -> None:
        f = self.fixture
        f.repository()
        for ancestor, descendant in ((f.s, f.s), (f.t, f.r), (f.r, f.s)):
            self.reject(admission.strict_ancestor, f.repo, ancestor, descendant)
        f.command("checkout", "--orphan", "unrelated")
        f.command("rm", "-rf", ".")
        put(f.repo / "unrelated.txt", b"synthetic unrelated\n", 0o644)
        unrelated = f.commit("unrelated root")
        self.reject(admission.strict_ancestor, f.repo, f.s, unrelated)

    def test_dirty_tracked_rejected_without_mutation(self) -> None:
        f = self.fixture
        f.repository()
        path = f.repo / admission.METADATA_PATH
        path.write_bytes(path.read_bytes() + b" ")
        before = path.read_bytes()
        self.reject(admission.pair_preflight, f.repo, f.r, f.seed_root, f.review_path, f.package, f.evidence)
        self.assertEqual(path.read_bytes(), before)

    def test_untracked_rejected_without_cleanup(self) -> None:
        f = self.fixture
        f.repository()
        path = f.repo / "untracked.txt"
        put(path, b"keep this\n", 0o644)
        self.reject(admission.pair_preflight, f.repo, f.r, f.seed_root, f.review_path, f.package, f.evidence)
        self.assertEqual(path.read_bytes(), b"keep this\n")

    def test_ignored_build_output_rejected(self) -> None:
        f = self.fixture
        f.repository()
        (f.repo / ".git/info/exclude").write_text("target/\n")
        (f.repo / "target").mkdir()
        self.assertEqual(f.command("status", "--porcelain"), "")
        self.reject(admission.pair_preflight, f.repo, f.r, f.seed_root, f.review_path,
                    f.package, f.evidence)
        self.assertTrue((f.repo / "target").is_dir())

    def test_target_artifact_filename_drift_rejected(self) -> None:
        f = self.fixture
        f.repository()
        path = f.repo / admission.ARTIFACT_PATH
        value = json.loads(path.read_bytes())
        value["package_filename"] = f.package.name
        path.write_bytes(storage.canonical(value))
        f.commit("target filename drift")
        self.reject(admission.pair_preflight, f.repo, f.r, f.seed_root, f.review_path,
                    f.package, f.evidence)

    def test_metadata_locks_dependency_and_revision_drift(self) -> None:
        f = self.fixture
        source = f.candidate["source_metadata"]
        valid_target = dict(source, debian_revision="3", package_version="26.7.1+40-3")
        contract.validate_metadata_pair(source, valid_target, f.candidate)
        for key, value in (("debian_revision", "4"), ("package_version", "26.7.1+40-4"),
                           ("ffi_abi_version", 10), ("rime_data_lock_sha256", sha("other")),
                           ("hard_dependencies", ["other"]), ("unknown", "value")):
            bad = dict(valid_target, **{key: value})
            self.reject(contract.validate_metadata_pair, source, bad, f.candidate)
        f.repository()
        lock = f.repo / contract.LOCKS[0]
        lock.write_bytes(lock.read_bytes() + b"\n# drift\n")
        f.commit("changed target lock")
        self.reject(admission.pair_preflight, f.repo, f.r, f.seed_root, f.review_path, f.package, f.evidence)

    def test_baseline_absent_candidate_cannot_admit_target(self) -> None:
        f = self.fixture
        f.repository()
        self.reject(admission.pair_preflight, f.repo, f.s, f.seed_root, f.review_path, f.package, f.evidence)
        self.assertFalse((f.repo / "target").exists())

    def test_r_baseline_cannot_be_changed_in_t(self) -> None:
        f = self.fixture
        f.repository()
        path = f.repo / admission.BASELINE_PATH
        bad = copy.deepcopy(f.baseline)
        bad["review_sha256"] = sha("replacement")
        path.write_bytes(storage.canonical(bad))
        f.commit("replaced baseline")
        self.reject(admission.pair_preflight, f.repo, f.r, f.seed_root, f.review_path, f.package, f.evidence)

    def test_host_guest_readback_mismatch(self) -> None:
        f = self.fixture
        f.make_seed()
        path = f.seed_root / "host-readback.json"
        value = copy.deepcopy(f.observations["host-readback"])
        value["facts"]["result_sha256"] = sha("wrong result")
        path.write_bytes(storage.canonical(value))
        seed = copy.deepcopy(f.seed)
        for entry in seed["attachments"]:
            if entry["role"] == "host-readback":
                entry["file"] = storage.read_file(path)[1].record()
        (f.seed_root / "seed.evidence.json").write_bytes(storage.canonical(seed))
        self.reject(admission.verify_seed, f.seed_root, f.candidate)

    def test_review_and_receipt_digest_cannot_be_substituted(self) -> None:
        f = self.fixture
        f.repository()
        value = copy.deepcopy(f.review)
        value["receipt_sha256"] = sha("wrong receipt")
        f.review_path.write_bytes(storage.canonical(value))
        self.reject(admission.pair_preflight, f.repo, f.r, f.seed_root, f.review_path, f.package, f.evidence)

    def test_seed_unknown_file_and_empty_directory(self) -> None:
        f = self.fixture
        f.make_seed()
        put(f.seed_root / "unknown.json", {})
        self.reject(admission.verify_seed, f.seed_root, f.candidate)
        # A second independent inventory, never remove the first failed material.
        root = self.root / "empty-directory-case"
        storage.new_private_root(root)
        item = storage.write_exclusive(root / "a.json", storage.canonical({}))
        (root / "unknown").mkdir(mode=0o700)
        self.reject(storage.check_tree, root, {"a.json": item}, mode=0o600)

    def test_symlink_file_and_parent_rejected(self) -> None:
        link = self.root / "link"
        link.symlink_to(self.fixture.package)
        self.reject(storage.read_file, link, mode=0o644)
        parent = self.root / "parent-link"
        parent.symlink_to(self.root, target_is_directory=True)
        self.reject(storage.read_file, parent / self.fixture.package.name, mode=0o644)

    def test_hardlink_and_mode_owner_drift_rejected(self) -> None:
        path = self.fixture.package
        os.link(path, self.root / "hardlink")
        self.reject(storage.read_file, path, mode=0o644)
        other = self.root / "mode.json"
        put(other, {})
        other.chmod(0o644)
        self.reject(storage.read_file, other)
        info = other.stat()
        fake = list(info)
        fake[4] = os.getuid() + 1
        with patch.object(Path, "lstat", return_value=os.stat_result(fake)):
            self.reject(storage.read_file, other, mode=0o644)

    def test_same_byte_inode_replacement_rejected(self) -> None:
        path = self.fixture.package
        raw, snapshot = storage.read_file(path, mode=0o644)
        replacement = self.root / "replacement"
        put(replacement, raw, 0o644)
        os.replace(replacement, path)
        self.reject(storage.read_file, path, mode=0o644, previous=snapshot)

    def test_replacement_during_read_rejected(self) -> None:
        path = self.fixture.package
        replacement = self.root / "replacement"
        put(replacement, path.read_bytes(), 0o644)
        original_read = os.read
        changed = False

        def replacing_read(descriptor: int, size: int) -> bytes:
            nonlocal changed
            value = original_read(descriptor, size)
            if not changed:
                changed = True
                os.replace(replacement, path)
            return value

        with patch.object(storage.os, "read", side_effect=replacing_read):
            self.reject(storage.read_file, path, mode=0o644)

    def test_same_version_different_bytes_rejected(self) -> None:
        f = self.fixture
        f.package.write_bytes(f.package.read_bytes().replace(b"source", b"mutate"))
        self.reject(stage.stage_source, f.package, f.evidence, self.root / "output", f.source)
        self.assertFalse((self.root / "output").exists())

    def test_mixed_evidence_rejected_before_output(self) -> None:
        f = self.fixture
        value = json.loads(f.evidence.read_bytes())
        value["package"]["sha256"] = sha("different package")
        f.evidence.write_bytes(storage.canonical(value))
        source = copy.deepcopy(f.source)
        source["artifact_evidence"] = storage.read_file(f.evidence, mode=0o644)[1].record()
        self.reject(stage.stage_source, f.package, f.evidence, self.root / "output", source)
        self.assertFalse((self.root / "output").exists())

    def test_exact_source_copy_and_publication(self) -> None:
        f = self.fixture
        staging = self.root / "staging"
        files = stage.stage_source(f.package, f.evidence, staging, f.source)
        output = self.root / "published"
        storage.publish(staging, output, files, mode=0o644)
        self.assertEqual((output / f.package.name).read_bytes(), f.package.read_bytes())
        self.assertFalse(staging.exists())
        self.assertEqual(output.stat().st_mode & 0o777, 0o700)

    def test_existing_output_is_preserved(self) -> None:
        f = self.fixture
        output = self.root / "output"
        storage.new_private_root(output)
        put(output / "keep.json", {"keep": True})
        self.reject(stage.stage_source, f.package, f.evidence, output, f.source)
        self.assertEqual(json.loads((output / "keep.json").read_bytes()), {"keep": True})

    def test_copy_partial_initialization_preserved(self) -> None:
        f = self.fixture
        output = self.root / "output"
        original_write = stage.write_exclusive

        def fail_evidence(path, data, mode=0o600):
            if path.name == f.evidence.name:
                raise OSError("synthetic evidence write failure")
            return original_write(path, data, mode)

        with patch.object(stage, "write_exclusive", side_effect=fail_evidence):
            self.reject(stage.stage_source, f.package, f.evidence, output, f.source)
        self.assertTrue((output / f.package.name).is_file())
        self.assertTrue((output / "failure.json").is_file())
        self.assertFalse((output / f.evidence.name).exists())

    def test_copy_panic_preserves_first_file_and_diagnostic(self) -> None:
        f = self.fixture
        output = self.root / "output"
        original_write = stage.write_exclusive

        def panic(path, data, mode=0o600):
            if path.name == f.evidence.name:
                raise KeyboardInterrupt("synthetic panic")
            return original_write(path, data, mode)

        with patch.object(stage, "write_exclusive", side_effect=panic):
            with self.assertRaises(KeyboardInterrupt):
                stage.stage_source(f.package, f.evidence, output, f.source)
        self.assertTrue((output / f.package.name).is_file())
        self.assertEqual(json.loads((output / "failure.json").read_bytes())["error_type"],
                         "KeyboardInterrupt")

    def test_fsync_failure_preserves_partial_file(self) -> None:
        path = self.root / "partial.json"
        with patch.object(storage.os, "fsync", side_effect=OSError("synthetic fsync")):
            self.reject(storage.write_exclusive, path, b"partial material\n")
        self.assertEqual(path.read_bytes(), b"partial material\n")

    def test_initial_directory_sync_failure_preserves_directory(self) -> None:
        output = self.root / "initializing"
        with patch.object(storage, "fsync_directory", side_effect=OSError("initial sync")):
            self.reject(storage.new_private_root, output)
        self.assertTrue(output.is_dir())
        self.assertEqual(output.stat().st_mode & 0o777, 0o700)

    def test_publish_file_sync_failure_preserves_staging(self) -> None:
        f = self.fixture
        root, output = self.root / "staging", self.root / "output"
        files = stage.stage_source(f.package, f.evidence, root, f.source)
        with patch.object(storage.os, "fsync", side_effect=OSError("publish file sync")):
            self.reject(storage.publish, root, output, files, mode=0o644)
        self.assertTrue((root / f.package.name).is_file())
        self.assertFalse(output.exists())

    def test_publish_same_byte_inode_replacement_rejected(self) -> None:
        f = self.fixture
        root, output = self.root / "staging", self.root / "output"
        files = stage.stage_source(f.package, f.evidence, root, f.source)
        replacement = self.root / "same-bytes"
        put(replacement, f.package.read_bytes(), 0o644)
        os.replace(replacement, root / f.package.name)
        self.reject(storage.publish, root, output, files, mode=0o644)
        self.assertTrue(root.exists())
        self.assertFalse(output.exists())

    def test_publish_rename_failure_preserves_staging(self) -> None:
        f = self.fixture
        root = self.root / "staging"
        files = stage.stage_source(f.package, f.evidence, root, f.source)
        with patch.object(storage, "rename_directory_no_replace", side_effect=OSError("rename")):
            self.reject(storage.publish, root, self.root / "output", files, mode=0o644)
        self.assertTrue((root / f.package.name).exists())
        self.assertFalse((self.root / "output").exists())

    def test_publish_parent_fsync_failure_preserves_published(self) -> None:
        f = self.fixture
        root, output = self.root / "staging", self.root / "output"
        files = stage.stage_source(f.package, f.evidence, root, f.source)
        original = storage.fsync_directory

        def fail_parent(path):
            if path == output.parent:
                raise OSError("synthetic parent sync")
            return original(path)

        with patch.object(storage, "fsync_directory", side_effect=fail_parent):
            self.reject(storage.publish, root, output, files, mode=0o644)
        self.assertTrue((output / f.package.name).is_file())
        self.assertFalse(root.exists())

    def test_publish_conflict_never_overwrites_destination(self) -> None:
        f = self.fixture
        root, output = self.root / "staging", self.root / "output"
        files = stage.stage_source(f.package, f.evidence, root, f.source)
        original = storage.rename_directory_no_replace

        def conflict(source, destination):
            destination.mkdir(mode=0o700)
            put(destination / "keep.json", {"keep": True})
            return original(source, destination)

        with patch.object(storage, "rename_directory_no_replace", side_effect=conflict):
            self.reject(storage.publish, root, output, files, mode=0o644)
        self.assertTrue((root / f.package.name).is_file())
        self.assertTrue((output / "keep.json").is_file())

    def test_copy_input_replaced_after_first_read_preserves_output(self) -> None:
        f = self.fixture
        original = stage.write_exclusive
        replacement = self.root / "replacement"
        put(replacement, f.package.read_bytes(), 0o644)

        def replace_after_copy(path, data, mode=0o600):
            result = original(path, data, mode)
            if path.name == f.package.name:
                os.replace(replacement, f.package)
            return result

        output = self.root / "output"
        with patch.object(stage, "write_exclusive", side_effect=replace_after_copy):
            self.reject(stage.stage_source, f.package, f.evidence, output, f.source)
        self.assertTrue((output / "failure.json").is_file())

    def test_pair_evidence_roles_dependencies_and_distinct_target(self) -> None:
        f = self.fixture
        f.make_seed()
        f.make_baseline()
        executables = copy.deepcopy(f.source["executables"])
        for value in executables.values():
            value["commit"] = "3" * 40
        acceptance = copy.deepcopy(executables["production"])
        acceptance["file"] = {"filename": "radishlex-linux-l6-acceptance", "size": 200,
                              "sha256": sha("acceptance")}
        acceptance["feature_profile"] = "compile-isolated-l6-acceptance-checkpoints"
        executables["acceptance"] = acceptance
        pair = {"format": contract.PAIR_FORMAT, "profile": contract.PROFILE,
                "matrix_profile": contract.MATRIX, "source_commit": f.source["commit"],
                "baseline_commit": "2" * 40, "target_commit": "3" * 40,
                "baseline_sha256": sha("baseline"), "seed_sha256": f.baseline["seed_sha256"],
                "review_sha256": f.baseline["review_sha256"], "source": f.source,
                "target": {"package": {"filename": "radishlex_26.7.1+40-3_arm64.deb",
                                       "size": 100, "sha256": sha("target")},
                           "artifact_evidence": {"filename": "radishlex_26.7.1+40-3_arm64.deb.evidence.json",
                                                 "size": 200, "sha256": sha("target evidence")},
                           "manifest_sha256": sha("target manifest"),
                           "dependencies_sha256": f.source["dependencies_sha256"]},
                "executables": executables, "environment_sha256": sha("target environment")}
        args = (f.baseline, "2" * 40, "3" * 40, sha("baseline"))
        contract.validate_pair(pair, *args)
        for key in ("package", "artifact_evidence"):
            bad = copy.deepcopy(pair)
            bad["target"][key]["sha256"] = f.source[key]["sha256"]
            self.reject(contract.validate_pair, bad, *args)
        bad = copy.deepcopy(pair)
        bad["target"]["dependencies_sha256"] = sha("new dependency")
        self.reject(contract.validate_pair, bad, *args)
        bad = copy.deepcopy(pair)
        bad["executables"]["production"]["feature_profile"] = "acceptance"
        self.reject(contract.validate_pair, bad, *args)


def main() -> int:
    global ROOT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-root", required=True, type=Path)
    args = parser.parse_args()
    ROOT = args.test_root
    storage.new_private_root(ROOT)
    suite = unittest.defaultTestLoader.loadTestsFromTestCase(SourceFirstTests)
    names = unittest.defaultTestLoader.getTestCaseNames(SourceFirstTests)
    put(ROOT / "plan.json", {"cases": names, "budget_seconds": 300,
                             "scope": "synthetic-repository-only", "cleanup": "none"})
    print(f"Registered synthetic cases, retain all results: {ROOT}", flush=True)
    started = time.monotonic()
    signal.signal(signal.SIGALRM, budget_expired)
    signal.setitimer(signal.ITIMER_REAL, 300)
    try:
        result = unittest.TextTestRunner(verbosity=2).run(suite)
    except BudgetExpired:
        put(ROOT / "timeout.json", {"budget_seconds": 300, "result": "incomplete-preserved"})
        return 124
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
    elapsed = round(time.monotonic() - started, 3)
    put(ROOT / "result.json", {"tests": result.testsRun, "failures": len(result.failures),
                              "errors": len(result.errors), "elapsed_seconds": elapsed,
                              "within_budget": elapsed <= 300})
    return 0 if result.wasSuccessful() and elapsed <= 300 else 1


if __name__ == "__main__":
    raise SystemExit(main())

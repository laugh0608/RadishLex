#!/usr/bin/env python3
from __future__ import annotations

import copy
import json
import stat
import struct
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import l6_release_pair


TARGET_COMMIT = "b" * 40


def write_canonical(path: Path, value: object, mode: int = 0o644) -> None:
    path.write_bytes(l6_release_pair.canonical_json_bytes(value))
    path.chmod(mode)


def environment() -> dict[str, object]:
    return {
        "architecture": "arm64",
        "format_version": 1,
        "operating_system": {
            "codename": "trixie",
            "id": "debian",
            "version_id": "13",
        },
        "profile": "debian13-arm64-release-pair-build-v1",
        "tools": {
            "cargo": "cargo 1.88.0",
            "cmake": "cmake version 3.31.6",
            "dpkg_shlibdeps": "Debian dpkg-shlibdeps version 1.22.21",
            "flutter": "3.32.8",
            "git": "git version 2.47.2",
            "rustc": "rustc 1.88.0",
        },
    }


def write_elf(path: Path, *, acceptance: bool) -> None:
    interpreter = (l6_release_pair.ELF_INTERPRETER + "\0").encode("ascii")
    value = bytearray(120 + len(interpreter))
    value[:16] = b"\x7fELF\x02\x01\x01" + b"\0" * 9
    struct.pack_into("<HHIQQQIHHHHHH", value, 16, 3, 183, 1, 0, 64, 0, 0, 64, 56, 1, 0, 0, 0)
    struct.pack_into("<IIQQQQQQ", value, 64, 3, 4, 120, 0, 0, len(interpreter), len(interpreter), 1)
    value[120 : 120 + len(interpreter)] = interpreter
    if acceptance:
        value.extend(b"\0" + b"\0".join(l6_release_pair.ACCEPTANCE_MARKERS) + b"\0")
    path.write_bytes(value)
    path.chmod(0o755)


class ReleasePairFixture:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.repository_contract = l6_release_pair.load_frozen_contract()
        self.source = self.release("source", "1", b"source-package")
        self.target = self.release("target", "2", b"target-package")
        self.contract = copy.deepcopy(self.repository_contract)
        source_evidence = self.verifier(self.source)
        self.contract["source"]["chain_anchor"] = {
            "artifact_evidence": {
                "filename": self.source.artifact_evidence.name,
                "sha256": l6_release_pair.sha256_bytes(
                    self.source.artifact_evidence.read_bytes()
                ),
                "size": self.source.artifact_evidence.stat().st_size,
            },
            "package": source_evidence["package"],
            "policy": "prior-terminal-installed-artifact-v1",
        }
        self.maintenance = root / "radishlex-linux-maintenance"
        self.acceptance = root / "radishlex-linux-l6-acceptance"
        write_elf(self.maintenance, acceptance=False)
        write_elf(self.acceptance, acceptance=True)

    def release(
        self, role: str, revision: str, package_bytes: bytes
    ) -> l6_release_pair.ReleaseInput:
        package_version = f"26.7.1+38-{revision}"
        release_root = self.root / f"{role}-root"
        release_root.mkdir(mode=0o755)
        package = self.root / f"radishlex_{package_version}_arm64.deb"
        package.write_bytes(package_bytes)
        package.chmod(0o644)
        evidence_path = self.root / f"{package.name}.evidence.json"
        evidence = {
            "package": {
                "filename": package.name,
                "sha256": l6_release_pair.sha256_bytes(package_bytes),
                "size": len(package_bytes),
            },
            "control": {
                "architecture": "arm64",
                "depends": [
                    "libc6 (>= 2.38)",
                    "fcitx5 (>= 5.1.11)",
                    "fonts-dejavu-core",
                    "fonts-noto-cjk",
                ],
                "version": package_version,
            },
            "product_manifest": {
                "sha256": l6_release_pair.sha256_bytes(
                    f"manifest-{revision}".encode("ascii")
                )
            },
        }
        write_canonical(evidence_path, evidence)
        return l6_release_pair.ReleaseInput(
            role, release_root, package, evidence_path
        )

    @staticmethod
    def verifier(release: l6_release_pair.ReleaseInput) -> dict[str, object]:
        return l6_release_pair.load_json(
            release.artifact_evidence, "synthetic artifact evidence", canonical=True
        )

    def record(self) -> dict[str, object]:
        return l6_release_pair.build_record(
            self.contract,
            environment(),
            self.source,
            self.target,
            self.maintenance,
            self.acceptance,
            target_commit=TARGET_COMMIT,
            source_verifier=self.verifier,
            target_verifier=self.verifier,
        )

    def publish(self, record: dict[str, object]) -> Path:
        published = self.root / "published"
        published.mkdir(mode=0o755)
        write_canonical(
            published / "build-environment.json", record["build_environment"]
        )
        for role, release_input in (
            ("source", self.source),
            ("target", self.target),
        ):
            artifacts = published / role / "artifacts"
            artifacts.mkdir(parents=True, mode=0o755)
            for source in (
                release_input.package,
                release_input.artifact_evidence,
            ):
                destination = artifacts / source.name
                destination.write_bytes(source.read_bytes())
                destination.chmod(0o644)
        for source in (self.maintenance, self.acceptance):
            destination = published / source.name
            destination.write_bytes(source.read_bytes())
            destination.chmod(0o755)
        evidence_path = published / "release-pair.evidence.json"
        write_canonical(evidence_path, record)
        return evidence_path


class L6ReleasePairTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(
            prefix="radishlex-l6-pair-test."
        )
        self.root = Path(self.temporary.name).resolve()
        self.fixture = ReleasePairFixture(self.root)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_contract_binds_adjacent_revision_and_shared_product_identity(self) -> None:
        contract = self.fixture.repository_contract
        self.assertEqual(contract["source"]["repository_commit"], l6_release_pair.SOURCE_COMMIT)
        self.assertEqual(contract["source"]["package_version"], "26.7.1+38-1")
        self.assertEqual(
            contract["source"]["chain_anchor"]["package"]["sha256"],
            "09ed122804b11767b8ac7cd69c323c1f6eef511fd6ae7284d75756fb60569bec",
        )
        self.assertFalse(contract["build"]["source_rebuild"])
        self.assertEqual(contract["target"]["package_version"], "26.7.1+38-2")
        self.assertEqual(contract["shared_contract"]["ffi_abi_version"], 9)

    def test_current_privacy_data_cannot_qualify_as_the_frozen_target(self) -> None:
        source = l6_release_pair.git_json_at_commit(
            l6_release_pair.REPO_ROOT, l6_release_pair.SOURCE_COMMIT,
            l6_release_pair.METADATA_RELATIVE,
        )
        target = dict(source, debian_revision="2", package_version="26.7.1+38-2")
        current = json.loads(
            (l6_release_pair.REPO_ROOT / l6_release_pair.METADATA_RELATIVE).read_text()
        )
        target["rime_data_lock_sha256"] = current["rime_data_lock_sha256"]
        with self.assertRaisesRegex(
            l6_release_pair.L6ReleasePairError, "rime_data_lock_sha256"
        ):
            l6_release_pair.load_contract(source_metadata=source, target_metadata=target)

    def test_current_build_cannot_qualify_as_the_frozen_target(self) -> None:
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "build_number"):
            l6_release_pair.load_contract()

    def test_frozen_declaration_does_not_hide_other_metadata_drift(self) -> None:
        source = l6_release_pair.git_json_at_commit(
            l6_release_pair.REPO_ROOT, l6_release_pair.SOURCE_COMMIT,
            l6_release_pair.METADATA_RELATIVE,
        )
        target = dict(source, debian_revision="2", package_version="26.7.1+38-2")
        target["privacy_format_version"] += 1
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "privacy_format_version"):
            l6_release_pair.load_contract(source_metadata=source, target_metadata=target)

    def test_clean_root_rejects_preexisting_ignored_build_output(self) -> None:
        clean = self.root / "clean-output-test"
        clean.mkdir(mode=0o755)
        l6_release_pair.require_clean_build_outputs(clean, "synthetic root")
        (clean / "target").mkdir(mode=0o755)
        with self.assertRaisesRegex(
            l6_release_pair.L6ReleasePairError, "preexisting build output"
        ):
            l6_release_pair.require_clean_build_outputs(clean, "synthetic root")

    def test_stage_source_anchor_copies_only_exact_frozen_bytes(self) -> None:
        output = self.root / "staged-source"
        with patch.object(
            l6_release_pair, "verify_target_root", return_value=TARGET_COMMIT
        ):
            l6_release_pair.stage_source_anchor(
                self.fixture.source.package,
                self.fixture.source.artifact_evidence,
                self.root,
                output,
                self.fixture.contract,
            )
        for source in (
            self.fixture.source.package,
            self.fixture.source.artifact_evidence,
        ):
            staged = output / source.name
            self.assertEqual(staged.read_bytes(), source.read_bytes())
            self.assertEqual(stat.S_IMODE(staged.stat().st_mode), 0o644)

    def test_stage_source_anchor_rejects_rebuilt_bytes_before_output(self) -> None:
        rebuilt = b"same-version-rebuilt-source"
        self.fixture.source.package.write_bytes(rebuilt)
        evidence = self.fixture.verifier(self.fixture.source)
        evidence["package"]["sha256"] = l6_release_pair.sha256_bytes(rebuilt)
        evidence["package"]["size"] = len(rebuilt)
        write_canonical(self.fixture.source.artifact_evidence, evidence)
        output = self.root / "rejected-source"
        with patch.object(
            l6_release_pair, "verify_target_root", return_value=TARGET_COMMIT
        ):
            with self.assertRaisesRegex(
                l6_release_pair.L6ReleasePairError, "chain anchor"
            ):
                l6_release_pair.stage_source_anchor(
                    self.fixture.source.package,
                    self.fixture.source.artifact_evidence,
                    self.root,
                    output,
                    self.fixture.contract,
                )
        self.assertFalse(output.exists())

    def test_record_revalidates_git_identity_after_builder_outputs_exist(self) -> None:
        source = self.root / "source-repository"
        target = self.root / "target-repository"
        source.mkdir(mode=0o755)

        def git(root: Path, *arguments: str) -> str:
            result = subprocess.run(
                ["/usr/bin/git", "-C", str(root), *arguments],
                check=True,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            return result.stdout.strip()

        git(source, "init", "--quiet")
        git(source, "config", "user.name", "RadishLex L6 Test")
        git(source, "config", "user.email", "l6-test@radishlex.invalid")
        (source / ".gitignore").write_text("target/\n", encoding="utf-8")
        (source / "identity.txt").write_text("source\n", encoding="utf-8")
        git(source, "add", ".gitignore", "identity.txt")
        git(source, "commit", "--quiet", "-m", "source")
        source_commit = git(source, "rev-parse", "HEAD")

        subprocess.run(
            ["/usr/bin/git", "clone", "--quiet", str(source), str(target)],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        git(target, "config", "user.name", "RadishLex L6 Test")
        git(target, "config", "user.email", "l6-test@radishlex.invalid")
        (target / "identity.txt").write_text("target\n", encoding="utf-8")
        git(target, "add", "identity.txt")
        git(target, "commit", "--quiet", "-m", "target")

        (source / "target").mkdir(mode=0o755)
        (target / "target").mkdir(mode=0o755)
        with patch.object(l6_release_pair, "REPO_ROOT", target), patch.object(
            l6_release_pair, "SOURCE_COMMIT", source_commit
        ):
            with self.assertRaisesRegex(
                l6_release_pair.L6ReleasePairError, "preexisting build output"
            ):
                l6_release_pair.verify_target_root(target)
            target_commit = l6_release_pair.verify_target_root(
                target,
                require_absent_build_outputs=False,
            )
            self.assertEqual(target_commit, git(target, "rev-parse", "HEAD"))

            (target / "identity.txt").write_text("dirty\n", encoding="utf-8")
            with self.assertRaisesRegex(
                l6_release_pair.L6ReleasePairError, "must be clean"
            ):
                l6_release_pair.verify_target_root(
                    target,
                    require_absent_build_outputs=False,
                )

    def test_record_is_canonical_redacted_and_compile_identity_separated(self) -> None:
        record = self.fixture.record()
        l6_release_pair.validate_record(record)
        serialized = l6_release_pair.canonical_json_bytes(record)
        reparsed = json.loads(serialized)
        self.assertEqual(reparsed, record)
        self.assertNotIn(str(self.root), serialized.decode("utf-8"))
        self.assertNotIn("operation_id", serialized.decode("utf-8"))
        self.assertNotEqual(
            record["executables"][0]["sha256"],
            record["executables"][1]["sha256"],
        )
        self.assertEqual(
            [item["role"] for item in record["executables"]],
            ["production-maintenance", "acceptance-controller"],
        )

    def test_published_pair_rehashes_every_frozen_file(self) -> None:
        record = self.fixture.record()
        evidence_path = self.fixture.publish(record)
        l6_release_pair.verify_published_record(
            evidence_path, record, self.fixture.contract
        )
        target_package = (
            evidence_path.parent
            / "target"
            / "artifacts"
            / record["releases"]["target"]["package"]["filename"]
        )
        target_package.write_bytes(target_package.read_bytes() + b"tamper")
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "differs from evidence"):
            l6_release_pair.verify_published_record(
                evidence_path, record, self.fixture.contract
            )

    def test_published_pair_rejects_unbound_files(self) -> None:
        record = self.fixture.record()
        evidence_path = self.fixture.publish(record)
        extra = evidence_path.parent / "unbound.txt"
        extra.write_text("unexpected\n", encoding="utf-8")
        extra.chmod(0o644)
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "inventory differs"):
            l6_release_pair.verify_published_record(
                evidence_path, record, self.fixture.contract
            )

    def test_non_adjacent_target_revision_is_rejected(self) -> None:
        contract = copy.deepcopy(self.fixture.contract)
        contract["target"]["debian_revision"] = "3"
        contract["target"]["package_version"] = "26.7.1+38-3"
        target_metadata = json.loads(
            (l6_release_pair.REPO_ROOT / l6_release_pair.METADATA_RELATIVE).read_text(
                encoding="utf-8"
            )
        )
        target_metadata["debian_revision"] = "3"
        target_metadata["package_version"] = "26.7.1+38-3"
        source_metadata = copy.deepcopy(target_metadata)
        source_metadata["debian_revision"] = "1"
        source_metadata["package_version"] = "26.7.1+38-1"
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "not adjacent"):
            l6_release_pair.validate_metadata_pair(
                source_metadata, target_metadata, contract
            )

    def test_shared_contract_drift_is_rejected(self) -> None:
        contract = copy.deepcopy(self.fixture.contract)
        contract["shared_contract"]["userdb_schema_version"] = 10
        source_metadata = l6_release_pair.git_json_at_commit(
            l6_release_pair.REPO_ROOT, l6_release_pair.SOURCE_COMMIT,
            l6_release_pair.METADATA_RELATIVE,
        )
        target_metadata = dict(
            source_metadata, debian_revision="2", package_version="26.7.1+38-2"
        )
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "shared product"):
            l6_release_pair.validate_metadata_pair(
                source_metadata, target_metadata, contract
            )

    def test_same_package_bytes_are_rejected(self) -> None:
        target_bytes = self.fixture.source.package.read_bytes()
        self.fixture.target.package.write_bytes(target_bytes)
        evidence = self.fixture.verifier(self.fixture.target)
        evidence["package"]["sha256"] = l6_release_pair.sha256_bytes(target_bytes)
        evidence["package"]["size"] = len(target_bytes)
        write_canonical(self.fixture.target.artifact_evidence, evidence)
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "must be distinct"):
            self.fixture.record()

    def test_same_version_rebuilt_source_is_rejected_by_chain_anchor(self) -> None:
        rebuilt = b"same-version-rebuilt-source"
        self.fixture.source.package.write_bytes(rebuilt)
        evidence = self.fixture.verifier(self.fixture.source)
        evidence["package"]["sha256"] = l6_release_pair.sha256_bytes(rebuilt)
        evidence["package"]["size"] = len(rebuilt)
        write_canonical(self.fixture.source.artifact_evidence, evidence)
        with self.assertRaisesRegex(
            l6_release_pair.L6ReleasePairError, "prior-terminal chain anchor"
        ):
            self.fixture.record()

    def test_source_evidence_drift_is_rejected_by_chain_anchor(self) -> None:
        evidence = self.fixture.verifier(self.fixture.source)
        evidence["dependency_analysis"] = {"profile": "synthetic-drift"}
        write_canonical(self.fixture.source.artifact_evidence, evidence)
        with self.assertRaisesRegex(
            l6_release_pair.L6ReleasePairError, "prior-terminal chain anchor"
        ):
            self.fixture.record()

    def test_same_manifest_identity_is_rejected(self) -> None:
        source_evidence = self.fixture.verifier(self.fixture.source)
        target_evidence = self.fixture.verifier(self.fixture.target)
        target_evidence["product_manifest"]["sha256"] = source_evidence[
            "product_manifest"
        ]["sha256"]
        write_canonical(self.fixture.target.artifact_evidence, target_evidence)
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "manifests must be distinct"):
            self.fixture.record()

    def test_production_executable_cannot_contain_acceptance_markers(self) -> None:
        write_elf(self.fixture.maintenance, acceptance=True)
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "production executable"):
            self.fixture.record()

    def test_acceptance_executable_requires_compile_identity_markers(self) -> None:
        write_elf(self.fixture.acceptance, acceptance=False)
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "lacks compile identity"):
            self.fixture.record()

    def test_wrong_architecture_is_rejected(self) -> None:
        value = bytearray(self.fixture.acceptance.read_bytes())
        struct.pack_into("<H", value, 18, 62)
        self.fixture.acceptance.write_bytes(value)
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "not AArch64"):
            self.fixture.record()

    def test_environment_rejects_local_paths(self) -> None:
        invalid = environment()
        invalid["tools"]["flutter"] = "/home/builder/flutter 3.32.8"
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "local path"):
            l6_release_pair.validate_environment(invalid)

    def test_evidence_rejects_unknown_or_raw_process_fields(self) -> None:
        record = self.fixture.record()
        record["worker_pid"] = 42
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "fields differ"):
            l6_release_pair.validate_record(record)

    def test_artifact_verifier_failure_is_closed(self) -> None:
        def reject(_: l6_release_pair.ReleaseInput) -> dict[str, object]:
            raise l6_release_pair.L6ReleasePairError("synthetic verifier rejection")

        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "verifier rejection"):
            l6_release_pair.build_record(
                self.fixture.contract,
                environment(),
                self.fixture.source,
                self.fixture.target,
                self.fixture.maintenance,
                self.fixture.acceptance,
                target_commit=TARGET_COMMIT,
                source_verifier=reject,
                target_verifier=reject,
            )

    def test_executable_mode_is_part_of_identity(self) -> None:
        self.fixture.acceptance.chmod(0o700)
        self.assertEqual(stat.S_IMODE(self.fixture.acceptance.stat().st_mode), 0o700)
        with self.assertRaisesRegex(l6_release_pair.L6ReleasePairError, "mode is unsafe"):
            self.fixture.record()


if __name__ == "__main__":
    unittest.main()

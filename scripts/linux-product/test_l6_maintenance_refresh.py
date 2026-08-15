#!/usr/bin/env python3
from __future__ import annotations

import copy
import json
import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import l6_maintenance_refresh
import l6_release_pair
from test_l6_release_pair import ReleasePairFixture, TARGET_COMMIT, write_elf


REFRESH_COMMIT = "e" * 40


def write_canonical(path: Path, value: object) -> None:
    path.write_bytes(l6_maintenance_refresh.canonical_json_bytes(value))
    path.chmod(0o644)


def refresh_environment() -> dict[str, object]:
    return {
        "architecture": "arm64",
        "format_version": 1,
        "operating_system": {
            "codename": "trixie",
            "id": "debian",
            "version_id": "13",
        },
        "profile": "debian13-arm64-maintenance-refresh-build-v1",
        "tools": {
            "cargo": "cargo 1.89.0",
            "git": "git version 2.47.2",
            "rustc": "rustc 1.89.0",
        },
    }


class MaintenanceRefreshFixture:
    def __init__(self, root: Path) -> None:
        self.root = root
        pair_root = root / "pair"
        pair_root.mkdir(mode=0o755)
        self.pair = ReleasePairFixture(pair_root)
        self.pair_record = self.pair.record()
        self.record_path = root / "release-pair.evidence.json"
        write_canonical(self.record_path, self.pair_record)
        self.inputs = l6_maintenance_refresh.BaseInputs(
            self.record_path,
            self.pair.target.package,
            self.pair.target.artifact_evidence,
        )
        self.contract = copy.deepcopy(l6_maintenance_refresh.EXPECTED_CONTRACT)
        target = self.pair_record["releases"]["target"]
        production = self.pair_record["executables"][0]
        self.contract["base_release_pair"] = {
            "record": {
                "filename": self.record_path.name,
                "sha256": l6_maintenance_refresh.sha256_bytes(
                    self.record_path.read_bytes()
                ),
            },
            "target": {
                "artifact_evidence": {
                    "filename": target["artifact_evidence"]["filename"],
                    "sha256": target["artifact_evidence"]["sha256"],
                },
                "debian_revision": target["debian_revision"],
                "package": {
                    "filename": target["package"]["filename"],
                    "sha256": target["package"]["sha256"],
                },
                "package_version": target["package_version"],
                "production_maintenance_sha256": production["sha256"],
                "repository_commit": TARGET_COMMIT,
            },
        }
        self.maintenance = root / "radishlex-linux-maintenance"
        write_elf(self.maintenance, acceptance=False)
        self.maintenance.write_bytes(self.maintenance.read_bytes() + b"\0refresh-v1")
        self.maintenance.chmod(0o755)

    def record(self) -> dict[str, object]:
        return l6_maintenance_refresh.build_record(
            self.contract,
            refresh_environment(),
            self.inputs,
            self.maintenance,
            refresh_commit=REFRESH_COMMIT,
            pair_contract=self.pair.contract,
        )

    def publication(self) -> tuple[Path, dict[str, object]]:
        publication = self.root / "maintenance-refresh"
        with patch.object(
            l6_maintenance_refresh,
            "verify_refresh_root",
            return_value=REFRESH_COMMIT,
        ):
            staged_record, staged_package, staged_evidence = (
                l6_maintenance_refresh.stage_base_inputs(
                    self.inputs,
                    self.root,
                    publication,
                    self.contract,
                    pair_contract=self.pair.contract,
                )
            )
        staged_maintenance = publication / self.maintenance.name
        staged_maintenance.write_bytes(self.maintenance.read_bytes())
        staged_maintenance.chmod(0o755)
        environment_path = publication / "build-environment.json"
        write_canonical(environment_path, refresh_environment())
        staged_inputs = l6_maintenance_refresh.BaseInputs(
            staged_record, staged_package, staged_evidence
        )
        record = l6_maintenance_refresh.build_record(
            self.contract,
            refresh_environment(),
            staged_inputs,
            staged_maintenance,
            refresh_commit=REFRESH_COMMIT,
            pair_contract=self.pair.contract,
        )
        evidence_path = publication / "maintenance-refresh.evidence.json"
        write_canonical(evidence_path, record)
        return evidence_path, record


class L6MaintenanceRefreshTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(
            prefix="radishlex-l6-maintenance-refresh-test."
        )
        self.root = Path(self.temporary.name).resolve()
        self.fixture = MaintenanceRefreshFixture(self.root)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_repository_contract_binds_frozen_target_and_repair_fix(self) -> None:
        contract = l6_maintenance_refresh.load_contract()
        self.assertFalse(contract["build"]["package_rebuild"])
        self.assertEqual(
            contract["base_release_pair"]["record"]["sha256"],
            l6_maintenance_refresh.BASE_RECORD_SHA256,
        )
        self.assertEqual(
            contract["base_release_pair"]["target"]["package"]["sha256"],
            l6_maintenance_refresh.BASE_PACKAGE_SHA256,
        )
        self.assertEqual(
            contract["refresh"]["required_ancestor_commit"],
            l6_maintenance_refresh.REPAIR_FIX_COMMIT,
        )
        self.assertEqual(
            contract["executable"]["role"], "production-maintenance"
        )

    def test_record_is_canonical_redacted_and_production_only(self) -> None:
        record = self.fixture.record()
        l6_maintenance_refresh.validate_record(record, self.fixture.contract)
        serialized = l6_maintenance_refresh.canonical_json_bytes(record)
        self.assertEqual(json.loads(serialized), record)
        self.assertNotIn(str(self.root), serialized.decode("utf-8"))
        self.assertNotIn("operation_id", serialized.decode("utf-8"))
        self.assertNotIn("acceptance-controller", serialized.decode("utf-8"))
        self.assertEqual(record["executable"]["role"], "production-maintenance")
        self.assertNotEqual(
            record["executable"]["sha256"],
            self.fixture.contract["base_release_pair"]["target"][
                "production_maintenance_sha256"
            ],
        )

    def test_base_record_drift_is_rejected(self) -> None:
        self.fixture.record_path.write_bytes(
            self.fixture.record_path.read_bytes() + b"\n"
        )
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "record SHA-256",
        ):
            self.fixture.record()

    def test_target_package_drift_is_rejected(self) -> None:
        self.fixture.pair.target.package.write_bytes(
            self.fixture.pair.target.package.read_bytes() + b"tamper"
        )
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "size differs",
        ):
            self.fixture.record()

    def test_base_production_executable_identity_drift_is_rejected(self) -> None:
        contract = copy.deepcopy(self.fixture.contract)
        contract["base_release_pair"]["target"][
            "production_maintenance_sha256"
        ] = "1" * 64
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "base production maintenance",
        ):
            l6_maintenance_refresh.build_record(
                contract,
                refresh_environment(),
                self.fixture.inputs,
                self.fixture.maintenance,
                refresh_commit=REFRESH_COMMIT,
                pair_contract=self.fixture.pair.contract,
            )

    def test_frozen_base_executable_cannot_be_reused(self) -> None:
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "must differ",
        ):
            l6_maintenance_refresh.build_record(
                self.fixture.contract,
                refresh_environment(),
                self.fixture.inputs,
                self.fixture.pair.maintenance,
                refresh_commit=REFRESH_COMMIT,
                pair_contract=self.fixture.pair.contract,
            )

    def test_acceptance_capability_is_rejected(self) -> None:
        write_elf(self.fixture.maintenance, acceptance=True)
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "acceptance capability",
        ):
            self.fixture.record()

    def test_wrong_architecture_is_rejected(self) -> None:
        value = bytearray(self.fixture.maintenance.read_bytes())
        value[18:20] = (62).to_bytes(2, "little")
        self.fixture.maintenance.write_bytes(value)
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "not AArch64",
        ):
            self.fixture.record()

    def test_unknown_or_raw_process_evidence_is_rejected(self) -> None:
        record = self.fixture.record()
        record["operation_id"] = "synthetic"
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "fields differ",
        ):
            l6_maintenance_refresh.validate_record(record, self.fixture.contract)

    def test_environment_rejects_local_paths(self) -> None:
        environment = refresh_environment()
        environment["tools"]["cargo"] = "/home/builder/cargo 1.89.0"
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "local path",
        ):
            l6_maintenance_refresh.validate_environment(environment)

    def test_stage_base_copies_only_record_bound_target_bytes(self) -> None:
        output = self.root / "staged"
        with patch.object(
            l6_maintenance_refresh,
            "verify_refresh_root",
            return_value=REFRESH_COMMIT,
        ):
            record, package, evidence = l6_maintenance_refresh.stage_base_inputs(
                self.fixture.inputs,
                self.root,
                output,
                self.fixture.contract,
                pair_contract=self.fixture.pair.contract,
            )
        for path, source in (
            (record, self.fixture.record_path),
            (package, self.fixture.pair.target.package),
            (evidence, self.fixture.pair.target.artifact_evidence),
        ):
            self.assertEqual(path.read_bytes(), source.read_bytes())
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o644)
            self.assertEqual(path.stat().st_nlink, 1)

    def test_published_refresh_rehashes_every_frozen_file(self) -> None:
        evidence_path, record = self.fixture.publication()
        l6_maintenance_refresh.verify_published_record(
            evidence_path,
            record,
            self.fixture.contract,
            pair_contract=self.fixture.pair.contract,
        )
        maintenance = evidence_path.parent / "radishlex-linux-maintenance"
        maintenance.write_bytes(maintenance.read_bytes() + b"tamper")
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "size differs",
        ):
            l6_maintenance_refresh.verify_published_record(
                evidence_path,
                record,
                self.fixture.contract,
                pair_contract=self.fixture.pair.contract,
            )

    def test_published_refresh_rejects_acceptance_or_unbound_files(self) -> None:
        evidence_path, record = self.fixture.publication()
        acceptance = evidence_path.parent / "radishlex-linux-l6-acceptance"
        acceptance.write_bytes(b"not permitted")
        acceptance.chmod(0o755)
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "inventory differs",
        ):
            l6_maintenance_refresh.verify_published_record(
                evidence_path,
                record,
                self.fixture.contract,
                pair_contract=self.fixture.pair.contract,
            )

    def test_published_refresh_rejects_hard_linked_file(self) -> None:
        evidence_path, record = self.fixture.publication()
        linked = self.root / "linked-maintenance"
        os.link(evidence_path.parent / "radishlex-linux-maintenance", linked)
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "link count",
        ):
            l6_maintenance_refresh.verify_published_record(
                evidence_path,
                record,
                self.fixture.contract,
                pair_contract=self.fixture.pair.contract,
            )

    def test_publish_is_atomic_and_reverified(self) -> None:
        evidence_path, record = self.fixture.publication()
        output = self.root / "published-refresh"
        published = l6_maintenance_refresh.publish_staging(
            evidence_path.parent,
            output,
            self.fixture.contract,
            pair_contract=self.fixture.pair.contract,
        )
        self.assertFalse(evidence_path.parent.exists())
        self.assertEqual(published, output / "maintenance-refresh.evidence.json")
        l6_maintenance_refresh.verify_published_record(
            published,
            record,
            self.fixture.contract,
            pair_contract=self.fixture.pair.contract,
        )

    def test_publish_refuses_existing_output(self) -> None:
        evidence_path, _ = self.fixture.publication()
        output = self.root / "existing"
        output.mkdir()
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "absent absolute path",
        ):
            l6_maintenance_refresh.publish_staging(
                evidence_path.parent,
                output,
                self.fixture.contract,
                pair_contract=self.fixture.pair.contract,
            )

    def test_atomic_publish_primitive_never_replaces_existing_output(self) -> None:
        source = self.root / "rename-source"
        destination = self.root / "rename-destination"
        source.mkdir()
        destination.mkdir()
        (source / "source-sentinel").write_text("source\n", encoding="utf-8")
        (destination / "destination-sentinel").write_text(
            "destination\n", encoding="utf-8"
        )
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "without overwrite",
        ):
            l6_maintenance_refresh.rename_directory_no_replace(source, destination)
        self.assertEqual(
            (source / "source-sentinel").read_text(encoding="utf-8"), "source\n"
        )
        self.assertEqual(
            (destination / "destination-sentinel").read_text(encoding="utf-8"),
            "destination\n",
        )


class L6MaintenanceRefreshRootTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(
            prefix="radishlex-l6-maintenance-refresh-root-test."
        )
        self.root = Path(self.temporary.name).resolve()
        self.git("init", "--quiet")
        self.git("config", "user.name", "RadishLex L6 Test")
        self.git("config", "user.email", "l6-test@radishlex.invalid")
        metadata = self.root / "packaging/linux/product.json"
        metadata.parent.mkdir(parents=True)
        metadata.write_text('{"format_version":1}\n', encoding="utf-8")
        (self.root / ".gitignore").write_text("target/\n", encoding="utf-8")
        self.git("add", ".gitignore", "packaging/linux/product.json")
        self.git("commit", "--quiet", "-m", "base")
        self.base_commit = self.git("rev-parse", "HEAD")
        (self.root / "fix.txt").write_text("repair fix\n", encoding="utf-8")
        self.git("add", "fix.txt")
        self.git("commit", "--quiet", "-m", "fix")
        self.fix_commit = self.git("rev-parse", "HEAD")
        (self.root / "refresh.txt").write_text("refresh contract\n", encoding="utf-8")
        self.git("add", "refresh.txt")
        self.git("commit", "--quiet", "-m", "refresh")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def git(self, *arguments: str) -> str:
        result = subprocess.run(
            ["/usr/bin/git", "-C", str(self.root), *arguments],
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        return result.stdout.strip()

    def verify(self, *, require_absent_build_outputs: bool = True) -> str:
        with patch.object(l6_maintenance_refresh, "REPO_ROOT", self.root), patch.object(
            l6_maintenance_refresh, "BASE_TARGET_COMMIT", self.base_commit
        ), patch.object(
            l6_maintenance_refresh, "REPAIR_FIX_COMMIT", self.fix_commit
        ):
            return l6_maintenance_refresh.verify_refresh_root(
                self.root,
                require_absent_build_outputs=require_absent_build_outputs,
            )

    def test_refresh_root_requires_clean_descendant_and_unchanged_metadata(self) -> None:
        self.assertEqual(self.verify(), self.git("rev-parse", "HEAD"))
        metadata = self.root / "packaging/linux/product.json"
        metadata.write_text('{"format_version":2}\n', encoding="utf-8")
        self.git("add", "packaging/linux/product.json")
        self.git("commit", "--quiet", "-m", "metadata drift")
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "must not change target product metadata",
        ):
            self.verify()

    def test_refresh_root_rejects_dirty_or_preexisting_build_output(self) -> None:
        (self.root / "dirty.txt").write_text("dirty\n", encoding="utf-8")
        with self.assertRaisesRegex(
            l6_release_pair.L6ReleasePairError, "must be clean"
        ):
            self.verify()
        (self.root / "dirty.txt").unlink()
        (self.root / "target").mkdir()
        with self.assertRaisesRegex(
            l6_release_pair.L6ReleasePairError, "preexisting build output"
        ):
            self.verify()

    def test_refresh_root_rejects_commit_without_repair_fix(self) -> None:
        self.git("checkout", "--quiet", "--detach", self.base_commit)
        (self.root / "alternate.txt").write_text("alternate\n", encoding="utf-8")
        self.git("add", "alternate.txt")
        self.git("commit", "--quiet", "-m", "alternate")
        with self.assertRaisesRegex(
            l6_maintenance_refresh.L6MaintenanceRefreshError,
            "does not contain the required repair fix",
        ):
            self.verify()


if __name__ == "__main__":
    unittest.main()

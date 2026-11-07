from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from validation_impact import QUALITY_JOBS, check_quality, reliability_required


class ImpactTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        packages = {
            "crates/shared": ('shared', ''),
            "crates/userdb": ('radishlex-ime-userdb', '[dependencies]\nshared = {path="../shared"}\n'),
            "crates/ui": ('ui', ''),
        }
        members = json.dumps(list(packages))
        (self.root / 'Cargo.toml').write_text(f'[workspace]\nmembers = {members}\n')
        for path, (name, dependencies) in packages.items():
            directory = self.root / path
            directory.mkdir(parents=True)
            (directory / 'Cargo.toml').write_text(f'[package]\nname="{name}"\nversion="0.1.0"\n{dependencies}')

    def test_dependency_change_requires_consumer_reliability(self):
        self.assertTrue(reliability_required(['crates/shared/src/lib.rs'], self.root))
        self.assertTrue(reliability_required(['crates/userdb/deleted.rs'], self.root))
        self.assertFalse(reliability_required(['crates/ui/src/lib.rs'], self.root))

    def test_inherited_renamed_path_dependency_is_propagated(self):
        manifest = self.root / "Cargo.toml"
        manifest.write_text(manifest.read_text() + '\n[workspace.dependencies]\nshared_alias = {path="crates/shared", package="shared"}\n')
        (self.root / "crates/userdb/Cargo.toml").write_text(
            '[package]\nname="radishlex-ime-userdb"\nversion="0.1.0"\n[dependencies]\nshared_alias = {workspace=true}\n')
        self.assertTrue(reliability_required(["crates/shared/src/renamed.rs"], self.root))

    def test_full_diff_cannot_be_waived_by_a_later_document_change(self):
        self.assertTrue(reliability_required(['docs/change.md', 'crates/shared/src/lib.rs'], self.root))
        self.assertFalse(reliability_required(['docs/change.md'], self.root))
        self.assertFalse(reliability_required(['crates/userdb/README.md', '.github/rulesets/README.md'], self.root))

    def test_unknown_paths_configuration_and_empty_diff_fail_closed(self):
        for paths in ([], ['Cargo.lock'], ['scripts/validation_suites.py'], ['new-platform/file'], ['.github/workflows/quality.yml']):
            with self.subTest(paths=paths):
                self.assertTrue(reliability_required(paths, self.root))
        with self.assertRaises(ValueError):
            reliability_required(['../outside'], self.root)


class AggregateTests(unittest.TestCase):
    def results(self):
        return {name: {'result': 'success'} for name in QUALITY_JOBS}

    def test_only_inapplicable_reliability_may_be_skipped(self):
        results = self.results()
        results['reliability']['result'] = 'skipped'
        check_quality(results, False)
        with self.assertRaises(ValueError):
            check_quality(results, True)
        results['rust']['result'] = 'skipped'
        with self.assertRaises(ValueError):
            check_quality(results, False)

    def test_failure_cancellation_missing_and_unexpected_jobs_are_rejected(self):
        for state in ('failure', 'cancelled', None):
            results = self.results()
            results['reliability']['result'] = state
            with self.assertRaises(ValueError):
                check_quality(results, False)
        for delta in ('missing', 'extra'):
            results = self.results()
            results.pop('go') if delta == 'missing' else results.update(extra={'result': 'success'})
            with self.assertRaises(ValueError):
                check_quality(results, True)

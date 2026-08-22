#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import ModuleType


SCRIPTS_ROOT = Path(__file__).resolve().parent


def load_script_module(name: str, filename: str) -> ModuleType:
    spec = importlib.util.spec_from_file_location(name, SCRIPTS_ROOT / filename)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load checker module: {filename}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


check_docs = load_script_module("radishlex_check_docs", "check-docs.py")
check_text_files = load_script_module("radishlex_check_text_files", "check-text-files.py")


class CheckerFixture(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.repo_root = Path(self.temporary_directory.name)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write(self, relative_path: str, content: str = "# Fixture\n") -> None:
        path = self.repo_root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


class DocumentationCheckerTests(CheckerFixture):
    def test_root_community_documents_are_entry_documents_and_are_scanned(self) -> None:
        root_documents = (
            "AGENTS.md",
            "CLAUDE.md",
            "CODE_OF_CONDUCT.md",
            "CONTRIBUTING.md",
            "README.md",
            "SECURITY.md",
        )
        for relative_path in root_documents:
            self.write(relative_path)
        self.write("docs/README.md")

        scanned = {
            path.relative_to(self.repo_root).as_posix()
            for path in check_docs.iter_markdown_files(self.repo_root)
        }

        self.assertTrue(set(root_documents).issubset(scanned))
        for relative_path in ("CODE_OF_CONDUCT.md", "CONTRIBUTING.md", "SECURITY.md"):
            self.assertEqual(check_docs.doc_kind(relative_path), "entry")

    def test_collaboration_documents_must_be_identical(self) -> None:
        self.write("AGENTS.md", "# Shared\n")
        self.write("CLAUDE.md", "# Shared\n")

        self.assertTrue(check_docs.collaboration_docs_match(self.repo_root))

        self.write("CLAUDE.md", "# Drifted\n")

        self.assertFalse(check_docs.collaboration_docs_match(self.repo_root))


class TextCheckerTests(CheckerFixture):
    def test_repository_files_include_tracked_and_untracked_but_not_ignored_files(self) -> None:
        subprocess.run(
            ["git", "init", "--quiet"],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )
        self.write(".gitignore", "ignored.md\n")
        self.write("tracked.md")
        self.write("untracked.md")
        self.write("ignored.md")
        subprocess.run(
            ["git", "add", ".gitignore", "tracked.md"],
            cwd=self.repo_root,
            check=True,
            capture_output=True,
            text=True,
        )

        paths = {
            path.as_posix()
            for path in check_text_files.repository_files(self.repo_root)
        }

        self.assertIn("tracked.md", paths)
        self.assertIn("untracked.md", paths)
        self.assertNotIn("ignored.md", paths)


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path


COLLABORATION_DOCS = {"AGENTS.md", "CLAUDE.md"}
ENTRY_DOCS = {
    "README.md",
    "docs/README.md",
    "docs/status/current.md",
}
REFERENCE_PREFIXES = ("docs/reference/", "docs/archive/")
DEVLOG_PREFIX = "docs/devlogs/"


def line_count(content: str) -> int:
    if not content:
        return 0
    count = len(content.split("\n"))
    if content.endswith("\n"):
        count -= 1
    return count


def doc_kind(relative_path: str) -> str:
    if relative_path in COLLABORATION_DOCS:
        return "collaboration"
    if relative_path in ENTRY_DOCS or relative_path.endswith("/README.md"):
        return "entry"
    if relative_path.startswith(DEVLOG_PREFIX):
        return "devlog"
    if relative_path.startswith(REFERENCE_PREFIXES):
        return "reference"
    return "active"


def iter_markdown_files(repo_root: Path) -> list[Path]:
    paths = [repo_root / "AGENTS.md", repo_root / "CLAUDE.md", repo_root / "README.md"]
    docs_root = repo_root / "docs"
    if docs_root.is_dir():
        paths.extend(sorted(docs_root.rglob("*.md")))
    return [path for path in paths if path.is_file()]


def main() -> int:
    repo_root = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
    errors: list[str] = []
    warnings: list[str] = []

    for full_path in iter_markdown_files(repo_root):
        relative_path = full_path.relative_to(repo_root).as_posix()
        content = full_path.read_text(encoding="utf-8")
        lines = line_count(content)
        chars = len(content)
        kind = doc_kind(relative_path)

        if kind == "collaboration":
            if chars > 16000:
                errors.append(f"{relative_path}: collaboration doc has {chars} chars, over 16000 hard limit")
            elif chars > 14000:
                warnings.append(f"{relative_path}: collaboration doc has {chars} chars, over 14000 target budget")
        elif kind == "entry":
            if lines > 180:
                errors.append(f"{relative_path}: entry document has {lines} lines, over 180 line hard limit")
            elif chars > 10000:
                warnings.append(f"{relative_path}: entry document has {chars} chars, over 10000 char target budget")
        elif kind == "active":
            if lines > 800:
                errors.append(f"{relative_path}: active document has {lines} lines, over 800 line hard limit")
            elif lines > 500:
                warnings.append(f"{relative_path}: active document has {lines} lines, over 500 line target budget")
        elif kind in {"devlog", "reference"}:
            if lines > 1200:
                warnings.append(f"{relative_path}: {kind} document has {lines} lines; consider splitting or indexing it")

    if warnings:
        print("Documentation budget warnings:", file=sys.stderr)
        for warning in warnings:
            print(warning, file=sys.stderr)

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    if warnings:
        print("Documentation budget check passed with warnings.")
    else:
        print("Documentation budget check passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

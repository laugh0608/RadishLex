#!/usr/bin/env python3
from __future__ import annotations

import subprocess
import sys
from pathlib import Path


TEXT_EXTENSIONS = {
    ".arb",
    ".c",
    ".cc",
    ".cfg",
    ".cmake",
    ".cpp",
    ".cs",
    ".css",
    ".dart",
    ".dockerignore",
    ".editorconfig",
    ".gitattributes",
    ".gitignore",
    ".go",
    ".gradle",
    ".h",
    ".hpp",
    ".html",
    ".ini",
    ".java",
    ".js",
    ".json",
    ".jsonc",
    ".kt",
    ".kts",
    ".m",
    ".md",
    ".mm",
    ".plist",
    ".ps1",
    ".py",
    ".rs",
    ".sh",
    ".sql",
    ".swift",
    ".toml",
    ".ts",
    ".tsx",
    ".txt",
    ".xml",
    ".yaml",
    ".yml",
}
TEXT_FILENAMES = {
    "Dockerfile",
    "LICENSE",
    "Makefile",
}
SOURCE_EXTENSIONS = {
    ".c",
    ".cc",
    ".cpp",
    ".cs",
    ".dart",
    ".go",
    ".h",
    ".hpp",
    ".java",
    ".js",
    ".kt",
    ".kts",
    ".m",
    ".mm",
    ".ps1",
    ".py",
    ".rs",
    ".sh",
    ".swift",
    ".ts",
    ".tsx",
}
SKIP_TRAILING_WHITESPACE_EXTENSIONS = {".md", ".csv"}
CRLF_ALLOWED_EXTENSIONS = {".bat", ".cmd"}


def repository_files(repo_root: Path) -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=repo_root,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode == 0:
        return [Path(line) for line in result.stdout.splitlines() if line.strip()]

    return [
        path.relative_to(repo_root)
        for path in repo_root.rglob("*")
        if path.is_file() and ".git" not in path.relative_to(repo_root).parts
    ]


def is_text_file(relative_path: Path) -> bool:
    return relative_path.name in TEXT_FILENAMES or relative_path.suffix.lower() in TEXT_EXTENSIONS


def line_count(content: str) -> int:
    if not content:
        return 0
    count = len(content.split("\n"))
    if content.endswith("\n"):
        count -= 1
    return count


def main() -> int:
    repo_root = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path(__file__).resolve().parents[1]
    errors: list[str] = []

    for relative_path in repository_files(repo_root):
        if not is_text_file(relative_path):
            continue

        full_path = repo_root / relative_path
        if not full_path.is_file():
            continue

        data = full_path.read_bytes()
        if not data:
            continue

        display_path = relative_path.as_posix()
        suffix = relative_path.suffix.lower()
        if data.startswith(b"\xef\xbb\xbf"):
            errors.append(f"{display_path}: contains UTF-8 BOM")
            continue

        try:
            content = data.decode("utf-8")
        except UnicodeDecodeError:
            errors.append(f"{display_path}: is not valid UTF-8")
            continue

        if suffix not in CRLF_ALLOWED_EXTENSIONS and "\r" in content:
            errors.append(f"{display_path}: contains CR or CRLF line endings")

        if not content.endswith("\n"):
            errors.append(f"{display_path}: missing final newline")

        if suffix not in SKIP_TRAILING_WHITESPACE_EXTENSIONS:
            for index, line in enumerate(content.split("\n"), start=1):
                if line.endswith(" ") or line.endswith("\t"):
                    errors.append(f"{display_path}:{index}: trailing whitespace")
                    break

        if suffix in SOURCE_EXTENSIONS:
            lines = line_count(content)
            if lines > 1500:
                errors.append(f"{display_path}: source file has {lines} lines, over 1500 line hard limit")

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    print("Text file hygiene passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

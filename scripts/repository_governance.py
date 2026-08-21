#!/usr/bin/env python3
from __future__ import annotations

import re
from pathlib import Path
from urllib.parse import unquote


INLINE_MARKDOWN_LINK = re.compile(
    r"!?\[[^\]]*\]\(([^)\s]+)(?:\s+['\"][^'\"]*['\"])?\)"
)
REFERENCE_MARKDOWN_LINK = re.compile(r"^\s{0,3}\[(?!\^)[^\]]+\]:\s*(\S+)")
REMOTE_LINK_PREFIXES = ("#", "/", "http://", "https://", "mailto:")


def _relative(repo_root: Path, path: Path) -> str:
    full_path = path if path.is_absolute() else repo_root / path
    return full_path.relative_to(repo_root).as_posix()


def _markdown_targets(content: str) -> list[str]:
    targets: list[str] = []
    fence: str | None = None

    for line in content.splitlines():
        stripped = line.lstrip()
        marker = "```" if stripped.startswith("```") else "~~~" if stripped.startswith("~~~") else None
        if marker is not None:
            if fence is None:
                fence = marker
            elif fence == marker:
                fence = None
            continue
        if fence is not None:
            continue

        targets.extend(match.group(1) for match in INLINE_MARKDOWN_LINK.finditer(line))
        reference = REFERENCE_MARKDOWN_LINK.match(line)
        if reference is not None:
            targets.append(reference.group(1))

    return targets


def check_markdown_links(repo_root: Path, paths: list[Path]) -> list[str]:
    errors: list[str] = []
    resolved_root = repo_root.resolve()

    for path in paths:
        full_path = path if path.is_absolute() else repo_root / path
        if not full_path.is_file() or full_path.suffix.lower() != ".md":
            continue

        relative_path = _relative(repo_root, path)
        try:
            content = full_path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue

        for raw_target in _markdown_targets(content):
            target = unquote(raw_target.strip("<>"))
            if not target or target.startswith(REMOTE_LINK_PREFIXES):
                continue
            target = target.split("#", 1)[0].split("?", 1)[0]
            if not target:
                continue

            resolved = (full_path.parent / target).resolve()
            try:
                resolved.relative_to(resolved_root)
            except ValueError:
                errors.append(f"relative link escapes repository: {relative_path} -> {target}")
                continue
            if not resolved.exists():
                errors.append(f"broken relative link: {relative_path} -> {target}")

    return errors


def _require_phrases(repo_root: Path, relative_path: str, phrases: tuple[str, ...]) -> list[str]:
    path = repo_root / relative_path
    if not path.is_file():
        return [f"missing community governance file: {relative_path}"]

    content = path.read_text(encoding="utf-8")
    return [
        f"{relative_path} is missing governance contract: {phrase}"
        for phrase in phrases
        if phrase not in content
    ]


def check_community_governance(repo_root: Path) -> list[str]:
    contracts = {
        "README.md": (
            "[贡献指南](CONTRIBUTING.md)",
            "[社区行为准则](CODE_OF_CONDUCT.md)",
            "[安全策略](SECURITY.md)",
        ),
        "SECURITY.md": (
            "远程仓库已启用",
            "https://github.com/laugh0608/RadishLex/security/advisories/new",
        ),
        "CONTRIBUTING.md": (
            "Issue chooser",
            "[安全策略](SECURITY.md)",
            "[社区行为准则](CODE_OF_CONDUCT.md)",
        ),
        "CODE_OF_CONDUCT.md": ("[安全策略](SECURITY.md)",),
        ".github/ISSUE_TEMPLATE/config.yml": (
            "blank_issues_enabled: false",
            "https://github.com/laugh0608/RadishLex/security/policy",
        ),
        ".github/ISSUE_TEMPLATE/bug-report.yml": (
            "可利用的安全漏洞请使用仓库 Security 页面私下报告",
            "合成数据或脱敏聚合",
            "P0-P3",
        ),
        ".github/ISSUE_TEMPLATE/change-proposal.yml": (
            "安全漏洞请使用 SECURITY.md 中的私密渠道",
            "隐私、安全与数据生命周期",
            "静态、合成、实机、连续事务及人工证据",
        ),
    }

    errors: list[str] = []
    for relative_path, phrases in contracts.items():
        errors.extend(_require_phrases(repo_root, relative_path, phrases))
    return errors

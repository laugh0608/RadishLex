#!/usr/bin/env python3
from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from repository_governance import check_community_governance, check_markdown_links


class RepositoryFixture(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.repo_root = Path(self.temporary_directory.name)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write(self, relative_path: str, content: str) -> None:
        path = self.repo_root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")


class MarkdownLinkTests(RepositoryFixture):
    def test_accepts_existing_remote_fragment_encoded_and_fenced_links(self) -> None:
        self.write("docs/target file.md", "# Target\n")
        self.write(
            "README.md",
            "\n".join(
                (
                    "[relative](docs/target%20file.md#target)",
                    "[remote](https://example.com/missing)",
                    "[fragment](#local)",
                    "```markdown",
                    "[example](missing-example.md)",
                    "```",
                    "",
                )
            ),
        )

        self.assertEqual(check_markdown_links(self.repo_root, [Path("README.md")]), [])

    def test_reports_missing_reference_style_and_escaping_links(self) -> None:
        self.write(
            "docs/source.md",
            "[missing](missing.md)\n[escape]: ../../outside.md\n",
        )

        self.assertEqual(
            check_markdown_links(self.repo_root, [Path("docs/source.md")]),
            [
                "broken relative link: docs/source.md -> missing.md",
                "relative link escapes repository: docs/source.md -> ../../outside.md",
            ],
        )


class CommunityGovernanceTests(RepositoryFixture):
    def write_valid_contract(self) -> None:
        self.write(
            "README.md",
            "[贡献指南](CONTRIBUTING.md) [社区行为准则](CODE_OF_CONDUCT.md) "
            "[安全策略](SECURITY.md)\n",
        )
        self.write(
            "SECURITY.md",
            "远程仓库已启用\n"
            "https://github.com/laugh0608/RadishLex/security/advisories/new\n",
        )
        self.write(
            "CONTRIBUTING.md",
            "Issue chooser [安全策略](SECURITY.md) [社区行为准则](CODE_OF_CONDUCT.md)\n",
        )
        self.write("CODE_OF_CONDUCT.md", "[安全策略](SECURITY.md)\n")
        self.write(
            ".github/ISSUE_TEMPLATE/config.yml",
            "blank_issues_enabled: false\n"
            "https://github.com/laugh0608/RadishLex/security/policy\n",
        )
        self.write(
            ".github/ISSUE_TEMPLATE/bug-report.yml",
            "可利用的安全漏洞请使用仓库 Security 页面私下报告\n"
            "合成数据或脱敏聚合\nP0-P3\n",
        )
        self.write(
            ".github/ISSUE_TEMPLATE/change-proposal.yml",
            "安全漏洞请使用 SECURITY.md 中的私密渠道\n"
            "隐私、安全与数据生命周期\n"
            "静态、合成、实机、连续事务及人工证据\n",
        )

    def test_accepts_complete_contract(self) -> None:
        self.write_valid_contract()
        self.assertEqual(check_community_governance(self.repo_root), [])

    def test_reports_missing_entry_and_private_reporting_contract(self) -> None:
        self.write_valid_contract()
        self.write("README.md", "[贡献指南](CONTRIBUTING.md)\n")
        self.write(".github/ISSUE_TEMPLATE/config.yml", "blank_issues_enabled: true\n")

        errors = check_community_governance(self.repo_root)

        self.assertIn(
            "README.md is missing governance contract: [社区行为准则](CODE_OF_CONDUCT.md)",
            errors,
        )
        self.assertIn(
            ".github/ISSUE_TEMPLATE/config.yml is missing governance contract: "
            "blank_issues_enabled: false",
            errors,
        )
        self.assertIn(
            ".github/ISSUE_TEMPLATE/config.yml is missing governance contract: "
            "https://github.com/laugh0608/RadishLex/security/policy",
            errors,
        )


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
MAX_COMMITTED_PATH_LENGTH = 180
REQUIRED_FILES = [
    ".editorconfig",
    ".gitattributes",
    ".gitignore",
    ".github/PULL_REQUEST_TEMPLATE.md",
    ".github/rulesets/README.md",
    ".github/rulesets/master-protection.json",
    ".github/workflows/pr-check.yml",
    ".github/workflows/release-check.yml",
    "AGENTS.md",
    "CLAUDE.md",
    "Cargo.lock",
    "Cargo.toml",
    "server/sync-server/go.mod",
    "server/sync-server/internal/storage/store.go",
    "server/sync-server/migrations/0001_init.sql",
    "crates/ime-cli/Cargo.toml",
    "crates/ime-cli/src/lib.rs",
    "crates/ime-cli/src/main.rs",
    "LICENSE",
    "README.md",
    "crates/ime-core/Cargo.toml",
    "crates/ime-core/src/lib.rs",
    "crates/ime-runtime/Cargo.toml",
    "crates/ime-runtime/src/lib.rs",
    "crates/ime-engine-rime/Cargo.toml",
    "crates/ime-engine-rime/build.rs",
    "crates/ime-engine-rime/src/lib.rs",
    "docs/engine-boundary.md",
    "docs/engine-rime-adapter.md",
    "docs/privacy-sync.md",
    "docs/repository-layout.md",
    "docs/roadmap.md",
    "docs/technical-plan.md",
    "platforms/macos-imk/Sources/main.m",
    "platforms/macos-imk/Tools/tis_source_status.m",
    "platforms/macos-imk/build-bundle.sh",
    "platforms/macos-imk/cleanup-user-install.sh",
    "scripts/check-android-target.py",
    "scripts/check-android-target.sh",
    "scripts/check-docs.py",
    "scripts/check-docs.sh",
    "scripts/check-manager-ffi-smoke.sh",
    "scripts/check-manager.sh",
    "scripts/check-macos-imk.sh",
    "scripts/check-macos-imk-native.sh",
    "scripts/cleanup-macos-imk.sh",
    "scripts/macos-imk/native_manifest.py",
    "scripts/macos-imk/test_native_manifest.py",
    "scripts/check-repo.py",
    "scripts/check-repo.sh",
    "scripts/check-sync-deployment-evidence.py",
    "scripts/check-sync-deployment-evidence.sh",
    "scripts/check-sync-server-connection-health.py",
    "scripts/check-sync-server-connection-health.sh",
    "scripts/check-text-files.py",
    "scripts/check-text-files.sh",
    "tests/fixtures/sync-deployment-evidence-valid.txt",
    "apps/radishlex-manager/README.md",
    "apps/radishlex-manager/pubspec.yaml",
    "apps/radishlex-manager/lib/main.dart",
    "apps/radishlex-manager/tool/ffi_bridge_smoke.dart",
    "apps/radishlex-manager/test/widget_test.dart",
]
REQUIRED_STATUS_CHECKS = {
    "Repo Hygiene",
    "Repository Baseline",
    "Rust Clippy",
    "Flutter Manager",
    "Go Quality",
}
CONVENTIONAL_COMMIT_PATTERN = "^(feat|fix|docs|refactor|test|chore|ci|build|perf|revert)(\\([a-z0-9._/-]+\\))?!?: .+"


def run_script(script_name: str, args: list[str] | None = None) -> None:
    command = [sys.executable, str(REPO_ROOT / "scripts" / script_name)]
    if args:
        command.extend(args)
    result = subprocess.run(command, cwd=REPO_ROOT)
    if result.returncode != 0:
        raise SystemExit(result.returncode)


def run_command(command: list[str], cwd: Path = REPO_ROOT) -> None:
    try:
        result = subprocess.run(command, cwd=cwd)
    except FileNotFoundError as exc:
        raise SystemExit(f"{command[0]} is required to run repository baseline checks.") from exc
    if result.returncode != 0:
        raise SystemExit(result.returncode)


def read_text(relative_path: str) -> str:
    return (REPO_ROOT / relative_path).read_text(encoding="utf-8")


def load_json(relative_path: str) -> Any:
    try:
        return json.loads(read_text(relative_path))
    except Exception as exc:
        raise SystemExit(f"failed to parse {relative_path}: {exc}") from exc


def check_required_files() -> None:
    for relative_path in REQUIRED_FILES:
        if not (REPO_ROOT / relative_path).is_file():
            raise SystemExit(f"missing required file: {relative_path}")


def check_collaboration_docs() -> None:
    agents = read_text("AGENTS.md")
    claude = read_text("CLAUDE.md")
    if agents != claude:
        raise SystemExit("AGENTS.md and CLAUDE.md must stay synchronized")

    required_phrases = [
        "RadishLex Source-Available License",
        "P0 数据永不同步",
        "Engine 边界约束",
        "不要把 RadishLex 做成云端实时输入法 API",
    ]
    for phrase in required_phrases:
        if phrase not in agents:
            raise SystemExit(f"AGENTS.md is missing required collaboration phrase: {phrase}")


def check_license_wording() -> None:
    readme = read_text("README.md")
    forbidden_phrases = [
        "开源中文输入系统",
    ]
    for phrase in forbidden_phrases:
        if phrase in readme:
            raise SystemExit(f"README.md uses license-conflicting wording: {phrase}")

    if "源代码可见中文输入系统" not in readme:
        raise SystemExit("README.md should describe RadishLex as a source-available input system")


def required_status_contexts(ruleset: dict[str, Any]) -> set[str]:
    for rule in ruleset.get("rules", []):
        if rule.get("type") != "required_status_checks":
            continue
        checks = rule.get("parameters", {}).get("required_status_checks", [])
        return {str(check.get("context")) for check in checks}
    return set()


def commit_message_pattern(ruleset: dict[str, Any]) -> str | None:
    for rule in ruleset.get("rules", []):
        if rule.get("type") == "commit_message_pattern":
            return str(rule.get("parameters", {}).get("pattern"))
    return None


def check_ruleset_and_workflows() -> None:
    ruleset = load_json(".github/rulesets/master-protection.json")
    if ruleset.get("target") != "branch":
        raise SystemExit("master-protection ruleset must target branch")
    if ruleset.get("enforcement") != "active":
        raise SystemExit("master-protection ruleset must be active")

    include_refs = set(ruleset.get("conditions", {}).get("ref_name", {}).get("include", []))
    if {"refs/heads/master", "refs/heads/main"} - include_refs:
        raise SystemExit("master-protection ruleset must include refs/heads/master and refs/heads/main")

    contexts = required_status_contexts(ruleset)
    if contexts != REQUIRED_STATUS_CHECKS:
        raise SystemExit(
            "ruleset required checks mismatch: "
            f"expected {sorted(REQUIRED_STATUS_CHECKS)}, got {sorted(contexts)}"
        )

    if commit_message_pattern(ruleset) != CONVENTIONAL_COMMIT_PATTERN:
        raise SystemExit("ruleset conventional commit pattern does not match repository convention")

    pr_workflow = read_text(".github/workflows/pr-check.yml")
    if not pr_workflow.startswith("name: PR Checks\n"):
        raise SystemExit("pr-check workflow must use the PR Checks name")
    for context in REQUIRED_STATUS_CHECKS:
        if f"name: {context}" not in pr_workflow:
            raise SystemExit(f"pr-check workflow is missing job name: {context}")
    if "push:" in pr_workflow or "workflow_dispatch:" in pr_workflow:
        raise SystemExit("pr-check workflow must only run for pull requests")
    if "pull_request:" not in pr_workflow:
        raise SystemExit("pr-check workflow must run for pull requests")
    for target_branch in ("dev", "master"):
        if f"      - {target_branch}\n" not in pr_workflow:
            raise SystemExit(f"pr-check workflow is missing target branch: {target_branch}")
    if "      - main\n" in pr_workflow:
        raise SystemExit("pr-check workflow must not run for main pull requests")
    if "git diff --check" not in pr_workflow:
        raise SystemExit("pr-check workflow must check PR diff whitespace")

    release_workflow = read_text(".github/workflows/release-check.yml")
    forbidden_release_triggers = ("pull_request:", "branches:", "workflow_dispatch:")
    if any(trigger in release_workflow for trigger in forbidden_release_triggers):
        raise SystemExit("release-check workflow must only run for release tags")
    for tag_pattern in ('"v*-dev"', '"v*-test"', '"v*-release"'):
        if f"      - {tag_pattern}\n" not in release_workflow:
            raise SystemExit(f"release-check workflow is missing tag pattern: {tag_pattern}")
    for context in ("Release Repo Hygiene", "Release Repository Baseline"):
        if f"name: {context}" not in release_workflow:
            raise SystemExit(f"release-check workflow is missing job name: {context}")


def iter_repository_paths() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit("failed to list repository files")
    return [Path(line) for line in result.stdout.splitlines() if line.strip()]


def check_path_budget() -> None:
    for path in iter_repository_paths():
        relative_path = path.as_posix()
        if len(relative_path) > MAX_COMMITTED_PATH_LENGTH:
            raise SystemExit(
                "repository path exceeds path budget "
                f"({MAX_COMMITTED_PATH_LENGTH}): {relative_path}"
            )


def check_rust_workspace() -> None:
    run_command(["cargo", "fmt", "--check"])
    run_command(["cargo", "test", "--workspace"])


def check_go_server() -> None:
    run_command(["go", "test", "./..."], cwd=REPO_ROOT / "server" / "sync-server")


def check_deployment_evidence() -> None:
    run_script("check-sync-deployment-evidence.py", ["--self-test"])


def check_sync_connection_health() -> None:
    run_script("check-sync-server-connection-health.py", ["--self-test"])


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run RadishLex repository baseline checks.")
    parser.add_argument("--skip-text-files", action="store_true", help="Skip text hygiene checks.")
    parser.add_argument("--skip-docs", action="store_true", help="Skip documentation budget checks.")
    parser.add_argument("--skip-rust", action="store_true", help="Skip Rust workspace checks.")
    parser.add_argument("--skip-go", action="store_true", help="Skip Go server checks.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    if not args.skip_text_files:
        run_script("check-text-files.py", [str(REPO_ROOT)])
    if not args.skip_docs:
        run_script("check-docs.py", [str(REPO_ROOT)])

    check_required_files()
    check_collaboration_docs()
    check_license_wording()
    check_ruleset_and_workflows()
    check_path_budget()
    check_deployment_evidence()
    check_sync_connection_health()
    if not args.skip_go:
        check_go_server()
    if not args.skip_rust:
        check_rust_workspace()

    print("Repository baseline passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

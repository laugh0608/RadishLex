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
    "crates/ime-ffi/src/abi/user_terms.rs",
    "docs/engine-boundary.md",
    "docs/engine-rime-adapter.md",
    "docs/privacy-sync.md",
    "docs/repository-layout.md",
    "docs/roadmap.md",
    "docs/technical-plan.md",
    "docs/macos-product-package-boundary.md",
    "docs/macos-data-upgrade-coordinator.md",
    "docs/macos-installation-transaction.md",
    "docs/macos-installer-app-boundary.md",
    "docs/adr/0008-macos-installation-carrier.md",
    "docs/adr/0007-apple-secure-enclave-p256-backend.md",
    "docs/runbooks/apple-secure-enclave-p256-backend.md",
    "docs/runbooks/apple-secure-enclave-key-agreement-backend.md",
    "docs/runbooks/macos-m2-manager-product-acceptance.md",
    "docs/runbooks/macos-installer-user-domain-acceptance.md",
    "docs/runbooks/macos-release-carrier.md",
    "platforms/macos-imk/Sources/main.m",
    "platforms/macos-imk/Tools/tis_source_status.m",
    "platforms/macos-imk/Tools/test_data_cleanup.c",
    "platforms/macos-imk/build-bundle.sh",
    "platforms/macos-imk/cleanup-m2-manager-test-data.sh",
    "platforms/macos-imk/cleanup-user-install.sh",
    "platforms/linux-fcitx5/CMakeLists.txt",
    "platforms/linux-fcitx5/README.md",
    "platforms/linux-fcitx5/include/radishlex/linux/ffi_projection.h",
    "platforms/linux-fcitx5/include/radishlex/linux/key_projection.h",
    "platforms/linux-fcitx5/include/radishlex/linux/xdg_paths.h",
    "platforms/linux-fcitx5/config/radishlex-addon.conf.in",
    "platforms/linux-fcitx5/config/radishlex.conf.in",
    "platforms/linux-fcitx5/src/fcitx_addon.cpp",
    "platforms/linux-fcitx5/src/fcitx_addon.h",
    "platforms/linux-fcitx5/src/ffi_projection.cpp",
    "platforms/linux-fcitx5/src/key_projection.cpp",
    "platforms/linux-fcitx5/src/linked_ffi_api.cpp",
    "platforms/linux-fcitx5/src/xdg_paths.cpp",
    "platforms/linux-fcitx5/tests/ffi_projection_test.cpp",
    "platforms/linux-fcitx5/tests/xdg_paths_test.cpp",
    "platforms/macos-product/UpgradePreflightHost/Sources/RLXUpgradePreflight.h",
    "platforms/macos-product/UpgradePreflightHost/Sources/RLXUpgradePreflight.m",
    "platforms/macos-product/UpgradePreflightHost/Sources/main.m",
    "platforms/macos-product/UpgradePreflightHost/Tests/contract_smoke.m",
    "platforms/macos-product/UpgradePreflightHost/build.sh",
    "platforms/macos-product/UpgradePreflightHost/check.sh",
    "platforms/macos-product/UpgradeCoordinatorAdapter/Cargo.toml",
    "platforms/macos-product/UpgradeCoordinatorAdapter/src/lib.rs",
    "platforms/macos-product/UpgradeCoordinatorAdapter/tests/product_coordination.rs",
    "platforms/macos-product/UpgradeCoordinatorAdapter/fixtures/qualification.marker",
    "platforms/macos-product/UpgradeCoordinatorAdapter/fixtures/source-product.json",
    "platforms/macos-product/InstallAdapter/Cargo.toml",
    "platforms/macos-product/InstallAdapter/README.md",
    "platforms/macos-product/InstallAdapter/src/lib.rs",
    "platforms/macos-product/InstallAdapter/src/manifest.rs",
    "platforms/macos-product/InstallAdapter/src/codesign.rs",
    "platforms/macos-product/InstallCoordinatorAdapter/Cargo.toml",
    "platforms/macos-product/InstallCoordinatorAdapter/README.md",
    "platforms/macos-product/InstallCoordinatorAdapter/src/lib.rs",
    "platforms/macos-product/InstallCoordinatorAdapter/src/tests.rs",
    "platforms/macos-product/InstallerDriver/Cargo.toml",
    "platforms/macos-product/InstallerDriver/README.md",
    "platforms/macos-product/InstallerDriver/src/lib.rs",
    "platforms/macos-product/InstallerDriver/src/tests.rs",
    "platforms/macos-product/InstallerExecutor/Cargo.toml",
    "platforms/macos-product/InstallerExecutor/README.md",
    "platforms/macos-product/InstallerExecutor/src/lib.rs",
    "platforms/macos-product/InstallerExecutor/src/tests.rs",
    "platforms/macos-product/InstallerBridge/Cargo.toml",
    "platforms/macos-product/InstallerBridge/README.md",
    "platforms/macos-product/InstallerBridge/include/radishlex_installer_bridge.h",
    "platforms/macos-product/InstallerBridge/src/lib.rs",
    "platforms/macos-product/InstallerBridge/src/tests.rs",
    "platforms/macos-product/InstallerApp/README.md",
    "platforms/macos-product/InstallerApp/Resources/Info.plist.in",
    "platforms/macos-product/InstallerApp/Sources/RLXInstallerApplicationMenu.h",
    "platforms/macos-product/InstallerApp/Sources/RLXInstallerApplicationMenu.m",
    "platforms/macos-product/InstallerApp/Sources/RLXInstallerPresentation.h",
    "platforms/macos-product/InstallerApp/Sources/RLXInstallerPresentation.m",
    "platforms/macos-product/InstallerApp/Sources/RLXInstallerBridge.h",
    "platforms/macos-product/InstallerApp/Sources/RLXInstallerBridge.m",
    "platforms/macos-product/InstallerApp/Sources/main.m",
    "platforms/macos-product/InstallerApp/Tests/presentation_contract.m",
    "platforms/macos-product/InstallerApp/build.sh",
    "platforms/macos-product/InstallerApp/check.sh",
    "scripts/check-android-target.py",
    "scripts/check-android-target.sh",
    "scripts/check-docs.py",
    "scripts/check-docs.sh",
    "scripts/check-manager-ffi-smoke.sh",
    "scripts/check-manager-product.sh",
    "scripts/check-linux-fcitx5.sh",
    "scripts/build-macos-product.sh",
    "scripts/build-macos-install-payload.sh",
    "scripts/build-macos-release-installer.sh",
    "scripts/build-macos-release-dmg.sh",
    "scripts/notarize-macos-release-dmg.sh",
    "scripts/macos-product/release_identity.py",
    "scripts/macos-product/test_release_identity.py",
    "scripts/macos-product/community_release.py",
    "scripts/macos-product/test_community_release.py",
    "scripts/macos-product/release_carrier.py",
    "scripts/macos-product/test_release_carrier.py",
    "scripts/check-macos-product-metadata.sh",
    "scripts/check-macos-install-layout.sh",
    "scripts/check-macos-install-adapter.sh",
    "scripts/check-macos-install-coordinator.sh",
    "scripts/check-macos-installer.sh",
    "scripts/check-macos-release-carrier.sh",
    "scripts/check-product-install-core.sh",
    "scripts/check-macos-upgrade-preflight.sh",
    "scripts/check-macos-upgrade-coordinator.sh",
    "scripts/check-macos-upgrade-product-coordination.sh",
    "scripts/check-manager.sh",
    "scripts/build-manager-macos-product.sh",
    "scripts/build-manager-macos-dpk-qualified-product.sh",
    "scripts/embed-manager-native-library.sh",
    "scripts/check-macos-imk.sh",
    "scripts/check-macos-imk-native.sh",
    "scripts/cleanup-macos-m2-manager-test-data.sh",
    "scripts/cleanup-macos-imk.sh",
    "scripts/run-manager-apple-secure-enclave-p256-product-smoke.sh",
    "scripts/run-manager-apple-secure-enclave-key-agreement-product-smoke.sh",
    "scripts/macos-imk/native_manifest.py",
    "scripts/macos-imk/test_native_manifest.py",
    "scripts/macos-product/product_manifest.py",
    "scripts/macos-product/test_product_manifest.py",
    "scripts/macos-product/install_layout.py",
    "scripts/macos-product/test_install_layout.py",
    "scripts/prepare-rime-product-data.sh",
    "scripts/rime-product/product_data.py",
    "scripts/rime-product/test_product_data.py",
    "version.json",
    "packaging/macos/product.json",
    "packaging/macos/install-layout.json",
    "packaging/macos/README.md",
    "packaging/rime/README.md",
    "packaging/rime/product-rime-data.json",
    "packaging/rime/data/default.yaml",
    "packaging/rime/data/radishlex_pinyin.schema.yaml",
    "packaging/rime/data/pinyin_simp.dict.yaml",
    "packaging/rime/licenses/rime-pinyin-simp/LICENSE",
    "packaging/rime/licenses/rime-pinyin-simp/AUTHORS",
    "crates/ime-product-install/Cargo.toml",
    "crates/ime-product-install/README.md",
    "crates/ime-product-install/src/lib.rs",
    "crates/ime-product-install/src/filesystem.rs",
    "scripts/check-repo.py",
    "scripts/check-repo.sh",
    "scripts/check-sync-deployment-evidence.py",
    "scripts/check-sync-deployment-evidence.sh",
    "scripts/check-sync-server-deployment-rehearsal.py",
    "scripts/check-sync-server-deployment-rehearsal.sh",
    "scripts/check-sync-server-local-https.py",
    "scripts/check-sync-server-local-https.sh",
    "scripts/check-sync-server-connection-health.py",
    "scripts/check-sync-server-connection-health.sh",
    "scripts/check-text-files.py",
    "scripts/check-text-files.sh",
    "tests/fixtures/sync-deployment-evidence-valid.txt",
    "apps/radishlex-manager/README.md",
    "apps/radishlex-manager/pubspec.yaml",
    "apps/radishlex-manager/lib/main.dart",
    "apps/radishlex-manager/lib/src/bridge/manager_platform_control.dart",
    "apps/radishlex-manager/lib/src/bridge/method_channel_manager_platform_control.dart",
    "apps/radishlex-manager/macos/Runner/MainFlutterWindow.swift",
    "apps/radishlex-manager/macos/Runner/DPKQualification.entitlements",
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


def check_manager_product_runtime_contract() -> None:
    factory = read_text(
        "apps/radishlex-manager/lib/src/bridge/manager_bridge_factory.dart"
    )
    for phrase in (
        "defaultValue: 'product'",
        "ManagerRuntimeMode.demo",
        "UnavailableManagerBridge",
        "MethodChannelManagerPlatformControl",
    ):
        if phrase not in factory:
            raise SystemExit(f"manager product bootstrap is missing contract phrase: {phrase}")
    if "Platform.environment" in factory or "fixture_fallback" in factory:
        raise SystemExit("manager product bootstrap must not use environment or fixture fallback")

    swift_bridge = read_text(
        "apps/radishlex-manager/macos/Runner/MainFlutterWindow.swift"
    )
    for phrase in (
        "FileManager.default",
        "libradishlex_ime_ffi.dylib",
        "CFPreferencesCopyValue",
        "CFPreferencesSetValue",
        "CFPreferencesSynchronize",
        "restorePrivacyModeState",
        "kCFPreferencesCurrentUser",
        "kCFPreferencesAnyHost",
        "lstat",
        ".posixPermissions: 0o700",
        ".posixPermissions: 0o600",
    ):
        if phrase not in swift_bridge:
            raise SystemExit(f"manager macOS runtime bridge is missing contract phrase: {phrase}")

    for entitlements_path in (
        "apps/radishlex-manager/macos/Runner/DebugProfile.entitlements",
        "apps/radishlex-manager/macos/Runner/Release.entitlements",
    ):
        if "com.apple.security.app-sandbox" in read_text(entitlements_path):
            raise SystemExit(f"M2 manager must not enable App Sandbox: {entitlements_path}")

    xcode_project = read_text(
        "apps/radishlex-manager/macos/Runner.xcodeproj/project.pbxproj"
    )
    if "Embed RadishLex Native Library" not in xcode_project:
        raise SystemExit("manager Xcode target must embed the RadishLex native library")

    embed_script = read_text("scripts/embed-manager-native-library.sh")
    for symbol in (
        "_radishlex_ffi_contract",
        "_radishlex_manager_sync_product_status",
        "_radishlex_apple_secure_enclave_key_agreement_product_status",
        "_radishlex_apple_secure_enclave_key_agreement_product_smoke",
        "_radishlex_userdb_deleted_terms_new",
        "_radishlex_userdb_restore_term",
    ):
        if symbol not in embed_script:
            raise SystemExit(f"manager native bundle gate is missing symbol: {symbol}")


def check_macos_product_metadata() -> None:
    run_command([str(REPO_ROOT / "scripts/check-macos-product-metadata.sh")])


def check_product_install_core() -> None:
    run_command([str(REPO_ROOT / "scripts/check-product-install-core.sh")])


def check_macos_install_adapter() -> None:
    if sys.platform != "darwin":
        return
    run_command([str(REPO_ROOT / "scripts/check-macos-install-adapter.sh")])


def check_macos_install_coordinator() -> None:
    if sys.platform != "darwin":
        return
    run_command([str(REPO_ROOT / "scripts/check-macos-install-coordinator.sh")])


def check_macos_installer() -> None:
    if sys.platform != "darwin":
        return
    run_command([str(REPO_ROOT / "scripts/check-macos-installer.sh")])


def check_macos_release_carrier() -> None:
    run_command([str(REPO_ROOT / "scripts/check-macos-release-carrier.sh")])


def check_macos_upgrade_preflight() -> None:
    if sys.platform != "darwin":
        return
    run_command([str(REPO_ROOT / "scripts/check-macos-upgrade-coordinator.sh")])


def check_linux_fcitx5() -> None:
    run_command([str(REPO_ROOT / "scripts/check-linux-fcitx5.sh")])


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


def check_sync_deployment_hardening() -> None:
    production_compose = read_text("deploy/sync-server/docker-compose.yaml")
    for phrase in (
        'user: "${RADISHLEX_SYNC_RUNTIME_UID:-10001}:${RADISHLEX_SYNC_RUNTIME_GID:-10001}"',
        "read_only: true",
        "cap_drop:\n      - ALL",
        "no-new-privileges:true",
        '"${RADISHLEX_SYNC_BIND:-127.0.0.1}:${RADISHLEX_SYNC_PORT:-7319}:7319"',
    ):
        if phrase not in production_compose:
            raise SystemExit(f"production sync compose is missing hardening contract: {phrase}")

    dockerfile = read_text("server/sync-server/Dockerfile")
    if "USER 10001:10001" not in dockerfile:
        raise SystemExit("sync server runtime image must use the fixed non-root identity")

    private_storage = read_text("server/sync-server/internal/runtime/private_storage.go")
    for phrase in ("0o700", "0o600", "os.Lstat", "os.ModeSymlink", "os.Chmod"):
        if phrase not in private_storage:
            raise SystemExit(f"sync private storage gate is missing contract phrase: {phrase}")

    rehearsal = read_text("scripts/check-sync-server-deployment-rehearsal.py")
    for phrase in ("runtime_identity()", "0o700", "0o600", "assert_backup_safe", "refuses to run the sync container as root"):
        if phrase not in rehearsal:
            raise SystemExit(f"sync deployment rehearsal is missing hardening contract: {phrase}")


def check_sync_connection_health() -> None:
    run_script("check-sync-server-connection-health.py", ["--self-test"])


def check_sync_local_https() -> None:
    run_script("check-sync-server-local-https.py", ["--self-test"])


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
    check_manager_product_runtime_contract()
    check_macos_product_metadata()
    check_product_install_core()
    check_macos_install_adapter()
    check_macos_install_coordinator()
    check_macos_installer()
    check_macos_release_carrier()
    check_macos_upgrade_preflight()
    check_linux_fcitx5()
    check_ruleset_and_workflows()
    check_path_budget()
    check_deployment_evidence()
    check_sync_deployment_hardening()
    check_sync_connection_health()
    check_sync_local_https()
    if not args.skip_go:
        check_go_server()
    if not args.skip_rust:
        check_rust_workspace()

    print("Repository baseline passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Conservative PR impact selection and fail-closed quality aggregation."""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RELIABILITY_PACKAGES = {
    "radishlex-ime-product-install", "radishlex-ime-product-upgrade",
    "radishlex-ime-userdb", "radishlex-ime-ffi",
    "radishlex-macos-upgrade-coordinator", "radishlex-macos-product-install",
    "radishlex-macos-product-install-coordinator", "radishlex-macos-installer-driver",
    "radishlex-macos-installer-executor", "radishlex-macos-installer-bridge",
}
QUALITY_JOBS = {"hygiene", "rust", "manager", "go", "reliability"}


def reliability_required(paths: list[str], root: Path = ROOT) -> bool:
    if not paths:
        return True  # An empty/unknown diff must never waive a specialist.
    workspace = tomllib.loads((root / "Cargo.toml").read_text())
    packages = {}
    for member in workspace["workspace"]["members"]:
        directory = root / member
        manifest = tomllib.loads((directory / "Cargo.toml").read_text())
        packages[manifest["package"]["name"]] = (directory, manifest)
    affected = set()
    for path in paths:
        relative = Path(path)
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError("diff path escapes repository")
        if path.endswith(".md") or path.startswith("docs/"):
            continue
        owners = [name for name, (directory, _) in packages.items()
                  if relative.is_relative_to(directory.relative_to(root))]
        if owners:
            affected.update(owners)
            continue
        if (path.startswith("docs/") or path in {
            "README.md", "CONTRIBUTING.md", "AGENTS.md", "CLAUDE.md",
            "SECURITY.md", "CODE_OF_CONDUCT.md", "LICENSE",
        } or path.startswith("server/sync-server/")):
            continue
        if path.startswith("apps/radishlex-manager/") and (
            path.endswith(".dart") or path.endswith("pubspec.yaml") or path.endswith("pubspec.lock")):
            continue
        # Toolchains, lockfiles, build/validation scripts, platform bridges and
        # unclassified paths are deliberately conservative.
        return True
    changed = True
    while changed:
        changed = False
        for name, (directory, manifest) in packages.items():
            if name in affected:
                continue
            sections = [manifest.get(key, {}) for key in
                        ("dependencies", "dev-dependencies", "build-dependencies")]
            for target in manifest.get("target", {}).values():
                sections.extend(target.get(key, {}) for key in
                                ("dependencies", "dev-dependencies", "build-dependencies"))
            dependencies = set()
            names_by_path = {path.resolve(): package for package, (path, _) in packages.items()}
            for section in sections:
                for alias, dependency in section.items():
                    if not isinstance(dependency, dict):
                        continue
                    dependency_root = directory
                    if dependency.get("workspace"):
                        dependency = workspace["workspace"]["dependencies"][alias]
                        dependency_root = root
                    if isinstance(dependency, dict) and "path" in dependency:
                        target = (dependency_root / dependency["path"]).resolve()
                        if target in names_by_path:
                            dependencies.add(names_by_path[target])
            if affected & dependencies:
                affected.add(name)
                changed = True
    return bool(affected & RELIABILITY_PACKAGES)


def check_quality(results: dict, required: bool) -> None:
    if set(results) != QUALITY_JOBS:
        raise ValueError("quality job inventory mismatch")
    for name in sorted(QUALITY_JOBS):
        result = results[name].get("result")
        if name == "reliability" and not required and result == "skipped":
            continue
        if result != "success":
            raise ValueError(f"required quality job {name}: {result}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("plan", "aggregate"))
    args = parser.parse_args()
    if args.command == "aggregate":
        required = os.environ["RELIABILITY_REQUIRED"]
        if required not in ("true", "false"):
            raise ValueError("invalid reliability decision")
        check_quality(json.loads(os.environ["QUALITY_RESULTS"]), required == "true")
        print("All required quality components passed.")
        return
    base, head = os.environ["BASE_SHA"], os.environ["HEAD_SHA"]
    if not all(len(ref) in (40, 64) and all(c in "0123456789abcdef" for c in ref)
               for ref in (base, head)):
        raise ValueError("expected complete commit SHAs")
    paths = subprocess.check_output(
        ["git", "diff", "--name-only", "--no-renames", "-z", f"{base}...{head}", "--"], cwd=ROOT,
    ).decode().rstrip("\0").split("\0")
    required = reliability_required([path for path in paths if path])
    value = str(required).lower()
    print(json.dumps({"base": base, "head": head, "reliability": required, "paths": paths}))
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        output.write(f"reliability={value}\n")


if __name__ == "__main__":
    main()

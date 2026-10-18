"""One command inventory for standalone and repository macOS/core gates.

The repository runs default Rust targets once via workspace --all-targets and
--doc. Feature-specific tests and native checks stay explicit. Phase execution
is partial and cannot print a full-gate success message.
"""
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from validation_process import run_command

ROOT = Path(__file__).resolve().parents[1]
INSTALL = "radishlex-macos-product-install-coordinator"
UPGRADE = "radishlex-macos-upgrade-coordinator"
CORE = "radishlex-ime-product-install"
ADAPTER = "radishlex-macos-product-install"
INSTALLER = [f"radishlex-macos-installer-{part}" for part in ("driver", "executor", "bridge")]


def cargo(verb: str, package: str, *args: str) -> list[str]:
    return ["cargo", verb, "--locked", "-p", package, *args]


def clippy(package: str, *args: str) -> list[str]:
    return cargo("clippy", package, *args, "--", "-D", "warnings")


def script(path: str, *args: str) -> list[str]:
    return [str(ROOT / path), *args]


SUITES = {
    "product-install-core": {
        "static": [clippy(CORE, "--all-targets")],
        "default-tests": [cargo("test", CORE, "--all-targets")],
    },
    "macos-install-adapter": {
        "static": [clippy(ADAPTER, "--all-targets")],
        "default-tests": [cargo("test", ADAPTER, "--all-targets")],
    },
    "macos-install-coordinator": {
        "static": [
            clippy(INSTALL, "--all-targets"),
            clippy(INSTALL, "--features", "qualification-harness", "--test", "cancellation_archive"),
        ],
        "default-tests": [cargo("test", INSTALL, "--all-targets")],
        "extra-tests": [cargo("test", INSTALL, "--features", "qualification-harness", "--test", "cancellation_archive")],
    },
    "macos-installer": {
        "static": [clippy(package, "--all-targets") for package in INSTALLER],
        "default-tests": [cargo("test", package, "--all-targets") for package in INSTALLER],
        "native": [
            [sys.executable, str(ROOT / "scripts/macos-product/test_release_identity.py")],
            script("platforms/macos-product/InstallerApp/check.sh"),
        ],
    },
    "macos-upgrade-coordinator": {
        "static": [
            clippy(UPGRADE, "--all-targets"),
            clippy(UPGRADE, "--features", "qualification-harness", "--all-targets"),
            cargo("test", UPGRADE, "--features", "qualification-harness", "--no-run"),
        ],
        "default-tests": [cargo("test", UPGRADE)],
        "native": [
            ["bash", "-n", str(ROOT / "scripts/check-macos-upgrade-product-coordination.sh")],
            script("platforms/macos-product/UpgradePreflightHost/check.sh"),
            script("platforms/macos-product/UpgradeValidationHosts/check.sh"),
        ],
    },
}
PHASES = ("static", "default-tests", "extra-tests", "native")


def available_suites(platform: str) -> list[str]:
    return [name for name in SUITES if platform == "darwin" or not name.startswith("macos-")]


def repository_rust_commands(platform: str, phase: str) -> list[list[str]]:
    if phase == "static":
        return [["cargo", "fmt", "--all", "--", "--check"],
                clippy("radishlex-ime-userdb", "--all-targets", "--features", "synthetic-schema-observer"),
                clippy("radishlex-ime-sync", "--all-targets")] + [
            command for name in available_suites(platform) for command in SUITES[name].get("static", [])
        ]
    if phase == "tests":
        return [["cargo", "test", "--locked", "--workspace", "--all-targets"],
                ["cargo", "test", "--locked", "--workspace", "--doc"]] + [
            command for name in available_suites(platform) for command in SUITES[name].get("extra-tests", [])
        ]
    raise ValueError(f"unknown repository phase: {phase}")


def run_suite(name: str, phase: str = "all") -> None:
    if name.startswith("macos-") and sys.platform != "darwin":
        raise SystemExit("macOS is required for this gate")
    if name == "macos-upgrade-coordinator" and phase in ("all", "native"):
        marker = ROOT / "platforms/macos-product/UpgradeCoordinatorAdapter/fixtures/qualification.marker"
        if marker.read_text().strip() != "radishlex-upgrade-qualification-v1":
            raise SystemExit("invalid qualification marker")
    for selected in PHASES if phase == "all" else (phase,):
        for command in SUITES[name].get(selected, []):
            run_command(command, ROOT)
    print(f"{name}: {'gate passed' if phase == 'all' else phase + ' phase passed (partial gate)' }.", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("suite", choices=SUITES)
    parser.add_argument("--phase", choices=("all", *PHASES), default="all")
    args = parser.parse_args()
    run_suite(args.suite, args.phase)


if __name__ == "__main__":
    main()

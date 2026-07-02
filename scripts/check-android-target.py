#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import platform
import re
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TARGET = "aarch64-linux-android"
DEFAULT_API_LEVEL = 35
TARGET_CLANG_PREFIXES = {
    "aarch64-linux-android": "aarch64-linux-android",
    "armv7-linux-androideabi": "armv7a-linux-androideabi",
    "i686-linux-android": "i686-linux-android",
    "x86_64-linux-android": "x86_64-linux-android",
}


def run_capture(command: list[str]) -> subprocess.CompletedProcess[str]:
    try:
        return subprocess.run(
            command,
            cwd=REPO_ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
    except FileNotFoundError as exc:
        raise SystemExit(f"{command[0]} is required for Android target checks.") from exc


def candidate_sdk_roots() -> list[Path]:
    roots: list[Path] = []
    for env_name in ("ANDROID_HOME", "ANDROID_SDK_ROOT"):
        value = os.environ.get(env_name)
        if value:
            roots.append(Path(value).expanduser())
    roots.extend(
        [
            Path("~/Library/Android/sdk").expanduser(),
            Path("~/Android/Sdk").expanduser(),
        ]
    )

    unique_roots: list[Path] = []
    seen: set[Path] = set()
    for root in roots:
        resolved = root.resolve() if root.exists() else root
        if resolved not in seen:
            seen.add(resolved)
            unique_roots.append(root)
    return unique_roots


def find_android_sdk() -> Path:
    for root in candidate_sdk_roots():
        if (root / "ndk").is_dir():
            return root
    searched = ", ".join(str(root) for root in candidate_sdk_roots())
    raise SystemExit(
        "Android SDK with side-by-side NDK was not found. "
        f"Searched: {searched}. Set ANDROID_HOME or ANDROID_SDK_ROOT."
    )


def ndk_revision(ndk_dir: Path) -> tuple[int, ...]:
    source_properties = ndk_dir / "source.properties"
    if not source_properties.is_file():
        return (0,)
    text = source_properties.read_text(encoding="utf-8", errors="replace")
    match = re.search(r"^Pkg\.Revision\s*=\s*([0-9.]+)", text, re.MULTILINE)
    if not match:
        return (0,)
    return tuple(int(part) for part in match.group(1).split(".") if part.isdigit())


def find_ndk(sdk_root: Path) -> Path:
    ndk_root = sdk_root / "ndk"
    ndk_dirs = [path for path in ndk_root.iterdir() if path.is_dir()] if ndk_root.is_dir() else []
    ndk_dirs = [path for path in ndk_dirs if (path / "toolchains" / "llvm").is_dir()]
    if not ndk_dirs:
        raise SystemExit(f"No usable NDK found under {ndk_root}.")
    return sorted(ndk_dirs, key=ndk_revision)[-1]


def host_tags() -> list[str]:
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == "darwin":
        tags = ["darwin-aarch64", "darwin-x86_64"] if machine in {"arm64", "aarch64"} else ["darwin-x86_64"]
    elif system == "linux":
        tags = ["linux-x86_64"]
    elif system == "windows":
        tags = ["windows-x86_64"]
    else:
        tags = []
    return tags


def find_prebuilt_bin(ndk_dir: Path) -> Path:
    prebuilt_root = ndk_dir / "toolchains" / "llvm" / "prebuilt"
    for tag in host_tags():
        candidate = prebuilt_root / tag / "bin"
        if candidate.is_dir():
            return candidate
    available = ", ".join(path.name for path in prebuilt_root.iterdir() if path.is_dir())
    raise SystemExit(f"No NDK prebuilt toolchain matches this host. Available: {available}")


def find_linker(ndk_dir: Path, target: str, api_level: int) -> Path:
    prefix = TARGET_CLANG_PREFIXES.get(target)
    if prefix is None:
        supported = ", ".join(sorted(TARGET_CLANG_PREFIXES))
        raise SystemExit(f"Unsupported Android Rust target {target}. Supported: {supported}")
    linker = find_prebuilt_bin(ndk_dir) / f"{prefix}{api_level}-clang"
    if not linker.is_file():
        raise SystemExit(f"Android linker not found: {linker}")
    return linker


def installed_rust_targets() -> set[str]:
    result = run_capture(["rustup", "target", "list", "--installed"])
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        raise SystemExit(result.returncode)
    return {line.strip() for line in result.stdout.splitlines() if line.strip()}


def linker_env_name(target: str) -> str:
    return f"CARGO_TARGET_{target.upper().replace('-', '_')}_LINKER"


def run_cargo_check(target: str, linker: Path) -> int:
    env = os.environ.copy()
    env[linker_env_name(target)] = str(linker)
    command = [
        "cargo",
        "check",
        "-p",
        "radishlex-ime-crypto",
        "--features",
        "android-keystore",
        "--target",
        target,
    ]
    print("Running:", " ".join(command))
    sys.stdout.flush()
    result = subprocess.run(command, cwd=REPO_ROOT, env=env)
    return result.returncode


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Check RadishLex Android Rust target build prerequisites and cargo check."
    )
    parser.add_argument("--target", default=DEFAULT_TARGET, help=f"Rust Android target. Default: {DEFAULT_TARGET}.")
    parser.add_argument("--api-level", type=int, default=DEFAULT_API_LEVEL, help=f"Android API level for clang. Default: {DEFAULT_API_LEVEL}.")
    parser.add_argument("--preflight-only", action="store_true", help="Only check SDK, NDK, linker, and Rust target availability.")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    sdk_root = find_android_sdk()
    ndk_dir = find_ndk(sdk_root)
    linker = find_linker(ndk_dir, args.target, args.api_level)
    targets = installed_rust_targets()

    print(f"Android SDK: {sdk_root}")
    print(f"Android NDK: {ndk_dir.name}")
    print(f"Android linker: {linker}")
    print(f"Rust target: {args.target}")

    if args.target not in targets:
        sys.stdout.flush()
        print(f"Missing Rust target: {args.target}", file=sys.stderr)
        print(f"Install with: rustup target add {args.target}", file=sys.stderr)
        return 2

    print("Android Rust target preflight passed.")
    if args.preflight_only:
        return 0
    return run_cargo_check(args.target, linker)


if __name__ == "__main__":
    raise SystemExit(main())

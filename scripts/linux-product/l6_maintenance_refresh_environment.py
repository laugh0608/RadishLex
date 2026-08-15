#!/usr/bin/env python3
from __future__ import annotations

import os
import platform
import subprocess
from pathlib import Path
from typing import Any

import l6_release_pair
from l6_maintenance_refresh_io import L6MaintenanceRefreshError


ENVIRONMENT_KEYS = {
    "architecture",
    "format_version",
    "operating_system",
    "profile",
    "tools",
}


def safe_version(value: str, label: str) -> str:
    try:
        return l6_release_pair.safe_version(value, label)
    except l6_release_pair.L6ReleasePairError as exc:
        raise L6MaintenanceRefreshError(str(exc)) from exc


def tool_version(command: list[str], label: str) -> str:
    try:
        result = subprocess.run(
            command,
            check=True,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            env={**os.environ, "LC_ALL": "C"},
        )
    except (OSError, subprocess.CalledProcessError) as exc:
        raise L6MaintenanceRefreshError(f"cannot read {label} version: {exc}") from exc
    value = result.stdout.splitlines()[0] if result.stdout.splitlines() else ""
    return safe_version(value, label)


def collect_environment() -> dict[str, Any]:
    if platform.system() != "Linux" or platform.machine() not in {"aarch64", "arm64"}:
        raise L6MaintenanceRefreshError(
            "maintenance refresh build evidence requires ARM64 Linux"
        )
    os_release: dict[str, str] = {}
    try:
        for line in Path("/etc/os-release").read_text(encoding="utf-8").splitlines():
            if "=" in line:
                key, value = line.split("=", 1)
                os_release[key] = value.strip().strip('"')
    except OSError as exc:
        raise L6MaintenanceRefreshError(
            f"cannot read /etc/os-release: {exc}"
        ) from exc
    environment = {
        "architecture": "arm64",
        "format_version": 1,
        "operating_system": {
            "codename": os_release.get("VERSION_CODENAME"),
            "id": os_release.get("ID"),
            "version_id": os_release.get("VERSION_ID"),
        },
        "profile": "debian13-arm64-maintenance-refresh-build-v1",
        "tools": {
            "cargo": tool_version(["cargo", "--version"], "cargo"),
            "git": tool_version(["/usr/bin/git", "--version"], "git"),
            "rustc": tool_version(["rustc", "--version"], "rustc"),
        },
    }
    validate_environment(environment)
    return environment


def validate_environment(value: dict[str, Any]) -> None:
    if not isinstance(value, dict):
        raise L6MaintenanceRefreshError(
            "maintenance refresh build environment must be an object"
        )
    if set(value) != ENVIRONMENT_KEYS:
        raise L6MaintenanceRefreshError(
            "maintenance refresh build environment fields differ from format v1"
        )
    if value.get("format_version") != 1 or value.get("architecture") != "arm64":
        raise L6MaintenanceRefreshError(
            "maintenance refresh build environment identity is invalid"
        )
    if value.get("profile") != "debian13-arm64-maintenance-refresh-build-v1":
        raise L6MaintenanceRefreshError(
            "maintenance refresh build environment profile is invalid"
        )
    if value.get("operating_system") != {
        "codename": "trixie",
        "id": "debian",
        "version_id": "13",
    }:
        raise L6MaintenanceRefreshError(
            "maintenance refresh operating system identity is invalid"
        )
    tools = value.get("tools")
    if not isinstance(tools, dict) or set(tools) != {"cargo", "git", "rustc"}:
        raise L6MaintenanceRefreshError(
            "maintenance refresh tool inventory is invalid"
        )
    for label, version in tools.items():
        if not isinstance(version, str):
            raise L6MaintenanceRefreshError(f"{label} version must be text")
        safe_version(version, label)

#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


def require_order(source: str, label: str, markers: list[str]) -> None:
    positions = []
    for marker in markers:
        position = source.find(marker)
        if position < 0:
            raise SystemExit(f"{label} is missing startup marker: {marker}")
        positions.append(position)
    if positions != sorted(positions) or len(set(positions)) != len(positions):
        raise SystemExit(f"{label} startup markers are not strictly ordered")


def main() -> None:
    manager = (REPO_ROOT / "apps/radishlex-manager/linux/runner/main.cc").read_text(
        encoding="utf-8"
    )
    require_order(
        manager,
        "Linux Manager",
        [
            "umask(0077);",
            "authorizeLinkedStartup(",
            "my_application_new();",
            "g_application_run(",
        ],
    )

    addon = (
        REPO_ROOT / "platforms/linux-fcitx5/src/fcitx_addon.cpp"
    ).read_text(encoding="utf-8")
    factory = addon[addon.index("EngineFactory::create") :]
    require_order(
        factory,
        "Linux Fcitx factory",
        ["authorizeLinkedStartup(", "new Engine("],
    )

    addon_header = (
        REPO_ROOT / "platforms/linux-fcitx5/src/fcitx_addon.h"
    ).read_text(encoding="utf-8")
    if "StartupPermit startup_permit" not in addon_header:
        raise SystemExit("Fcitx Engine construction is not guarded by StartupPermit")

    startup = (
        REPO_ROOT / "platforms/linux-fcitx5/src/product_startup.cpp"
    ).read_text(encoding="utf-8")
    for forbidden in (
        "resolveProductionXdgPaths",
        "preparePrivateProductPaths",
        "ManagerRuntimeBridge",
        "session_new",
        "userdb",
        "privacy",
        "rime_runtime",
    ):
        if forbidden in startup:
            raise SystemExit(
                f"Linux startup adapter enters business initialization: {forbidden}"
            )

    linked_startup = (
        REPO_ROOT / "platforms/linux-fcitx5/src/linked_product_startup.cpp"
    ).read_text(encoding="utf-8")
    linked_authorize = linked_startup[linked_startup.index("authorizeLinkedStartup") :]
    require_order(
        linked_authorize,
        "Linux linked startup",
        [
            "canonicalObjectPath(",
            "expectedFfiPath(",
            "validateStartupApiOrigins(",
            "validateFcitxApiOrigins(",
            "authorizeStartup(",
        ],
    )
    for symbol in (
        "radishlex_linux_product_startup_gate",
        "radishlex_session_new_personalized_rime",
        "radishlex_session_handle_key_event",
        "radishlex_error_message",
        "radishlex_error_free",
    ):
        if symbol not in linked_startup:
            raise SystemExit(f"Linux linked startup does not bind FFI origin: {symbol}")

    manager_cmake = (
        REPO_ROOT / "apps/radishlex-manager/linux/CMakeLists.txt"
    ).read_text(encoding="utf-8")
    for profile in ("development-staged", "system-product"):
        if profile not in manager_cmake:
            raise SystemExit(f"Manager native startup profile is missing: {profile}")

    fcitx_cmake = (
        REPO_ROOT / "platforms/linux-fcitx5/CMakeLists.txt"
    ).read_text(encoding="utf-8")
    for identity in (
        "RADISHLEX_LINUX_STARTUP_BUILD_IDENTITY=1",
        "RADISHLEX_LINUX_STARTUP_BUILD_IDENTITY=2",
    ):
        if identity not in fcitx_cmake:
            raise SystemExit(f"Fcitx startup compile identity is missing: {identity}")

    print("Linux Manager/Fcitx startup ordering contract passed")


if __name__ == "__main__":
    main()

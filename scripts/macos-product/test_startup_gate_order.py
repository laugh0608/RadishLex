#!/usr/bin/env python3
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]


def require_order(text: str, markers: list[str], label: str) -> None:
    positions = []
    for marker in markers:
        position = text.find(marker)
        if position < 0:
            raise SystemExit(f"{label} is missing startup marker: {marker}")
        positions.append(position)
    if positions != sorted(positions) or len(set(positions)) != len(positions):
        raise SystemExit(f"{label} startup gates are not strictly ordered")


def require_markers(text: str, markers: list[str], label: str) -> None:
    for marker in markers:
        if marker not in text:
            raise SystemExit(f"{label} is missing fail-closed marker: {marker}")


def main() -> None:
    manager = (
        REPO_ROOT / "apps/radishlex-manager/macos/Runner/AppDelegate.swift"
    ).read_text(encoding="utf-8")
    require_order(
        manager,
        [
            "let installGate = RadishLexProductInstallStartupGate.inspect()",
            "let dataGate = RadishLexProductUpgradeStartupGate.inspect()",
            "super.applicationWillFinishLaunching(notification)",
        ],
        "Manager",
    )
    require_markers(
        manager,
        [
            "guard installGate.installAllowed else",
            "guard dataGate.dataAllowed else",
            "guard version == 1 && errorCode == 0 else",
            "return [10, 11, 14].contains(receiptState)",
            "return [9, 10, 12].contains(receiptState)",
        ],
        "Manager",
    )

    input_method = (
        REPO_ROOT / "platforms/macos-imk/Sources/main.m"
    ).read_text(encoding="utf-8")
    require_order(
        input_method,
        [
            "radishlex_product_install_startup_gate(",
            "radishlex_product_upgrade_startup_gate(",
            "[[IMKServer alloc] initWithName:",
            "[[NSApplication sharedApplication] run]",
        ],
        "InputMethod",
    )
    require_markers(
        input_method,
        [
            "installGateResult.error_code == RADISHLEX_STARTUP_GATE_ERROR_NONE",
            "RADISHLEX_INSTALL_RECEIPT_STATE_ABORTED_PRESERVED",
            "RADISHLEX_INSTALL_RECEIPT_STATE_ROLLED_BACK",
            "dataGateResult.error_code == RADISHLEX_STARTUP_GATE_ERROR_NONE",
            "RADISHLEX_UPGRADE_RECEIPT_STATE_ABORTED_PRESERVED",
            "RADISHLEX_UPGRADE_RECEIPT_STATE_ROLLED_BACK",
        ],
        "InputMethod",
    )

    manager_startup = manager[
        manager.find("let installGate =") : manager.find(
            "super.applicationWillFinishLaunching(notification)"
        )
    ]
    input_method_startup = input_method[
        input_method.find("radishlex_product_install_startup_gate(") : input_method.find(
            "[[IMKServer alloc] initWithName:"
        )
    ]
    startup_sources = manager_startup + input_method_startup
    for forbidden in ("getenv(", "ProcessInfo.processInfo.environment", "UserDefaults"):
        if forbidden in startup_sources:
            raise SystemExit(f"startup gate source contains forbidden identity input: {forbidden}")


if __name__ == "__main__":
    main()

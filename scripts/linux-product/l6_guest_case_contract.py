#!/usr/bin/env python3
from __future__ import annotations

import argparse
from dataclasses import dataclass


BUILD_ENVIRONMENT_FILENAME = "build-environment.json"
INPUT_INVENTORY_ORDER = "utf8-bytewise-relative-path-v1"
GUEST_AGENT_TRUST_POLICY = "result-and-evidence-file-readback-v1"
STARTUP_FFI_PATH = (
    "target-startup/usr/lib/aarch64-linux-gnu/radishlex/manager/lib/"
    "libradishlex_ime_ffi.so"
)
MAX_READBACK_BYTES = 1024 * 1024


class LinuxL6GuestCaseContractError(ValueError):
    pass


@dataclass(frozen=True)
class TrustedGuestReadback:
    result: str
    evidence: str


@dataclass(frozen=True)
class GuestAgentObservation:
    observer_exit_code: int | None
    observer_stdout: bytes
    observer_stderr: bytes
    result_file: bytes | None
    evidence_file: bytes | None

    def require_file_readback(self) -> TrustedGuestReadback:
        return TrustedGuestReadback(
            result=_canonical_readback_text(self.result_file, "result"),
            evidence=_canonical_readback_text(self.evidence_file, "evidence"),
        )


@dataclass(frozen=True)
class StartupExpectation:
    decision: str
    reason: str
    receipt_terminal: str | None
    wire_output: str


STARTUP_EXPECTATIONS = {
    "fresh-absent": StartupExpectation(
        decision="FailedClosed",
        reason="ReceiptMissing",
        receipt_terminal=None,
        wire_output="0:1:4:15:0|error-absent",
    ),
    "removed-terminal": StartupExpectation(
        decision="FailedClosed",
        reason="RemovedProgram",
        receipt_terminal="completed",
        wire_output="0:1:4:24:6|error-absent",
    ),
}


def canonical_input_inventory(
    source_package: str, target_package: str
) -> tuple[str, ...]:
    source_package = _package_basename(source_package, "source package")
    target_package = _package_basename(target_package, "target package")
    if source_package == target_package:
        raise LinuxL6GuestCaseContractError(
            "source and target package names must differ"
        )

    paths = (
        BUILD_ENVIRONMENT_FILENAME,
        "case.sh",
        "radishlex-linux-l6-acceptance",
        "radishlex-linux-maintenance",
        "release-pair.evidence.json",
        "snapshot-identity.evidence.txt",
        f"source/artifacts/{source_package}",
        f"source/artifacts/{source_package}.evidence.json",
        "startup.py",
        STARTUP_FFI_PATH,
        f"target/artifacts/{target_package}",
        f"target/artifacts/{target_package}.evidence.json",
    )
    if len(paths) != len(set(paths)):
        raise LinuxL6GuestCaseContractError("input inventory contains duplicates")
    return tuple(sorted(paths, key=lambda item: item.encode("utf-8")))


def startup_expectation(case: str) -> StartupExpectation:
    try:
        return STARTUP_EXPECTATIONS[case]
    except KeyError as exc:
        raise LinuxL6GuestCaseContractError(
            f"unknown L6 guest startup case: {case}"
        ) from exc


def validate_guest_case_contract() -> None:
    inventory = canonical_input_inventory(
        "radishlex_26.7.1+38-1_arm64.deb",
        "radishlex_26.7.1+38-2_arm64.deb",
    )
    if inventory != tuple(sorted(inventory, key=lambda item: item.encode("utf-8"))):
        raise LinuxL6GuestCaseContractError(
            "input inventory must use canonical UTF-8 byte ordering"
        )
    if inventory.count(BUILD_ENVIRONMENT_FILENAME) != 1:
        raise LinuxL6GuestCaseContractError(
            "input inventory must contain one build environment file"
        )
    if "build-environment.evidence.json" in inventory:
        raise LinuxL6GuestCaseContractError(
            "legacy build environment alias is forbidden"
        )
    if inventory.index(STARTUP_FFI_PATH) > inventory.index(
        "target/artifacts/radishlex_26.7.1+38-2_arm64.deb"
    ):
        raise LinuxL6GuestCaseContractError(
            "target-startup must precede target artifacts in canonical inventory"
        )

    fresh = startup_expectation("fresh-absent")
    removed = startup_expectation("removed-terminal")
    if fresh == removed or fresh.receipt_terminal is not None:
        raise LinuxL6GuestCaseContractError(
            "fresh absent and removed terminal startup states must remain distinct"
        )


def _package_basename(value: str, label: str) -> str:
    if (
        not value
        or value in (".", "..")
        or "/" in value
        or "\\" in value
        or "\x00" in value
        or not value.isascii()
        or any(
            not (character.isalnum() or character in "._+~%:-")
            for character in value
        )
        or not value.endswith("_arm64.deb")
    ):
        raise LinuxL6GuestCaseContractError(
            f"{label} must be a safe Debian ARM64 package basename"
        )
    return value


def _canonical_readback_text(value: bytes | None, label: str) -> str:
    if value is None or not value or len(value) > MAX_READBACK_BYTES:
        raise LinuxL6GuestCaseContractError(
            f"guest {label} file readback must be present and bounded"
        )
    try:
        text = value.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise LinuxL6GuestCaseContractError(
            f"guest {label} file readback must be UTF-8"
        ) from exc
    if (
        text.startswith("\ufeff")
        or "\x00" in text
        or "\r" in text
        or not text.endswith("\n")
    ):
        raise LinuxL6GuestCaseContractError(
            f"guest {label} file readback must use canonical LF text"
        )
    return text


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate the repository-only Linux L6 guest-case contract."
    )
    parser.add_argument("command", choices=("validate",))
    return parser.parse_args()


def main() -> int:
    parse_args()
    validate_guest_case_contract()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

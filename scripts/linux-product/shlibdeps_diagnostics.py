#!/usr/bin/env python3
from __future__ import annotations

import argparse
import collections
import os
import subprocess
from pathlib import Path

import product_metadata
import rootfs as rootfs_contract


MAX_DIAGNOSTICS_SIZE = 64 * 1024
DEPENDENCY_ANALYSIS_PROFILE = "dpkg-shlibdeps-debian13-arm64-v1"
PRIVATE_LIBRARY_NAMES = (
    "libflutter_linux_gtk.so",
    "libradishlex_ime_ffi.so",
)
EXPECTED_DIAGNOSTICS = collections.Counter(
    {
        (
            "dpkg-shlibdeps: warning: can't extract name and version from "
            "library name 'libflutter_linux_gtk.so'"
        ): 2,
        (
            "dpkg-shlibdeps: warning: can't extract name and version from "
            "library name 'libradishlex_ime_ffi.so'"
        ): 2,
        "dpkg-shlibdeps: warning: diversions involved - output may be incorrect": 2,
        " diversion by libc6 from: /lib/ld-linux-aarch64.so.1": 1,
        (
            " diversion by libc6 to: "
            "/lib/ld-linux-aarch64.so.1.usr-is-merged"
        ): 1,
    }
)


class ShlibdepsDiagnosticsError(RuntimeError):
    pass


def validate_diagnostics(value: bytes) -> None:
    if len(value) > MAX_DIAGNOSTICS_SIZE:
        raise ShlibdepsDiagnosticsError(
            "dpkg-shlibdeps diagnostics exceed the size limit"
        )
    try:
        text = value.decode("utf-8")
    except UnicodeDecodeError as exc:
        raise ShlibdepsDiagnosticsError(
            "dpkg-shlibdeps diagnostics are not UTF-8"
        ) from exc
    if not text.endswith("\n") or "\r" in text:
        raise ShlibdepsDiagnosticsError(
            "dpkg-shlibdeps diagnostics are not canonical text"
        )
    actual = collections.Counter(text[:-1].split("\n"))
    if actual != EXPECTED_DIAGNOSTICS:
        unexpected = actual - EXPECTED_DIAGNOSTICS
        missing = EXPECTED_DIAGNOSTICS - actual
        details = []
        if unexpected:
            details.append(f"unexpected={dict(unexpected)}")
        if missing:
            details.append(f"missing={dict(missing)}")
        raise ShlibdepsDiagnosticsError(
            "dpkg-shlibdeps diagnostics differ from the fixed profile: "
            + "; ".join(details)
        )


def require_private_libraries(
    product_rootfs: Path,
    metadata: product_metadata.LinuxProductMetadata,
) -> None:
    manager_library_root = (
        product_rootfs
        / "usr/lib"
        / metadata.multiarch_tuple
        / "radishlex/manager/lib"
    )
    addon_root = product_rootfs / "usr/lib" / metadata.multiarch_tuple / "fcitx5"
    paths = (
        manager_library_root / PRIVATE_LIBRARY_NAMES[0],
        manager_library_root / PRIVATE_LIBRARY_NAMES[1],
        addon_root / PRIVATE_LIBRARY_NAMES[1],
    )
    contents: dict[Path, bytes] = {}
    for path in paths:
        try:
            canonical = rootfs_contract.require_canonical_input(
                path, f"private library {path.name}", directory=False
            )
            value = canonical.read_bytes()
        except (OSError, rootfs_contract.LinuxRootfsError) as exc:
            raise ShlibdepsDiagnosticsError(str(exc)) from exc
        if not value.startswith(b"\x7fELF"):
            raise ShlibdepsDiagnosticsError(
                f"private library is not an ELF payload: {path}"
            )
        contents[path] = value
    if contents[paths[1]] != contents[paths[2]]:
        raise ShlibdepsDiagnosticsError(
            "Manager and Fcitx private FFI libraries differ"
        )


def checked_output(arguments: list[str]) -> str:
    environment = {
        **os.environ,
        "LC_ALL": "C",
        "DPKG_COLORS": "never",
        "DPKG_NLS": "0",
    }
    try:
        result = subprocess.run(
            arguments,
            check=True,
            capture_output=True,
            text=True,
            encoding="utf-8",
            env=environment,
            timeout=10,
        )
    except (OSError, subprocess.SubprocessError) as exc:
        raise ShlibdepsDiagnosticsError(
            f"cannot verify Debian loader ownership: {exc}"
        ) from exc
    if result.stderr:
        raise ShlibdepsDiagnosticsError(
            f"Debian loader ownership command wrote stderr: {arguments[0]}"
        )
    return result.stdout


def require_libc6_usrmerge(metadata: product_metadata.LinuxProductMetadata) -> None:
    if metadata.debian_release != "13" or metadata.debian_architecture != "arm64":
        raise ShlibdepsDiagnosticsError(
            "libc6 usrmerge profile is restricted to Debian 13 ARM64"
        )
    loader = Path("/lib/ld-linux-aarch64.so.1")
    resolved_loader = Path("/usr/lib/aarch64-linux-gnu/ld-linux-aarch64.so.1")
    try:
        actual_resolved = loader.resolve(strict=True)
    except OSError as exc:
        raise ShlibdepsDiagnosticsError(
            f"cannot resolve the Debian ARM64 loader: {exc}"
        ) from exc
    if actual_resolved != resolved_loader:
        raise ShlibdepsDiagnosticsError(
            "Debian ARM64 loader differs from the fixed usrmerge profile"
        )
    checks = (
        (
            ["dpkg-query", "-S", str(resolved_loader)],
            f"libc6:arm64: {resolved_loader}\n",
        ),
        (["dpkg-divert", "--listpackage", str(loader)], "libc6\n"),
        (
            ["dpkg-divert", "--truename", str(loader)],
            f"{loader}.usr-is-merged\n",
        ),
    )
    for arguments, expected in checks:
        if checked_output(arguments) != expected:
            raise ShlibdepsDiagnosticsError(
                "Debian ARM64 loader ownership differs from the fixed "
                f"usrmerge profile: {arguments[0]}"
            )


def verify(product_rootfs: Path, diagnostics_path: Path) -> None:
    metadata = product_metadata.LinuxProductMetadata.load()
    try:
        product_rootfs = rootfs_contract.require_canonical_input(
            product_rootfs, "Linux product rootfs", directory=True
        )
        diagnostics_path = rootfs_contract.require_canonical_input(
            diagnostics_path,
            "dpkg-shlibdeps diagnostics",
            directory=False,
        )
        diagnostics = diagnostics_path.read_bytes()
    except (OSError, rootfs_contract.LinuxRootfsError) as exc:
        raise ShlibdepsDiagnosticsError(str(exc)) from exc
    validate_diagnostics(diagnostics)
    require_private_libraries(product_rootfs, metadata)
    require_libc6_usrmerge(metadata)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Verify the fixed Debian 13 ARM64 dpkg-shlibdeps diagnostic profile."
        )
    )
    parser.add_argument("--rootfs", type=Path, required=True)
    parser.add_argument("--diagnostics", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        verify(args.rootfs, args.diagnostics)
    except (
        ShlibdepsDiagnosticsError,
        product_metadata.LinuxProductMetadataError,
    ) as exc:
        raise SystemExit(str(exc)) from exc
    print(
        "dpkg-shlibdeps diagnostics match the fixed private-library and "
        "libc6 usrmerge profile."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

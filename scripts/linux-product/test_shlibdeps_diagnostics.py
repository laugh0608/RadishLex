#!/usr/bin/env python3
from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import product_metadata
import shlibdeps_diagnostics


GOOD_DIAGNOSTICS = b"""dpkg-shlibdeps: warning: can't extract name and version from library name 'libradishlex_ime_ffi.so'
dpkg-shlibdeps: warning: can't extract name and version from library name 'libradishlex_ime_ffi.so'
dpkg-shlibdeps: warning: can't extract name and version from library name 'libflutter_linux_gtk.so'
dpkg-shlibdeps: warning: can't extract name and version from library name 'libflutter_linux_gtk.so'
dpkg-shlibdeps: warning: diversions involved - output may be incorrect
 diversion by libc6 from: /lib/ld-linux-aarch64.so.1
dpkg-shlibdeps: warning: diversions involved - output may be incorrect
 diversion by libc6 to: /lib/ld-linux-aarch64.so.1.usr-is-merged
"""


class ShlibdepsDiagnosticsTest(unittest.TestCase):
    def test_exact_diagnostics_are_accepted_independent_of_order(self) -> None:
        lines = GOOD_DIAGNOSTICS.decode("utf-8").splitlines()
        reordered = ("\n".join(reversed(lines)) + "\n").encode("utf-8")

        shlibdeps_diagnostics.validate_diagnostics(GOOD_DIAGNOSTICS)
        shlibdeps_diagnostics.validate_diagnostics(reordered)

    def test_unknown_diagnostic_is_rejected(self) -> None:
        value = GOOD_DIAGNOSTICS + b"dpkg-shlibdeps: warning: unknown\n"
        with self.assertRaisesRegex(
            shlibdeps_diagnostics.ShlibdepsDiagnosticsError,
            "unexpected",
        ):
            shlibdeps_diagnostics.validate_diagnostics(value)

    def test_missing_diagnostic_is_rejected(self) -> None:
        value = GOOD_DIAGNOSTICS.replace(
            b" diversion by libc6 from: /lib/ld-linux-aarch64.so.1\n",
            b"",
        )
        with self.assertRaisesRegex(
            shlibdeps_diagnostics.ShlibdepsDiagnosticsError,
            "missing",
        ):
            shlibdeps_diagnostics.validate_diagnostics(value)

    def test_noncanonical_diagnostics_are_rejected(self) -> None:
        with self.assertRaisesRegex(
            shlibdeps_diagnostics.ShlibdepsDiagnosticsError,
            "canonical text",
        ):
            shlibdeps_diagnostics.validate_diagnostics(
                GOOD_DIAGNOSTICS.replace(b"\n", b"\r\n")
            )

    def test_private_libraries_are_fixed_elf_payloads(self) -> None:
        metadata = product_metadata.LinuxProductMetadata.load()
        with tempfile.TemporaryDirectory() as temporary:
            rootfs = Path(temporary).resolve()
            manager = (
                rootfs
                / "usr/lib"
                / metadata.multiarch_tuple
                / "radishlex/manager/lib"
            )
            addon = rootfs / "usr/lib" / metadata.multiarch_tuple / "fcitx5"
            manager.mkdir(parents=True)
            addon.mkdir(parents=True)
            (manager / "libflutter_linux_gtk.so").write_bytes(b"\x7fELFflutter")
            (manager / "libradishlex_ime_ffi.so").write_bytes(b"\x7fELFffi")
            (addon / "libradishlex_ime_ffi.so").write_bytes(b"\x7fELFffi")

            shlibdeps_diagnostics.require_private_libraries(rootfs, metadata)

            (addon / "libradishlex_ime_ffi.so").write_bytes(b"\x7fELFdrift")
            with self.assertRaisesRegex(
                shlibdeps_diagnostics.ShlibdepsDiagnosticsError,
                "libraries differ",
            ):
                shlibdeps_diagnostics.require_private_libraries(rootfs, metadata)


if __name__ == "__main__":
    unittest.main()

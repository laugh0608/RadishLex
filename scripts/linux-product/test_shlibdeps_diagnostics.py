#!/usr/bin/env python3
from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

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

        shlibdeps_diagnostics.validate_diagnostics(GOOD_DIAGNOSTICS, ffi_has_soname=True)
        shlibdeps_diagnostics.validate_diagnostics(reordered, ffi_has_soname=True)

    def test_warning_count_follows_ffi_soname(self) -> None:
        no_ffi_warnings = GOOD_DIAGNOSTICS.replace(
            shlibdeps_diagnostics.FFI_WARNING.encode() + b"\n", b""
        )
        shlibdeps_diagnostics.validate_diagnostics(no_ffi_warnings, ffi_has_soname=False)
        for diagnostics, has_soname, error in (
            (GOOD_DIAGNOSTICS, False, "unexpected"),
            (no_ffi_warnings, True, "missing"),
        ):
            with self.assertRaisesRegex(shlibdeps_diagnostics.ShlibdepsDiagnosticsError, error):
                shlibdeps_diagnostics.validate_diagnostics(diagnostics, ffi_has_soname=has_soname)

    def test_ffi_soname_requires_a_readable_dynamic_section_and_fixed_name(self) -> None:
        ffi = Path("/example/libradishlex_ime_ffi.so")
        header = "Dynamic section at offset 0x100 contains 2 entries:\n"
        for tag, expected in (
            ("", False),
            (" 0x000e (SONAME) Library soname: [libradishlex_ime_ffi.so]\n", True),
        ):
            with patch.object(shlibdeps_diagnostics, "checked_output", return_value=header + tag):
                self.assertEqual(shlibdeps_diagnostics.ffi_has_fixed_soname(ffi), expected)
        for dynamic in (
            "There is no dynamic section in this file.\n",
            header + " 0x000e (SONAME) Library soname: [wrong.so]\n",
        ):
            with patch.object(shlibdeps_diagnostics, "checked_output", return_value=dynamic):
                with self.assertRaises(shlibdeps_diagnostics.ShlibdepsDiagnosticsError):
                    shlibdeps_diagnostics.ffi_has_fixed_soname(ffi)

    def test_unknown_diagnostic_is_rejected(self) -> None:
        value = GOOD_DIAGNOSTICS + b"dpkg-shlibdeps: warning: unknown\n"
        with self.assertRaisesRegex(
            shlibdeps_diagnostics.ShlibdepsDiagnosticsError,
            "unexpected",
        ):
            shlibdeps_diagnostics.validate_diagnostics(value, ffi_has_soname=True)

    def test_missing_diagnostic_is_rejected(self) -> None:
        value = GOOD_DIAGNOSTICS.replace(
            b" diversion by libc6 from: /lib/ld-linux-aarch64.so.1\n",
            b"",
        )
        with self.assertRaisesRegex(
            shlibdeps_diagnostics.ShlibdepsDiagnosticsError,
            "missing",
        ):
            shlibdeps_diagnostics.validate_diagnostics(value, ffi_has_soname=True)

    def test_noncanonical_diagnostics_are_rejected(self) -> None:
        with self.assertRaisesRegex(
            shlibdeps_diagnostics.ShlibdepsDiagnosticsError,
            "canonical text",
        ):
            shlibdeps_diagnostics.validate_diagnostics(
                GOOD_DIAGNOSTICS.replace(b"\n", b"\r\n"), ffi_has_soname=True
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

#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import l6_v4_canonical_input_bundle as input_bundle
import l6_v4_canonical_input_install as guest_installer


class LinuxL6V4CanonicalInputBundleTests(unittest.TestCase):
    def test_archive_is_deterministic_root_owned_ustar(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            payloads = tuple(
                input_bundle.ArchivePayload(
                    path=path,
                    mode=(
                        0o755
                        if path
                        in {
                            "radishlex-linux-l6-acceptance",
                            "radishlex-linux-maintenance",
                        }
                        else 0o600
                    ),
                    size=len(data := f"payload:{path}\n".encode("ascii")),
                    sha256=hashlib.sha256(data).hexdigest(),
                    source=data,
                )
                for path in guest_installer.CANONICAL_INVENTORY
            )
            first = root / "first.ustar"
            second = root / "second.ustar"

            input_bundle.build_canonical_archive(first, payloads)
            input_bundle.build_canonical_archive(second, payloads)

            self.assertEqual(first.read_bytes(), second.read_bytes())
            observed = input_bundle.inspect_bundle(first)
            self.assertEqual(
                tuple(member.as_json() for member in observed),
                tuple(payload.as_json() for payload in payloads),
            )
            raw = first.read_bytes()
            self.assertEqual(raw[257:265], b"ustar\x0000")
            self.assertEqual(raw[-1024:], b"\x00" * 1024)

    def test_archive_rejects_noncanonical_inventory(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "invalid.ustar"
            payload = input_bundle.ArchivePayload(
                path="case.sh",
                mode=0o600,
                size=1,
                sha256=hashlib.sha256(b"x").hexdigest(),
                source=b"x",
            )
            with self.assertRaisesRegex(
                input_bundle.CanonicalInputBundleError,
                "archive-inventory-contract-mismatch",
            ):
                input_bundle.build_canonical_archive(path, (payload,))
            self.assertFalse(path.exists())

    def test_case_derivation_requires_every_exact_replacement_count(self) -> None:
        template = (
            b"install_prepared\n"
            b"install-prepared\n"
            b'.checkpoint == "prepared"\n'
            b'.state == "prepared"\n'
            b"receipt=install|not_applicable|prepared|chain-1\n"
            b'case "$1" in\n'
            b"  preflight) preflight ;;\n"
            b"  crash) crash ;;\n"
            b"  inspect-crash) inspect_crash ;;\n"
            b"  *) fail command ;;\n"
            b"esac\n"
        )
        replacements = tuple(
            (old, new, 1)
            for old, new, _ in input_bundle.CASE_REPLACEMENTS
        )
        rendered = template
        for old, new, _ in replacements:
            rendered = rendered.replace(old, new)
        with (
            mock.patch.object(
                input_bundle,
                "CASE_TEMPLATE_SHA256",
                hashlib.sha256(template).hexdigest(),
            ),
            mock.patch.object(
                input_bundle,
                "RENDERED_CASE_SHA256",
                hashlib.sha256(rendered).hexdigest(),
            ),
            mock.patch.object(input_bundle, "CASE_REPLACEMENTS", replacements),
        ):
            self.assertEqual(input_bundle.render_case(template), rendered)
            with self.assertRaisesRegex(
                input_bundle.CanonicalInputBundleError,
                "case-template-sha256-mismatch",
            ):
                input_bundle.render_case(template + b"drift\n")

    def test_case_derivation_keeps_resume_out_of_initial_dispatch(self) -> None:
        template = (
            b"install_prepared\n"
            b"install-prepared\n"
            b'.checkpoint == "prepared"\n'
            b'.state == "prepared"\n'
            b"receipt=install|not_applicable|prepared|chain-1\n"
            b'case "$1" in\n'
            b"  preflight) preflight ;;\n"
            b"  crash) crash ;;\n"
            b"  inspect-crash) inspect_crash ;;\n"
            b"  resume) resume ;;\n"
            b"  *) fail command ;;\n"
            b"esac\n"
        )
        replacements = tuple(
            (old, new, 1)
            for old, new, _ in input_bundle.CASE_REPLACEMENTS
        )
        rendered = template
        for old, new, _ in replacements:
            rendered = rendered.replace(old, new)
        with (
            mock.patch.object(
                input_bundle,
                "CASE_TEMPLATE_SHA256",
                hashlib.sha256(template).hexdigest(),
            ),
            mock.patch.object(
                input_bundle,
                "RENDERED_CASE_SHA256",
                hashlib.sha256(rendered).hexdigest(),
            ),
            mock.patch.object(input_bundle, "CASE_REPLACEMENTS", replacements),
        ):
            with self.assertRaisesRegex(
                input_bundle.CanonicalInputBundleError,
                "rendered-case-dispatch-mismatch",
            ):
                input_bundle.render_case(template)

    def test_snapshot_identity_binds_v4_clone_and_generation_head(self) -> None:
        first = input_bundle.render_snapshot_identity(
            repository_head="a" * 40,
            generator_sha256="b" * 64,
        )
        second = input_bundle.render_snapshot_identity(
            repository_head="a" * 40,
            generator_sha256="b" * 64,
        )
        self.assertEqual(first, second)
        text = first.decode("ascii")
        self.assertIn(f"clone_uuid={input_bundle.TARGET_UUID}\n", text)
        self.assertIn("bundle_generation_repository_head=" + "a" * 40, text)
        self.assertIn(
            "clone_prepared_manifest_sha256="
            + input_bundle.CLONE_PREPARED_MANIFEST_SHA256,
            text,
        )
        self.assertIn("operation_id=not-generated\n", text)
        self.assertIn("acceptance_maintenance_dpkg=not-run\n", text)

    def test_request_requires_all_four_authorizations(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            request = make_request(
                root,
                authorized_no_guest_vm_or_system_mutation=False,
            )
            with self.assertRaisesRegex(
                input_bundle.CanonicalInputBundleError,
                "authorized-no-guest-vm-or-system-mutation-required",
            ):
                request.validate()

    def test_request_rejects_output_inside_repository(self) -> None:
        request = input_bundle.CanonicalInputBundleRequest(
            repository_root=input_bundle.REPO_ROOT,
            expected_repository_head="a" * 40,
            output_root=input_bundle.REPO_ROOT / "forbidden",
            authorized_host_input_bundle_generation=True,
            authorized_exact_frozen_sources=True,
            authorized_create_new_freeze=True,
            authorized_no_guest_vm_or_system_mutation=True,
        )
        with self.assertRaisesRegex(
            input_bundle.CanonicalInputBundleError,
            "output-root-must-not-overlap-repository",
        ):
            request.validate()

    def test_exact_source_open_rejects_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = root / "target"
            target.write_bytes(b"frozen\n")
            target.chmod(0o600)
            link = root / "link"
            link.symlink_to(target)
            with self.assertRaisesRegex(
                input_bundle.CanonicalInputBundleError,
                "cannot-open-source-file",
            ):
                input_bundle._open_exact_source(
                    link,
                    0o600,
                    len(b"frozen\n"),
                    hashlib.sha256(b"frozen\n").hexdigest(),
                )

    def test_evidence_parser_rejects_duplicate_fields(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "evidence.txt"
            path.write_text("field=one\nfield=two\n", encoding="ascii")
            with self.assertRaisesRegex(
                input_bundle.CanonicalInputBundleError,
                "clone-prepared-evidence-field-invalid",
            ):
                input_bundle._parse_evidence(path)


def make_request(
    temporary: Path,
    **overrides: object,
) -> input_bundle.CanonicalInputBundleRequest:
    values: dict[str, object] = {
        "repository_root": input_bundle.REPO_ROOT,
        "expected_repository_head": "a" * 40,
        "output_root": temporary / "output",
        "authorized_host_input_bundle_generation": True,
        "authorized_exact_frozen_sources": True,
        "authorized_create_new_freeze": True,
        "authorized_no_guest_vm_or_system_mutation": True,
    }
    values.update(overrides)
    return input_bundle.CanonicalInputBundleRequest(**values)


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import product_data


class ProductDataTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="radishlex-rime-product-")
        self.root = Path(self.temporary.name)
        self.package = self.root / "packaging/rime"
        data = self.package / "data"
        licenses = self.package / "licenses/rime-pinyin-simp"
        data.mkdir(parents=True)
        licenses.mkdir(parents=True)
        (data / "default.yaml").write_text(
            "schema_list:\n  - schema: radishlex_pinyin\nmenu:\n  page_size: 5\n",
            encoding="utf-8",
        )
        (data / "radishlex_pinyin.schema.yaml").write_text(
            "schema:\n  schema_id: radishlex_pinyin\ntranslator:\n  dictionary: pinyin_simp\n",
            encoding="utf-8",
        )
        (data / "pinyin_simp.dict.yaml").write_text(
            "---\nname: pinyin_simp\n...\n示例\tshi li\t1\n", encoding="utf-8"
        )
        (licenses / "LICENSE").write_text("Apache-2.0 fixture\n", encoding="utf-8")
        (licenses / "AUTHORS").write_text("Synthetic author\n", encoding="utf-8")
        self.lock_path = self.package / "product-rime-data.json"
        self.lock = {
            "format_version": 1,
            "schema_id": "radishlex_pinyin",
            "assets": [
                self.asset(
                    "radishlex-rime-default",
                    "configuration",
                    "data/default.yaml",
                    "default.yaml",
                ),
                self.asset(
                    "radishlex-pinyin-schema",
                    "schema",
                    "data/radishlex_pinyin.schema.yaml",
                    "radishlex_pinyin.schema.yaml",
                ),
                self.asset(
                    "rime-pinyin-simp-dictionary",
                    "dictionary",
                    "data/pinyin_simp.dict.yaml",
                    "pinyin_simp.dict.yaml",
                ),
            ],
            "license_files": [
                self.license_file("LICENSE"),
                self.license_file("AUTHORS"),
            ],
        }
        self.write_lock()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def asset(
        self, asset_id: str, kind: str, source_path: str, runtime_path: str
    ) -> dict[str, object]:
        return {
            "asset_id": asset_id,
            "kind": kind,
            "source_path": source_path,
            "runtime_path": runtime_path,
            "sha256": product_data.sha256(self.package / source_path),
            "license_id": "Apache-2.0",
            "provenance": {
                "type": "radishlex-authored",
                "description": "Synthetic product-data fixture.",
            },
        }

    def license_file(self, name: str) -> dict[str, str]:
        source = f"licenses/rime-pinyin-simp/{name}"
        return {
            "component": "rime-pinyin-simp",
            "source_path": source,
            "runtime_path": f"Licenses/rime-pinyin-simp/{name}",
            "sha256": product_data.sha256(self.package / source),
        }

    def write_lock(self) -> None:
        self.lock_path.write_text(
            json.dumps(self.lock, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )

    def validate(self) -> dict[str, object]:
        return product_data.validate_source(self.lock_path, self.package)

    def test_repository_product_data_contract(self) -> None:
        product_data.validate_source()

    def test_assemble_is_deterministic_and_verifiable(self) -> None:
        first = self.root / "first"
        second = self.root / "second"
        product_data.assemble(first, self.lock_path, self.package)
        product_data.assemble(second, self.lock_path, self.package)
        product_data.verify_assembly(first, self.lock_path, self.package)
        product_data.verify_assembly(second, self.lock_path, self.package)

        first_files = {
            path.relative_to(first).as_posix(): path.read_bytes()
            for path in first.rglob("*")
            if path.is_file()
        }
        second_files = {
            path.relative_to(second).as_posix(): path.read_bytes()
            for path in second.rglob("*")
            if path.is_file()
        }
        self.assertEqual(first_files, second_files)
        self.assertIn("Licenses/rime-pinyin-simp/LICENSE", first_files)
        self.assertIn("SourceManifest.json", first_files)

    def test_source_hash_mismatch_is_rejected(self) -> None:
        (self.package / "data/pinyin_simp.dict.yaml").write_text(
            "tampered\n", encoding="utf-8"
        )
        with self.assertRaisesRegex(product_data.ProductDataError, "hash mismatch"):
            self.validate()

    def test_unlocked_source_file_is_rejected(self) -> None:
        (self.package / "data/unlocked.yaml").write_text("unlocked\n", encoding="utf-8")
        with self.assertRaisesRegex(product_data.ProductDataError, "inventory differs"):
            self.validate()

    def test_unknown_lock_field_is_rejected(self) -> None:
        self.lock["unexpected"] = True
        self.write_lock()
        with self.assertRaisesRegex(product_data.ProductDataError, "fields do not match"):
            self.validate()

    def test_assembly_mutation_and_extra_file_are_rejected(self) -> None:
        output = self.root / "assembled"
        product_data.assemble(output, self.lock_path, self.package)
        (output / "pinyin_simp.dict.yaml").write_text("tampered\n", encoding="utf-8")
        with self.assertRaisesRegex(product_data.ProductDataError, "hash mismatch"):
            product_data.verify_assembly(output, self.lock_path, self.package)

        product_data.assemble(self.root / "clean", self.lock_path, self.package)
        clean = self.root / "clean"
        (clean / "unexpected.txt").write_text("unexpected\n", encoding="utf-8")
        with self.assertRaisesRegex(product_data.ProductDataError, "inventory differs"):
            product_data.verify_assembly(clean, self.lock_path, self.package)

    def test_source_symlink_is_rejected(self) -> None:
        dictionary = self.package / "data/pinyin_simp.dict.yaml"
        target = self.root / "outside.dict.yaml"
        target.write_bytes(dictionary.read_bytes())
        dictionary.unlink()
        dictionary.symlink_to(target)
        with self.assertRaisesRegex(product_data.ProductDataError, "regular repository file"):
            self.validate()


if __name__ == "__main__":
    unittest.main()

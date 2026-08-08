#!/usr/bin/env python3
from __future__ import annotations

import unittest
from dataclasses import replace

from l6_release_pair_contract import (
    L6ReleasePairContractError,
    ReleasePairSources,
    validate_release_pair_contract,
)


class L6ReleasePairSourceContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.sources = ReleasePairSources.load()

    def test_repository_release_pair_contract_is_complete(self) -> None:
        validate_release_pair_contract(self.sources)

    def test_guest_or_package_database_mutation_is_rejected(self) -> None:
        for token in (
            "\napt-get install flutter\n",
            "\n/usr/bin/dpkg --install package.deb\n",
            "\ndocker run debian:13\n",
            "\n--force-confnew\n",
            "\ncat value > /var/lib/dpkg/status\n",
        ):
            with self.subTest(token=token):
                with self.assertRaises(L6ReleasePairContractError):
                    validate_release_pair_contract(
                        replace(self.sources, builder=self.sources.builder + token)
                    )

    def test_runtime_activation_and_path_overrides_are_rejected(self) -> None:
        for token in (
            "\n# RADISHLEX_L6_CHECKPOINT=prepared\n",
            "\n# --dpkg-program /tmp/wrapper\n",
            "\n# --state-root /tmp/state\n",
        ):
            with self.subTest(token=token):
                with self.assertRaises(L6ReleasePairContractError):
                    validate_release_pair_contract(
                        replace(self.sources, tool=self.sources.tool + token)
                    )

    def test_compile_identity_and_publish_order_drift_is_rejected(self) -> None:
        mutations = (
            self.sources.builder.replace("--no-default-features", ""),
            self.sources.builder.replace(
                'python3 "${pair_tool}" verify',
                'python3 "${pair_tool}" omitted',
            ),
            self.sources.builder.replace(
                'build_release source "${source_root}"',
                'build_release source "${target_root}"',
            ),
        )
        for mutation in mutations:
            with self.subTest():
                with self.assertRaises(L6ReleasePairContractError):
                    validate_release_pair_contract(
                        replace(self.sources, builder=mutation)
                    )

    def test_source_ffi_include_is_root_local_and_ambient_paths_are_scrubbed(self) -> None:
        mutations = (
            self.sources.builder.replace(
                'if [[ "${role}" == "source" ]]',
                'if [[ "${role}" == "target" ]]',
            ),
            self.sources.builder.replace(
                '"CPLUS_INCLUDE_PATH=${root}/crates/ime-ffi/include"',
                '"CPLUS_INCLUDE_PATH=/usr/include"',
            ),
            self.sources.builder.replace("-u CPATH", ""),
            self.sources.builder.replace("-u C_INCLUDE_PATH", ""),
            self.sources.builder.replace("-u CPLUS_INCLUDE_PATH", ""),
            self.sources.builder.replace("-u OBJC_INCLUDE_PATH", ""),
        )
        for mutation in mutations:
            with self.subTest():
                with self.assertRaises(L6ReleasePairContractError):
                    validate_release_pair_contract(
                        replace(self.sources, builder=mutation)
                    )


if __name__ == "__main__":
    unittest.main()

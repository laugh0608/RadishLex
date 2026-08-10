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

    def test_source_anchor_staging_identity_drift_is_rejected(self) -> None:
        for token in (
            "def read_exact_file",
            "def stage_exact_source_anchor",
            "os.O_EXCL",
            "os.fsync",
        ):
            with self.subTest(token=token):
                with self.assertRaises(L6ReleasePairContractError):
                    validate_release_pair_contract(
                        replace(
                            self.sources,
                            anchor=self.sources.anchor.replace(token, "omitted"),
                        )
                    )

    def test_compile_identity_and_publish_order_drift_is_rejected(self) -> None:
        mutations = (
            self.sources.builder.replace("--no-default-features", ""),
            self.sources.builder.replace(
                'verify_release_artifact target "${target_package}"',
                'verify_release_artifact target omitted',
            ),
            self.sources.builder.replace(
                'python3 "${pair_tool}" verify',
                'python3 "${pair_tool}" omitted',
            ),
            self.sources.builder.replace(
                'python3 "${pair_tool}" stage-source',
                'python3 "${pair_tool}" omitted-source-stage',
            ),
        )
        for mutation in mutations:
            with self.subTest():
                with self.assertRaises(L6ReleasePairContractError):
                    validate_release_pair_contract(
                        replace(self.sources, builder=mutation)
                    )
        with self.assertRaises(L6ReleasePairContractError):
            validate_release_pair_contract(
                replace(
                    self.sources,
                    tool=self.sources.tool.replace(
                        "require_absent_build_outputs=False", ""
                    ),
                )
            )

    def test_source_anchor_inputs_and_target_build_environment_are_fixed(self) -> None:
        mutations = (
            self.sources.builder + "\nsource_root=/tmp/rebuilt-source\n",
            self.sources.builder.replace(
                '--source-package "${source_package_input}"',
                '--source-package "${target_package}"',
            ),
            self.sources.builder.replace(
                '--source-artifact-evidence "${source_artifact_evidence_input}"',
                '--source-artifact-evidence "${target_package}.evidence.json"',
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

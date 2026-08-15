#!/usr/bin/env python3
from __future__ import annotations

import unittest
from dataclasses import replace

from l6_maintenance_refresh_contract import (
    L6MaintenanceRefreshContractError,
    MaintenanceRefreshSources,
    validate_maintenance_refresh_contract,
)


class L6MaintenanceRefreshSourceContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.sources = MaintenanceRefreshSources.load()

    def test_repository_maintenance_refresh_contract_is_complete(self) -> None:
        validate_maintenance_refresh_contract(self.sources)

    def test_package_rebuild_acceptance_and_guest_mutation_are_rejected(self) -> None:
        for token in (
            "\n./scripts/build-linux-deb-artifact.sh\n",
            "\ncargo build -p radishlex-linux-l6-acceptance\n",
            "\n/usr/bin/dpkg --install package.deb\n",
            "\nutmctl start synthetic\n",
            "\n--authorized-system-mutation\n",
        ):
            with self.subTest(token=token):
                with self.assertRaises(L6MaintenanceRefreshContractError):
                    validate_maintenance_refresh_contract(
                        replace(self.sources, builder=self.sources.builder + token)
                    )

    def test_runtime_activation_and_path_overrides_are_rejected(self) -> None:
        for token in (
            "\n# RADISHLEX_L6_CHECKPOINT=prepared\n",
            "\n# --dpkg-program /tmp/wrapper\n",
            "\n# --state-root /tmp/state\n",
        ):
            with self.subTest(token=token):
                with self.assertRaises(L6MaintenanceRefreshContractError):
                    validate_maintenance_refresh_contract(
                        replace(self.sources, tool=self.sources.tool + token)
                    )

    def test_frozen_anchor_or_repair_ancestry_drift_is_rejected(self) -> None:
        for token in (
            "cda70afa89b3f0ee05235b95dcc346eaeea9805c4b87af9d451ce1900f00659b",
            "b211d9406825515b2ba1c473b5f98069de00fa709505e2ba3ed3cb9b8af7d09c",
            "2a1132c6fb27d4ca2e5e4bbd76864e3e753749c2bb43cae82287635b1c2d0e1b",
            "b060c2403424560e9ac0c11f890838894d19d153a89541575ade9afd87fe7d81",
            "b0197f5a0f0a88e53cf4b6bf862252a4374197de",
        ):
            with self.subTest(token=token):
                with self.assertRaises(L6MaintenanceRefreshContractError):
                    validate_maintenance_refresh_contract(
                        replace(
                            self.sources,
                            contract=self.sources.contract.replace(token, "0" * 64),
                        )
                    )

    def test_builder_order_and_production_identity_are_fixed(self) -> None:
        mutations = (
            self.sources.builder.replace("--no-default-features", ""),
            self.sources.builder.replace(
                '"${artifact_verifier}"', '"${omitted_verifier}"'
            ),
            self.sources.builder.replace(
                'python3 "${refresh_tool}" stage-base',
                'python3 "${refresh_tool}" omitted-stage',
            ),
            self.sources.builder.replace(
                'python3 "${refresh_tool}" publish',
                'python3 "${refresh_tool}" omitted-publish',
            ),
        )
        for mutation in mutations:
            with self.subTest():
                with self.assertRaises(L6MaintenanceRefreshContractError):
                    validate_maintenance_refresh_contract(
                        replace(self.sources, builder=mutation)
                    )

    def test_atomic_publish_and_record_verification_are_fixed(self) -> None:
        for token in (
            "rename_directory_no_replace(staging, output)",
            "fsync_directory(output.parent)",
            "l6_release_pair.validate_record(record, pair_contract)",
            "require_git_ancestor(root, REPAIR_FIX_COMMIT, refresh_commit)",
        ):
            with self.subTest(token=token):
                with self.assertRaises(L6MaintenanceRefreshContractError):
                    validate_maintenance_refresh_contract(
                        replace(self.sources, tool=self.sources.tool.replace(token, "omitted"))
                    )
        for token in (
            "os.O_EXCL",
            "os.fsync",
            "def write_exclusive_file",
            "def rename_directory_no_replace",
            "renameat2",
            "RENAME_NOREPLACE",
            "renameatx_np",
            "RENAME_EXCL",
        ):
            with self.subTest(token=token):
                with self.assertRaises(L6MaintenanceRefreshContractError):
                    validate_maintenance_refresh_contract(
                        replace(self.sources, io=self.sources.io.replace(token, "omitted"))
                    )


if __name__ == "__main__":
    unittest.main()

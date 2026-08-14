#!/usr/bin/env python3
from __future__ import annotations

import unittest
from dataclasses import replace

from l6_controller_contract import (
    ControllerSources,
    LinuxL6ControllerContractError,
    validate_controller_contract,
)


class LinuxL6ControllerContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.sources = ControllerSources.load()

    def test_repository_controller_contract_is_complete(self) -> None:
        validate_controller_contract(self.sources)

    def test_production_runtime_activation_is_rejected(self) -> None:
        changed = replace(
            self.sources,
            product_main=self.sources.product_main + '\n// --authorized-l6-crash\n',
        )
        with self.assertRaises(LinuxL6ControllerContractError):
            validate_controller_contract(changed)

    def test_process_group_or_pipe_contract_drift_is_rejected(self) -> None:
        for changed_process in (
            self.sources.acceptance_process.replace(".process_group(0)", ""),
            self.sources.acceptance_process.replace('fs::read_dir("/proc")', ""),
            self.sources.acceptance_process + '\n// receipt.json polling\n',
        ):
            with self.subTest():
                with self.assertRaises(LinuxL6ControllerContractError):
                    validate_controller_contract(
                        replace(self.sources, acceptance_process=changed_process)
                    )

    def test_environment_path_and_raw_evidence_fields_are_rejected(self) -> None:
        mutations = (
            replace(
                self.sources,
                acceptance_command=(
                    self.sources.acceptance_command + '\n// std::env::var("RADISHLEX_L6")\n'
                ),
            ),
            replace(
                self.sources,
                acceptance_command=self.sources.acceptance_command + '\n// --state-root\n',
            ),
            replace(
                self.sources,
                acceptance_evidence=self.sources.acceptance_evidence + '\n// pid: u32\n',
            ),
        )
        for changed in mutations:
            with self.subTest():
                with self.assertRaises(LinuxL6ControllerContractError):
                    validate_controller_contract(changed)


if __name__ == "__main__":
    unittest.main()

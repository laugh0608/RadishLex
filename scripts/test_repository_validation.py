from __future__ import annotations

import contextlib
import io
import os
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

from validation_process import run_command
from validation_suites import PHASES, ROOT, SUITES, repository_rust_commands, run_suite, verify_reliability_inventory


class ValidationPlanTests(unittest.TestCase):
    def test_full_repository_runs_default_targets_and_docs_once(self):
        commands = repository_rust_commands("darwin", "tests")
        self.assertEqual(commands[:2], [
            ["cargo", "test", "--locked", "--workspace", "--all-targets"],
            ["cargo", "test", "--locked", "--workspace", "--doc"],
        ])
        for command in commands[2:]:
            self.assertIn("--features", command)
        self.assertEqual(len(commands), 4)
        self.assertIn("cancellation_archive", commands[3])
        self.assertNotIn("--ignored", sum(commands, []))

    def test_standalone_gates_retain_every_phase(self):
        with patch("validation_suites.sys.platform", "darwin"), patch("validation_suites.run_reliability_command") as reliability:
            for name, suite in SUITES.items():
                with self.subTest(name=name), patch("validation_suites.run_command") as run:
                    with contextlib.redirect_stdout(io.StringIO()):
                        run_suite(name)
                    self.assertEqual([call.args[0] for call in run.call_args_list],
                                     [command for phase in PHASES if phase != "reliability" for command in suite.get(phase, [])])
                    self.assertEqual([call.args[0] for call in reliability.call_args_list], suite.get("reliability", []))
                    reliability.reset_mock()
                    self.assertTrue(suite["default-tests"])

    def test_static_phases_do_not_run_long_tests_or_real_qualification(self):
        commands = repository_rust_commands("darwin", "static")
        self.assertEqual(commands[0], ["cargo", "fmt", "--all", "--", "--check"])
        feature_builds = [c for c in commands if c[1] == "test"]
        self.assertEqual(len(feature_builds), 1)
        self.assertIn("--no-run", feature_builds[0])
        self.assertIn("qualification-harness", feature_builds[0])
        self.assertNotIn("--ignored", sum(commands, []))

    def test_non_macos_plan_retains_workspace_without_native_suites(self):
        commands = repository_rust_commands("linux", "tests")
        self.assertEqual(len(commands), 3)
        self.assertIn("--workspace", commands[0])
        static = repository_rust_commands("linux", "static")
        self.assertFalse(any("radishlex-macos-" in " ".join(c) for c in static))

    def test_reliability_is_explicit_and_never_selects_child_entries_globally(self):
        routine = repository_rust_commands("darwin", "tests")
        self.assertFalse(any("reliability-matrix" in command for command in routine))
        specialist = repository_rust_commands("darwin", "reliability")
        self.assertEqual(len(specialist), 2)
        for command in specialist:
            self.assertIn("reliability-matrix", command)
            self.assertEqual(command[-1], "reliability_matrix_")
            self.assertNotIn("--ignored", command)
            self.assertNotIn("--workspace", command)

    def test_workspace_lint_has_one_owner_before_platform_feature_variants(self):
        commands = repository_rust_commands("darwin", "static")
        workspace = [command for command in commands if "--workspace" in command]
        self.assertEqual(len(workspace), 1)
        self.assertIn("clippy", workspace[0])

    def test_empty_or_missing_reliability_inventory_never_passes(self):
        import json
        package = "radishlex-macos-installer-executor"
        expected = json.loads((ROOT / "scripts/reliability-cases.json").read_text())[package]
        verify_reliability_inventory(package, "\n".join(name + ": test" for name in expected))
        for listing in ("0 tests", expected[0] + ": test", "unexpected: test"):
            with self.assertRaises(SystemExit):
                verify_reliability_inventory(package, listing)

    def test_partial_phase_never_claims_complete_gate(self):
        output = io.StringIO()
        with patch("validation_suites.run_command"), contextlib.redirect_stdout(output):
            run_suite("product-install-core", "static")
        self.assertIn("partial gate", output.getvalue())
        self.assertNotIn("gate passed", output.getvalue())


@unittest.skipUnless(os.name == "posix", "process-group contract is POSIX-only")
class ValidationProcessTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.child_script = self.root / "child.py"
        self.child_script.write_text(
            "import signal, sys, time\nfrom pathlib import Path\n"
            "root = Path(sys.argv[1])\n"
            "def stop(*args):\n    (root / 'stopped').write_text('yes')\n    sys.exit(0)\n"
            "signal.signal(signal.SIGTERM, stop)\n"
            "(root / 'ready').write_text('yes')\n"
            "while True: time.sleep(0.1)\n"
        )
        self.parent_script = self.root / "parent.py"
        self.parent_script.write_text(
            "import subprocess, sys, time\nfrom pathlib import Path\n"
            "root = Path(sys.argv[1])\n"
            "child = subprocess.Popen([sys.executable, str(root / 'child.py'), str(root)])\n"
            "while not (root / 'ready').exists(): time.sleep(0.01)\n"
            "if sys.argv[2] == 'wait':\n    child.wait()\n"
            "else: sys.exit(int(sys.argv[2]))\n"
        )

    def tearDown(self):
        self.temporary.cleanup()

    def test_capture_keeps_child_output_separate_from_runner_status(self):
        with contextlib.redirect_stdout(io.StringIO()):
            result = run_command([sys.executable, "-c", "print('owned output')"], self.root, capture=True)
        self.assertEqual(result, "owned output\n")

    def test_success_and_failure_reap_owned_orphan(self):
        for code in (0, 7):
            with self.subTest(code=code):
                for name in ("ready", "stopped"):
                    (self.root / name).unlink(missing_ok=True)
                command = [sys.executable, str(self.parent_script), str(self.root), str(code)]
                with contextlib.redirect_stdout(io.StringIO()):
                    if code:
                        with self.assertRaises(SystemExit) as raised:
                            run_command(command, self.root)
                        self.assertEqual(raised.exception.code, code)
                    else:
                        run_command(command, self.root)
                self.assertEqual((self.root / "stopped").read_text(), "yes")

    def test_term_reports_interruption_and_stops_owned_descendants(self):
        launcher = self.root / "launcher.py"
        launcher.write_text(
            "import sys\nfrom pathlib import Path\n"
            f"sys.path.insert(0, {str(ROOT / 'scripts')!r})\n"
            "from validation_process import run_command\n"
            f"run_command([sys.executable, {str(self.parent_script)!r}, {str(self.root)!r}, 'wait'], Path({str(self.root)!r}))\n"
        )
        process = subprocess.Popen([sys.executable, str(launcher)], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        try:
            deadline = time.monotonic() + 10
            while not (self.root / "ready").exists():
                if time.monotonic() > deadline or process.poll() is not None:
                    self.fail("owned child did not become ready")
                time.sleep(0.01)
            process.send_signal(signal.SIGTERM)
            output, _ = process.communicate(timeout=12)
            self.assertEqual(process.returncode, 143, output)
            self.assertIn("not passed", output)
            self.assertEqual((self.root / "stopped").read_text(), "yes")
        finally:
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
                process.wait(timeout=12)


if __name__ == "__main__":
    unittest.main()

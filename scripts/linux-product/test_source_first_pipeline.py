#!/usr/bin/env python3
"""Bounded synthetic tests of assembly, private capture and target orchestration."""
from __future__ import annotations

import argparse
import copy
import json
import os
import signal
import shlex
import subprocess
import sys
import time
import unittest
from pathlib import Path
from unittest.mock import patch

import source_first as admission
import source_first_build as build
import source_first_capture as capture
import source_first_contract as contract
import source_first_io as storage
import source_first_process as process
from test_l6_release_pair import write_elf
from test_source_first import BudgetExpired, Fixture, budget_expired, put as fixture_put, sha


ROOT: Path


def put(path, value, mode=0o600):
    # mkdir(parents=True, mode=0700) applies mode only to the leaf directory.
    # Create each new private parent explicitly; never chmod retained failed fixtures.
    missing = []
    parent = path.parent
    while not parent.exists():
        missing.append(parent)
        parent = parent.parent
    for directory in reversed(missing):
        storage.new_private_root(directory)
    return fixture_put(path, value, mode)


def binary(path: Path, *, acceptance: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    write_elf(path, acceptance=acceptance)
    with path.open("ab") as file:
        file.write(path.name.encode())


class SyntheticCommands:
    def __init__(self, log_root, budget, environment) -> None:
        self.environment = environment
        self.deadline = time.monotonic() + budget
        self.calls: list[str] = []
        self.hook = None

    def run(self, arguments, cwd, label):
        self.calls.append(label)
        if self.hook:
            self.hook(arguments, cwd, label)
        if label in {"production", "acceptance"}:
            names = (["radishlex-linux-l6-acceptance"] if label == "acceptance" else
                     ["radishlex-linux-maintenance", "radishlex-linux-artifact-verifier"])
            for name in names:
                executable = cwd / "target/release" / name
                binary(executable, acceptance=label == "acceptance")
                deps = executable.parent / "deps"
                deps.mkdir(exist_ok=True, mode=0o700)
                os.link(executable, deps / (name.replace("-", "_") + "-0123456789abcdef"))
        if label == "artifact":
            output = Path(arguments[arguments.index("--output-dir") + 1])
            metadata = json.loads((cwd / admission.METADATA_PATH).read_bytes())
            version = metadata["package_version"]
            package = output / f"radishlex_{version}_arm64.deb"
            identity = put(package, ("synthetic artifact " + version + "\n").encode(), 0o644)
            value = {"package": identity,
                     "control": {"version": version, "architecture": "arm64",
                                 "depends": ["libc6 (>= 2.41)", "fonts-noto-cjk"]},
                     "product_manifest": {"sha256": sha("manifest " + version)}}
            put(package.with_name(package.name + ".evidence.json"), value, 0o644)
        if label in {"actual-artifact", "actual-source"}:
            return b"artifact_verification=ok\n"
        return b"synthetic command completed\n"


class PipelineTests(unittest.TestCase):
    def setUp(self):
        self.root = ROOT / self._testMethodName
        storage.new_private_root(self.root)
        self.fixture = Fixture(self.root)
        self.work, self.output = self.root / "work", self.root / "published"
        self.commands = None
        self.verifier_calls = 0
        self.source_checks = 0

    def reject(self, call, *args, **kwargs):
        with self.assertRaises((storage.SourceFirstError, OSError, RuntimeError)) as caught:
            call(*args, **kwargs)
        return caught.exception

    def prepare_build(self, *, source=False):
        f = self.fixture
        f.repository()
        if source:
            f.command("checkout", f.s)
        (f.repo / ".git/info/exclude").write_text(
            "target/\napps/radishlex-manager/.dart_tool/\n")
        self.pub_config = f.repo / "apps/radishlex-manager/.dart_tool/package_config.json"
        put(self.pub_config, {"configVersion": 2, "packages": []}, 0o644)
        self.environment = {"format": build.SOURCE_ENVIRONMENT, "network": "loopback-only",
                            "toolchain": {"synthetic": "no real tool invoked"},
                            "pub_config_sha256": storage.read_file(self.pub_config, mode=0o644)[1].sha256}
        self.environment_path = self.root / "environment.json"
        put(self.environment_path, self.environment)

    def factory(self, *args):
        self.commands = SyntheticCommands(*args)
        self.commands.hook = getattr(self, "hook", None)
        return self.commands

    def assemble(self, *, source=False):
        f = self.fixture
        kwargs = {} if source else {"review_commit": f.r, "seed_root": f.seed_root,
                                   "review_path": f.review_path, "source_package": f.package,
                                   "source_evidence": f.evidence}
        with patch.object(build, "BuildCommands", side_effect=self.factory), patch.object(
                build, "inspect_environment", return_value=(
                    self.environment, storage.read_file(self.pub_config, mode=0o644)[1])):
            return build.build(f.repo, self.work, self.output, self.environment_path, 300, **kwargs)

    def prepare_capture(self):
        f = self.fixture
        self.verifier = self.root / "radishlex-linux-artifact-verifier"
        binary(self.verifier)
        f.source["executables"]["verifier"] = build.executable_record(
            self.verifier, f.source["commit"], "verifier")
        f.make_seed()
        self.capture_root = self.root / "private-capture"
        storage.new_private_root(self.capture_root)
        roles = {}
        self.receipt_result = {}
        for role in sorted(contract.ROLES):
            facts = copy.deepcopy(f.observations[role]["facts"])
            material_keys = capture.MATERIAL_FIELDS.get(role, set())
            digest_keys = material_keys or {"observation_sha256"}
            for key in digest_keys:
                facts.pop(key)
            report_path = "reports/" + role + ".json"
            report = {"format": capture.REPORT_FORMAT, "role": role,
                      "binding": f.seed["binding"], "facts": facts}
            report_identity = put(self.capture_root / report_path, report)
            materials = {}
            for key in sorted(material_keys):
                path = "materials/" + role + "/" + key + ".json"
                value = {"synthetic": key}
                if key == "receipt_sha256":
                    value = b'{"synthetic":"raw receipt, not a product fixture"}\n'
                    self.receipt_result["receipt_sha256"] = storage.digest(value)
                elif key == "root_staging_sha256":
                    value = {"root_identity_sha256": sha("root identity"),
                             "binding": f.seed["binding"], "staging_inventory_sha256": sha("staging")}
                elif key in {"before_sha256", "after_sha256"}:
                    value = {"xdg": "absent"}
                elif key == "result_sha256":
                    value = {"status": "completed", "synthetic": "fixed guest result"}
                elif "inventory" in key:
                    value = {"synthetic": "same closed inventory"}
                materials[key] = {"path": path, "file": put(self.capture_root / path, value)}
            if role == "receipt":
                self.receipt_result.update(facts=facts, operation_sha256=f.seed["binding"]["operation_sha256"],
                                           root_identity_sha256=sha("root identity"))
            roles[role] = {"report": {"path": report_path, "file": report_identity},
                           "materials": materials}
        self.capture_value = {"format": capture.CAPTURE_FORMAT, "profile": contract.PROFILE,
                              "source": f.source, "binding": f.seed["binding"], "roles": roles}
        put(self.capture_root / "capture.json", self.capture_value)

    def collect(self, verifier=None, source_check=None):
        f = self.fixture

        def check_source(*args):
            self.source_checks += 1
            if source_check:
                return source_check(*args)
            return None

        def check_receipt(*args):
            self.verifier_calls += 1
            return verifier(*args) if verifier else self.receipt_result

        with patch.object(capture, "verify_receipt", side_effect=check_receipt), patch.object(
                capture.admission, "source_git_inputs", side_effect=check_source):
            return capture.collect(self.capture_root, f.package, f.evidence, self.verifier,
                                   self.work, self.output, 300, f.candidate)

    def replace_capture_material(self, role, key, value):
        reference = self.capture_value["roles"][role]["materials"][key]
        path = self.capture_root / reference["path"]
        path.write_bytes(storage.canonical(value))
        reference["file"] = storage.read_file(path)[1].record()
        (self.capture_root / "capture.json").write_bytes(storage.canonical(self.capture_value))

    def test_source_artifact_pipeline_uses_existing_steps_without_pair_or_acceptance(self):
        self.prepare_build(source=True)
        self.assemble(source=True)
        source, _ = storage.load_json(self.output / "source.json")
        contract.validate_source(source, self.fixture.candidate)
        self.assertNotIn("acceptance", self.commands.calls)
        self.assertEqual(self.commands.calls.count("manager"), 1)
        self.assertTrue((self.output / "inventory.json").is_file())
        self.assertFalse((self.fixture.repo / admission.BASELINE_PATH).exists())
        for executable in source["executables"].values():
            name = executable["file"]["filename"]
            original = self.fixture.repo / "target/release" / name
            published = self.output / name
            self.assertEqual(original.stat().st_nlink, 2)
            self.assertEqual(published.stat().st_nlink, 1)
            self.assertNotEqual(original.stat().st_ino, published.stat().st_ino)
            self.assertEqual(original.read_bytes(), published.read_bytes())

    def test_cargo_executable_rejects_hardlink_outside_deps(self):
        original = self.root / "target/release/radishlex-linux-maintenance"
        binary(original)
        (original.parent / "deps").mkdir(mode=0o700)
        os.link(original, self.root / "radishlex_linux_maintenance-0123456789abcdef")
        output = self.root / "copied"
        error = self.reject(build.copy_cargo_executable, original, output)
        self.assertIn("outside its expected deps path", str(error))
        self.assertFalse(output.exists())

    def test_target_only_pipeline_preserves_source_and_validates_both_actual_artifacts(self):
        self.prepare_build()
        before = storage.read_file(self.fixture.package, mode=0o644)[1]
        self.assemble()
        pair, _ = storage.load_json(self.output / "pair.evidence.json")
        contract.validate_pair(pair, self.fixture.baseline, self.fixture.r, self.fixture.t,
                               storage.digest(storage.canonical(self.fixture.baseline)))
        self.assertEqual(self.commands.calls.count("manager"), 1)
        self.assertEqual(self.commands.calls[-2:], ["actual-artifact", "actual-source"])
        storage.read_file(self.fixture.package, mode=0o644, previous=before)
        self.assertEqual((self.output / "source" / self.fixture.package.name).read_bytes(),
                         self.fixture.package.read_bytes())

    def test_missing_review_baseline_stops_before_work_or_commands(self):
        self.prepare_build()
        self.fixture.r = self.fixture.s
        self.reject(self.assemble)
        self.assertIsNone(self.commands)
        self.assertFalse(self.work.exists())

    def test_environment_drift_stops_before_commands(self):
        self.prepare_build(source=True)
        self.environment_path.write_bytes(storage.canonical({"wrong": "environment"}))
        self.reject(self.assemble, source=True)
        self.assertIsNone(self.commands)
        self.assertFalse(self.work.exists())

    def test_existing_publication_never_overwritten(self):
        self.prepare_build()
        storage.new_private_root(self.output)
        put(self.output / "keep.json", {"keep": True})
        self.reject(self.assemble)
        self.assertIsNone(self.commands)
        self.assertTrue((self.output / "keep.json").is_file())

    def test_build_failure_runs_once_and_preserves_source_copy_and_phase(self):
        self.prepare_build()

        def fail(arguments, cwd, label):
            if label == "addon":
                raise storage.SourceFirstError("synthetic addon failure")

        self.hook = fail
        self.reject(self.assemble)
        self.assertEqual(self.commands.calls.count("addon"), 1)
        self.assertTrue((self.work / "handoff/source" / self.fixture.package.name).is_file())
        self.assertEqual(json.loads((self.work / "failure.json").read_bytes())["phase"], "target-artifact")

    def test_source_inode_replacement_during_build_rejected_and_retained(self):
        self.prepare_build()
        replacement = self.root / "same-bytes"
        put(replacement, self.fixture.package.read_bytes(), 0o644)

        def replace(arguments, cwd, label):
            if label == "manager":
                os.replace(replacement, self.fixture.package)

        self.hook = replace
        self.reject(self.assemble)
        self.assertTrue((self.work / "failure.json").is_file())
        self.assertFalse(self.output.exists())

    def test_seed_inode_replacement_during_build_rejected(self):
        self.prepare_build()
        seed = self.fixture.seed_root / "receipt.json"
        replacement = self.root / "same-seed-bytes"
        put(replacement, seed.read_bytes())

        def replace(arguments, cwd, label):
            if label == "manager":
                os.replace(replacement, seed)

        self.hook = replace
        self.reject(self.assemble)
        self.assertFalse(self.output.exists())
        self.assertTrue((self.work / "failure.json").is_file())

    def test_unknown_artifact_output_rejected(self):
        self.prepare_build(source=True)

        def unknown(arguments, cwd, label):
            if label == "production":
                put(self.work / "handoff/artifacts/unknown.json", {})

        self.hook = unknown
        self.reject(self.assemble, source=True)
        self.assertTrue((self.work / "handoff/artifacts/unknown.json").is_file())
        self.assertFalse(self.output.exists())

    def test_post_publish_sync_failure_preserves_output_without_success(self):
        self.prepare_build(source=True)
        original = storage.fsync_directory

        def fail(path):
            if self.output.exists() and path == self.output.parent:
                raise OSError("synthetic post-publication sync failure")
            return original(path)

        with patch.object(storage, "fsync_directory", side_effect=fail):
            self.reject(self.assemble, source=True)
        self.assertTrue((self.output / "source.json").is_file())
        self.assertTrue((self.work / "failure.json").is_file())

    def test_invalid_build_budget_has_no_output_or_commands(self):
        self.prepare_build(source=True)
        self.reject(build.build, self.fixture.repo, self.work, self.output, self.environment_path, 0)
        self.assertFalse(self.work.exists())

    def test_real_command_runner_retains_one_synthetic_log(self):
        logs = self.root / "logs"
        storage.new_private_root(logs)
        runner = process.BuildCommands(logs, 5, {"PATH": "/usr/bin:/bin"})
        output = runner.run([sys.executable, "-c", "print('synthetic command')"], self.root, "synthetic")
        self.assertEqual(output, b"synthetic command\n")
        self.assertEqual((logs / "01-synthetic.log").read_bytes(), output)

    def test_command_deadline_exhausted_before_child_creation(self):
        logs = self.root / "logs"
        storage.new_private_root(logs)
        runner = process.BuildCommands(logs, 1, {})
        runner.deadline = time.monotonic() - 1
        with patch.object(process.subprocess, "Popen", side_effect=AssertionError("must not spawn")):
            self.reject(runner.run, ["unused"], self.root, "expired")
        self.assertEqual(list(logs.iterdir()), [])

    def test_actual_addon_script_retains_failed_temporary_build(self):
        tools = self.root / "tools"
        storage.new_private_root(tools)
        put(tools / "python3",
            ("#!/bin/bash\nexec " + shlex.quote(sys.executable) + ' "$@"\n').encode(), 0o755)
        put(tools / "uname", b'#!/bin/bash\nif [[ "$1" == -s ]]; then echo Linux; else echo aarch64; fi\n', 0o755)
        for name in ("cmake", "ctest", "file", "ldd", "readelf", "strings"):
            put(tools / name, b"#!/bin/bash\nexit 17\n", 0o755)
        temporary = self.root / "temporary"
        storage.new_private_root(temporary)
        result = subprocess.run(
            ["/bin/bash", str(admission.REPO_ROOT / "scripts/build-linux-product-addon-stage.sh"),
             "--ffi-library", str(self.fixture.package), "--output", str(self.root / "addon")],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10,
            env={"PATH": str(tools) + ":/usr/bin:/bin", "TMPDIR": str(temporary),
                 "RADISHLEX_PRESERVE_BUILD_FAILURES": "1", "PYTHONDONTWRITEBYTECODE": "1"})
        put(self.root / "script.stdout", result.stdout or b"\n")
        put(self.root / "script.stderr", result.stderr or b"\n")
        self.assertEqual(result.returncode, 17)
        self.assertIn(b"Failed addon build retained:", result.stderr)
        self.assertEqual(len(list(self.root.glob(".radishlex-addon.*"))), 1)

    def test_actual_manager_offline_mode_sets_cargo_offline_before_compilation(self):
        repository = self.root / "offline-script"
        storage.new_private_root(repository)
        script = repository / "scripts/build-manager-linux-product.sh"
        put(script, (admission.REPO_ROOT / "scripts/build-manager-linux-product.sh").read_bytes(), 0o755)
        put(repository / "apps/radishlex-manager/.dart_tool/package_config.json",
            {"configVersion": 2, "packages": []}, 0o644)
        tools = self.root / "tools"
        storage.new_private_root(tools)
        put(tools / "uname", b'#!/bin/bash\nif [[ "$1" == -s ]]; then echo Linux; else echo aarch64; fi\n', 0o755)
        put(tools / "cargo", b'#!/bin/bash\nprintf "offline=%s\\n" "$CARGO_NET_OFFLINE"\nexit 19\n', 0o755)
        result = subprocess.run(
            ["/bin/bash", str(script), "--system-product-offline"],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5,
            env={"PATH": str(tools) + ":/usr/bin:/bin", "CARGO_HOME": str(self.root / "cargo")})
        put(self.root / "script.stdout", result.stdout or b"\n")
        put(self.root / "script.stderr", result.stderr or b"\n")
        self.assertEqual(result.returncode, 19)
        self.assertIn(b"offline=true", result.stdout)

    def test_collects_closed_seed_without_exporting_private_receipt(self):
        self.prepare_capture()
        self.collect()
        seed, files, _ = admission.verify_seed(self.output, self.fixture.candidate)
        self.assertEqual(len(files), 13)
        self.assertEqual(seed["binding"], self.fixture.seed["binding"])
        self.assertTrue((self.capture_root / "materials/receipt/receipt_sha256.json").is_file())
        self.assertFalse((self.output / "materials").exists())
        self.assertEqual(json.loads((self.work / "result.json").read_bytes())["raw_material_review"],
                         "required")

    def test_capture_unknown_file_rejected_before_work(self):
        self.prepare_capture()
        put(self.capture_root / "unknown.json", {})
        self.reject(self.collect)
        self.assertFalse(self.work.exists())

    def test_capture_source_git_provenance_required_before_verifier(self):
        self.prepare_capture()

        def reject_source(*args):
            raise storage.SourceFirstError("synthetic source Git mismatch")

        error = self.reject(self.collect, None, reject_source)
        self.assertIn("synthetic source Git mismatch", str(error))
        self.assertEqual(self.source_checks, 1)
        self.assertEqual(self.verifier_calls, 0)
        self.assertFalse(self.work.exists())
        self.assertFalse(self.output.exists())

    def test_capture_path_traversal_rejected(self):
        self.prepare_capture()
        self.capture_value["roles"]["postflight"]["report"]["path"] = "../outside.json"
        (self.capture_root / "capture.json").write_bytes(storage.canonical(self.capture_value))
        self.reject(self.collect)
        self.assertFalse(self.work.exists())

    def test_typed_receipt_exit_only_is_not_success(self):
        self.prepare_capture()
        error = self.reject(self.collect, lambda *args: {"exit_code": 0})
        self.assertIn("typed receipt result fields differ", str(error))
        self.assertEqual(self.verifier_calls, 1)
        self.assertTrue((self.work / "failure.json").is_file())
        self.assertFalse(self.output.exists())

    def test_receipt_operation_cannot_be_substituted(self):
        self.prepare_capture()
        self.receipt_result["operation_sha256"] = sha("wrong operation")
        error = self.reject(self.collect)
        self.assertIn("receipt operation differs", str(error))
        self.assertEqual(self.verifier_calls, 1)
        self.assertFalse(self.output.exists())

    def test_receipt_root_identity_must_match_retained_root_capture(self):
        self.prepare_capture()
        self.receipt_result["root_identity_sha256"] = sha("wrong root")
        error = self.reject(self.collect)
        self.assertIn("receipt/root physical identity differs", str(error))
        self.assertEqual(self.verifier_calls, 1)
        self.assertFalse(self.output.exists())

    def test_xdg_raw_fingerprints_cannot_claim_unchanged_when_different(self):
        self.prepare_capture()
        self.replace_capture_material("xdg", "after_sha256", {"xdg": "changed"})
        error = self.reject(self.collect)
        self.assertIn("XDG fingerprint differs", str(error))
        self.assertEqual(self.verifier_calls, 1)
        self.assertTrue((self.work / "handoff/receipt.json").is_file())
        self.assertFalse(self.output.exists())

    def test_host_raw_result_must_match_guest_raw_result(self):
        self.prepare_capture()
        self.replace_capture_material("host-readback", "result_sha256", {"result": "other"})
        error = self.reject(self.collect)
        self.assertIn("guest/host result differs", str(error))
        self.assertEqual(self.verifier_calls, 1)
        self.assertFalse(self.output.exists())

    def test_capture_replacement_during_verifier_is_rejected(self):
        self.prepare_capture()
        path = self.capture_root / "reports/startup.json"
        replacement = self.root / "same-capture-bytes"
        put(replacement, path.read_bytes())

        def replace(*args):
            os.replace(replacement, path)
            return self.receipt_result

        error = self.reject(self.collect, replace)
        self.assertIn("input identity replaced", str(error))
        self.assertEqual(self.verifier_calls, 1)
        self.assertTrue((self.work / "failure.json").is_file())
        self.assertFalse(self.output.exists())

    def test_verifier_replacement_during_collection_is_rejected(self):
        self.prepare_capture()
        replacement = self.root / "same-verifier-bytes"
        put(replacement, self.verifier.read_bytes(), 0o755)

        def replace(*args):
            os.replace(replacement, self.verifier)
            return self.receipt_result

        error = self.reject(self.collect, replace)
        self.assertIn("input identity replaced", str(error))
        self.assertEqual(self.verifier_calls, 1)
        self.assertFalse(self.output.exists())

    def test_collection_panic_preserves_partial_output_and_raw_capture(self):
        self.prepare_capture()
        original = capture.write_exclusive

        def panic(path, data, mode=0o600):
            if path.name == "network.json":
                raise KeyboardInterrupt("synthetic collection panic")
            return original(path, data, mode)

        with patch.object(capture, "write_exclusive", side_effect=panic):
            with self.assertRaises(KeyboardInterrupt):
                self.collect()
        self.assertTrue((self.work / "handoff/dependencies.json").is_file())
        self.assertTrue((self.work / "failure.json").is_file())
        self.assertTrue((self.capture_root / "capture.json").is_file())


def main():
    global ROOT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-root", required=True, type=Path)
    args = parser.parse_args()
    ROOT = args.test_root
    storage.new_private_root(ROOT)
    names = unittest.defaultTestLoader.getTestCaseNames(PipelineTests)
    put(ROOT / "plan.json", {"cases": names, "budget_seconds": 300, "cleanup": "none",
                             "scope": "synthetic commands, capture, lineage; no VM or build"})
    print(f"Registered new pipeline cases; preserve all materials: {ROOT}", flush=True)
    signal.signal(signal.SIGALRM, budget_expired)
    signal.setitimer(signal.ITIMER_REAL, 300)
    started = time.monotonic()
    try:
        result = unittest.TextTestRunner(verbosity=2, failfast=True).run(
            unittest.defaultTestLoader.loadTestsFromTestCase(PipelineTests))
    except BudgetExpired:
        put(ROOT / "timeout.json", {"result": "incomplete-preserved", "budget_seconds": 300})
        return 124
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
    elapsed = round(time.monotonic() - started, 3)
    put(ROOT / "result.json", {"tests": result.testsRun, "failures": len(result.failures),
                              "errors": len(result.errors), "elapsed_seconds": elapsed,
                              "within_budget": elapsed <= 300})
    return 0 if result.wasSuccessful() and elapsed <= 300 else 1


if __name__ == "__main__":
    raise SystemExit(main())

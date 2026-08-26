#!/usr/bin/env python3
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import l6_utm_install_artifacts_staged_boot_classification_bindings as bindings
import l6_utm_install_artifacts_staged_boot_classification_resolution as resolution
import l6_utm_install_artifacts_staged_guest_agent_bindings as guest_bindings
import l6_utm_install_artifacts_staged_guest_agent_resolution as guest_control
import l6_v4_boot_transport_probe as guest_probe
import test_l6_utm_install_artifacts_staged_boot_start_resolution as boot_test
import test_l6_utm_install_artifacts_staged_runtime_resolution as runtime_test


class ClassificationRunner(boot_test.FakeRunner):
    def __init__(
        self,
        request: resolution.BootClassificationResolutionRequest,
        probe_bytes: bytes,
        *,
        readiness_states: list[str] | None = None,
        boot_hash: str = boot_test.NEW_BOOT_HASH,
    ) -> None:
        states = list(readiness_states or ["unavailable", "ready"])
        handles = [None] * 5 + [(42, "QEMULauncher")] * (
            1 + 3 + len(states) + 1
        )
        super().__init__(
            request,
            probe_bytes,
            boot_hash=boot_hash,
            handle_identities=handles,
        )
        self.readiness_states = states

    def run(self, argv, timeout_seconds, *, stdin_file=None):
        if argv == guest_control.readiness_argv(self.request):
            del timeout_seconds, stdin_file
            self.calls.append(argv)
            if not self.readiness_states:
                raise AssertionError("unexpected readiness call")
            state = self.readiness_states.pop(0)
            if state == "ready":
                return boot_test.observation(argv)
            if state == "unavailable":
                return boot_test.observation(
                    argv,
                    stderr=guest_bindings.REQUIRED_AGENT_UNAVAILABLE_STDERR,
                )
            raise AssertionError(f"unknown readiness state: {state}")
        return super().run(
            argv, timeout_seconds, stdin_file=stdin_file
        )


class BootClassificationResolutionTests(unittest.TestCase):
    def test_binding_preserves_execution_upstream_after_frozen_result(self) -> None:
        request = make_request(
            Path("/tmp/radishlex-boot-classification-binding-test")
        )
        transport_upstream = SimpleNamespace(label="boot-transport")
        prior_boot_start = SimpleNamespace(upstream=transport_upstream)
        prior_guest_agent = SimpleNamespace(upstream=prior_boot_start)
        frozen_result = SimpleNamespace(
            evidence={
                "guest_agent_result_bindings_sha256": "a" * 64,
                "prior_guest_agent_entries_verified": 26,
                "prior_guest_agent_marker": "absent-after-probe-exec",
                "prior_guest_agent_outcome": "state-indeterminate",
            },
            expected_boot_id_sha256=boot_test.ORIGINAL_BOOT_HASH,
            prior_backend_pid=bindings.result_bindings.REQUIRED_BACKEND_PID,
            probe_bytes=b"current fixed probe",
            upstream=prior_guest_agent,
        )

        with mock.patch.object(
            bindings.result_bindings,
            "validate_guest_agent_result_bindings",
            return_value=frozen_result,
        ):
            binding = bindings.validate_boot_classification_bindings(request)

        self.assertIs(binding.upstream, transport_upstream)
        self.assertEqual(
            binding.evidence["prior_guest_agent_entries_verified"], 26
        )
        self.assertEqual(
            binding.evidence["prior_guest_agent_marker"],
            "absent-after-probe-exec",
        )

    def test_stopped_target_waits_for_agent_and_classifies_new_boot(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = boot_test.make_binding()
            runner = ClassificationRunner(request, binding.probe_bytes)

            result, source = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "new-boot-started")
            self.assertEqual(
                result.exit_code, resolution.EXIT_NEW_BOOT_STARTED
            )
            self.assertTrue(source.file_object.closed)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["agent_readiness_invocations"], 2)
            self.assertEqual(terminal["agent_readiness_state"], "ready")
            self.assertEqual(
                terminal["readiness_identity_observation_count"], 2
            )
            self.assertEqual(terminal["guest_exec_invocations"], 5)
            self.assertEqual(terminal["format"], resolution.EVIDENCE_FORMAT)
            self.assertEqual(
                read_json(request.output_root / "request.json")["format"],
                resolution.EVIDENCE_FORMAT,
            )
            runtime_test.assert_manifest_valid(self, request.output_root)
            self.assert_forbidden_actions_absent(runner.calls)

    def test_probe_cli_uses_fixed_boot_start_scope(self) -> None:
        request = make_request(
            Path("/tmp/radishlex-boot-classification-cli-test")
        )
        binding = boot_test.make_binding()

        argv = resolution.boot_control.probe_argv(request, binding)
        args = guest_probe.parse_args(argv[8:])

        self.assertEqual(
            args.control_scope, guest_probe.CONTROL_SCOPE_BOOT_START
        )
        self.assertEqual(args.control_root, Path(request.guest_control_root))
        self.assertEqual(
            guest_probe.control_root_for(
                args.control_scope, args.attempt_id
            ),
            args.control_root,
        )

    def test_agent_readiness_exhaustion_fails_closed_before_probe(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            binding = boot_test.make_binding()
            runner = ClassificationRunner(
                request,
                binding.probe_bytes,
                readiness_states=["unavailable"] * 60,
            )

            result, _ = run_case(request, binding, runner)

            self.assertEqual(result.outcome, "state-indeterminate")
            self.assertEqual(result.guest_probe_invocations, 0)
            terminal = read_json(request.output_root / "terminal.json")
            self.assertEqual(terminal["agent_readiness_invocations"], 60)
            self.assertEqual(
                terminal["agent_readiness_state"], "unavailable"
            )
            self.assertEqual(terminal["automatic_retry"], "not-performed")
            self.assertFalse(
                (request.output_root / "guest-control-root-create.json").exists()
            )
            self.assert_forbidden_actions_absent(runner.calls)

    def test_authorization_and_prior_root_overlap_reject(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            request = make_request(Path(temporary))
            unauthorized = replace_request(
                request,
                authorized_bound_prior_guest_agent_result=False,
            )
            with self.assertRaisesRegex(
                resolution.boot_control.BootStartResolutionError,
                "authorized-bound-prior-guest-agent-result-required",
            ):
                unauthorized.validate()

            overlapping = replace_request(
                request,
                output_root=request.prior_guest_agent_root / "new",
            )
            with self.assertRaisesRegex(
                resolution.boot_control.BootStartResolutionError,
                "output-root-must-not-overlap-prior-guest-agent-root",
            ):
                overlapping.validate()

    def assert_forbidden_actions_absent(
        self, calls: list[tuple[str, ...]]
    ) -> None:
        flattened = [" ".join(call) for call in calls]
        for forbidden in (
            "utmctl status",
            "utmctl start",
            " resume ",
            " retry ",
            " stop ",
            " quit ",
            " dpkg ",
        ):
            self.assertFalse(any(forbidden in value for value in flattened))


def make_request(
    root: Path,
) -> resolution.BootClassificationResolutionRequest:
    old = boot_test.make_request(root)
    values = dict(old.__dict__)
    values.update(
        output_root=root / "boot-classification-output",
        boot_start_attempt_id=(
            bindings.REQUIRED_BOOT_CLASSIFICATION_ATTEMPT_ID
        ),
        prior_guest_agent_root=root / "prior-guest-agent",
        prior_guest_agent_manifest_sha256=(
            bindings.REQUIRED_PRIOR_GUEST_AGENT_MANIFEST_SHA256
        ),
        guest_agent_attempt_id=(
            bindings.REQUIRED_PRIOR_GUEST_AGENT_ATTEMPT_ID
        ),
        prior_guest_agent_attempt_id=(
            bindings.REQUIRED_PRIOR_GUEST_AGENT_ATTEMPT_ID
        ),
        boot_classification_attempt_id=(
            bindings.REQUIRED_BOOT_CLASSIFICATION_ATTEMPT_ID
        ),
        agent_readiness_attempts=60,
        authorized_install_artifacts_staged_boot_classification_resolution=True,
        authorized_bound_prior_guest_agent_result=True,
        authorized_bounded_read_only_guest_agent_readiness=True,
    )
    return resolution.BootClassificationResolutionRequest(**values)


def run_case(
    request: resolution.BootClassificationResolutionRequest,
    binding: SimpleNamespace,
    runner: ClassificationRunner,
):
    source = boot_test.SyntheticSource(boot_test.CloseTracker())
    result = resolution.run_boot_classification_resolution(
        request,
        runner=runner,
        binding_validator=lambda _: binding,
        source_opener=lambda _: source,
        source_revalidator=lambda _request, _source: {
            "descriptor_unchanged": True,
            "format": "synthetic-source-v1",
            "inventory_unchanged": True,
            "sha256": (
                resolution.boot_control.reactivation_control.REQUIRED_SOURCE_BUNDLE_SHA256
            ),
            "size": (
                resolution.boot_control.reactivation_control.REQUIRED_SOURCE_BUNDLE_SIZE
            ),
        },
        target_validator=lambda _: runtime_test.target_identity(request),
        sleeper=lambda _: None,
    )
    return result, source


def replace_request(
    request: resolution.BootClassificationResolutionRequest,
    **changes: object,
) -> resolution.BootClassificationResolutionRequest:
    values = dict(request.__dict__)
    values.update(changes)
    return resolution.BootClassificationResolutionRequest(**values)


def read_json(path: Path) -> dict[str, object]:
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict)
    return value


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Portable source-first admission contracts. No system observations are executed here."""
from __future__ import annotations

import re
from typing import Any

from source_first_io import (MAX_ELF, MAX_JSON, MAX_PACKAGE, SourceFirstError,
                             canonical, digest, relative_path)


PROFILE = "debian13-arm64-source-first-v1"
MATRIX = "debian13-arm64-ephemeral-v1"
CANDIDATE_FORMAT = "radishlex-linux-source-candidate-v1"
BASELINE_FORMAT = "radishlex-linux-source-baseline-v1"
SEED_FORMAT = "radishlex-linux-source-seed-evidence-v1"
PAIR_FORMAT = "radishlex-linux-source-first-pair-evidence-v1"
OBSERVATION_FORMAT = "radishlex-linux-source-seed-observation-v1"
REVIEW_FORMAT = "radishlex-linux-source-review-v1"
DATA_FIELDS = {"ffi_abi_version", "userdb_schema_version", "runtime_layout", "data_layout",
               "settings_format_version", "privacy_format_version", "rime_schema_id",
               "rime_data_lock_sha256"}
LOCKS = ("Cargo.lock", "apps/radishlex-manager/pubspec.lock",
         "packaging/rime/product-rime-data.json")
RECIPE_PATHS = (
    "scripts/build-manager-linux-product.sh", "scripts/build-linux-product-addon-stage.sh",
    "scripts/build-linux-deb-artifact.sh", "scripts/linux-product/rootfs.py",
    "scripts/linux-product/deb_artifact.py", "packaging/linux/install-layout.json",
    "packaging/linux/debian/artifact.json",
    "scripts/linux-product/source_first.py", "scripts/linux-product/source_first_contract.py",
    "scripts/linux-product/source_first_io.py", "scripts/linux-product/source_first_stage.py",
    "scripts/linux-product/source_first_build.py", "scripts/linux-product/source_first_capture.py",
    "scripts/linux-product/source_first_process.py",
    "platforms/linux-product/src/bin/radishlex-linux-artifact-verifier.rs",
    "platforms/linux-product/src/model.rs", "scripts/check-linux-product-layout.sh",
    "platforms/linux-product/src/model/source_seed.rs",
)
ROLES = {"preflight", "receipt", "postflight", "startup", "quiescence", "dependencies",
         "network", "xdg", "guest-result", "host-readback", "host-stop", "input-stability"}
BINDING_KEYS = {"attempt", "guest_sha256", "boot_sha256", "operation_sha256",
                "executor_sha256", "source_sha256"}


def fields(value: Any, keys: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != keys:
        raise SourceFirstError(f"{label} fields differ")


def exact(actual: Any, expected: Any, label: str) -> None:
    # bool is not an integer in the wire format, even though Python says True == 1.
    equal = (actual == expected if isinstance(actual, bytes) and isinstance(expected, bytes)
             else canonical(actual) == canonical(expected))
    if not equal:
        raise SourceFirstError(f"{label} differs")


def hex_value(value: Any, length: int = 64) -> None:
    if (not isinstance(value, str) or not re.fullmatch(rf"[0-9a-f]{{{length}}}", value) or
            value == "0" * length):
        raise SourceFirstError("invalid or empty digest/commit")


def identity(value: Any, *, limit: int, filename: str | None = None) -> None:
    fields(value, {"filename", "size", "sha256"}, "file identity")
    name = relative_path(value["filename"])
    if "/" in name or (filename is not None and name != filename):
        raise SourceFirstError("artifact basename differs")
    if type(value["size"]) is not int or not 0 < value["size"] <= limit:
        raise SourceFirstError("artifact size exceeds limit")
    hex_value(value["sha256"])


def header(value: dict[str, Any], format_name: str) -> None:
    exact(value["format"], format_name, "format")
    exact(value["profile"], PROFILE, "profile")


def validate_candidate(value: Any) -> None:
    fields(value, {"format", "profile", "matrix_profile", "source_metadata",
                   "target_revision", "target_version", "seed_roles"}, "candidate")
    header(value, CANDIDATE_FORMAT)
    exact(value["matrix_profile"], MATRIX, "matrix profile")
    exact(value["seed_roles"], sorted(ROLES), "required seed roles")
    metadata = value["source_metadata"]
    from product_metadata import (INTEGER_FIELDS, METADATA_KEYS, STRING_FIELDS,
                                  LinuxProductMetadata, LinuxProductMetadataError,
                                  require_positive_integer, require_string)
    try:
        fields(metadata, METADATA_KEYS, "candidate metadata")
        for key in INTEGER_FIELDS:
            require_positive_integer(metadata[key], key)
        for key in STRING_FIELDS:
            require_string(metadata[key], key)
        dependencies = metadata["hard_dependencies"]
        if (not isinstance(dependencies, list) or not dependencies or
                not all(isinstance(item, str) for item in dependencies) or
                len(dependencies) != len(set(dependencies))):
            raise SourceFirstError("invalid candidate dependencies")
        for dependency in dependencies:
            require_string(dependency, "dependency")
        LinuxProductMetadata(**dict(metadata, hard_dependencies=tuple(dependencies))).validate()
    except (LinuxProductMetadataError, TypeError) as exc:
        raise SourceFirstError(f"invalid candidate metadata: {exc}") from exc
    fixed = {"product_id": "radishlex-linux", "package_name": "radishlex",
             "product_version": "26.7.1", "build_number": "40",
             "debian_revision": "2", "package_version": "26.7.1+40-2",
             "debian_release": "13", "debian_codename": "trixie",
             "debian_architecture": "arm64", "multiarch_tuple": "aarch64-linux-gnu",
             "distribution_identity": "debian-local-deb-v1",
             "rime_data_lock_sha256": "5722b2f31769758d392db395e0c367f2a30e189f25e7cc23bf05a3e3385698a1"}
    for key, expected in fixed.items():
        exact(metadata[key], expected, f"candidate {key}")
    exact(value["target_revision"], "3", "target revision")
    exact(value["target_version"], "26.7.1+40-3", "target version")


def artifact_identity(source: dict[str, Any], candidate: dict[str, Any]) -> dict[str, Any]:
    metadata = candidate["source_metadata"]
    return {
        "product_id": metadata["product_id"],
        "distribution_identity": metadata["distribution_identity"],
        "package_name": metadata["package_name"], "package_version": metadata["package_version"],
        "architecture": metadata["debian_architecture"],
        "package_filename": source["package"]["filename"],
        "evidence_filename": source["artifact_evidence"]["filename"],
        "package_size": source["package"]["size"],
        "evidence_size": source["artifact_evidence"]["size"],
        "package_sha256": source["package"]["sha256"],
        "evidence_sha256": source["artifact_evidence"]["sha256"],
        "product_manifest_sha256": source["manifest_sha256"],
        "data_contract": {key: metadata[key] for key in DATA_FIELDS},
    }


def validate_source(source: Any, candidate: dict[str, Any]) -> None:
    fields(source, {"commit", "metadata_sha256", "locks", "recipe_sha256",
                    "environment_sha256", "package", "artifact_evidence", "manifest_sha256",
                    "dependencies_sha256", "executables"}, "source")
    hex_value(source["commit"], 40)
    for key in ("metadata_sha256", "recipe_sha256", "environment_sha256",
                "manifest_sha256", "dependencies_sha256"):
        hex_value(source[key])
    exact(source["metadata_sha256"], digest(canonical(candidate["source_metadata"])),
          "source metadata")
    fields(source["locks"], set(LOCKS), "source locks")
    for value in source["locks"].values():
        hex_value(value)
    exact(source["locks"][LOCKS[2]], candidate["source_metadata"]["rime_data_lock_sha256"],
          "Rime source lock")
    package = "radishlex_26.7.1+40-2_arm64.deb"
    identity(source["package"], limit=MAX_PACKAGE, filename=package)
    identity(source["artifact_evidence"], limit=MAX_JSON, filename=package + ".evidence.json")
    fields(source["executables"], {"production", "verifier"}, "source executables")
    seen: set[str] = set()
    for role, binary in (("production", "radishlex-linux-maintenance"),
                         ("verifier", "radishlex-linux-artifact-verifier")):
        value = source["executables"][role]
        fields(value, {"file", "commit", "feature_profile", "elf"}, "source executable")
        identity(value["file"], limit=MAX_ELF, filename=binary)
        exact(value["commit"], source["commit"], "executor commit")
        exact(value["feature_profile"], "no-default-features", "executor features")
        exact(value["elf"], {"class": "ELF64", "machine": "AArch64",
                            "interpreter": "/lib/ld-linux-aarch64.so.1",
                            "type": "dynamic-or-executable"}, "executor ELF")
        seen.add(value["file"]["sha256"])
    if len(seen) != 2:
        raise SourceFirstError("source executors are not distinct")


def validate_binding(binding: Any, source: dict[str, Any]) -> None:
    fields(binding, BINDING_KEYS, "seed binding")
    if not isinstance(binding["attempt"], str) or not re.fullmatch(
            r"[a-z0-9][a-z0-9-]{7,79}", binding["attempt"]):
        raise SourceFirstError("invalid attempt identifier")
    for key in BINDING_KEYS - {"attempt"}:
        hex_value(binding[key])
    exact(binding["source_sha256"], digest(canonical(source)), "seed source binding")
    exact(binding["executor_sha256"], source["executables"]["production"]["file"]["sha256"],
          "seed production binding")


def validate_seed(seed: Any, candidate: dict[str, Any]) -> None:
    validate_candidate(candidate)
    fields(seed, {"format", "profile", "source", "binding", "attachments"}, "seed")
    header(seed, SEED_FORMAT)
    validate_source(seed["source"], candidate)
    validate_binding(seed["binding"], seed["source"])
    attachments = seed["attachments"]
    if not isinstance(attachments, list) or len(attachments) != len(ROLES):
        raise SourceFirstError("seed attachment count differs")
    roles: set[str] = set()
    paths: set[str] = set()
    total = 0
    for value in attachments:
        fields(value, {"role", "path", "file"}, "seed attachment")
        role, path = value["role"], relative_path(value["path"])
        if (not isinstance(role, str) or role not in ROLES or role in roles or
                path in paths or path == "seed.evidence.json"):
            raise SourceFirstError("duplicate/unknown attachment role/path")
        identity(value["file"], limit=MAX_JSON, filename=path.rsplit("/", 1)[-1])
        total += value["file"]["size"]
        roles.add(role)
        paths.add(path)
    if roles != ROLES or total > 16 * 1024 * 1024:
        raise SourceFirstError("seed inventory exceeds contract")


def validate_observation(value: Any, role: str, seed: dict[str, Any],
                         candidate: dict[str, Any]) -> None:
    fields(value, {"format", "role", "binding", "facts"}, "seed observation")
    exact(value["format"], OBSERVATION_FORMAT, "observation format")
    exact(value["role"], role, "observation role")
    exact(value["binding"], seed["binding"], "observation binding")
    source = seed["source"]
    artifact = artifact_identity(source, candidate)
    facts = value["facts"]
    expected: dict[str, dict[str, Any]] = {
        "preflight": {"package": "not-installed", "state_root": "absent",
                      "programs": "absent", "xdg": "absent", "profile": "absent",
                      "unknown_entries": 0, "guard": "absent", "processes": 0},
        "receipt": {"operation_kind": "install", "version_relation": "not_applicable",
                    "state": "completed", "chain_length": 1,
                    "initial_package": {"state": "not_installed", "artifact": None},
                    "source_artifact": None, "source_proof": None,
                    "target_artifact": artifact,
                    "target_proof": {"state": "installed", "artifact": artifact},
                    "failure_code": None, "failure_after_state": None,
                    "manual_recovery_required": False},
        "postflight": {"package": "installed", "artifact": artifact, "audit": "clean",
                       "inventory": "matched", "manifest": "matched", "md5": "matched",
                       "sha256": "matched", "root_staging_receipt": "matched"},
        "startup": {"manager": "AllowedProduct", "fcitx": "AllowedProduct",
                    "artifact": artifact},
        "quiescence": {"business_processes": 0, "package_mutations": 0,
                       "guard": "normal-terminal", "guard_probe_writes": 0},
        "dependencies": {"dependencies_sha256": source["dependencies_sha256"],
                         "versions": "matched", "font_family": "matched",
                         "font_glyph": "matched", "font_owner": "matched"},
        "network": {"policy": "isolated", "offline": "verified"},
        "xdg": {"before": "absent", "after": "absent", "profile_before": "absent",
                "profile_after": "absent", "writes": 0},
        "guest-result": {"status": "completed", "artifact": artifact},
        "host-readback": {"agent_result": "matched", "fixed_files": "matched",
                          "inventory": "closed"},
        "host-stop": {"running_guests_before": 1, "target": "stopped",
                      "other_running_guests_after": 0, "method": "normal-shutdown",
                      "host_cross_check": "matched"},
        "input-stability": {"before_after": "identity-and-hash-matched",
                            "package": source["package"],
                            "artifact_evidence": source["artifact_evidence"]},
    }
    # These digests point at privately retained raw evidence; they are not proof by themselves.
    digest_fields = {"preflight": {"observation_sha256"},
                     "receipt": {"receipt_sha256", "root_staging_sha256"},
                     "postflight": {"observation_sha256"}, "startup": {"observation_sha256"},
                     "quiescence": {"observation_sha256"},
                     "dependencies": {"observation_sha256"}, "network": {"observation_sha256"},
                     "xdg": {"before_sha256", "after_sha256"},
                     "guest-result": {"result_sha256"},
                     "host-readback": {"result_sha256", "guest_inventory_sha256",
                                       "host_inventory_sha256"},
                     "host-stop": {"observation_sha256"},
                     "input-stability": {"observation_sha256"}}[role]
    fields(facts, set(expected[role]) | digest_fields, f"{role} facts")
    for key, item in expected[role].items():
        exact(facts[key], item, f"{role}/{key}")
    for key in digest_fields:
        hex_value(facts[key])
    if role == "xdg":
        exact(facts["before_sha256"], facts["after_sha256"], "XDG fingerprint")


def validate_baseline(value: Any, candidate: dict[str, Any], seed: dict[str, Any],
                       seed_sha256: str) -> None:
    validate_seed(seed, candidate)
    fields(value, {"format", "profile", "candidate_sha256", "source", "seed_sha256",
                   "binding", "review_sha256", "inventory_sha256", "receipt_sha256",
                   "guest_result_sha256", "target_revision", "target_version"}, "baseline")
    header(value, BASELINE_FORMAT)
    for key in ("candidate_sha256", "seed_sha256", "review_sha256", "inventory_sha256",
                "receipt_sha256", "guest_result_sha256"):
        hex_value(value[key])
    exact(value["candidate_sha256"], digest(canonical(candidate)), "baseline candidate")
    exact(value["seed_sha256"], seed_sha256, "baseline seed")
    exact(value["source"], seed["source"], "baseline source")
    exact(value["binding"], seed["binding"], "baseline binding")
    exact(value["inventory_sha256"], digest(canonical(seed["attachments"])), "baseline inventory")
    for key in ("target_revision", "target_version"):
        exact(value[key], candidate[key], f"baseline {key}")


def validate_review(review: Any, baseline: dict[str, Any]) -> None:
    fields(review, {"format", "profile", "seed_sha256", "source_commit", "binding",
                    "inventory_sha256", "receipt_sha256", "checks"}, "review")
    header(review, REVIEW_FORMAT)
    for key in ("seed_sha256", "binding", "inventory_sha256", "receipt_sha256"):
        exact(review[key], baseline[key], f"review {key}")
    exact(review["source_commit"], baseline["source"]["commit"], "review source")
    exact(review["checks"], {role: "raw-material-reviewed" for role in sorted(ROLES)},
          "independent review checks")


def validate_metadata_pair(source: dict[str, Any], target: dict[str, Any],
                           candidate: dict[str, Any]) -> None:
    exact(source, candidate["source_metadata"], "S metadata")
    expected = dict(source, debian_revision=candidate["target_revision"],
                    package_version=candidate["target_version"])
    exact(target, expected, "T metadata and adjacent revision")


def validate_candidate_metadata(metadata: dict[str, Any], candidate: dict[str, Any]) -> None:
    """The static gate applies to S/R and T; neither branch grants target admission."""
    source = candidate["source_metadata"]
    if canonical(metadata) != canonical(source):
        validate_metadata_pair(source, metadata, candidate)


def validate_pair(value: Any, baseline: dict[str, Any], baseline_commit: str,
                  target_commit: str, baseline_sha256: str) -> None:
    fields(value, {"format", "profile", "matrix_profile", "source_commit", "baseline_commit",
                   "target_commit", "baseline_sha256", "seed_sha256", "review_sha256",
                   "source", "target", "executables", "environment_sha256"}, "pair")
    header(value, PAIR_FORMAT)
    exact(value["matrix_profile"], MATRIX, "pair matrix")
    expected = {"source_commit": baseline["source"]["commit"],
                "baseline_commit": baseline_commit, "target_commit": target_commit,
                "baseline_sha256": baseline_sha256, "seed_sha256": baseline["seed_sha256"],
                "review_sha256": baseline["review_sha256"], "source": baseline["source"]}
    for key, item in expected.items():
        exact(value[key], item, f"pair {key}")
    for key in ("source_commit", "baseline_commit", "target_commit"):
        hex_value(value[key], 40)
    if len({value[key] for key in ("source_commit", "baseline_commit", "target_commit")}) != 3:
        raise SourceFirstError("pair commits are not distinct")
    hex_value(value["environment_sha256"])
    target = value["target"]
    fields(target, {"package", "artifact_evidence", "manifest_sha256", "dependencies_sha256"},
           "pair target")
    package = "radishlex_26.7.1+40-3_arm64.deb"
    identity(target["package"], limit=MAX_PACKAGE, filename=package)
    identity(target["artifact_evidence"], limit=MAX_JSON, filename=package + ".evidence.json")
    for key in ("manifest_sha256", "dependencies_sha256"):
        hex_value(target[key])
    exact(target["dependencies_sha256"], baseline["source"]["dependencies_sha256"],
          "pair dependencies")
    for key in ("package", "artifact_evidence"):
        if target[key]["sha256"] == baseline["source"][key]["sha256"]:
            raise SourceFirstError("target reuses source bytes")
    if target["manifest_sha256"] == baseline["source"]["manifest_sha256"]:
        raise SourceFirstError("target reuses source manifest")
    fields(value["executables"], {"production", "verifier", "acceptance"}, "pair executables")
    hashes: set[str] = set()
    for role, binary in (("production", "radishlex-linux-maintenance"),
                         ("verifier", "radishlex-linux-artifact-verifier"),
                         ("acceptance", "radishlex-linux-l6-acceptance")):
        executable = value["executables"][role]
        fields(executable, {"file", "commit", "feature_profile", "elf"}, "pair executable")
        identity(executable["file"], limit=MAX_ELF, filename=binary)
        exact(executable["commit"], target_commit, "pair executor commit")
        exact(executable["elf"], baseline["source"]["executables"]["production"]["elf"],
              "pair executor ELF")
        exact(executable["feature_profile"], "compile-isolated-l6-acceptance-checkpoints"
              if role == "acceptance" else "no-default-features", "pair executor features")
        hashes.add(executable["file"]["sha256"])
    if len(hashes) != 3:
        raise SourceFirstError("pair executors are not distinct")

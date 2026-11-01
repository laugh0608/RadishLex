#!/usr/bin/env python3
"""Exact source copy for the new baseline; does not rebuild or verify historical install."""
from __future__ import annotations

from pathlib import Path
from typing import Any

from source_first_contract import exact
from source_first_io import (MAX_JSON, MAX_PACKAGE, FileSnapshot, SourceFirstError,
                             canonical, check_tree, digest, fsync_directory,
                             new_private_root, parse_json, read_file, write_exclusive)


def read_source_inputs(package: Path, evidence: Path, source: dict[str, Any]) -> tuple[
        bytes, bytes, FileSnapshot, FileSnapshot]:
    package_bytes, package_before = read_file(
        package, limit=MAX_PACKAGE, mode=0o644, expected=source["package"])
    evidence_bytes, evidence_before = read_file(
        evidence, limit=MAX_JSON, mode=0o644, expected=source["artifact_evidence"])
    artifact_evidence = parse_json(evidence_bytes)
    exact(artifact_evidence.get("package"), source["package"], "source evidence package")
    control = artifact_evidence.get("control")
    if not isinstance(control, dict):
        raise SourceFirstError("source evidence control is absent")
    exact(control.get("version"), "26.7.1+40-2", "source control version")
    exact(control.get("architecture"), "arm64", "source control architecture")
    dependencies = control.get("depends")
    if (not isinstance(dependencies, list) or not dependencies or
            not all(isinstance(item, str) and item and "\n" not in item for item in dependencies)):
        raise SourceFirstError("source dependencies are invalid")
    exact(digest(("\n".join(dependencies) + "\n").encode()),
          source["dependencies_sha256"], "source dependencies")
    manifest = artifact_evidence.get("product_manifest")
    if not isinstance(manifest, dict):
        raise SourceFirstError("source evidence manifest is absent")
    exact(manifest.get("sha256"), source["manifest_sha256"], "source manifest")
    return package_bytes, evidence_bytes, package_before, evidence_before


def stage_source(package: Path, evidence: Path, output: Path,
                 source: dict[str, Any]) -> dict[str, FileSnapshot]:
    package_bytes, evidence_bytes, package_before, evidence_before = read_source_inputs(
        package, evidence, source)
    # Creation and every subsequent failure retain this attempt; no cleanup handler.
    new_private_root(output)
    try:
        files = {
            package.name: write_exclusive(output / package.name, package_bytes, 0o644),
            evidence.name: write_exclusive(output / evidence.name, evidence_bytes, 0o644),
        }
        read_file(package, limit=MAX_PACKAGE, mode=0o644, previous=package_before)
        read_file(evidence, mode=0o644, previous=evidence_before)
        fsync_directory(output)
        check_tree(output, files, mode=0o644)
        return files
    except BaseException as exc:
        # Diagnostics contain a stable phase and exception class, never source contents.
        try:
            write_exclusive(output / "failure.json",
                            canonical({"phase": "source-copy", "error_type": type(exc).__name__,
                                       "result": "incomplete-preserved"}))
            fsync_directory(output)
        except BaseException as diagnostic_error:
            raise exc from diagnostic_error
        raise

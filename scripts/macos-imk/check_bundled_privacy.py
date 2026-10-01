#!/usr/bin/env python3
"""Exercise an assembled macOS FFI with synthetic data in new private directories."""

import argparse
import hashlib
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile


REPO_ROOT = Path(__file__).resolve().parents[2]


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product-root", type=Path, required=True)
    args = parser.parse_args()
    product = args.product_root.resolve(strict=True)
    bundle = product / "Components/RadishLexInputMethod.app"
    frameworks = bundle / "Contents/Frameworks"
    resources = bundle / "Contents/Resources"
    library = frameworks / "libradishlex_ime_ffi.dylib"
    source = REPO_ROOT / "platforms/macos-imk/Tests/bundled_privacy_smoke.c"
    subprocess.run([
        "python3", str(REPO_ROOT / "scripts/macos-product/product_manifest.py"),
        "verify", "--manager-bundle", str(product / "Components/radishlex_manager.app"),
        "--input-method-bundle", str(bundle), "--license", str(product / "LICENSE"),
        "--manifest", str(product / "ProductManifest.json"),
    ], check=True)
    work = Path(tempfile.mkdtemp(prefix="radishlex-bundled-privacy-"))
    print(f"Synthetic evidence: {work}", flush=True)
    binary = work / "probe"
    subprocess.run([
        "clang", "-std=c11", "-D_DARWIN_C_SOURCE", "-Wall", "-Wextra", "-Werror",
        "-I" + str(REPO_ROOT / "crates/ime-ffi/include"), str(source), str(library),
        "-Wl,-rpath," + str(frameworks), "-o", str(binary),
    ], check=True)
    report = {
        "product_root": str(product), "probe_source_sha256": digest(source),
        "ffi_sha256": digest(library),
        "product_manifest_sha256": digest(product / "ProductManifest.json"),
        "cases": [],
    }
    for mode in range(7):
        scenario = work / f"case-{mode}"
        scenario.mkdir(mode=0o700)
        with (scenario / "stdout.log").open("w") as stdout, (scenario / "stderr.log").open("w") as stderr:
            subprocess.run([
                str(binary), str(resources / "RimeData"), str(scenario), str(mode),
            ], stdout=stdout, stderr=stderr, check=True, timeout=60)
        database = scenario / "userdb.sqlite3"
        connection = sqlite3.connect(database.as_uri() + "?mode=ro", uri=True)
        try:
            events = connection.execute("SELECT COUNT(*) FROM selection_events").fetchone()[0]
            terms = connection.execute("SELECT COUNT(*) FROM user_terms").fetchone()[0]
            integrity = connection.execute("PRAGMA integrity_check").fetchall()
        finally:
            connection.close()
        expected = 1 if mode == 0 else 0
        if events != expected or terms != expected or integrity != [("ok",)]:
            raise RuntimeError(f"case {mode}: unexpected persistent learning or integrity")
        if list((scenario / "user").rglob("*.userdb*")):
            raise RuntimeError(f"case {mode}: unexpected Rime learning store")
        report["cases"].append({
            "case": mode, "selection_events": events, "user_terms": terms,
            "native": (scenario / "stdout.log").read_text().strip(),
        })
    if digest(library) != report["ffi_sha256"]:
        raise RuntimeError("FFI changed during the smoke")
    (work / "result.json").write_text(json.dumps(report, indent=2) + "\n")
    print("Bundled FFI privacy smoke passed: seven synthetic processes; no GUI.")


if __name__ == "__main__":
    main()

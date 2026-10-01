#!/usr/bin/env python3
"""Qualify copied payloads or build an independent recovery Installer; never install."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile


REPO = Path(__file__).resolve().parents[2]
TOOLS = Path(__file__).resolve().parent


def run(args: list[str | Path], *, env: dict[str, str] | None = None) -> None:
    subprocess.run([str(arg) for arg in args], cwd=REPO, env=env, check=True)


def canonical_directory(value: str) -> Path:
    path = Path(value)
    if not path.is_absolute() or not path.is_dir() or path.resolve() != path:
        raise ValueError("input must be an existing canonical absolute directory")
    return path


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def inventory(root: Path, *, identities: bool = True) -> dict[str, object]:
    result: dict[str, object] = {}
    for parent, directories, files in os.walk(root, followlinks=False):
        paths = [Path(parent)] + [Path(parent) / name for name in files]
        paths += [Path(parent) / name for name in directories if (Path(parent) / name).is_symlink()]
        directories[:] = [name for name in directories if not (Path(parent) / name).is_symlink()]
        for path in paths:
            meta = path.lstat()
            item: dict[str, object] = {"mode": stat.S_IMODE(meta.st_mode)}
            if identities:
                item.update(device=meta.st_dev, inode=meta.st_ino, owner=meta.st_uid,
                            group=meta.st_gid, links=meta.st_nlink)
            if path.is_symlink():
                item.update(kind="symlink", target=os.readlink(path))
            elif path.is_file():
                item.update(kind="file", size=meta.st_size, sha256=digest(path))
            elif path.is_dir():
                item.update(kind="directory")
            else:
                raise ValueError("payload contains an unsupported filesystem object")
            result[str(path.relative_to(root))] = item
    return dict(sorted(result.items()))


def write_new(path: Path, value: object) -> None:
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2)
        stream.write("\n")


def qualify(source: Path, target: Path) -> None:
    initial = {"source": inventory(source), "target": inventory(target)}
    root = Path(tempfile.mkdtemp(prefix="radishlex-recovery-payload-qualification-")).resolve()
    root.chmod(0o700)
    print(f"Recovery qualification evidence: {root}", flush=True)
    write_new(root / "input-inventory.json", initial)
    marker = root / "radishlex-upgrade-qualification.marker"
    marker.write_text("radishlex-upgrade-qualification-v1\n", encoding="utf-8")
    marker.chmod(0o600)
    try:
        for name, original in [("source", source), ("target", target)]:
            copy = root / f"{name}-payload"
            run(["/usr/bin/ditto", original, copy])
            copy.chmod(0o700)
            (copy / "Product").chmod(0o700)
        env = os.environ.copy()
        env.update(CARGO_NET_OFFLINE="true",
                   RADISHLEX_INSTALL_QUALIFICATION_ROOT=str(root),
                   RADISHLEX_INSTALL_QUALIFICATION_SOURCE_PAYLOAD=str(root / "source-payload"),
                   RADISHLEX_INSTALL_QUALIFICATION_TARGET_PAYLOAD=str(root / "target-payload"),
                   RADISHLEX_INSTALL_QUALIFICATION_SOURCE_PRODUCT=str(root / "source-payload/Product"),
                   RADISHLEX_INSTALL_QUALIFICATION_TARGET_PRODUCT=str(root / "target-payload/Product"))
        run(["cargo", "test", "--offline", "--locked", "-p",
             "radishlex-macos-product-install-coordinator", "--features", "qualification-harness",
             "--test", "product_recovery", "--", "--nocapture"], env=env)
    finally:
        final = {"source": inventory(source), "target": inventory(target)}
        write_new(root / "input-postflight.json", {"unchanged": final == initial})
        if final != initial:
            raise ValueError("input payload identity changed; preserve all evidence")
    write_new(root / "qualification-result.json", {"passed": True,
              "scope": "copied real payloads, synthetic home, no installed-product mutation"})


def build(payload: Path, output: Path) -> None:
    if not output.is_absolute() or output.resolve() != output or os.path.lexists(output):
        raise ValueError("output must be a new canonical absolute path; existing outputs are preserved")
    if output == payload or output in payload.parents or payload in output.parents:
        raise ValueError("output and input payload must not overlap")
    before = inventory(payload)
    content = inventory(payload, identities=False)
    run(["python3", TOOLS / "install_layout.py", "verify", "--payload-root", payload])
    output.mkdir(parents=True, mode=0o700)
    write_new(output / "input-inventory.json", before)
    env = os.environ.copy()
    env.update(CARGO_NET_OFFLINE="true", RADISHLEX_INSTALLER_PAYLOAD_ROOT=str(payload),
               RADISHLEX_INSTALLER_CODESIGN_IDENTITY="-")
    try:
        run([REPO / "platforms/macos-product/InstallerApp/build.sh"], env=env)
        installer = output / "RadishLex Installer.app"
        run(["/usr/bin/ditto", REPO / "target/macos-product/installer-app/RadishLex Installer.app", installer])
        embedded = installer / "Contents/Resources/InstallPayload"
        if inventory(embedded, identities=False) != content:
            raise ValueError("embedded payload is not the exact preserved input")
        manager = embedded / "Product/Components/radishlex_manager.app"
        input_method = embedded / "Product/Components/RadishLexInputMethod.app"
        identity = installer / "Contents/Resources/ReleaseIdentity.json"
        identity_args = ["--installer-bundle", installer, "--manager-bundle", manager,
                         "--input-method-bundle", input_method]
        run(["python3", TOOLS / "release_identity.py", "create", *identity_args, "--output", identity])
        run(["/usr/bin/codesign", "--force", "--sign", "-", "--timestamp=none", installer])
        run(["/usr/bin/codesign", "--verify", "--deep", "--strict", installer])
        run(["python3", TOOLS / "release_identity.py", "verify", *identity_args, "--identity", identity])
        if inventory(embedded, identities=False) != content:
            raise ValueError("sealing changed the embedded payload")
        commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=REPO, text=True).strip()
        clean = not subprocess.check_output(["git", "status", "--porcelain=v1"], cwd=REPO, text=True)
        write_new(output / "RecoveryBuild.json", {
            "format_version": 1, "source_commit": commit, "source_worktree_clean": clean,
            "installer_executable_sha256": digest(installer / "Contents/MacOS/RadishLex Installer"),
            "release_identity_sha256": digest(identity),
            "payload_manifest_sha256": digest(embedded / "InstallPayloadManifest.json"),
            "scope": "independent recovery Installer; no GUI launch or installation",
        })
    finally:
        after = inventory(payload)
        write_new(output / "input-postflight.json", {"unchanged": before == after})
        if before != after:
            raise ValueError("input payload changed; preserve the output and original input")
    print(f"Independent recovery Installer: {installer}", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    qualification = commands.add_parser("qualify")
    qualification.add_argument("--source-payload", required=True)
    qualification.add_argument("--target-payload", required=True)
    builder = commands.add_parser("build")
    builder.add_argument("--payload-root", required=True)
    builder.add_argument("--output", required=True)
    args = parser.parse_args()
    if args.command == "qualify":
        qualify(canonical_directory(args.source_payload), canonical_directory(args.target_payload))
    else:
        build(canonical_directory(args.payload_root), Path(args.output))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Prepare and execute the separately authorized Q1 path experiment; retain all evidence."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys

RELATIVE = {
    "home": "",
    "application_support": "Library/Application Support",
    "runtime_data": "Library/Application Support/RadishLex",
    "userdb": "Library/Application Support/RadishLex/userdb.sqlite3",
    "settings": "Library/Application Support/RadishLex/manager-settings.json",
    "rime": "Library/Application Support/RadishLex/Rime",
    "manager_bundle": "Applications/RadishLex Manager.app",
    "input_method_bundle": "Library/Input Methods/RadishLexInputMethod.app",
}
COMPILE = ["/usr/bin/clang", "-Wall", "-Wextra", "-Werror", "-fobjc-arc", "-fmodules",
           "-fmodules-cache-path=module-cache",
           "-framework", "Foundation", "source39-path-probe.m", "-o", "source39-path-probe"]


def validate_sample(value, modified):
    if (not isinstance(value, dict) or set(value) !=
            {"schema", "support_url_count", "gate_home_shape", "paths"}):
        raise ValueError("invalid_probe_shape")
    if (type(value["schema"]) is not int or value["schema"] != 1 or
            type(value["support_url_count"]) is not int or value["support_url_count"] != 1 or
            value["gate_home_shape"] is not True or not isinstance(value["paths"], dict) or
            set(value["paths"]) != set(RELATIVE)):
        raise ValueError("invalid_probe_coverage")
    row_keys = {"matches_expected", "in_synthetic_home", "same_user_home_shape",
                "classification", "relative_path"}
    for key, relative in RELATIVE.items():
        row = value["paths"][key]
        if not isinstance(row, dict) or set(row) != row_keys:
            raise ValueError("invalid_path_shape")
        if (row["matches_expected"] is not modified or
                row["in_synthetic_home"] is not modified or
                row["same_user_home_shape"] is not True or
                row["classification"] != ("synthetic_home" if modified else "real_user_domain") or
                row["relative_path"] != (relative if modified else None)):
            raise ValueError("path_resolution_rejected")


def private_directory(path, identity=None):
    info = path.lstat()
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or
            stat.S_IMODE(info.st_mode) != 0o700 or
            (identity is not None and (info.st_dev, info.st_ino) != identity)):
        raise ValueError("private_directory_changed")
    return info.st_dev, info.st_ino


def redact(raw):
    value = raw.decode("utf-8", errors="replace")
    candidates = [str(Path(__file__).resolve().parents[1])]
    candidates.extend(os.environ.get(key, "") for key in ("HOME", "home", "CODEX_HOME"))
    for candidate in sorted(set(candidates), key=len, reverse=True):
        if len(candidate) > 1:
            value = value.replace(candidate, "<redacted-path>")
    return re.sub(r"/(?:Users|home)/[^/\s]+", "<real_user_domain>", value)


def run_step(root, name, command, environment, identities):
    # Popen/communicate has no timeout, retry, cleanup, or implicit kill on exception.
    for path, identity in identities.items():
        private_directory(path, identity)
    process = subprocess.Popen(command, cwd=root, env=environment,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    (root / (name + ".pid")).write_text(str(process.pid) + "\n", encoding="utf-8")
    stdout, stderr = process.communicate()
    for path, identity in identities.items():
        private_directory(path, identity)
    (root / (name + ".stdout.log")).write_text(redact(stdout), encoding="utf-8")
    (root / (name + ".stderr.log")).write_text(redact(stderr), encoding="utf-8")
    if process.returncode != 0:
        raise ValueError(name + "_failed")
    return stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-root", required=True,
                        help="new direct /private/tmp/radishlex-source39-paths-<safe-name>")
    parser.add_argument("--execute", action="store_true", help="execute only after A1 authorization")
    args = parser.parse_args()
    if not args.execute:
        parser.error("--execute requires separate A1 authorization; no files created")
    if not re.fullmatch(r"/private/tmp/radishlex-source39-paths-[A-Za-z0-9][A-Za-z0-9_-]{0,63}",
                        args.run_root):
        parser.error("run root must match the documented new direct temporary path")
    root = Path(args.run_root)
    if (sys.platform != "darwin" or "CFFIXED_USER_HOME" in os.environ or
            os.path.realpath("/private/tmp") != "/private/tmp" or os.path.lexists(root) or
            not os.access("/usr/bin/clang", os.X_OK)):
        parser.error("preflight rejected: platform, inherited override, root, or compiler")
    source = Path(__file__).resolve().parent / "macos-product/source39-path-probe.m"
    source_bytes = source.read_bytes()
    root.mkdir(mode=0o700)
    root_identity = private_directory(root)
    result = {"schema": 1, "run_id": root.name, "status": "rejected", "q2": "blocked",
              "source_sha256": hashlib.sha256(source_bytes).hexdigest(),
              "command_plan": {"compile": COMPILE,
                               "compile_environment": "inherited_plus_TMPDIR=<run-root>/compiler-tmp",
                               "samples": [
                  {"name": "baseline", "argv": ["./source39-path-probe", "<run-root>/synthetic-home"],
                   "environment": "inherited_without_CFFIXED_USER_HOME"},
                  {"name": "cffixed", "argv": ["./source39-path-probe", "<run-root>/synthetic-home"],
                   "environment": "inherited_plus_CFFIXED_USER_HOME=<run-root>/synthetic-home"}]},
              "completed": []}
    try:
        synthetic_home = root / "synthetic-home"
        synthetic_home.mkdir(mode=0o700)
        cache = root / "module-cache"
        cache.mkdir(mode=0o700)
        temporary = root / "compiler-tmp"
        temporary.mkdir(mode=0o700)
        identities = {root: root_identity, synthetic_home: private_directory(synthetic_home),
                      cache: private_directory(cache), temporary: private_directory(temporary)}
        (root / "source39-path-probe.m").write_bytes(source_bytes)
        environment = os.environ.copy()
        compiler_environment = environment.copy()
        compiler_environment["TMPDIR"] = str(temporary)
        run_step(root, "compile", COMPILE, compiler_environment, identities)
        result["completed"].append("compile")
        for name, modified in (("baseline", False), ("cffixed", True)):
            child_environment = environment.copy()
            if modified:
                child_environment["CFFIXED_USER_HOME"] = str(synthetic_home)
            output = run_step(root, name, ["./source39-path-probe", str(synthetic_home)],
                              child_environment, identities)
            sample = json.loads(output)
            validate_sample(sample, modified)
            (root / (name + ".json")).write_text(
                json.dumps(sample, indent=2) + "\n", encoding="utf-8")
            result["completed"].append(name)
        result["status"] = "path_experiment_passed"
    except (OSError, ValueError) as error:
        # Only fixed rejection codes or exception class names reach the manifest/stdout.
        result["reason"] = str(error) if type(error) is ValueError else type(error).__name__
    private_directory(root, root_identity)
    (root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"status": result["status"], "q2": "blocked", "run_id": root.name}))
    return 0 if result["status"] == "path_experiment_passed" else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError):
        print("path experiment rejected; preserve any created evidence", file=sys.stderr)
        raise SystemExit(1)

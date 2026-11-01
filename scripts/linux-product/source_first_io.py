#!/usr/bin/env python3
"""Bounded reads and durable, no-replace publication for source-first evidence."""
from __future__ import annotations

import hashlib
import json
import os
import stat
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

from l6_maintenance_refresh_io import fsync_directory, rename_directory_no_replace


MAX_JSON = 1024 * 1024
MAX_ATTACHMENTS = 512
MAX_ATTACHMENT_BYTES = 16 * 1024 * 1024
MAX_PACKAGE = 512 * 1024 * 1024
MAX_ELF = 256 * 1024 * 1024


class SourceFirstError(ValueError):
    pass


def canonical(value: Any) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n").encode()


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise SourceFirstError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def parse_json(raw: bytes, *, require_canonical: bool = True) -> dict[str, Any]:
    if len(raw) > MAX_JSON:
        raise SourceFirstError("JSON exceeds 1 MiB")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=unique_object)
        if not isinstance(value, dict):
            raise SourceFirstError("JSON must be an object")
        encoded = canonical(value)
    except (UnicodeError, ValueError, RecursionError) as exc:
        raise SourceFirstError(f"invalid JSON: {exc}") from exc
    if require_canonical and encoded != raw:
        raise SourceFirstError("JSON is not canonical")
    return value


def relative_path(value: Any) -> str:
    if not isinstance(value, str) or not value or len(value.encode()) > 180:
        raise SourceFirstError("invalid relative path length")
    path = PurePosixPath(value)
    if (path.is_absolute() or path.as_posix() != value or
            any(part in {".", ".."} for part in path.parts) or
            any(c not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_.+/" for c in value)):
        raise SourceFirstError("unsafe relative path")
    return value


def real_directory(path: Path, mode: int | None = None) -> os.stat_result:
    if not path.is_absolute() or path.resolve() != path or path.is_symlink():
        raise SourceFirstError(f"directory path is not absolute and real: {path}")
    info = path.lstat()
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or
            (mode is not None and stat.S_IMODE(info.st_mode) != mode)):
        raise SourceFirstError(f"directory owner/mode/type differs: {path}")
    return info


def stat_identity(info: os.stat_result) -> tuple[int, ...]:
    return (info.st_dev, info.st_ino, info.st_uid, info.st_gid, info.st_mode,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


@dataclass(frozen=True)
class FileSnapshot:
    path: Path
    identity: tuple[int, ...]
    sha256: str
    size: int

    def record(self) -> dict[str, Any]:
        return {"filename": self.path.name, "sha256": self.sha256, "size": self.size}


def read_file(path: Path, *, limit: int = MAX_JSON, mode: int = 0o600,
              expected: dict[str, Any] | None = None,
              previous: FileSnapshot | None = None) -> tuple[bytes, FileSnapshot]:
    """Pin the descriptor and compare both pathname and descriptor across the read."""
    real_directory(path.parent)
    before = path.lstat()
    if (not stat.S_ISREG(before.st_mode) or before.st_uid != os.getuid() or
            before.st_nlink != 1 or stat.S_IMODE(before.st_mode) != mode or
            before.st_size <= 0 or before.st_size > limit):
        raise SourceFirstError(f"unsafe file owner/mode/link/size: {path}")
    identity = stat_identity(before)
    if previous is not None and (previous.path != path or previous.identity != identity):
        raise SourceFirstError(f"input identity replaced: {path}")
    descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
    try:
        if stat_identity(os.fstat(descriptor)) != identity:
            raise SourceFirstError(f"file changed before read: {path}")
        chunks: list[bytes] = []
        remaining = limit + 1
        while remaining:
            chunk = os.read(descriptor, min(1024 * 1024, remaining))
            if not chunk:
                break
            chunks.append(chunk)
            remaining -= len(chunk)
        data = b"".join(chunks)
        if (len(data) != before.st_size or len(data) > limit or
                stat_identity(os.fstat(descriptor)) != identity or
                stat_identity(path.lstat()) != identity):
            raise SourceFirstError(f"file changed during read: {path}")
    finally:
        os.close(descriptor)
    snapshot = FileSnapshot(path, identity, digest(data), len(data))
    if expected is not None and snapshot.record() != expected:
        raise SourceFirstError(f"file bytes/filename differ: {path}")
    if previous is not None and snapshot.sha256 != previous.sha256:
        raise SourceFirstError(f"input bytes changed: {path}")
    return data, snapshot


def load_json(path: Path, *, mode: int = 0o600) -> tuple[dict[str, Any], FileSnapshot]:
    raw, snapshot = read_file(path, mode=mode)
    return parse_json(raw), snapshot


def write_exclusive(path: Path, data: bytes, mode: int = 0o600) -> FileSnapshot:
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL |
                         os.O_CLOEXEC | os.O_NOFOLLOW, mode)
    try:
        os.fchmod(descriptor, mode)
        view = memoryview(data)
        while view:
            count = os.write(descriptor, view)
            if count <= 0:
                raise SourceFirstError("short write")
            view = view[count:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)
    _, snapshot = read_file(path, limit=max(len(data), 1), mode=mode)
    if snapshot.sha256 != digest(data):
        raise SourceFirstError("exclusive copy readback differs")
    return snapshot


def new_private_root(path: Path) -> None:
    real_directory(path.parent)
    path.mkdir(mode=0o700)  # Exclusive, including dangling symlinks.
    path.chmod(0o700)
    fsync_directory(path.parent)


def check_tree(root: Path, files: dict[str, FileSnapshot], *, mode: int | None) -> None:
    real_directory(root, 0o700)
    expected_dirs = {"."}
    for relative in files:
        relative_path(relative)
        expected_dirs.update(p.as_posix() for p in PurePosixPath(relative).parents)
    found_files: set[str] = set()
    found_dirs = {"."}
    for parent, dirs, names in os.walk(root, followlinks=False):
        for name in dirs:
            directory = Path(parent) / name
            real_directory(directory, 0o700)
            found_dirs.add(directory.relative_to(root).as_posix())
        for name in names:
            path = Path(parent) / name
            relative = path.relative_to(root).as_posix()
            if relative not in files:
                raise SourceFirstError(f"unknown evidence file: {relative}")
            file_mode = mode if mode is not None else stat.S_IMODE(files[relative].identity[4])
            read_file(path, limit=max(files[relative].size, 1), mode=file_mode,
                      previous=files[relative])
            found_files.add(relative)
    if found_files != set(files) or found_dirs != expected_dirs:
        raise SourceFirstError("evidence inventory is not closed")


def publish(root: Path, output: Path, files: dict[str, FileSnapshot], *, mode: int | None) -> Path:
    """Failure leaves staging or published output intact; never cleanup or retry."""
    root_info = real_directory(root, 0o700)
    parent_info = real_directory(output.parent)
    if (root == output or output.exists() or output.is_symlink() or
            root_info.st_dev != parent_info.st_dev or root in output.parents):
        raise SourceFirstError("publication requires an absent same-volume sibling destination")
    check_tree(root, files, mode=mode)
    directories = {Path(parent): stat_identity(Path(parent).lstat())
                   for parent, _, _ in os.walk(root, followlinks=False)}
    for relative, snapshot in files.items():
        descriptor = os.open(root / relative, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
        try:
            if stat_identity(os.fstat(descriptor)) != snapshot.identity:
                raise SourceFirstError("publication file replaced before synchronization")
            os.fsync(descriptor)
            if stat_identity(os.fstat(descriptor)) != snapshot.identity:
                raise SourceFirstError("publication file changed during synchronization")
        finally:
            os.close(descriptor)
    for parent, _, _ in os.walk(root, topdown=False):
        fsync_directory(Path(parent))
    # Re-read after all synchronization; reject same-byte inode/metadata replacement.
    check_tree(root, files, mode=mode)
    if stat_identity(root.lstat()) != stat_identity(root_info):
        raise SourceFirstError("publication root changed")
    if any(stat_identity(path.lstat()) != identity for path, identity in directories.items()):
        raise SourceFirstError("publication directory replaced")
    rename_directory_no_replace(root, output)
    fsync_directory(output.parent)
    if root.parent != output.parent:
        fsync_directory(root.parent)
    relocated = {name: FileSnapshot(output / name, item.identity, item.sha256, item.size)
                 for name, item in files.items()}
    check_tree(output, relocated, mode=mode)
    return output

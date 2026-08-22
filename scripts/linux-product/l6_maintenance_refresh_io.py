#!/usr/bin/env python3
from __future__ import annotations

import ctypes
import os
import sys
from pathlib import Path


class L6MaintenanceRefreshError(RuntimeError):
    pass


def require_absent_output(path: Path, label: str) -> Path:
    if not path.is_absolute() or path.exists() or path.is_symlink():
        raise L6MaintenanceRefreshError(f"{label} must be an absent absolute path")
    parent = path.parent
    if parent.is_symlink() or not parent.is_dir() or parent.resolve() != parent:
        raise L6MaintenanceRefreshError(
            f"{label} parent must be an existing real directory"
        )
    return path


def write_exclusive_file(path: Path, data: bytes, mode: int) -> None:
    flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    try:
        descriptor = os.open(path, flags, mode)
    except OSError as exc:
        raise L6MaintenanceRefreshError(
            f"cannot create maintenance refresh file: {exc}"
        ) from exc
    try:
        os.fchmod(descriptor, mode)
        view = memoryview(data)
        while view:
            written = os.write(descriptor, view)
            if written <= 0:
                raise L6MaintenanceRefreshError(
                    "cannot complete maintenance refresh file copy"
                )
            view = view[written:]
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def fsync_directory(path: Path) -> None:
    descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def rename_directory_no_replace(source: Path, destination: Path) -> None:
    libc = ctypes.CDLL(None, use_errno=True)
    if sys.platform.startswith("linux"):
        rename = getattr(libc, "renameat2", None)
        current_working_directory = -100
        no_replace = 1  # RENAME_NOREPLACE
    elif sys.platform == "darwin":
        rename = getattr(libc, "renameatx_np", None)
        current_working_directory = -2
        no_replace = 0x00000004  # RENAME_EXCL
    else:
        rename = None
        current_working_directory = 0
        no_replace = 0
    if rename is None:
        raise L6MaintenanceRefreshError(
            "atomic no-replace rename is unavailable on this platform"
        )
    rename.argtypes = (
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_int,
        ctypes.c_char_p,
        ctypes.c_uint,
    )
    rename.restype = ctypes.c_int
    result = rename(
        current_working_directory,
        os.fsencode(source),
        current_working_directory,
        os.fsencode(destination),
        no_replace,
    )
    if result != 0:
        error_number = ctypes.get_errno()
        raise L6MaintenanceRefreshError(
            "cannot atomically publish maintenance refresh without overwrite: "
            f"{os.strerror(error_number)}"
        )

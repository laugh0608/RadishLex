#!/usr/bin/env python3
"""Single-attempt build command execution with a shared deadline and retained logs."""
from __future__ import annotations

import os
import selectors
import signal
import subprocess
import time
from pathlib import Path

from source_first_io import SourceFirstError, canonical, write_exclusive


MAX_LOG_BYTES = 8 * 1024 * 1024


class BuildCommands:
    def __init__(self, root: Path, budget_seconds: int, environment: dict[str, str]) -> None:
        if type(budget_seconds) is not int or not 1 <= budget_seconds <= 7200:
            raise SourceFirstError("explicit build budget must be 1..7200 seconds")
        self.root = root
        self.deadline = time.monotonic() + budget_seconds
        self.environment = environment
        self.sequence = 0

    def run(self, arguments: list[str], cwd: Path, label: str) -> bytes:
        remaining = self.deadline - time.monotonic()
        if remaining <= 0:
            raise SourceFirstError("build deadline exhausted before next command")
        self.sequence += 1
        prefix = f"{self.sequence:02d}-{label}"
        write_exclusive(self.root / (prefix + ".command.json"),
                        canonical({"arguments": arguments, "cwd": str(cwd)}))
        log_path = self.root / (prefix + ".log")
        descriptor = os.open(log_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        child = None
        selector = selectors.DefaultSelector()
        output = bytearray()
        try:
            child = subprocess.Popen(arguments, cwd=cwd, env=self.environment,
                                     stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                     stderr=subprocess.STDOUT, start_new_session=True,
                                     close_fds=True)
            if child.stdout is None:
                raise SourceFirstError("command output pipe unavailable")
            os.set_blocking(child.stdout.fileno(), False)
            selector.register(child.stdout, selectors.EVENT_READ)
            while selector.get_map():
                remaining = self.deadline - time.monotonic()
                if remaining <= 0:
                    raise SourceFirstError(f"command deadline exceeded: {label}")
                for key, _ in selector.select(min(remaining, 0.1)):
                    chunk = os.read(key.fileobj.fileno(), 65536)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    if len(output) + len(chunk) > MAX_LOG_BYTES:
                        raise SourceFirstError(f"command log exceeds 8 MiB: {label}")
                    output.extend(chunk)
                    view = memoryview(chunk)
                    while view:
                        count = os.write(descriptor, view)
                        if count <= 0:
                            raise SourceFirstError("command log short write")
                        view = view[count:]
            code = child.wait(timeout=max(0.001, self.deadline - time.monotonic()))
            if code:
                raise SourceFirstError(f"command failed once: {label}, exit={code}")
            os.fsync(descriptor)
            return bytes(output)
        except BaseException:
            if child is not None and child.returncode is None:
                # A future build invocation must explicitly authorize this process-group cleanup.
                # It only addresses the session created above, including any remaining descendants.
                try:
                    os.killpg(child.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()
            raise
        finally:
            selector.close()
            if child is not None and child.stdout is not None:
                child.stdout.close()
            os.close(descriptor)

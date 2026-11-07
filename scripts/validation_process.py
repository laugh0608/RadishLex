"""Serial validation commands with live timing and owned process-group cleanup.

No result cache or resume: every successful call represents a new execution.
Only the process group created for this command may receive a signal.
"""
from __future__ import annotations

import os
import shlex
import signal
import subprocess
import tempfile
import time
from pathlib import Path


class ValidationInterrupted(Exception):
    def __init__(self, signum: int):
        self.signum = signum


def _interrupt(signum: int, _frame: object) -> None:
    raise ValidationInterrupted(signum)


def _signal_group(process: subprocess.Popen, signum: int) -> bool:
    if os.name != "posix":
        if process.poll() is not None:
            return False
        process.terminate() if signum == signal.SIGTERM else process.kill()
        return True
    try:
        os.killpg(process.pid, signum)
        return True
    except ProcessLookupError:
        return False


def _cleanup(process: subprocess.Popen) -> None:
    # Also signal descendants after their original parent exited. A normal
    # command must not leave a server running in its private process group.
    if _signal_group(process, signal.SIGTERM):
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            pass
        if os.name == "posix":
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline:
                try:
                    os.killpg(process.pid, 0)
                except ProcessLookupError:
                    break
                time.sleep(0.05)
        _signal_group(process, getattr(signal, "SIGKILL", signal.SIGTERM))
    process.wait()


def run_command(command: list[str], cwd: Path, *, capture: bool = False) -> str | None:
    label = shlex.join(command)
    started = time.monotonic()
    print(f"[START] {label}", flush=True)
    handlers = {s: signal.getsignal(s) for s in (signal.SIGINT, signal.SIGTERM)}
    process = None
    output = tempfile.TemporaryFile(mode="w+t", encoding="utf-8") if capture else None
    outcome = "incomplete"
    try:
        for signum in handlers:
            signal.signal(signum, _interrupt)
        process = subprocess.Popen(command, cwd=cwd, stdout=output, start_new_session=os.name == "posix")
        while True:
            try:
                code = process.wait(timeout=30)
                break
            except subprocess.TimeoutExpired:
                print(f"[RUNNING {time.monotonic() - started:.0f}s] {label}", flush=True)
        if code != 0:
            outcome = f"failed ({code})"
            raise SystemExit(code if code > 0 else 128 - code)
        outcome = "passed"
        if output is not None:
            output.seek(0)
            return output.read()
    except FileNotFoundError as exc:
        raise SystemExit(f"{command[0]} is required for validation") from exc
    except ValidationInterrupted as exc:
        outcome = f"interrupted ({exc.signum}); not passed"
        raise SystemExit(128 + exc.signum) from exc
    finally:
        # A repeated interrupt must not prevent cleanup of this owned group.
        for signum in handlers:
            signal.signal(signum, signal.SIG_IGN)
        try:
            if process is not None:
                _cleanup(process)
        finally:
            if output is not None:
                output.close()
            for signum, handler in handlers.items():
                signal.signal(signum, handler)
        print(f"[END {time.monotonic() - started:.2f}s; {outcome}] {label}", flush=True)

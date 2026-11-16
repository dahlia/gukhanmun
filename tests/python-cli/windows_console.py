# Gukhanmun: Tests native and installed console control delivery on Windows.
# Copyright (C) 2026  Hong Minhee
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

"""Broadcast in a fresh console; never confuse process groups with Ctrl-C."""

import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def broadcast(executable, event):
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.DWORD)
    kernel.SetConsoleCtrlHandler.argtypes = [callback_type, wintypes.BOOL]
    kernel.SetConsoleCtrlHandler.restype = wintypes.BOOL
    kernel.GenerateConsoleCtrlEvent.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.GenerateConsoleCtrlEvent.restype = wintypes.BOOL
    # A real no-op callback, not SetConsoleCtrlHandler(NULL, TRUE), which
    # changes the inherited ignore flag. Keep it alive through child reap.
    callback = callback_type(lambda _: True)
    if not kernel.SetConsoleCtrlHandler(callback, True):
        raise ctypes.WinError(ctypes.get_last_error())
    child = subprocess.Popen([executable], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    try:
        time.sleep(2)
        if child.poll() is not None:
            raise RuntimeError("console target exited before control delivery")
        # This sender and target share this console. Group 0 broadcasts both
        # Ctrl-C and Ctrl-Break; no CREATE_NEW_PROCESS_GROUP disables Ctrl-C.
        if not kernel.GenerateConsoleCtrlEvent(event, 0):
            raise ctypes.WinError(ctypes.get_last_error())
        # communicate() closes stdin, which can let the converter exit on EOF
        # before Windows dispatches the control event to its handler.
        child.wait(timeout=20)
        child.communicate()
        if child.returncode & 0xffffffff != 0xC000013A:
            raise RuntimeError(f"control event {event} produced {child.returncode!r}")
        return child.returncode & 0xffffffff
    finally:
        if child.poll() is None:
            child.kill()
            child.communicate()
        kernel.SetConsoleCtrlHandler(callback, False)


def main():
    if os.name != "nt":
        raise RuntimeError("Windows console tests require a native Windows host")
    executable = str(Path(sys.argv[1]).resolve(strict=True))
    if len(sys.argv) == 3:
        print(json.dumps({"status": broadcast(executable, int(sys.argv[2]))}))
        return
    result = {}
    for event, name in ((0, "ctrl_c"), (1, "ctrl_break")):
        child = subprocess.run(
            [sys.executable, "-I", str(Path(__file__).resolve()), executable, str(event)],
            creationflags=subprocess.CREATE_NEW_CONSOLE, capture_output=True, timeout=40,
        )
        if child.returncode:
            raise RuntimeError(f"console allocation/control positive test failed: {child.stderr.decode(errors='replace')}")
        result[name] = json.loads(child.stdout)["status"]
    print(json.dumps(result))


if __name__ == "__main__":
    main()

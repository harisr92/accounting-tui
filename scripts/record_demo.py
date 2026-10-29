#!/usr/bin/env python3
"""Records the README's demo GIF: compter on the demo business, driven by the keys below.

Runs `asciinema rec` in a pseudo-terminal of a fixed size and types each key with a pause,
so the recording is the real app reacting, then renders it with `agg`. Run it from anywhere
after a UI change; it needs asciinema 3 and agg on the PATH:

    python3 scripts/record_demo.py
"""

import fcntl
import os
import pty
import select
import shutil
import struct
import subprocess
import sys
import termios
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "crates/compter-tui/screenshots"
COLS, ROWS = 120, 32

# One Dark, as the SVG screenshots draw the dark theme: background, foreground, 16 ANSI colours
THEME = ",".join([
    "1e2127", "abb2bf",
    "3f4451", "e06c75", "98c379", "e5c07b", "61afef", "c678dd", "56b6c2", "abb2bf",
    "5c6370", "e9969d", "b5e890", "f0d197", "8dc5f4", "d6a4e8", "8ad0d8", "ffffff",
])

ESC, ENTER, TAB = "\x1b", "\r", "\t"

# (keys, seconds to look at what they show). Each string is typed one key at a time.
TOUR = [
    ("", 2.5),            # the dashboard: this year's figures and who still owes
    ("2", 1.2),           # projects
    ("jjj", 1.5),         # Website Revamp, with its milestones
    (TAB, 0.6),           # into the milestones
    ("jj", 1.2),          # Launch, still pending
    ("i", 1.2),           # invoice it: a discount first
    ("10%", 1.0),
    (ENTER, 3.0),         # the invoice it would issue, GST worked out
    ("y", 2.5),           # issued
    ("3", 2.5),           # the new invoice heads the list
    ("5", 3.0),           # the year's reports: 44ADA, GST by quarter, TDS by client
    ("?", 2.5),           # every key, in the help
    (ESC, 1.0),
    ("q", 0.5),
]


def main() -> int:
    for tool in ("asciinema", "agg", "cargo"):
        if shutil.which(tool) is None:
            print(f"{tool} is not on the PATH", file=sys.stderr)
            return 1
    subprocess.run(
        ["cargo", "build", "--release", "-q", "-p", "compter-tui"], cwd=ROOT, check=True
    )
    OUT.mkdir(parents=True, exist_ok=True)
    cast = OUT / "demo.cast"
    gif = OUT / "demo.gif"
    record(ROOT / "target/release/compter-tui", cast)
    subprocess.run(
        [
            "agg", "--theme", THEME, "--font-size", "16", "--idle-time-limit", "3",
            "--last-frame-duration", "1", str(cast), str(gif),
        ],
        check=True,
    )
    cast.unlink()
    print(f"wrote {gif.relative_to(ROOT)}")
    return 0


def record(binary: Path, cast: Path) -> None:
    """Record `binary --demo` into `cast`, typing the TOUR into it"""
    pid, master = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execvp("asciinema", [
            "asciinema", "rec", "--quiet", "--overwrite",
            "--window-size", f"{COLS}x{ROWS}",
            "--command", f"{binary} --demo",
            str(cast),
        ])
    fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
    wait(master, 1.5)  # the app starting
    for keys, look in TOUR:
        for key in keys:
            os.write(master, key.encode())
            wait(master, 0.18)
        wait(master, look)
    # Until asciinema has written the cast and exited
    while drain(master, 0.2):
        pass
    os.waitpid(pid, 0)


def wait(master: int, seconds: float) -> None:
    """Keep reading the app's output for `seconds`, so it never blocks on a full terminal"""
    end = time.monotonic() + seconds
    while (left := end - time.monotonic()) > 0:
        drain(master, left)


def drain(master: int, timeout: float) -> bool:
    """Read what the terminal has; false once the recording has ended"""
    ready, _, _ = select.select([master], [], [], timeout)
    if not ready:
        return True
    try:
        return bool(os.read(master, 65536))
    except OSError:
        return False


if __name__ == "__main__":
    sys.exit(main())

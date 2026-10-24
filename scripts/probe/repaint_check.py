#!/usr/bin/env python3
"""Scratch harness for ci/idle-vblank-ab. Not for main.

Runs `idle_probe check` and, for every color the probe switches to while its
window is idle, screenshots the window centre and checks the color reached the
screen.

    repaint_check.py --label linux-head path/to/idle_probe
"""

import argparse
import os
import subprocess
import sys
import time

from PIL import ImageGrab


def grab(x, y):
    if sys.platform == "win32":
        image = ImageGrab.grab(bbox=(x, y, x + 1, y + 1), all_screens=True)
    else:
        image = ImageGrab.grab(bbox=(x, y, x + 1, y + 1), xdisplay=os.environ["DISPLAY"])
    return image.convert("RGB").getpixel((0, 0))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--label", required=True)
    parser.add_argument("--summary", required=True)
    parser.add_argument("binary")
    args = parser.parse_args()

    process = subprocess.Popen(
        [args.binary, "check"], stdout=subprocess.PIPE, text=True, bufsize=1
    )
    lines, failures = [], []
    for line in process.stdout:
        line = line.strip()
        print(f"{args.label}: {line}", flush=True)
        lines.append(line)
        if not line.startswith("COLOR "):
            continue
        fields = line.split()
        color = int(fields[1], 16)
        values = dict(zip(fields[2::2], fields[3::2]))
        scale = float(values["scale"])
        x = int((float(values["x"]) + float(values["w"]) / 2) * scale)
        y = int((float(values["y"]) + float(values["h"]) / 2) * scale)
        time.sleep(0.5)
        try:
            pixel = grab(x, y)
        except Exception as error:  # noqa: BLE001
            failures.append(f"screenshot failed: {error}")
            continue
        expected = ((color >> 16) & 255, (color >> 8) & 255, color & 255)
        ok = all(abs(a - b) <= 24 for a, b in zip(pixel, expected))
        note = f"pixel {pixel} expected {expected} {'ok' if ok else 'MISMATCH'}"
        print(f"{args.label}: {note}", flush=True)
        lines.append(note)
        if not ok:
            failures.append(note)
    code = process.wait()
    passed = code == 0 and not failures and any(line == "CHECK OK" for line in lines)
    with open(args.summary, "a") as out:
        out.write(f"### repaint check {args.label}: {'PASS' if passed else 'FAIL'}\n\n```\n")
        out.write("\n".join(lines) + "\n```\n\n")
    sys.exit(0 if passed else 1)


if __name__ == "__main__":
    main()

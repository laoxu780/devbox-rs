#!/usr/bin/env python3
"""Generate a simple DevBox app icon (PNG + ICO) without external Python deps."""

import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RES = ROOT / "resources"
RES.mkdir(exist_ok=True)

SIZE = 1024
DIGIT = [
    "11111100",
    "11111110",
    "11000110",
    "11000110",
    "11000110",
    "11000110",
    "11000110",
    "11111110",
    "11111100",
]


def lerp(a, b, t):
    return a + (b - a) * t


def pixel(x, y):
    # Rounded-rectangle mask.
    radius = 220
    cx = min(max(x, radius), SIZE - radius)
    cy = min(max(y, radius), SIZE - radius)
    inside = (x - cx) ** 2 + (y - cy) ** 2 <= radius ** 2
    if not inside:
        return (0, 0, 0, 0)

    t = (x + y) / (2 * SIZE)
    r = int(lerp(0x6D, 0x8F, t))
    g = int(lerp(0x5B, 0x80, t))
    b = int(lerp(0xD0, 0xF2, t))

    cell = 96
    grid_w = len(DIGIT[0]) * cell
    grid_h = len(DIGIT) * cell
    gx = (SIZE - grid_w) // 2
    gy = (SIZE - grid_h) // 2
    col = (x - gx) // cell
    row = (y - gy) // cell
    if 0 <= row < len(DIGIT) and 0 <= col < len(DIGIT[0]) and DIGIT[row][col] == "1":
        return (255, 255, 255, 255)
    return (r, g, b, 255)


def png_bytes(size, pixels):
    raw = bytearray()
    for y in range(size):
        raw.append(0)
        for x in range(size):
            raw.extend(pixels[(x, y)])

    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def ico_bytes(png):
    header = struct.pack("<HHH", 0, 1, 1)
    entry = struct.pack("<BBBBHHII", 0, 0, 0, 0, 1, 32, len(png), 22)
    return header + entry + png


pixels = {(x, y): pixel(x, y) for y in range(SIZE) for x in range(SIZE)}
png = png_bytes(SIZE, pixels)
(RES / "AppIcon.png").write_bytes(png)
(RES / "AppIcon.ico").write_bytes(ico_bytes(png))
print(f"wrote {RES / 'AppIcon.png'} and {RES / 'AppIcon.ico'}")

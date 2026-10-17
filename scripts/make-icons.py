"""Write the dashboard icons. No third-party modules: a solid accent tile with a notch."""

import struct
import zlib
from pathlib import Path

ACCENT = (0xE0, 0x7E, 0xF5, 255)
BG = (0x1A, 0x1D, 0x24, 255)


def png(size: int) -> bytes:
    rows = []
    for y in range(size):
        row = bytearray([0])
        for x in range(size):
            inset = size * 0.16
            radius = size * 0.22
            if inside_round(x, y, inset, size - inset, radius):
                row.extend(ACCENT)
            else:
                row.extend(BG)
        rows.append(bytes(row))
    raw = b"".join(rows)
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def inside_round(x, y, left, right, radius):
    if x < left or y < left or x >= right or y >= right:
        return False
    corners = (
        (left + radius, left + radius),
        (right - radius, left + radius),
        (left + radius, right - radius),
        (right - radius, right - radius),
    )
    for cx, cy in corners:
        dx, dy = x - cx, y - cy
        if abs(dx) <= radius and abs(dy) <= radius and dx * dx + dy * dy > radius * radius:
            # only the outer quarter of each corner
            if (x < cx) == (cx < (left + right) / 2) and (y < cy) == (cy < (left + right) / 2):
                return False
    return True


out = Path(__file__).resolve().parents[1] / "contrib" / "icons"
out.mkdir(parents=True, exist_ok=True)
for size in (48, 128, 256):
    (out / f"frame-audio-mixer-{size}.png").write_bytes(png(size))
print(out)

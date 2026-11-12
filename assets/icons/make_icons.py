#!/usr/bin/env python3
"""Generate the OpenCrosshair app icon set.

Outputs (next to this script):
  icon.png  256x256 RGBA
  icon.ico  multi-size ICO: 16, 32, 48, 64, 128, 256

Design: dark rounded square (#101318), blue (#3B82F6) crosshair with a centre gap,
cyan (#22D3EE) centre dot. Geometry is defined as signed distance functions so the
same drawing works with Pillow (if installed) or with the pure-python fallback writer.
Edges are anti-aliased from the SDF (coverage = clamp(0.5 - distance / pixel)).

Usage: python3 make_icons.py
"""
import math
import os
import struct
import sys
import zlib

BG = (0x10, 0x13, 0x18)
BLUE = (0x3B, 0x82, 0xF6)
CYAN = (0x22, 0xD3, 0xEE)

ICO_SIZES = [16, 32, 48, 64, 128, 256]
PNG_SIZE = 256
OUT_DIR = os.path.dirname(os.path.abspath(__file__))


def _seg_dist(px, py, ax, ay, bx, by):
    vx, vy = bx - ax, by - ay
    wx, wy = px - ax, py - ay
    t = (wx * vx + wy * vy) / (vx * vx + vy * vy)
    t = min(1.0, max(0.0, t))
    dx, dy = px - (ax + t * vx), py - (ay + t * vy)
    return math.hypot(dx, dy)


def _rounded_box_sd(px, py, half, radius):
    qx = abs(px) - half + radius
    qy = abs(py) - half + radius
    outside = math.hypot(max(qx, 0.0), max(qy, 0.0))
    inside = min(max(qx, qy), 0.0)
    return outside + inside - radius


def _coverage(sd, pixel):
    c = 0.5 - sd / pixel
    return 0.0 if c <= 0.0 else (1.0 if c >= 1.0 else c)


def render_rgba(size):
    """Return bytes of a size x size RGBA image (non-premultiplied)."""
    s = float(size)
    pixel = 1.0  # one output pixel in output units
    cx = cy = s / 2.0
    half = s / 2.0
    radius = 0.22 * s
    arm_outer = 0.40 * s
    arm_inner = 0.12 * s
    thick = max(0.06 * s, 1.0)
    dot_r = 0.075 * s
    ring_r = 0.30 * s
    ring_w = max(0.045 * s, 0.9)
    out = bytearray(size * size * 4)
    for j in range(size):
        for i in range(size):
            px = i + 0.5 - cx
            py = j + 0.5 - cy
            # Rounded square mask.
            a_bg = _coverage(_rounded_box_sd(px, py, half - 0.5, radius), pixel)
            if a_bg <= 0.0:
                continue  # fully transparent corner
            # Crosshair: four arms with a gap around the centre, plus a thin ring.
            d_arms = math.inf
            for (ax, ay, bx, by) in (
                (-arm_outer, 0, -arm_inner, 0), (arm_inner, 0, arm_outer, 0),
                (0, -arm_outer, 0, -arm_inner), (0, arm_inner, 0, arm_outer),
            ):
                d_arms = min(d_arms, _seg_dist(px, py, ax, ay, bx, by) - thick / 2.0)
            d_ring = abs(math.hypot(px, py) - ring_r) - ring_w / 2.0
            a_blue = _coverage(min(d_arms, d_ring), pixel)
            a_dot = _coverage(math.hypot(px, py) - dot_r, pixel)
            r, g, b = BG
            # Composite blue over background, then cyan dot over that.
            r, g, b = (r + (BLUE[0] - r) * a_blue, g + (BLUE[1] - g) * a_blue, b + (BLUE[2] - b) * a_blue)
            r, g, b = (r + (CYAN[0] - r) * a_dot, g + (CYAN[1] - g) * a_dot, b + (CYAN[2] - b) * a_dot)
            o = (j * size + i) * 4
            out[o] = int(round(r))
            out[o + 1] = int(round(g))
            out[o + 2] = int(round(b))
            out[o + 3] = int(round(255 * a_bg))
    return bytes(out)


# ---------- pure-python PNG / ICO writers (fallback when Pillow is missing) ----------

def _png_chunk(tag, data):
    crc = zlib.crc32(tag + data) & 0xFFFFFFFF
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)


def encode_png(rgba, w, h):
    raw = bytearray()
    stride = w * 4
    for y in range(h):
        raw.append(0)  # filter: none
        raw += rgba[y * stride:(y + 1) * stride]
    sig = b"\x89PNG\r\n\x1a\n"
    ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
    return sig + _png_chunk(b"IHDR", ihdr) + _png_chunk(b"IDAT", zlib.compress(bytes(raw), 9)) + _png_chunk(b"IEND", b"")


def encode_ico(entries):
    """entries: list of (size, png_bytes). PNG-compressed ICO entries (Windows Vista+)."""
    header = struct.pack("<HHH", 0, 1, len(entries))
    offset = 6 + 16 * len(entries)
    directory = b""
    blobs = b""
    for size, png in entries:
        wh = 0 if size >= 256 else size  # 0 encodes 256
        directory += struct.pack("<BBBBHHII", wh, wh, 0, 0, 1, 32, len(png), offset)
        offset += len(png)
        blobs += png
    return header + directory + blobs


def write_with_pillow(path_png, path_ico, rgba_for):
    from PIL import Image  # noqa: WPS433 (optional dependency)
    base = Image.frombytes("RGBA", (PNG_SIZE, PNG_SIZE), rgba_for(PNG_SIZE))
    base.save(path_png, format="PNG", optimize=True)
    images = [Image.frombytes("RGBA", (s, s), rgba_for(s)) for s in ICO_SIZES if s != ICO_SIZES[-1]]
    base.save(path_ico, format="ICO", sizes=[(s, s) for s in ICO_SIZES], append_images=images)


def write_pure(path_png, path_ico, rgba_for):
    with open(path_png, "wb") as f:
        f.write(encode_png(rgba_for(PNG_SIZE), PNG_SIZE, PNG_SIZE))
    entries = [(s, encode_png(rgba_for(s), s, s)) for s in ICO_SIZES]
    with open(path_ico, "wb") as f:
        f.write(encode_ico(entries))


def main():
    png_path = os.path.join(OUT_DIR, "icon.png")
    ico_path = os.path.join(OUT_DIR, "icon.ico")
    try:
        import PIL  # noqa: F401
        have_pillow = True
    except ImportError:
        have_pillow = False
    cache = {}

    def rgba_for(size):
        if size not in cache:
            cache[size] = render_rgba(size)
        return cache[size]

    if have_pillow:
        write_with_pillow(png_path, ico_path, rgba_for)
        backend = "Pillow"
    else:
        write_pure(png_path, ico_path, rgba_for)
        backend = "pure-python"
    print(f"wrote {png_path} and {ico_path} using {backend} backend", file=sys.stderr)


if __name__ == "__main__":
    main()

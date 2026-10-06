#!/usr/bin/env python3
"""Build Race Refinery brand assets: SVG sources + PNG/ICO exports."""

from __future__ import annotations

import io
import struct
import subprocess
import zlib
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
BRAND = ROOT / "brand"
PUBLIC = ROOT / "public" / "brand"
ICONS = ROOT / "src-tauri" / "icons"
ARTIFACTS = Path("/opt/cursor/artifacts/assets")

ACCENT = "#4AA3FF"
ACCENT_STRONG = "#1F6FEB"
APEX = "#B8E0FF"
BG = "#0B0E14"
SURFACE = "#12161F"
TEXT = "#E6EDF3"
MUTED = "#8B95A5"


def mark_paths(cx: float = 256, cy: float = 268, scale: float = 1.0) -> str:
    """Canonical apex + telemetry bars mark, centered in a 512 viewBox by default."""

    def p(x: float, y: float) -> str:
        return f"{cx + (x - 256) * scale:.2f},{cy + (y - 256) * scale:.2f}"

    # Racing line through an apex, then refined into rising telemetry bars.
    stroke = 36 * scale
    path = (
        f"M {p(96, 214)} "
        f"C {p(128, 214)} {p(148, 214)} {p(168, 230)} "
        f"C {p(188, 246)} {p(198, 278)} {p(218, 300)} "
        f"C {p(238, 322)} {p(262, 330)} {p(286, 312)} "
        f"L {p(330, 268)} "
        f"L {p(352, 292)} "
        f"L {p(382, 248)}"
    )
    bars = []
    bar_w = 22 * scale
    bar_gap = 18 * scale
    heights = [70, 110, 150]
    base_x = 400
    base_y = 330
    for i, h in enumerate(heights):
        x = base_x + i * (bar_w + bar_gap)
        y = base_y - h
        bars.append(
            f'<rect x="{cx + (x - 256) * scale:.2f}" y="{cy + (y - 256) * scale:.2f}" '
            f'width="{bar_w:.2f}" height="{h * scale:.2f}" rx="{6 * scale:.2f}" fill="{ACCENT}"/>'
        )
    apex = (
        f'<circle cx="{cx + (286 - 256) * scale:.2f}" cy="{cy + (312 - 256) * scale:.2f}" '
        f'r="{14 * scale:.2f}" fill="{APEX}"/>'
    )
    return (
        f'<path d="{path}" fill="none" stroke="{ACCENT}" '
        f'stroke-width="{stroke:.2f}" stroke-linecap="round" stroke-linejoin="round"/>'
        + "".join(bars)
        + apex
    )


def svg_mark(size: int = 512, padded: bool = True) -> str:
    scale = 0.78 if padded else 0.92
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 512 512" fill="none">
  {mark_paths(256, 256, scale)}
</svg>
"""


def svg_app_icon(size: int = 512) -> str:
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 512 512" fill="none">
  <defs>
    <linearGradient id="bg" x1="64" y1="32" x2="448" y2="480" gradientUnits="userSpaceOnUse">
      <stop stop-color="{SURFACE}"/>
      <stop offset="1" stop-color="{BG}"/>
    </linearGradient>
  </defs>
  <rect width="512" height="512" rx="112" fill="url(#bg)"/>
  {mark_paths(256, 262, 0.72)}
</svg>
"""


def svg_logo_horizontal(dark: bool = True, opaque_bg: bool = True) -> str:
    text = TEXT if dark else "#0B0E14"
    muted = MUTED if dark else "#5B6472"
    bg_rect = ""
    if opaque_bg:
        bg = BG if dark else "#F4F7FB"
        bg_rect = f'<rect width="1280" height="360" fill="{bg}"/>'
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="360" viewBox="0 0 1280 360" fill="none">
  {bg_rect}
  <g transform="translate(40, 48) scale(0.52)">
    {mark_paths(256, 256, 0.95)}
  </g>
  <text x="360" y="168" fill="{text}" font-family="Segoe UI, Bahnschrift, Arial Narrow, Arial, sans-serif"
        font-size="84" font-weight="700" letter-spacing="2">Race Refinery</text>
  <text x="364" y="224" fill="{muted}" font-family="Segoe UI, Bahnschrift, Arial, sans-serif"
        font-size="34" font-weight="400" letter-spacing="4">race telemetry</text>
</svg>
"""


def svg_logo_stacked(dark: bool = True) -> str:
    bg = BG if dark else "#F4F7FB"
    text = TEXT if dark else "#0B0E14"
    muted = MUTED if dark else "#5B6472"
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="768" height="768" viewBox="0 0 768 768" fill="none">
  <rect width="768" height="768" fill="{bg}"/>
  <g transform="translate(192, 96) scale(0.75)">
    {mark_paths(256, 256, 0.9)}
  </g>
  <text x="384" y="560" text-anchor="middle" fill="{text}"
        font-family="Segoe UI, Bahnschrift, Arial Narrow, Arial, sans-serif"
        font-size="64" font-weight="700" letter-spacing="1">Race Refinery</text>
  <text x="384" y="610" text-anchor="middle" fill="{muted}"
        font-family="Segoe UI, Bahnschrift, Arial, sans-serif"
        font-size="28" font-weight="400" letter-spacing="3">race telemetry</text>
</svg>
"""


def svg_wordmark(dark: bool = True) -> str:
    text = TEXT if dark else "#0B0E14"
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="900" height="160" viewBox="0 0 900 160" fill="none">
  <text x="0" y="105" fill="{text}" font-family="Segoe UI, Bahnschrift, Arial Narrow, Arial, sans-serif"
        font-size="96" font-weight="700" letter-spacing="2">Race Refinery</text>
</svg>
"""


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    print(f"wrote {path.relative_to(ROOT)}")


def render_svg(svg_path: Path, png_path: Path, width: int, height: int | None = None) -> None:
    height = height or width
    png_path.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [
            "rsvg-convert",
            "-w",
            str(width),
            "-h",
            str(height),
            str(svg_path),
            "-o",
            str(png_path),
        ],
        check=True,
    )
    print(f"rendered {png_path.relative_to(ROOT)} ({width}x{height})")


def png_chunk(tag: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)


def image_to_png_bytes(img: Image.Image) -> bytes:
    buf = io.BytesIO()
    img.save(buf, format="PNG")
    return buf.getvalue()


def write_ico(src: Path, dest: Path, sizes: list[int]) -> None:
    """Write a multi-size ICO containing embedded PNGs for each size."""
    img = Image.open(src).convert("RGBA")
    pngs: list[bytes] = []
    for size in sizes:
        buf = io.BytesIO()
        img.resize((size, size), Image.Resampling.LANCZOS).save(buf, format="PNG")
        pngs.append(buf.getvalue())

    # ICONDIR + ICONDIRENTRY headers, then PNG payloads.
    header = struct.pack("<HHH", 0, 1, len(sizes))
    entries = bytearray()
    offset = 6 + 16 * len(sizes)
    payloads = bytearray()
    for size, data in zip(sizes, pngs):
        w = 0 if size >= 256 else size
        h = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", w, h, 0, 0, 1, 32, len(data), offset)
        payloads += data
        offset += len(data)
    dest.write_bytes(header + entries + payloads)
    print(f"wrote {dest.relative_to(ROOT)} ({len(sizes)} sizes)")


def copy_promo() -> None:
    mapping = {
        "race-refinery-promo-hero.jpg": "promo-hero.jpg",
        "race-refinery-og-social.jpg": "og-social.jpg",
    }
    for src_name, dest_name in mapping.items():
        src = ARTIFACTS / src_name
        if not src.exists():
            print(f"skip missing promo source {src_name}")
            continue
        dest = BRAND / "promo" / dest_name
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_bytes(src.read_bytes())
        # Also mirror OG into public for web use
        if dest_name == "og-social.jpg":
            (PUBLIC / "og-social.jpg").write_bytes(src.read_bytes())
        if dest_name == "promo-hero.jpg":
            (PUBLIC / "promo-hero.jpg").write_bytes(src.read_bytes())
        print(f"copied promo {dest_name}")


def main() -> None:
    source = BRAND / "source"
    write(source / "mark.svg", svg_mark())
    write(source / "icon.svg", svg_app_icon())
    write(source / "logo-horizontal-dark.svg", svg_logo_horizontal(True))
    write(source / "logo-horizontal-light.svg", svg_logo_horizontal(False))
    write(source / "logo-stacked-dark.svg", svg_logo_stacked(True))
    write(source / "logo-stacked-light.svg", svg_logo_stacked(False))
    write(source / "wordmark-dark.svg", svg_wordmark(True))
    write(source / "wordmark-light.svg", svg_wordmark(False))

    # Public web/favicon assets
    write(PUBLIC / "mark.svg", svg_mark())
    write(PUBLIC / "icon.svg", svg_app_icon())
    write(PUBLIC / "logo-horizontal.svg", svg_logo_horizontal(True, opaque_bg=False))
    write(PUBLIC / "logo-horizontal-dark.svg", svg_logo_horizontal(True, opaque_bg=True))
    write(PUBLIC / "favicon.svg", svg_app_icon(64))

    # Raster exports for docs / store / README
    render_svg(source / "icon.svg", BRAND / "icon-1024.png", 1024)
    render_svg(source / "icon.svg", BRAND / "icon-512.png", 512)
    render_svg(source / "mark.svg", BRAND / "mark-512.png", 512)
    render_svg(source / "logo-horizontal-dark.svg", BRAND / "logo-horizontal-dark.png", 1280, 360)
    render_svg(source / "logo-horizontal-light.svg", BRAND / "logo-horizontal-light.png", 1280, 360)
    render_svg(source / "logo-stacked-dark.svg", BRAND / "logo-stacked-dark.png", 768)
    render_svg(source / "logo-stacked-light.svg", BRAND / "logo-stacked-light.png", 768)

    render_svg(PUBLIC / "icon.svg", PUBLIC / "icon-512.png", 512)
    render_svg(PUBLIC / "mark.svg", PUBLIC / "mark-256.png", 256)
    render_svg(PUBLIC / "logo-horizontal.svg", PUBLIC / "logo-horizontal.png", 960, 270)

    # Master for Tauri icon pipeline
    master = BRAND / "icon-1024.png"

    # Generate Tauri / Windows store sizes
    sizes = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
        "Square30x30Logo.png": 30,
        "Square44x44Logo.png": 44,
        "Square71x71Logo.png": 71,
        "Square89x89Logo.png": 89,
        "Square107x107Logo.png": 107,
        "Square142x142Logo.png": 142,
        "Square150x150Logo.png": 150,
        "Square284x284Logo.png": 284,
        "Square310x310Logo.png": 310,
        "StoreLogo.png": 50,
    }
    src_img = Image.open(master).convert("RGBA")
    ICONS.mkdir(parents=True, exist_ok=True)
    for name, size in sizes.items():
        out = ICONS / name
        src_img.resize((size, size), Image.Resampling.LANCZOS).save(out, format="PNG")
        print(f"icon {name}")

    write_ico(master, ICONS / "icon.ico", [16, 24, 32, 48, 64, 128, 256])

    # Minimal ICNS via iconutil unavailable on Linux; keep a high-res PNG named icon.icns
    # replacement: write a PNG-based ICNS-like fallback using Pillow if possible.
    # Tauri on Windows primarily needs .ico; regenerate .icns as multi-res PNG pack when possible.
    try:
        # Simple ICNS writer for common sizes
        icns_sizes = [16, 32, 64, 128, 256, 512, 1024]
        images = {}
        for s in icns_sizes:
            images[s] = src_img.resize((s, s), Image.Resampling.LANCZOS)

        # Use png2icns approach via raw ICNS container
        type_for = {
            16: b"icp4",
            32: b"icp5",
            64: b"icp6",
            128: b"ic07",
            256: b"ic08",
            512: b"ic09",
            1024: b"ic10",
        }
        entries = []
        for s, im in images.items():
            data = image_to_png_bytes(im)
            tag = type_for[s]
            entries.append(tag + struct.pack(">I", len(data) + 8) + data)
        body = b"".join(entries)
        icns = b"icns" + struct.pack(">I", len(body) + 8) + body
        (ICONS / "icon.icns").write_bytes(icns)
        print("wrote src-tauri/icons/icon.icns")
    except Exception as exc:  # noqa: BLE001
        print(f"icns generation skipped: {exc}")

    copy_promo()
    print("done")


if __name__ == "__main__":
    main()

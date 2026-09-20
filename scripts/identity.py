#!/usr/bin/env python3
"""Generate the causari identity assets into assets/.

The mark is "∵" (because): three discs, two causes above one effect. Every
file here is derived from four numbers (disc centres and radius) and four
colours, so the identity can be regenerated exactly, and nothing depends on a
design tool.

    python3 scripts/identity.py            # writes assets/*.svg and assets/*.png

PNG rendering needs Pillow and a monospace TTF; JetBrains Mono is preferred,
DejaVu Sans Mono is the fallback. SVGs need nothing.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets"

INK = "#0b0d10"
PAPER = "#f5f4ef"
GRAPHITE = "#3b4252"
MIST = "#9aa3ad"

# Disc geometry on a 100×100 canvas.
R = 14.5
DISCS = ((28, 34), (72, 34), (50, 72))

MONO_CANDIDATES = [
    "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Medium.ttf",
    "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf",
    "/usr/share/fonts/truetype/macos/JetBrainsMono-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
]
SVG_FONT = "'JetBrains Mono','SF Mono','Cascadia Mono','Fira Code',Consolas,monospace"


def discs_svg(fill: str, indent: str = "  ") -> str:
    return "\n".join(f'{indent}<circle cx="{x}" cy="{y}" r="{R}" fill="{fill}"/>' for x, y in DISCS)


def mark(fill: str) -> str:
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100" '
        'role="img" aria-label="Causari mark">\n' + discs_svg(fill) + "\n</svg>\n"
    )


def mark_b(fill: str) -> str:
    """Variant B: the causes are outlined, the effect is solid."""
    (x1, y1), (x2, y2), (x3, y3) = DISCS
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100" '
        'role="img" aria-label="Causari mark, outlined causes">\n'
        f'  <circle cx="{x1}" cy="{y1}" r="{R - 3}" fill="none" stroke="{fill}" stroke-width="6"/>\n'
        f'  <circle cx="{x2}" cy="{y2}" r="{R - 3}" fill="none" stroke="{fill}" stroke-width="6"/>\n'
        f'  <circle cx="{x3}" cy="{y3}" r="{R}" fill="{fill}"/>\n'
        "</svg>\n"
    )


def mark_c(fill: str) -> str:
    """Variant C: the DAG, edges drawn from causes to effect."""
    (x1, y1), (x2, y2), (x3, y3) = DISCS
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100" '
        'role="img" aria-label="Causari mark, causal graph">\n'
        f'  <g stroke="{fill}" stroke-width="3" stroke-linecap="round" fill="none">\n'
        f'    <line x1="{x1}" y1="{y1}" x2="{x3}" y2="{y3}"/>\n'
        f'    <line x1="{x2}" y1="{y2}" x2="{x3}" y2="{y3}"/>\n'
        "  </g>\n" + discs_svg(fill) + "\n</svg>\n"
    )


def favicon() -> str:
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="64" height="64" '
        'role="img" aria-label="Causari">\n'
        "  <style>\n"
        f"    .bg {{ fill: {INK}; }} .fg {{ fill: {PAPER}; }}\n"
        f"    @media (prefers-color-scheme: light) {{ .bg {{ fill: {INK}; }} .fg {{ fill: {PAPER}; }} }}\n"
        "  </style>\n"
        '  <rect class="bg" width="100" height="100" rx="22"/>\n'
        + "\n".join(f'  <circle cx="{x}" cy="{y}" r="{R}" class="fg"/>' for x, y in DISCS)
        + "\n</svg>\n"
    )


def wordmark(fill: str) -> str:
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 420 100" width="420" height="100" '
        'role="img" aria-label="causari">\n'
        '  <g transform="translate(0,0) scale(1)">\n'
        + discs_svg(fill, "      ")
        + f'\n  </g>\n  <text x="118" y="70" font-family="{SVG_FONT}" font-size="58" '
        f'font-weight="500" letter-spacing="-1" fill="{fill}">causari</text>\n</svg>\n'
    )


def write_svgs() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    files = {
        "mark.svg": mark(INK),
        "mark-white.svg": mark(PAPER),
        "mark-b.svg": mark_b(INK),
        "mark-b-white.svg": mark_b(PAPER),
        "mark-c.svg": mark_c(INK),
        "mark-c-white.svg": mark_c(PAPER),
        "favicon.svg": favicon(),
        "wordmark.svg": wordmark(INK),
        "wordmark-white.svg": wordmark(PAPER),
    }
    for name, body in files.items():
        (OUT / name).write_text(body, encoding="utf-8")
        print("wrote", OUT / name)


def font(size: int):
    from PIL import ImageFont

    for path in MONO_CANDIDATES:
        if os.path.exists(path):
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()


def draw_discs(draw, x0: float, y0: float, scale: float, fill: str) -> None:
    for cx, cy in DISCS:
        r = R * scale
        draw.ellipse(
            (x0 + cx * scale - r, y0 + cy * scale - r, x0 + cx * scale + r, y0 + cy * scale + r),
            fill=fill,
        )


def write_pngs() -> None:
    try:
        from PIL import Image, ImageDraw
    except ImportError:  # pragma: no cover
        print("Pillow not installed; SVGs written, PNGs skipped", file=sys.stderr)
        return

    # OG card, 1200×630, ink background.
    S = 2  # supersample for smooth discs
    img = Image.new("RGB", (1200 * S, 630 * S), INK)
    d = ImageDraw.Draw(img)
    draw_discs(d, 72 * S, 96 * S, 1.3 * S, PAPER)
    d.text((252 * S, 118 * S), "causari", font=font(72 * S), fill=PAPER)
    d.text((72 * S, 300 * S), "AI-written code has no author.", font=font(44 * S), fill=PAPER)
    d.text((72 * S, 360 * S), "It has causes. Causari proves them.", font=font(44 * S), fill=PAPER)
    d.text((72 * S, 470 * S), "re audit  ·  counts, not grades  ·  verifiable offline", font=font(26 * S), fill=MIST)
    d.text((72 * S, 540 * S), "causari.dev", font=font(26 * S), fill=MIST)
    img = img.resize((1200, 630), Image.LANCZOS)
    img.save(OUT / "og.png", optimize=True)
    print("wrote", OUT / "og.png")

    # README logo, 1040×200, transparent, ink (renders on GitHub light; dark uses the SVG).
    img = Image.new("RGBA", (1040 * S, 200 * S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    draw_discs(d, 20 * S, 20 * S, 1.6 * S, INK)
    d.text((210 * S, 46 * S), "causari", font=font(96 * S), fill=INK)
    img = img.resize((1040, 200), Image.LANCZOS)
    img.save(OUT / "logo-readme.png", optimize=True)
    print("wrote", OUT / "logo-readme.png")


if __name__ == "__main__":
    write_svgs()
    write_pngs()

#!/usr/bin/env python3
"""Make every WaveGlance icon from one drawing.

The drawing: a small oscilloscope screen with a yellow sine (CH1) and a cyan
square wave (CH2). This script draws it as SVG and as PNG (Pillow, 4x
supersampled) and writes the files for each platform:

  assets/icon/waveglance.svg          the drawing (Linux "scalable", README)
  assets/icon/icon-256.png            window icon of the desktop program
  assets/icon/macos/AppIcon.icns      macOS (Apple grid: 824 px body on a
                                      1024 px canvas, continuous corners,
                                      no shadow); made with `iconutil` (macOS)
  assets/icon/windows/waveglance.ico  Windows: 16, 20, 24, 30, 32, 36, 40, 48,
                                      60, 64, 72, 80, 96 and 256 px
  assets/icon/linux/hicolor/…         Linux (freedesktop hicolor theme)
  assets/favicon.svg, favicon.ico     web: browser tab
  assets/apple-touch-icon.png         web: iPhone / iPad home screen (180 px,
                                      no transparency: iOS rounds it)
  assets/icon-192.png, icon-512.png,  web: manifest icons (maskable: the
  assets/icon-maskable-512.png        drawing inside the 80 % safe circle)

Small sizes (32 px and less) use a simpler drawing: no grid lines and
thicker traces, so the waves stay clear.

Use: python3 scripts/make-icons.py   (needs Pillow; iconutil on macOS)
"""

import math
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
ICON = ASSETS / "icon"

BODY = (0x0A, 0x0A, 0x0A)
FRAME = (0x5A, 0x5A, 0x5A)
GRID = (0x3A, 0x3A, 0x3A)
CH1 = (0xFF, 0xEB, 0x00)
CH2 = (0x00, 0xE6, 0xFF)

# Geometry in units of the body (0..1).
SQUARE = [(0.125, 0.69), (0.28, 0.69), (0.28, 0.31), (0.50, 0.31),
          (0.50, 0.69), (0.72, 0.69), (0.72, 0.31), (0.875, 0.31)]


def sine(n=96):
    """1.25 periods from the centre line, up first, ending at the top."""
    pts = []
    for i in range(n + 1):
        x = 0.125 + 0.75 * i / n
        y = 0.5 - 0.27 * math.sin(2 * math.pi * 1.25 * (x - 0.125) / 0.75)
        pts.append((x, y))
    return pts


def style(small):
    """Line widths (body units) for the full and the small drawing."""
    if small:
        return dict(frame=0.06, grid=0.0, ch2=0.075, ch1=0.10)
    return dict(frame=0.047, grid=0.022, ch2=0.047, ch1=0.0625)


def squircle(n_pts=256, e=5.0):
    """A superellipse |x|^e + |y|^e = 1 (close to Apple's continuous
    corners), in body units."""
    pts = []
    for i in range(n_pts):
        t = 2 * math.pi * i / n_pts
        c, s = math.cos(t), math.sin(t)
        x = math.copysign(abs(c) ** (2 / e), c)
        y = math.copysign(abs(s) ** (2 / e), s)
        pts.append((0.5 + 0.5 * x, 0.5 + 0.5 * y))
    return pts


# ---------------------------------------------------------------- raster

def draw_body(size, small, shape, ss=4):
    """The body (screen, frame, traces) on a transparent square of `size`
    px. `shape`: "round" (rounded square, radius 18 %) or "squircle"."""
    S = size * ss
    im = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    st = style(small)
    fw = st["frame"] * S
    if shape == "squircle":
        outer = [(x * S, y * S) for x, y in squircle()]
        d.polygon(outer, fill=FRAME)
        k = 1 - 2 * st["frame"]
        inner = [((0.5 + (x - 0.5) * k) * S, (0.5 + (y - 0.5) * k) * S) for x, y in squircle()]
        d.polygon(inner, fill=BODY)
    else:
        r = 0.18 * S
        d.rounded_rectangle([0, 0, S - 1, S - 1], radius=r, fill=FRAME)
        d.rounded_rectangle([fw, fw, S - 1 - fw, S - 1 - fw], radius=max(r - fw, 0), fill=BODY)

    def line(pts, color, w):
        p = [(x * S, y * S) for x, y in pts]
        d.line(p, fill=color, width=round(w * S), joint="curve")
        for x, y in (p[0], p[-1]):
            h = w * S / 2
            d.ellipse([x - h, y - h, x + h, y + h], fill=color)

    if st["grid"]:
        g = st["grid"] * S
        d.rectangle([0.5 * S - g / 2, 0.125 * S, 0.5 * S + g / 2, 0.875 * S], fill=GRID)
        d.rectangle([0.125 * S, 0.5 * S - g / 2, 0.875 * S, 0.5 * S + g / 2], fill=GRID)
    line(SQUARE, CH2, st["ch2"])
    line(sine(), CH1, st["ch1"])
    return im.resize((size, size), Image.LANCZOS)


def plain(size, margin=0.0, small=None):
    """Rounded-square icon with a transparent margin (fraction of size)."""
    if small is None:
        small = size <= 32
    body = round(size * (1 - 2 * margin))
    im = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    off = (size - body) // 2
    im.alpha_composite(draw_body(body, small, "round"), (off, off))
    return im


def macos(size):
    """Apple grid: on 1024, the body is 824 px at (100, 100). No shadow:
    macOS adds it, and since macOS 26 a shape other than the standard one
    (a shadow makes the shape larger) puts the icon on a grey plate."""
    k = size / 1024
    body = round(824 * k)
    off = round(100 * k)
    small = size <= 32
    art = draw_body(body, small, "squircle")
    im = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    im.alpha_composite(art, (off, off))
    return im


# ---------------------------------------------------------------- SVG

def svg(small=False):
    st = style(small)
    U = 64  # SVG units per body side

    def path(pts):
        return "M" + " L".join(f"{x * U:.2f} {y * U:.2f}" for x, y in pts)

    fw = st["frame"] * U
    r = 0.18 * U
    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {U} {U}">',
        "  <!-- WaveGlance icon: a small scope screen with a yellow sine (CH1) and a cyan"
        " square wave (CH2). Made by scripts/make-icons.py. -->",
        f'  <rect x="{fw / 2:.2f}" y="{fw / 2:.2f}" width="{U - fw:.2f}" height="{U - fw:.2f}"'
        f' rx="{r - fw / 2:.2f}" fill="#0a0a0a" stroke="#5a5a5a" stroke-width="{fw:.2f}"/>',
    ]
    if st["grid"]:
        g = st["grid"] * U
        parts.append(
            f'  <path d="M32 8 V56 M8 32 H56" stroke="#3a3a3a" stroke-width="{g:.2f}"/>')
    parts.append(f'  <path d="{path(SQUARE)}" fill="none" stroke="#00e6ff"'
                 f' stroke-width="{st["ch2"] * U:.2f}" stroke-linejoin="round" stroke-linecap="round"/>')
    parts.append(f'  <path d="{path(sine(48))}" fill="none" stroke="#ffeb00"'
                 f' stroke-width="{st["ch1"] * U:.2f}" stroke-linejoin="round" stroke-linecap="round"/>')
    parts.append("</svg>\n")
    return "\n".join(parts)


# ---------------------------------------------------------------- macOS 26

def icon_composer(folder):
    """An Icon Composer file (`AppIcon.icon`): what macOS 26 and later use.
    The system draws the shape, the edge, the glass and the shadow; the
    file gives the background and the drawing as layers (1024 px, full
    bleed). Without it, macOS 26 shows the old .icns on a grey plate."""
    assets = folder / "Assets"
    assets.mkdir(parents=True, exist_ok=True)
    U = 1024
    # The drawing a little smaller than on the other icons: the system's
    # shape has no frame, so the waves need their own margin.
    k, o = 0.84, 0.08

    # The layers are PNG, not SVG: the glass effect closes an open SVG
    # path (a line from its first to its last point would appear).
    for old in assets.glob("*.svg"):
        old.unlink()
    st = style(False)
    ss = 4
    S = U * ss

    def layer(name, draw):
        im = Image.new("RGBA", (S, S), (0, 0, 0, 0))
        draw(ImageDraw.Draw(im))
        im.resize((U, U), Image.LANCZOS).save(assets / name)

    def to_px(pts):
        return [((o + x * k) * S, (o + y * k) * S) for x, y in pts]

    def trace(pts, color, w):
        def draw(d):
            p = to_px(pts)
            width = w * k * S
            d.line(p, fill=color, width=round(width), joint="curve")
            for x, y in (p[0], p[-1]):
                d.ellipse([x - width / 2, y - width / 2, x + width / 2, y + width / 2], fill=color)
        return draw

    def grid(d):
        g = st["grid"] * k * S
        a, b, c = (o + 0.125 * k) * S, (o + 0.875 * k) * S, (o + 0.5 * k) * S
        d.rectangle([c - g / 2, a, c + g / 2, b], fill=(0x4A, 0x4A, 0x4A))
        d.rectangle([a, c - g / 2, b, c + g / 2], fill=(0x4A, 0x4A, 0x4A))

    layer("grid.png", grid)
    layer("square.png", trace(SQUARE, CH2, st["ch2"]))
    layer("sine.png", trace(sine(), CH1, st["ch1"]))
    (folder / "icon.json").write_text("""{
  "fill" : {
    "solid" : "srgb:0.04000,0.04000,0.04000,1.00000"
  },
  "groups" : [
    {
      "layers" : [
        { "glass" : true, "image-name" : "sine.png", "name" : "CH1 sine" },
        { "glass" : true, "image-name" : "square.png", "name" : "CH2 square" },
        { "glass" : false, "image-name" : "grid.png", "name" : "Grid" }
      ],
      "shadow" : { "kind" : "neutral", "opacity" : 0.5 },
      "translucency" : { "enabled" : true, "value" : 0.2 }
    }
  ],
  "supported-platforms" : {
    "squares" : [ "macOS" ]
  }
}
""")


def compile_icon(folder, out):
    """Compile the .icon with Xcode's actool into Assets.car (macOS 26) and
    AppIcon.icns (made by actool from the same file, for macOS 11 to 15)."""
    if not shutil.which("xcrun"):
        print("xcrun not found (macOS with Xcode only): Assets.car not made", file=sys.stderr)
        return False
    with tempfile.TemporaryDirectory() as tmp:
        r = subprocess.run(
            ["xcrun", "actool", str(folder), "--compile", tmp,
             "--output-format", "human-readable-text", "--notices", "--warnings", "--errors",
             "--platform", "macosx", "--target-device", "mac",
             "--minimum-deployment-target", "11.0", "--app-icon", "AppIcon",
             "--output-partial-info-plist", str(Path(tmp) / "partial.plist")],
            capture_output=True, text=True)
        print(r.stdout.strip())
        made = sorted(p.name for p in Path(tmp).iterdir())
        print("actool made:", made)
        if r.returncode != 0 or "Assets.car" not in made:
            print(r.stderr, file=sys.stderr)
            return False
        shutil.copy(Path(tmp) / "Assets.car", out / "Assets.car")
        return True


# ---------------------------------------------------------------- files

def main():
    (ICON / "macos").mkdir(parents=True, exist_ok=True)
    (ICON / "windows").mkdir(parents=True, exist_ok=True)

    (ICON / "waveglance.svg").write_text(svg())
    # The favicon is seen at 16 to 32 px: the simple drawing.
    (ASSETS / "favicon.svg").write_text(svg(small=True))

    plain(256).save(ICON / "icon-256.png")

    # Windows: Microsoft lists these sizes for app icons; the art fills
    # the square with a small margin.
    win_sizes = [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 256]
    frames = [plain(s, margin=0.03 if s > 32 else 0.0) for s in win_sizes]
    frames[-1].save(ICON / "windows" / "waveglance.ico", format="ICO",
                    sizes=[(s, s) for s in win_sizes], append_images=frames[:-1])

    # Linux: hicolor theme.
    lin = ICON / "linux" / "hicolor"
    for s in [16, 24, 32, 48, 64, 128, 256, 512]:
        p = lin / f"{s}x{s}" / "apps"
        p.mkdir(parents=True, exist_ok=True)
        plain(s, margin=0.04 if s > 32 else 0.0).save(p / "waveglance.png")
    p = lin / "scalable" / "apps"
    p.mkdir(parents=True, exist_ok=True)
    (p / "waveglance.svg").write_text(svg())

    # macOS .icns from an .iconset.
    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / "AppIcon.iconset"
        iconset.mkdir()
        for base in [16, 32, 128, 256, 512]:
            macos(base).save(iconset / f"icon_{base}x{base}.png")
            macos(base * 2).save(iconset / f"icon_{base}x{base}@2x.png")
        macos(1024).save(ICON / "macos" / "icon-1024.png")
        icon_composer(ICON / "macos" / "AppIcon.icon")
        compile_icon(ICON / "macos" / "AppIcon.icon", ICON / "macos")
        if shutil.which("iconutil"):
            subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o",
                            str(ICON / "macos" / "AppIcon.icns")], check=True)
        else:
            print("iconutil not found (macOS only): AppIcon.icns not made", file=sys.stderr)

    # Web.
    fav = [plain(s) for s in (16, 32, 48)]
    fav[-1].save(ASSETS / "favicon.ico", format="ICO", sizes=[(16, 16), (32, 32), (48, 48)],
                 append_images=fav[:-1])
    plain(64).save(ASSETS / "favicon.png")
    # Apple touch icon: square, opaque (iOS adds the rounded corners).
    art = draw_body(180, False, "round")
    # Fill the transparent corners with the frame colour, so iOS's own mask
    # shows no black corners.
    bg = Image.new("RGBA", (180, 180), FRAME + (255,))
    bg.alpha_composite(art)
    bg.convert("RGB").save(ASSETS / "apple-touch-icon.png")
    plain(192, margin=0.04).save(ASSETS / "icon-192.png")
    plain(512, margin=0.04).save(ASSETS / "icon-512.png")
    # Maskable: full-bleed background, drawing inside the 80 % safe zone.
    mask = Image.new("RGBA", (512, 512), BODY + (255,))
    mask.alpha_composite(draw_body(352, False, "round"), (80, 80))
    mask.convert("RGB").save(ASSETS / "icon-maskable-512.png")
    print("icons written")


if __name__ == "__main__":
    main()

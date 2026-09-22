"""Export PixelStrict's code-native SVG mark to each platform's icon assets.

Uses Pillow only as a PNG encoder/drawing backend. Every edge is snapped to
integer pixels; no resampling or antialiasing. Run from any working directory.
"""
from __future__ import annotations

import io
import json
import struct
import xml.etree.ElementTree as ET
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
BRANDING = ROOT / "app/assets/branding"
BACKGROUND = "#111515"
ACCENT = "#d5fa7a"
NS = {"svg": "http://www.w3.org/2000/svg"}
SOURCE = ET.parse(BRANDING / "pixelstrict.svg").getroot()
BLOCKS = [tuple(float(rect.attrib[k]) for k in ("x", "y", "width", "height"))
          for rect in SOURCE.findall("svg:g/svg:rect", NS)]


def icon(size: int, *, apple_mobile: bool = False, mac: bool = False,
         mark_only: bool = False) -> Image.Image:
    image = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    padding = round(size * 0.09) if mac else 0
    extent = size - padding * 2
    point = lambda value: padding + round(value * extent / 16)
    if apple_mobile:
        draw.rectangle((0, 0, size - 1, size - 1), fill=BACKGROUND)
    elif not mark_only:
        # Same stepped silhouette as the SVG. Rectangles use exclusive right/bottom.
        for x, y, width, height in [(3, 0, 10, 16), (1, 1, 14, 14), (0, 3, 16, 10)]:
            draw.rectangle((point(x), point(y), point(x+width)-1, point(y+height)-1), fill=BACKGROUND)
    for x, y, width, height in BLOCKS:
        draw.rectangle((point(x), point(y), point(x+width)-1, point(y+height)-1), fill=ACCENT)
    return image.convert("RGB") if apple_mobile else image


def png(path: Path, image: Image.Image) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, format="PNG", optimize=True)


def windows_ico(path: Path) -> None:
    sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
    entries, payloads = [], []
    offset = 6 + 16 * len(sizes)
    for size in sizes:
        buffer = io.BytesIO()
        icon(size).save(buffer, format="PNG")
        data = buffer.getvalue()
        entries.append(struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(data), offset))
        payloads.append(data)
        offset += len(data)
    path.write_bytes(struct.pack("<HHH", 0, 1, len(sizes)) + b"".join(entries) + b"".join(payloads))
    with Image.open(path) as check:
        assert check.ico.sizes() == {(size, size) for size in sizes}


def apple_icons(platform: str) -> None:
    directory = ROOT / f"app/{platform}/Runner/Assets.xcassets/AppIcon.appiconset"
    for asset in json.loads((directory / "Contents.json").read_text())["images"]:
        size = round(float(asset["size"].split("x")[0]) * float(asset["scale"].rstrip("x")))
        png(directory / asset["filename"], icon(size, apple_mobile=platform == "ios", mac=platform == "macos"))


def android_icons() -> None:
    directory = ROOT / "app/android/app/src/main/res"
    for density, size in [("mdpi", 48), ("hdpi", 72), ("xhdpi", 96), ("xxhdpi", 144), ("xxxhdpi", 192)]:
        png(directory / f"mipmap-{density}/ic_launcher.png", icon(size))
    # 40x40 mark, centered within Android's 108x108 adaptive foreground.
    # Every point fits within the 66dp safe circle, including the outer corners.
    paths = []
    for x, y, width, height in BLOCKS:
        left, top = 34+(x-3)*4, 34+(y-3)*4
        paths.append(f"M{left:g},{top:g}h{width*4:g}v{height*4:g}h{-width*4:g}z")
    vector = ('<?xml version="1.0" encoding="utf-8"?>\n'
              '<vector xmlns:android="http://schemas.android.com/apk/res/android" '
              'android:width="108dp" android:height="108dp" '
              'android:viewportWidth="108" android:viewportHeight="108">\n'
              f'  <path android:fillColor="{ACCENT}" android:pathData="{" ".join(paths)}"/>\n'
              '</vector>\n')
    (directory / "drawable/ic_launcher_foreground.xml").write_text(vector, encoding="utf-8")


def main() -> None:
    png(BRANDING / "pixelstrict.png", icon(512))
    png(BRANDING / "pixelstrict-mark.png", icon(160, mark_only=True))
    windows_ico(ROOT / "app/windows/runner/resources/app_icon.ico")
    apple_icons("ios")
    apple_icons("macos")
    android_icons()
    print("Updated Windows ICO (9 sizes), Apple asset catalogs, Android legacy/adaptive mark, and Flutter mark.")


if __name__ == "__main__":
    main()

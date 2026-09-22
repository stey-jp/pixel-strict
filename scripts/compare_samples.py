"""Diagnostic figure and measurable reference comparison. Requires Pillow only."""
from pathlib import Path
import json
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[1]
samples = root / "samples"
names = ["reference", "pseudo", "auto", "surface-weak", "surface-strong"]
canvas = Image.new("RGB", (320 * len(names), 396), "#111515")
draw = ImageDraw.Draw(canvas)
reference = Image.open(samples / "house-reference.png").convert("RGBA")
summary = {}
for i, name in enumerate(names):
    im = Image.open(samples / f"house-{name}.png").convert("RGBA")
    canvas.paste(im.resize((288, 288), Image.Resampling.NEAREST), (i * 320 + 16, 46))
    draw.text((i * 320 + 16, 16), name.upper(), fill="#d5fa7a")
    colors = len(set(im.get_flattened_data()))
    draw.text((i * 320 + 16, 346), f"{im.width} x {im.height} / {colors} colors", fill="white")
    if name not in ("reference", "pseudo"):
        assert set(p[3] for p in im.get_flattened_data()) <= {0, 255}
        assert im.size == reference.size
        error = sum(abs(a[c] - b[c]) for a, b in zip(im.get_flattened_data(), reference.get_flattened_data()) for c in range(3)) / (im.width * im.height * 3)
        report = json.loads((samples / f"house-{name}.json").read_text())
        summary[name] = {"rgb_mae": round(error, 3), "colors": colors, "ms": round(report["processing_ms"], 2), "grid": report["grid"]}
        draw.text((i * 320 + 16, 368), f"RGB MAE {error:.2f} / {report['processing_ms']:.1f} ms", fill="#acb8b1")
canvas.save(samples / "comparison.png")
(samples / "comparison.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary, indent=2))

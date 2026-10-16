from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1] / "src-tauri" / "icons"


def make_icon(size: int) -> Image.Image:
    scale = 4
    canvas = Image.new("RGBA", (size * scale, size * scale), (0, 0, 0, 0))
    draw = ImageDraw.Draw(canvas)
    box = (0, 0, size * scale - 1, size * scale - 1)
    radius = int(size * scale * 0.22)
    draw.rounded_rectangle(box, radius=radius, fill=(57, 116, 237, 255))
    points = [
        (int(size * scale * 0.27), int(size * scale * 0.52)),
        (int(size * scale * 0.43), int(size * scale * 0.68)),
        (int(size * scale * 0.75), int(size * scale * 0.34)),
    ]
    draw.line(points, fill=(255, 255, 255, 255), width=max(4, int(size * scale * 0.09)), joint="curve")
    for point in points:
        r = max(2, int(size * scale * 0.045))
        draw.ellipse((point[0] - r, point[1] - r, point[0] + r, point[1] + r), fill=(255, 255, 255, 255))
    return canvas.resize((size, size), Image.Resampling.LANCZOS)


if __name__ == "__main__":
    ROOT.mkdir(parents=True, exist_ok=True)
    make_icon(32).save(ROOT / "32x32.png")
    make_icon(128).save(ROOT / "128x128.png")
    make_icon(256).save(ROOT / "128x128@2x.png")
    make_icon(256).save(ROOT / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])

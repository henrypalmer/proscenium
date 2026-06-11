"""Generate the placeholder app icon (src-tauri/icons/icon.ico).

A dark rounded square with a proscenium-arch motif. Replace with real
branding before distribution (Milestone 7).
"""

from pathlib import Path

from PIL import Image, ImageDraw

OUT_DIR = Path(__file__).resolve().parent.parent / "src-tauri" / "icons"

BG = (24, 24, 27, 255)  # zinc-900
ACCENT = (244, 244, 245, 255)  # zinc-100


def draw_icon(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    radius = size // 5
    d.rounded_rectangle((0, 0, size - 1, size - 1), radius=radius, fill=BG)

    # Proscenium arch: a rectangle opening with an arched top.
    margin = size // 5
    top = int(size * 0.30)
    bottom = int(size * 0.80)
    width = size // 14
    arch_box = (margin, margin, size - margin, top * 2)
    d.arc(arch_box, 180, 360, fill=ACCENT, width=width)
    d.line((margin, top, margin, bottom), fill=ACCENT, width=width)
    d.line((size - margin, top, size - margin, bottom), fill=ACCENT, width=width)
    d.line((margin - width // 2, bottom, size - margin + width // 2, bottom), fill=ACCENT, width=width)
    return img


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    base = draw_icon(256)
    base.save(
        OUT_DIR / "icon.ico",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    base.save(OUT_DIR / "icon.png")
    print(f"wrote {OUT_DIR / 'icon.ico'}")


if __name__ == "__main__":
    main()

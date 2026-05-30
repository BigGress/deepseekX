from __future__ import annotations

import math
import shutil
import subprocess
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFilter


ROOT = Path(__file__).resolve().parent
ICON_DIR = ROOT / "src-tauri" / "icons"
ICONSET_DIR = ICON_DIR / "icon.iconset"
MASTER_SIZE = 1024


def rounded_rect_mask(size: int, inset: int, radius: int) -> Image.Image:
    mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle(
        (inset, inset, size - inset, size - inset), radius=radius, fill=255
    )
    return mask


def diagonal_gradient(size: int, start: tuple[int, int, int], end: tuple[int, int, int]) -> Image.Image:
    img = Image.new("RGBA", (size, size))
    px = img.load()
    denom = max((size - 1) * 2, 1)
    for y in range(size):
        for x in range(size):
            t = (x + y) / denom
            px[x, y] = tuple(
                int(start[i] * (1 - t) + end[i] * t) for i in range(3)
            ) + (255,)
    return img


def vertical_gradient(size: int, top: tuple[int, int, int], bottom: tuple[int, int, int]) -> Image.Image:
    img = Image.new("RGBA", (size, size))
    px = img.load()
    denom = max(size - 1, 1)
    for y in range(size):
        t = y / denom
        row = tuple(int(top[i] * (1 - t) + bottom[i] * t) for i in range(3)) + (255,)
        for x in range(size):
            px[x, y] = row
    return img


def paste_with_mask(base: Image.Image, fill: Image.Image, mask: Image.Image) -> None:
    base.alpha_composite(Image.composite(fill, Image.new("RGBA", fill.size, (0, 0, 0, 0)), mask))


def add_glow(base: Image.Image, mask: Image.Image, color: tuple[int, int, int, int], blur: float) -> None:
    glow = Image.new("RGBA", base.size, color)
    blurred = mask.filter(ImageFilter.GaussianBlur(blur))
    base.alpha_composite(Image.composite(glow, Image.new("RGBA", glow.size, (0, 0, 0, 0)), blurred))


def make_stroke_mask(size: int, width: int) -> tuple[Image.Image, Image.Image]:
    d_mask = Image.new("L", (size, size), 0)
    draw = ImageDraw.Draw(d_mask)

    top_left = (260, 250)
    top_right = (680, 250)
    bottom_left = (260, 770)
    bottom_right = (680, 770)
    notch_inner = (210, 530)
    notch_join_top = (260, 470)
    notch_join_bottom = (260, 590)
    arc_box = (496, 250, 880, 770)

    draw.line([top_left, top_right], fill=255, width=width, joint="curve")
    draw.line([top_left, notch_join_top], fill=255, width=width, joint="curve")
    draw.line([notch_join_top, notch_inner], fill=255, width=width, joint="curve")
    draw.line([notch_inner, notch_join_bottom], fill=255, width=width, joint="curve")
    draw.line([notch_join_bottom, bottom_left], fill=255, width=width, joint="curve")
    draw.line([bottom_left, bottom_right], fill=255, width=width, joint="curve")
    draw.arc(arc_box, start=-90, end=90, fill=255, width=width)

    x_mask = Image.new("L", (size, size), 0)
    x_draw = ImageDraw.Draw(x_mask)
    center = (512, 512)
    nodes = {
        "tl": (380, 380),
        "tr": (644, 380),
        "bl": (380, 644),
        "br": (644, 644),
    }
    for node in nodes.values():
        x_draw.line([node, center], fill=255, width=width - 18, joint="curve")
    for node in nodes.values():
        radius = 28
        x_draw.ellipse(
            (node[0] - radius, node[1] - radius, node[0] + radius, node[1] + radius),
            fill=255,
        )
    x_draw.ellipse((492, 492, 532, 532), fill=255)

    return d_mask, x_mask


def build_master_icon() -> Image.Image:
    size = MASTER_SIZE
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))

    background_mask = rounded_rect_mask(size, inset=36, radius=180)
    shadow_mask = rounded_rect_mask(size, inset=52, radius=168)
    shadow = shadow_mask.filter(ImageFilter.GaussianBlur(24))
    canvas.alpha_composite(
        Image.composite(
            Image.new("RGBA", (size, size), (4, 10, 18, 180)),
            Image.new("RGBA", (size, size), (0, 0, 0, 0)),
            shadow,
        )
    )

    bg_fill = diagonal_gradient(size, (50, 63, 87), (12, 23, 39))
    paste_with_mask(canvas, bg_fill, background_mask)

    inner_highlight = background_mask.filter(ImageFilter.GaussianBlur(2))
    border_fill = vertical_gradient(size, (126, 142, 170), (40, 52, 75))
    border_mask = ImageChops.subtract(inner_highlight, shadow_mask)
    paste_with_mask(canvas, border_fill, border_mask)

    d_mask, x_mask = make_stroke_mask(size, width=72)

    add_glow(canvas, d_mask, (69, 208, 255, 80), blur=18)
    d_shadow = Image.composite(
        Image.new("RGBA", (size, size), (0, 8, 18, 110)),
        Image.new("RGBA", (size, size), (0, 0, 0, 0)),
        d_mask.filter(ImageFilter.GaussianBlur(10)),
    )
    d_shadow = ImageChops.offset(d_shadow, 0, 10)
    canvas.alpha_composite(d_shadow)
    d_fill = vertical_gradient(size, (113, 232, 255), (11, 168, 255))
    paste_with_mask(canvas, d_fill, d_mask)

    x_shadow = Image.composite(
        Image.new("RGBA", (size, size), (0, 12, 20, 135)),
        Image.new("RGBA", (size, size), (0, 0, 0, 0)),
        x_mask.filter(ImageFilter.GaussianBlur(10)),
    )
    x_shadow = ImageChops.offset(x_shadow, 0, 12)
    canvas.alpha_composite(x_shadow)
    add_glow(canvas, x_mask, (95, 255, 207, 80), blur=18)
    x_fill = vertical_gradient(size, (150, 255, 223), (55, 231, 183))
    paste_with_mask(canvas, x_fill, x_mask)

    # Gloss highlight.
    gloss_mask = rounded_rect_mask(size, inset=52, radius=160)
    gloss = Image.new("RGBA", (size, size), (255, 255, 255, 0))
    gloss_draw = ImageDraw.Draw(gloss)
    gloss_draw.ellipse((-140, -70, size + 70, 460), fill=(255, 255, 255, 34))
    canvas.alpha_composite(Image.composite(gloss, Image.new("RGBA", (size, size), (0, 0, 0, 0)), gloss_mask))

    return canvas


def save_resized(master: Image.Image, path: Path, size: int) -> None:
    image = master.resize((size, size), Image.Resampling.LANCZOS)
    image.save(path)


def build_iconset(master: Image.Image) -> None:
    if ICONSET_DIR.exists():
        shutil.rmtree(ICONSET_DIR)
    ICONSET_DIR.mkdir(parents=True)

    sizes = {
        "icon_16x16.png": 16,
        "icon_16x16@2x.png": 32,
        "icon_32x32.png": 32,
        "icon_32x32@2x.png": 64,
        "icon_128x128.png": 128,
        "icon_128x128@2x.png": 256,
        "icon_256x256.png": 256,
        "icon_256x256@2x.png": 512,
        "icon_512x512.png": 512,
        "icon_512x512@2x.png": 1024,
    }
    for name, size in sizes.items():
        save_resized(master, ICONSET_DIR / name, size)


def build_icns() -> None:
    iconutil = shutil.which("iconutil")
    if not iconutil:
        print("iconutil not found; skipping icon.icns generation")
        return
    subprocess.run(
        [iconutil, "-c", "icns", str(ICONSET_DIR), "-o", str(ICON_DIR / "icon.icns")],
        check=True,
    )


def main() -> None:
    ICON_DIR.mkdir(parents=True, exist_ok=True)
    master = build_master_icon()
    master.save(ICON_DIR / "app-icon-source.png")

    sizes = {
        "32x32.png": 32,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "256x256.png": 256,
        "512x512.png": 512,
        "1024x1024.png": 1024,
    }
    for name, size in sizes.items():
        save_resized(master, ICON_DIR / name, size)
        print(f"Created {name} ({size}x{size})")

    build_iconset(master)
    build_icns()
    print("Done")


if __name__ == "__main__":
    main()

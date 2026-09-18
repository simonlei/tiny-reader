"""生成 Tiny Reader 应用图标（一次性脚本，生成完可删）。

产出：
  src-tauri/icons/icon.png        512x512
  src-tauri/icons/32x32.png
  src-tauri/icons/128x128.png
  src-tauri/icons/128x128@2x.png  256x256
  src-tauri/icons/icon.ico        多尺寸
  src-tauri/icons/icon.icns       macOS
"""
import os
from PIL import Image, ImageDraw

# 脚本放在 tools/ 下，项目根是它的上一级
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "src-tauri", "icons")
os.makedirs(OUT, exist_ok=True)

BG = (26, 29, 35, 255)        # 深色底
RING = (76, 141, 255, 255)    # 主色蓝
BARS = (255, 255, 255, 255)   # 白


def make(size: int) -> Image.Image:
    s = size
    img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # 圆角矩形底
    r = int(s * 0.22)
    d.rounded_rectangle([0, 0, s - 1, s - 1], radius=r, fill=BG)

    # 一个"信号塔 / RSS 波纹"：右下角圆心 + 两条弧 + 一个点
    cx = cy = s * 0.72
    rmax = s * 0.50
    d.arc([cx - rmax, cy - rmax, cx + rmax, cy + rmax], start=150, end=270, fill=RING, width=max(1, int(s * 0.075)))
    rmid = s * 0.31
    d.arc([cx - rmid, cy - rmid, cx + rmid, cy + rmid], start=150, end=270, fill=RING, width=max(1, int(s * 0.075)))
    dot = s * 0.075
    d.ellipse([cx - dot, cy - dot, cx + dot, cy + dot], fill=RING)

    # 左上角三条"文本行"，示意文章列表
    x0 = s * 0.16
    w = s * 0.52
    th = max(1, int(s * 0.062))
    for i, frac in enumerate((0.34, 0.24, 0.30)):
        y = s * (0.20 + i * 0.155)
        d.rounded_rectangle([x0, y, x0 + w * frac, y + th], radius=th // 2, fill=BARS if i == 0 else (150, 158, 172, 255))

    return img


def main():
    icon512 = make(512)
    icon512.save(os.path.join(OUT, "icon.png"))

    for name, size in (("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256)):
        icon512.resize((size, size), Image.LANCZOS).save(os.path.join(OUT, name))

    ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    icon512.save(
        os.path.join(OUT, "icon.ico"),
        format="ICO",
        sizes=ico_sizes,
    )

    # macOS icns：Pillow 需要按 512 尺寸保存
    icon512.save(os.path.join(OUT, "icon.icns"), format="ICNS")

    for f in sorted(os.listdir(OUT)):
        print("  ->", f, os.path.getsize(os.path.join(OUT, f)), "bytes")


if __name__ == "__main__":
    main()

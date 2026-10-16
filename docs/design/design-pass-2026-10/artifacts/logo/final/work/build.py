#!/usr/bin/env python3
"""Build the final ShortWin logo assets, PNG exports and the test renders (pixel zooms)."""
import math, os, re, struct, subprocess, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geometry import *

FINAL = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORK = os.path.join(FINAL, "work")
REN = os.path.join(WORK, "render")
os.makedirs(REN, exist_ok=True)
CONCEPT = os.path.join(os.path.dirname(FINAL), "wordmark-first")


def write(name, text):
    with open(os.path.join(FINAL, name), "w") as f:
        f.write(text + "\n")
    print("wrote", name, len(text), "bytes")


def fit(span_l, span_r, H, t, y0, ya, shelf=None):
    """A W whose outer cap edges sit at span_l / span_r, keeping the concept's split of width between the
    stem lean (3.755) and the diagonal run (5.5) at H = 16, with the apex centre at ya."""
    c = (span_l + span_r) / 2
    s_mark, d_mark = 3.755 / 16, 5.5 / 16
    k = 1.0
    for _ in range(30):
        s = s_mark * k
        cut = t * math.sqrt(1 + s * s)
        xt = span_l + cut / 2
        k = (c - xt) / ((s_mark + d_mark) * H)
    s = s_mark * k
    cut = t * math.sqrt(1 + s * s)
    xt = span_l + cut / 2
    return W(xt=xt, xv=xt + s * H, cx=c, ya=ya, y0=y0, y1=y0 + H, t=t, shelf=shelf)


# ---------------------------------------------------------------- geometry
mark = W(xt=2.745, xv=6.5, cx=12, ya=10, y0=4, y1=20, t=3.4)                 # the concept, unchanged
mark_shelf = fit(3, 23, H=16, t=3.4, y0=4, ya=10, shelf=0)                   # W 3..23, flat entry from x = 0
fav = fit(2, 30, H=20, t=4.0, y0=6, ya=13.5)                                 # whole-pixel favicon grid
fav_shelf = fit(6, 30, H=20, t=4.0, y0=6, ya=13.5, shelf=2)
# 512 icons: the mark at 13.625x (W 299.75 x 218), centred
K = 218 / 16
icon = W(xt=256 + (2.745 - 12) * K, xv=256 + (6.5 - 12) * K, cx=256, ya=256 + (10 - 12) * K,
         y0=256 - 8 * K, y1=256 + 8 * K, t=3.4 * K, ov=0.4 * K)

for name, g, scales in (("mark", mark, (1, 28 / 24, 16 / 24)), ("mark-shelf", mark_shelf, (1, 28 / 24, 16 / 24)),
                        ("favicon", fav, (0.5, 1)), ("favicon-shelf", fav_shelf, (0.5, 1))):
    g.report(name, px_scales=scales)
icon.report("icon-512")
far = max(math.hypot(x - 256, y - 256) for x, y in (icon.cap_outer, icon.mirror(icon.cap_outer), icon.base_outer,
                                                    icon.mirror(icon.base_outer), icon.cap_inner))
print(f"icon-512 farthest corner from centre {far:.1f} (maskable safe radius 204.8)")

# ---------------------------------------------------------------- marks
write("mark.svg", svg((24, 24), 24, 24, mark.paths("currentColor", GOLD)))
write("mark-shelf.svg", svg((24, 24), 24, 24, mark_shelf.paths("currentColor", GOLD)))

TILE = f'<rect width="32" height="32" rx="7" fill="{BG1}"/>'
KEYLINE = f'<rect x="0.5" y="0.5" width="31" height="31" rx="6.5" fill="none" stroke="{LINE}"/>'
write("favicon.svg", svg((32, 32), 32, 32, TILE + fav.paths(TEXT0, GOLD)))
write("favicon-shelf.svg", svg((32, 32), 32, 32, TILE + fav_shelf.paths(TEXT0, GOLD)))
with open(os.path.join(WORK, "favicon-keyline.svg"), "w") as f:          # test only: 1-unit keyline on the tile
    f.write(svg((32, 32), 32, 32, TILE + KEYLINE + fav.paths(TEXT0, GOLD)) + "\n")

GRAD = ('<defs><radialGradient id="g" cx="0.5" cy="0.42" r="0.75">'
        f'<stop offset="0" stop-color="{BG2}"/><stop offset="1" stop-color="{BG0}"/></radialGradient></defs>')
write("icon-512-any.svg", svg((512, 512), 512, 512,
      GRAD + f'<rect width="512" height="512" rx="114" fill="{BG1}"/><rect width="512" height="512" rx="114" fill="url(#g)"/>'
      + icon.paths(TEXT0, GOLD)))
write("icon-512-maskable.svg", svg((512, 512), 512, 512,
      GRAD + f'<rect width="512" height="512" fill="{BG1}"/><rect width="512" height="512" fill="url(#g)"/>'
      + icon.paths(TEXT0, GOLD)))

# ---------------------------------------------------------------- lockups (Inter 700 outlines from the concept)
concept_lockup = open(os.path.join(CONCEPT, "lockup.svg")).read()
glyphs = re.findall(r'<path transform="translate\(112\.00 69\.83\)" d="([^"]+)"', concept_lockup)
assert len(glyphs) == 8, len(glyphs)
TX, BASE = 128, 69.83                      # mark box 0..96 (24 x 4) + 8 px x 4 = 32 gap -> text box at 128
text_w = 272.87                            # advance box of "ShortWin" at 60 px (n's ink ends at 272.87)
LW = math.ceil(TX + text_w + 4)            # 4 = the mark's own 1-unit margin at 4x, mirrored on the right


def lockup(text_a, text_b, mark_a, mark_b):
    short = "".join(f'<path d="{d}"/>' for d in glyphs[:5])
    win = "".join(f'<path d="{d}"/>' for d in glyphs[5:])
    body = (f'<g transform="scale(4)">{mark.paths(mark_a, mark_b)}</g>'
            f'<g transform="translate({TX} {BASE})" fill="{text_a}">{short}</g>'
            f'<g transform="translate({TX} {BASE})" fill="{text_b}">{win}</g>')
    return svg((LW, 96), LW, 96, body)


write("lockup.svg", lockup(TEXT0, TEXT0, TEXT0, GOLD))
write("lockup-two-tone.svg", lockup(TEXT0, GOLD, TEXT0, GOLD))
write("lockup-on-light.svg", lockup(BG0, GOLD_LIGHT, BG0, GOLD_LIGHT))
print(f"lockup viewBox 0 0 {LW} 96; S ink starts at {TX + 2.49}; W ink ends at 92 -> ink gap {TX + 2.49 - 92:.2f} at 4x "
      f"= {(TX + 2.49 - 92) / 4:.2f} px at 1x; gold-on-white contrast {GOLD_LIGHT}: {contrast(GOLD_LIGHT, '#ffffff'):.2f}:1")

# ---------------------------------------------------------------- rendering helpers
CHROME = ["chromium", "--headless=new", "--no-sandbox", "--disable-gpu", "--hide-scrollbars",
          "--force-device-scale-factor=1"]


def render(html_path, w, h, out_png, transparent=False):
    cmd = CHROME + ([f"--default-background-color=00000000"] if transparent else []) + \
          [f"--window-size={w},{h}", f"--screenshot={out_png}", "file://" + html_path]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    if not os.path.exists(out_png):
        raise SystemExit(f"render failed: {out_png}\n{r.stderr[-800:]}")


def png_size(path):
    with open(path, "rb") as f:
        head = f.read(24)
    assert head[:8] == b"\x89PNG\r\n\x1a\n", path
    return struct.unpack(">II", head[16:24])


def page(name, body, bg, w, h, color=TEXT0):
    p = os.path.join(REN, name + ".html")
    with open(p, "w") as f:
        f.write(f'<!doctype html><html><head><meta charset="utf-8"><style>html,body{{margin:0;padding:0;background:{bg};'
                f'color:{color};width:{w}px;height:{h}px;overflow:hidden}}svg,img{{display:block}}</style></head>'
                f'<body>{body}</body></html>')
    return p


def inline_svg(path, size):
    s = open(path).read()
    s = re.sub(r'\swidth="[^"]+"\sheight="[^"]+"', f' width="{size}" height="{size}"', s, count=1)
    return s


def zoom(src, factor, dst):
    subprocess.run(["magick", src, "-filter", "point", "-resize", f"{factor * 100}%", dst], check=True)


# ---------------------------------------------------------------- PNG exports
exports = [("apple-touch-icon.png", "icon-512-maskable.svg", 180, False), ("icon-192.png", "icon-512-maskable.svg", 192, False),
           ("icon-512.png", "icon-512-maskable.svg", 512, False), ("favicon-32.png", "favicon.svg", 32, True),
           ("favicon-16.png", "favicon.svg", 16, True)]
for out, src, size, transparent in exports:
    p = page("export-" + out, f'<img src="{os.path.join(FINAL, src)}" width="{size}" height="{size}">',
             "transparent" if transparent else BG1, size, size)
    render(p, size, size, os.path.join(FINAL, out), transparent=transparent)
    print("export", out, png_size(os.path.join(FINAL, out)))

# ---------------------------------------------------------------- test renders + pixel zooms
STRIPS = [("white", "#ffffff"), ("dee1e6", "#dee1e6"), ("202124", "#202124"), ("35363a", "#35363a"), ("0b0e11", BG0)]
jobs = []
for fav_name in ("favicon", "favicon-shelf"):
    for bgname, bg in STRIPS:
        for size, zf in ((16, 8), (32, 4)):
            jobs.append((f"{fav_name}-{size}-{bgname}", f'<img src="{os.path.join(FINAL, fav_name + ".svg")}" width="{size}" height="{size}">',
                         bg, size, zf, TEXT0))
for bgname, bg in STRIPS:
    for size, zf in ((16, 8), (32, 4)):
        jobs.append((f"favicon-keyline-{size}-{bgname}", f'<img src="{os.path.join(WORK, "favicon-keyline.svg")}" width="{size}" height="{size}">',
                     bg, size, zf, TEXT0))
for mk in ("mark", "mark-shelf"):
    for size, zf in ((16, 8), (24, 6), (28, 5), (48, 3)):
        jobs.append((f"{mk}-{size}", inline_svg(os.path.join(FINAL, mk + ".svg"), size), BG1, size, zf, TEXT0))
    # monochrome (both halves currentColor #aab4bf) at 24
    s = inline_svg(os.path.join(FINAL, mk + ".svg"), 24).replace(f'fill="{GOLD}"', 'fill="currentColor"')
    jobs.append((f"{mk}-24-mono", s, BG1, 24, 6, TEXT1))
for name, body, bg, size, zf, color in jobs:
    p = page(name, body, bg, size, size, color)
    out = os.path.join(REN, name + ".png")
    render(p, size, size, out)
    zoom(out, zf, os.path.join(REN, name + f"-x{zf}.png"))
print("rendered", len(jobs), "test cells into", REN)

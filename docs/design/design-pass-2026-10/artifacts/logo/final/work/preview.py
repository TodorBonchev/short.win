#!/usr/bin/env python3
"""Compose final/preview.html and render it to final/preview.png (1400 px wide)."""
import os, re, subprocess, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from geometry import *

FINAL = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WORK = os.path.join(FINAL, "work")


def read(name):
    return open(os.path.join(FINAL, name)).read().strip()


def inline(name, size, color=TEXT0, mono=False):
    s = read(name)
    s = re.sub(r'\swidth="[^"]+"\sheight="[^"]+"', f' width="{size}" height="{size}"', s, count=1)
    s = re.sub(r'\srole="img"\saria-label="[^"]+"', ' aria-hidden="true"', s, count=1)
    s = re.sub(r'<title>[^<]*</title>', '', s, count=1)
    if mono:
        s = s.replace(f'fill="{GOLD}"', 'fill="currentColor"')
    return f'<span class="ink" style="color:{color}">{s}</span>'


STRIPS = [("white", "#ffffff"), ("dee1e6", "#dee1e6"), ("202124", "#202124"), ("35363a", "#35363a"), ("0b0e11", BG0)]
CHIP = ' style="display:flex;align-items:center;gap:6px"'

# ---------------------------------------------------------------- 1. header mocks
def header(mark_html, word=True, width=760):
    brand = f'<a class="brand">{mark_html}' + ('<span class="word">ShortWin</span>' if word else '') + '</a>'
    return (f'<div class="hd" style="width:{width}px">{brand}<span class="badge">localnet</span>'
            '<nav><a class="on">Markets</a><a>Positions</a><a>Lend</a><a>Status</a></nav>'
            '<span class="grow"></span><button class="gold">Connect wallet</button></div>')


phone = (f'<div class="hd" style="width:360px;height:48px;padding:0 12px">'
         f'<a class="brand">{inline("mark.svg", 28)}</a><span class="badge">localnet</span><span class="grow"></span>'
         '<button class="gold">Connect</button></div>')
hdr_cell = f'''<section class="card panel wide"><h3>Header strip at 1×: mark 24 px + "ShortWin" Inter 650 15 px, 8 px gap · 44 px bar · 1 px #2a323b rule</h3>
<div class="stack">
<div class="mockrow">{header(inline("mark.svg", 24))}<span class="note">light: fall = currentColor #e8ecf1, check = #f0b90b</span></div>
<div class="mockrow">{header(inline("mark.svg", 24, TEXT1, mono=True))}<span class="note">monochrome: both halves #aab4bf (no seam at x = 12)</span></div>
<div class="mockrow">{phone}<span class="note">phone, under 400 px: 48 px bar, the mark alone at 28 px</span></div>
</div></section>'''

# ---------------------------------------------------------------- 2. favicon strips
def strip(name, bg, label):
    light = bg in ("#ffffff", "#dee1e6")
    return (f'<div class="strip" style="background:{bg};color:{"#202124" if light else "#e8eaed"}">'
            f'<div class="tab" style="background:{"#fff" if bg == "#dee1e6" else ("#f1f3f4" if light else "#35363a" if bg == "#202124" else "#202124")}">'
            f'<img src="favicon.svg" width="16" height="16">short.win</div>'
            f'<img src="favicon.svg" width="32" height="32"><img src="favicon.svg" width="64" height="64">'
            f'<img src="work/render/favicon-16-{name}-x8.png" width="128" height="128" title="16 px render at 8x">'
            f'<img src="work/render/favicon-32-{name}-x4.png" width="128" height="128" title="32 px render at 4x">'
            f'<span class="lab" style="color:{"#5f6368" if light else "#9aa0a6"}">{label}</span></div>')


fav_cell = ('<section class="card wide"><h3>favicon.svg — 16 px tab, 32, 64, then the 16 px render at 8× and the 32 px render at 4× — on white, #dee1e6, #202124, #35363a, #0b0e11</h3>'
            '<div class="strips">' + "".join(strip(n, bg, f"#{n}" if n != "white" else "white") for n, bg in STRIPS) + '</div>'
            '<div class="row" style="margin-top:10px;align-items:flex-start"><div class="sub"><h4>keyline test: 1-unit #2a323b ring on the tile, 32 px at 4× / 16 px at 8×, on #202124 and #35363a</h4><div class="row">'
            + "".join(f'<span class="chip" style="background:{bg}"><img src="work/render/favicon-keyline-32-{n}-x4.png" width="128" height="128"><img src="work/render/favicon-keyline-16-{n}-x8.png" width="128" height="128"></span>' for n, bg in STRIPS[2:4])
            + '</div></div><div class="sub"><h4>same cells without the keyline (the shipped file)</h4><div class="row">'
            + "".join(f'<span class="chip" style="background:{bg}"><img src="work/render/favicon-32-{n}-x4.png" width="128" height="128"><img src="work/render/favicon-16-{n}-x8.png" width="128" height="128"></span>' for n, bg in STRIPS[2:4])
            + '</div></div></div></section>')

# ---------------------------------------------------------------- 3. shelf comparison
def cmp_row(name, label):
    cells = "".join(f'<span class="cell"><span class="ink" style="height:48px;display:flex;align-items:center">{inline(name + ".svg", s)}</span><span class="lab">{s}</span></span>' for s in (16, 24, 28, 48))
    zooms = (f'<img src="work/render/{name}-16-x8.png" width="128" height="128" title="16 px at 8x">'
             f'<img src="work/render/{name}-24-x6.png" width="144" height="144" title="24 px at 6x">'
             f'<img src="work/render/{name}-24-mono-x6.png" width="144" height="144" title="24 px monochrome at 6x">')
    favs = "".join(f'<span class="cell"><span class="ink"><img src="favicon{"-shelf" if "shelf" in name else ""}.svg" width="{s}" height="{s}"></span><span class="lab">{s}</span></span>' for s in (16, 32))
    fz = f'<img src="work/render/favicon{"-shelf" if "shelf" in name else ""}-16-202124-x8.png" width="128" height="128" title="favicon 16 px at 8x on #202124">'
    return (f'<div class="cmp"><span class="cmpname">{label}</span><span class="row">{cells}</span><span class="row">{zooms}</span>'
            f'<span class="row">{favs}</span><span class="row">{fz}</span></div>')


cmp_cell = ('<section class="card panel wide"><h3>shelf vs no shelf — mark at 16 / 24 / 28 / 48 on #12161b, the 16 px, 24 px and 24 px monochrome renders zoomed, the favicon at 16 / 32 and its 16 px render at 8×</h3>'
            + cmp_row("mark", "mark.svg (no shelf, shipped)") + cmp_row("mark-shelf", "mark-shelf.svg (flat entry 0..6.47 at the cap)") + '</section>')

# ---------------------------------------------------------------- 4. app icons
icon_cell = ('<section class="card dark wide"><h3>icon-512-any.svg and icon-512-maskable.svg at 128 px (dashed = the 80 % safe circle), apple-touch-icon.png 180 at 1×, icon-192.png at 1×</h3>'
             '<div class="row" style="align-items:flex-end;gap:20px">'
             '<span class="cell"><img src="icon-512-any.svg" width="128" height="128"><span class="lab">any · rx 114</span></span>'
             '<span class="cell"><span class="safe"><img src="icon-512-maskable.svg" width="128" height="128"><i></i></span><span class="lab">maskable · full bleed</span></span>'
             '<span class="cell"><img src="apple-touch-icon.png" width="180" height="180"><span class="lab">apple-touch-icon.png 180</span></span>'
             '<span class="cell"><img src="icon-192.png" width="192" height="192"><span class="lab">icon-192.png</span></span>'
             '<span class="cell"><span class="row" style="gap:8px"><img src="favicon-32.png" width="32" height="32"><img src="favicon-16.png" width="16" height="16"></span><span class="lab">favicon-32/16.png</span></span>'
             '</div></section>')

# ---------------------------------------------------------------- 5. lockups
lock_cell = ('<section class="card panel wide"><h3>lockups at 420 px — lockup.svg · lockup-two-tone.svg · lockup-on-light.svg (text #0b0e11, check #b58900 = 3.2:1 on white)</h3>'
             '<div class="row" style="gap:24px;align-items:center"><img src="lockup.svg" width="420"><img src="lockup-two-tone.svg" width="420">'
             '<span style="background:#fff;padding:12px 10px;border-radius:6px;display:inline-block"><img src="lockup-on-light.svg" width="420"></span></div></section>')

css = f'''
@font-face{{font-family:"Inter";src:url("work/InterVariable.ttf") format("truetype");font-weight:100 900}}
*{{box-sizing:border-box}}
body{{margin:0;padding:16px;background:{BG2};color:{TEXT0};font:13px/1.4 system-ui,sans-serif;width:1400px;display:grid;grid-template-columns:1fr 1fr;gap:16px;align-content:start}}
.wide{{grid-column:1/-1}}
.card{{border-radius:8px;padding:12px 14px 14px;background:#fff;color:#111}} .card h3{{margin:0 0 10px;font:600 11.5px/1.3 system-ui;color:#555;text-transform:uppercase;letter-spacing:.04em}}
.card h4{{margin:0 0 6px;font:500 11px system-ui;color:{TEXT1}}}
.panel{{background:{BG0};color:{TEXT0}}} .panel h3{{color:{TEXT1}}} .dark{{background:{BG0};color:{TEXT0}}} .dark h3{{color:{TEXT1}}}
.row{{display:flex;align-items:center;gap:12px;flex-wrap:wrap}} .stack{{display:flex;flex-direction:column;gap:10px}} .grow{{flex:1}}
img,svg{{display:block}} .ink{{display:inline-flex;line-height:0}}
.lab{{font:11px ui-monospace,monospace;color:#888;display:block;margin-top:4px;text-align:center}}
.cell{{display:inline-flex;flex-direction:column;align-items:center}} .chip{{display:inline-flex;gap:8px;padding:8px;border-radius:4px}}
.sub{{flex:1}}
.hd{{display:flex;align-items:center;gap:16px;height:44px;padding:0 8px;background:{BG1};border-bottom:1px solid {LINE};border-radius:4px 4px 0 0}}
.brand{{display:flex;align-items:center;gap:8px;min-height:24px;font:650 15px/1 "Inter",system-ui,sans-serif;color:{TEXT0};letter-spacing:.01em}}
.word{{display:block}}
.badge{{font:12px/18px "Inter",system-ui;padding:0 4px;border:1px solid #66727f;border-radius:3px;color:{TEXT1};text-transform:uppercase}}
nav{{display:flex;align-self:stretch}} nav a{{display:inline-flex;align-items:center;padding:0 8px;font:500 13px "Inter",system-ui;color:{TEXT1};border-bottom:2px solid transparent;margin-bottom:-1px}} nav a.on{{color:{TEXT0};border-bottom-color:{GOLD}}}
.gold{{background:{GOLD};color:#111;border:0;border-radius:4px;font:600 13px "Inter",system-ui;padding:7px 12px}}
.mockrow{{display:flex;align-items:center;gap:16px}} .note{{font:11px ui-monospace,monospace;color:{TEXT1}}}
.strips{{display:grid;grid-template-columns:1fr 1fr;gap:8px}}
.strip{{display:flex;align-items:center;gap:14px;padding:6px 10px;border-radius:6px}}
.tab{{display:flex;align-items:center;gap:8px;padding:6px 12px 6px 10px;border-radius:8px 8px 0 0;font:12px system-ui;align-self:flex-end;margin-bottom:-6px}}
.strip .tab{{align-self:center;margin:0;border-radius:6px}}
.cmp{{display:flex;align-items:center;gap:20px;padding:8px 0;border-top:1px solid {LINE}}} .cmpname{{width:200px;font:11px ui-monospace,monospace;color:{TEXT1}}}
.safe{{position:relative;display:block}} .safe i{{position:absolute;inset:12.8px;border:1px dashed rgba(255,255,255,.55);border-radius:50%}}
'''
html = f'<!doctype html><html><head><meta charset="utf-8"><title>ShortWin logo — final preview</title><style>{css}</style></head><body>{hdr_cell}{fav_cell}{cmp_cell}{icon_cell}{lock_cell}</body></html>'
with open(os.path.join(FINAL, "preview.html"), "w") as f:
    f.write(html)
out = os.path.join(FINAL, "preview.png")
subprocess.run(["chromium", "--headless=new", "--no-sandbox", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
                "--window-size=1400,2600", f"--screenshot={out}", "file://" + os.path.join(FINAL, "preview.html")], capture_output=True, timeout=90)
subprocess.run(["magick", out, "-trim", "+repage", "-bordercolor", BG2, "-border", "16", out], check=True)
print(subprocess.run(["magick", "identify", out], capture_output=True, text=True).stdout.strip())

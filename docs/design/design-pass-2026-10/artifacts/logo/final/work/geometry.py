#!/usr/bin/env python3
"""ShortWin final mark: exact filled outlines of the chart-W.

The W is a stroked centreline polyline  (xt,y0) -> (xv,y1) -> (cx,ya) -> mirror,  mitre joins,
cut flat at the cap line y0 and clipped at the baseline y1 (so the valleys get Inter-style flat
bottoms). The left half (the fall = "short") carries on 0.4 units past the seam UNDER the gold half
(the check = "win") so a monochrome render shows no anti-aliasing hairline at x = cx.
Optional shelf: a flat price-line entry at cap height to the left of the first down-stroke.
"""
import math

GOLD, GOLD_LIGHT = "#f0b90b", "#b58900"
TEXT0, TEXT1 = "#e8ecf1", "#aab4bf"
BG0, BG1, BG2, LINE = "#0b0e11", "#12161b", "#181d23", "#2a323b"


def fmt(v, nd=2):
    s = f"{v:.{nd}f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def norm(dx, dy):
    l = math.hypot(dx, dy)
    return dx / l, dy / l


def isect(l1, l2):
    (p1, d1), (p2, d2) = l1, l2
    rx, ry = p2[0] - p1[0], p2[1] - p1[1]
    det = -d1[0] * d2[1] + d2[0] * d1[1]
    t = (-rx * d2[1] + d2[0] * ry) / det
    return (p1[0] + t * d1[0], p1[1] + t * d1[1])


def x_at(line, y):
    p, d = line
    return p[0] + (y - p[1]) * d[0] / d[1]


def y_at(line, x):
    p, d = line
    return p[1] + (x - p[0]) * d[1] / d[0]


class W:
    """Geometry of one W. All numbers in the file's user units."""

    def __init__(self, xt, xv, cx, ya, y0, y1, t, shelf=None, ov=0.4):
        self.__dict__.update(locals())
        h = t / 2
        ds = norm(xv - xt, y1 - y0)
        ns = (ds[1], -ds[0])                       # right-hand normal of the stem (towards the counter)
        self.stem_outer = ((xt - h * ns[0], y0 - h * ns[1]), ds)
        self.stem_inner = ((xt + h * ns[0], y0 + h * ns[1]), ds)
        dd = norm(cx - xv, ya - y1)                # rising diagonal, valley -> apex
        nd = (dd[1], -dd[0])                       # points up-left: the upper edge side
        self.diag_upper = ((xv + h * nd[0], y1 + h * nd[1]), dd)
        self.diag_lower = ((xv - h * nd[0], y1 - h * nd[1]), dd)
        self.cap_outer = (x_at(self.stem_outer, y0), y0)
        self.cap_inner = (x_at(self.stem_inner, y0), y0)
        self.counter = isect(self.stem_inner, self.diag_upper)
        self.apex_top = (cx, y_at(self.diag_upper, cx))
        self.apex_notch = (cx, y_at(self.diag_lower, cx))
        self.base_outer = (x_at(self.stem_outer, y1), y1)
        self.base_diag = (x_at(self.diag_lower, y1), y1)
        self.ov_top = (cx + ov, y_at(self.diag_upper, cx - ov))
        self.ov_notch = (cx + ov, y_at(self.diag_lower, cx - ov))
        if shelf is not None:
            self.shelf_tl = (shelf, y0)
            self.shelf_bl = (shelf, y0 + t)
            self.shelf_inner = (x_at(self.stem_outer, y0 + t), y0 + t)
        # derived figures for the README
        self.stem_angle = math.degrees(math.atan2(xv - xt, y1 - y0))      # from vertical
        self.diag_angle = math.degrees(math.atan2(y1 - ya, cx - xv))      # from horizontal
        self.cap_cut = self.cap_inner[0] - self.cap_outer[0]
        self.valley_flat = self.base_diag[0] - self.base_outer[0]
        self.apex_vert = self.apex_notch[1] - self.apex_top[1]

    def mirror(self, p):
        return (2 * self.cx - p[0], p[1])

    @staticmethod
    def path(points, nd=2):
        return "M" + "L".join(f"{fmt(x, nd)} {fmt(y, nd)}" for x, y in points) + "Z"

    def left_d(self, nd=2):
        if self.shelf is None:
            pts = [self.cap_outer, self.base_outer, self.base_diag, self.apex_notch, self.ov_notch,
                   self.ov_top, self.apex_top, self.counter, self.cap_inner]
        else:
            pts = [self.shelf_tl, self.cap_inner, self.counter, self.apex_top, self.ov_top, self.ov_notch,
                   self.apex_notch, self.base_diag, self.base_outer, self.shelf_inner, self.shelf_bl]
        return self.path(pts, nd)

    def right_d(self, nd=2):
        m = self.mirror
        pts = [self.apex_notch, m(self.base_diag), m(self.base_outer), m(self.cap_outer), m(self.cap_inner),
               m(self.counter), self.apex_top]
        return self.path(pts, nd)

    def paths(self, left_fill, right_fill, nd=2, right_class=None):
        r = f' class="{right_class}"' if right_class else ""
        rf = f' fill="{right_fill}"' if right_fill else ""
        return (f'<path d="{self.left_d(nd)}" fill="{left_fill}"/>'
                f'<path{r} d="{self.right_d(nd)}"{rf}/>')

    def extent(self):
        x0 = self.shelf if self.shelf is not None else self.cap_outer[0]
        return x0, self.mirror(self.cap_outer)[0]

    def counter_rows(self, px_scale):
        """Width of the left counter (in device px) at the centre of each device-pixel row."""
        rows = []
        y = self.y0
        step = 1 / px_scale
        while y + step / 2 < self.counter[1]:
            yc = y + step / 2
            xl = x_at(self.stem_inner, yc)
            # the top of the counter is open across to the mirrored stem until the apex
            if yc < self.apex_top[1]:
                xr = self.mirror((x_at(self.stem_inner, yc), yc))[0]
            else:
                xr = x_at(self.diag_upper, yc)
            rows.append((round(y * px_scale), (xr - xl) * px_scale))
            y += step
        return rows

    def report(self, name, px_scales=()):
        print(f"[{name}] cap {fmt(self.y0)}  base {fmt(self.y1)}  x {fmt(self.extent()[0])}..{fmt(self.extent()[1])}"
              f"  stroke {fmt(self.t)}  cap cut {self.cap_cut:.2f}  valley flat {self.valley_flat:.2f}"
              f"  stem {self.stem_angle:.1f} deg from vertical  diagonal {self.diag_angle:.1f} deg from horizontal")
        print(f"      apex tip ({fmt(self.apex_top[0])},{self.apex_top[1]:.2f}) notch y {self.apex_notch[1]:.2f}"
              f" (vertical thickness {self.apex_vert:.2f})  counter bottom ({self.counter[0]:.2f},{self.counter[1]:.2f})"
              f"  valley flat {self.base_outer[0]:.2f}..{self.base_diag[0]:.2f}")
        if self.shelf is not None:
            print(f"      shelf: top edge {fmt(self.shelf)}..{self.cap_inner[0]:.2f} ({self.cap_inner[0]-self.shelf:.2f}),"
                  f" bottom edge {fmt(self.shelf)}..{self.shelf_inner[0]:.2f} ({self.shelf_inner[0]-self.shelf:.2f}),"
                  f" thickness {fmt(self.t)}")
        for s in px_scales:
            rows = self.counter_rows(s)
            txt = "  ".join(f"r{r}:{w:.1f}" for r, w in rows)
            print(f"      left counter width in px at scale {s} (row: px): {txt}")


def svg(view, w, h, body, title="ShortWin"):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {view[0]} {view[1]}" width="{w}" height="{h}" '
            f'role="img" aria-label="{title}"><title>{title}</title>{body}</svg>')


def contrast(hex1, hex2):
    def lum(h):
        r, g, b = (int(h[i:i + 2], 16) / 255 for i in (1, 3, 5))
        f = lambda c: c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4
        return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
    a, b = sorted((lum(hex1), lum(hex2)), reverse=True)
    return (a + 0.05) / (b + 0.05)

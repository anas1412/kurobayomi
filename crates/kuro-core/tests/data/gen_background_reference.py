#!/usr/bin/env python3
"""Regenerates background_reference.json: a line-by-line Python port of
ImageUtil.chooseBackground (aniyomi core/common .../ImageUtil.kt:338-580) run over the synthetic
images below. Out-of-range samples read as transparent (deviation D-6).
Usage: python3 gen_background_reference.py > background_reference.json"""
import json

WHITE = (255, 255, 255, 255)


def is_dark(p): return p[0] < 40 and p[1] < 40 and p[2] < 40 and p[3] > 200
def is_white(p): return p[0] + p[1] + p[2] > 740
def close(p, o): return abs(p[0]-o[0]) < 30 and abs(p[1]-o[1]) < 30 and abs(p[2]-o[2]) < 30


def make(c):
    w, h = c["width"], c["height"]
    px = [[tuple(c["fill"])] * w for _ in range(h)]
    for x0, y0, x1, y1, r, g, b, a in c["rects"]:
        for y in range(y0, min(y1, h)):
            for x in range(x0, min(x1, w)):
                px[y][x] = (r, g, b, a)
    return px


def choose(c):
    img = make(c); W = c["width"]; H = c["height"]; landscape = c["landscape"]
    def at(x, y):
        if x < 0 or y < 0 or x >= W or y >= H: return (0, 0, 0, 0)
        return img[y][x]
    solid = lambda p: {"solid": list(p)}
    if W < 50 or H < 50: return solid(WHITE)
    top = 5; bot = H - 5
    left = int(W * 0.0275); right = W - left
    midX = W // 2; midY = H // 2
    offX = int(W * 0.01); lOff = left - offX; rOff = right + offX
    tl, tr = at(left, top), at(right, top)
    ml, mr = at(left, midY), at(right, midY)
    tc = at(midX, top); bl = at(left, bot); bc = at(midX, bot); br = at(right, bot)
    tlD, trD, mlD, mrD, tmD, blD, brD = map(is_dark, (tl, tr, ml, mr, tc, bl, br))
    darkBG = (tlD and (blD or brD or trD or mlD or tmD)) or (trD and (brD or blD or mrD or tmD))
    ring = [tl, tc, tr, br, bc, bl]
    if all((not is_white(p)) and close(p, ring[(i+1) % 6]) for i, p in enumerate(ring)):
        return solid(tl)
    if sum(1 for p in (tl, tr, bl, br) if is_white(p)) > 2: darkBG = False
    black = tl if tlD else tr if trD else bl if blD else br if brD else WHITE
    oW = oB = 0; tBS = tWS = bBS = bWS = 0
    for x in (left, right, lOff, rOff):
        wS = wP = bS = bP = 0; bStreak = wStreak = False
        notOff = x in (left, right)
        step = H // 25
        for index, y in enumerate(range(0, H, step)):
            pixel = at(x, y)
            pOff = at(x + (-offX if x < W // 2 else offX), y)
            if is_white(pixel):
                wS += 1; wP += 1
                if notOff: oW += 1
                if wS > 14: wStreak = True
                if wS > 6 and wS >= index - 1: tWS = wS
            else:
                wS = 0
                if is_dark(pixel) and is_dark(pOff):
                    bP += 1
                    if notOff: oB += 1
                    bS += 1
                    if bS >= 14: bStreak = True
                    continue
            if bS > 6 and bS >= index - 1: tBS = bS
            bS = 0
        if bS > 6: bBS = bS
        elif wS > 6: bWS = wS
        if bP > 22:
            if x in (right, rOff): black = tr if trD else br if brD else black
            darkBG = True; oW = 0
            break
        elif bStreak:
            darkBG = True
            if x in (right, rOff): black = tr if trD else br if brD else black
            if bP > 18:
                oW = 0
                break
        elif wStreak or wP > 22:
            darkBG = False
    topBlack = tBS > tWS; botBlack = bBS > bWS
    if oW > 9 and oW > oB: darkBG = False
    if topBlack and botBlack: darkBG = True
    if landscape: return solid(black if darkBG else WHITE)
    botW = is_white(bl) and is_white(br); topW = is_white(tl) and is_white(tr)
    topD = tlD and trD; botD = blD and brD
    topOffD = is_dark(at(lOff, top)) and is_dark(at(rOff, top))
    botOffD = is_dark(at(lOff, bot)) and is_dark(at(rOff, bot))
    bands = lambda t, b: {"bands": {"top": list(t), "bottom": list(b)}}
    if darkBG and botW: return bands(black, WHITE)
    if darkBG and topW: return bands(WHITE, black)
    if darkBG: return solid(black)
    if topBlack or (topD and topOffD and (tmD or oB > 9)): return bands(black, WHITE)
    if botBlack or (botD and botOffD and (is_dark(bc) or oB > 9)): return bands(WHITE, black)
    return solid(WHITE)


K = [0, 0, 0, 255]; Wt = [255, 255, 255, 255]
def case(name, w, h, fill, rects=(), landscape=False):
    return {"name": name, "width": w, "height": h, "fill": fill,
            "rects": [list(r) for r in rects], "landscape": landscape}

cases = [
    case("tiny_width", 49, 300, K), case("tiny_height", 300, 49, K),
    case("solid_black", 200, 300, K), case("solid_white", 200, 300, Wt),
    case("red_over_blue", 200, 300, [30, 30, 200, 255], [(0, 0, 200, 150, 200, 30, 30, 255)]),
    case("black_frame_10", 200, 300, Wt, [(0, 0, 200, 10, 0, 0, 0, 255), (0, 290, 200, 300, 0, 0, 0, 255),
                                           (0, 0, 10, 300, 0, 0, 0, 255), (190, 0, 200, 300, 0, 0, 0, 255)]),
    case("black_frame_landscape", 200, 300, Wt, [(0, 0, 200, 10, 0, 0, 0, 255), (0, 290, 200, 300, 0, 0, 0, 255),
                                                  (0, 0, 10, 300, 0, 0, 0, 255), (190, 0, 200, 300, 0, 0, 0, 255)], True),
    case("dark_page_landscape", 200, 300, [10, 10, 10, 255], [(60, 60, 140, 240, 255, 255, 255, 255)], True),
    case("dark_sides_white_centre", 200, 300, Wt, [(0, 0, 12, 300, 0, 0, 0, 255), (188, 0, 200, 300, 0, 0, 0, 255)]),
    case("dark_top_half", 200, 300, Wt, [(0, 0, 200, 150, 0, 0, 0, 255)]),
    case("dark_bottom_half", 200, 300, Wt, [(0, 150, 200, 300, 0, 0, 0, 255)]),
    case("black_top_white_bottom_corners", 200, 300, Wt, [(0, 0, 200, 140, 0, 0, 0, 255), (0, 140, 200, 300, 255, 255, 255, 255)]),
    case("dark_top_strip_only", 200, 300, Wt, [(0, 0, 200, 20, 0, 0, 0, 255)]),
    case("light_grey", 200, 300, [200, 200, 200, 255]),
    case("grey_vs_white_corners", 200, 300, Wt, [(0, 0, 50, 50, 120, 120, 120, 255)]),
    case("transparent_dark", 200, 300, [0, 0, 0, 100]),
    case("exactly_50x50_white", 50, 50, Wt), case("exactly_50x50_black_frame", 50, 50, Wt, [(0, 0, 4, 50, 0, 0, 0, 255), (46, 0, 50, 50, 0, 0, 0, 255)]),
    case("wide_page_dark_left_right", 400, 200, Wt, [(0, 0, 20, 200, 0, 0, 0, 255), (380, 0, 400, 200, 0, 0, 0, 255)]),
    case("wide_page_dark_left_right_landscape", 400, 200, Wt, [(0, 0, 20, 200, 0, 0, 0, 255), (380, 0, 400, 200, 0, 0, 0, 255)], True),
]
for w in range(100, 110):
    cases.append(case(f"width_{w}_black_right_edge", w, 200, Wt, [(w - 4, 0, w, 200, 0, 0, 0, 255)]))
    cases.append(case(f"width_{w}_black_both_edges_landscape", w, 200, Wt,
                      [(0, 0, 4, 200, 0, 0, 0, 255), (w - 4, 0, w, 200, 0, 0, 0, 255)], True))
for c in cases:
    c["expected"] = choose(c)
print(json.dumps(cases, indent=1))

"""OpenWX brand kit: SVG (text as outlines, Barlow OFL), PNG sizes and a Windows .ico.

Layout matches the owner's artwork: OPEN (white) + WX (orange) in Barlow Bold, BROADCASTING
(slate) in Barlow Regular with wide tracking, centred on navy.
"""
import sys, os
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from PIL import Image, ImageDraw, ImageFont

FONTS, OUT = sys.argv[1], sys.argv[2]
os.makedirs(OUT, exist_ok=True)
NAVY, NAVY_TOP, NAVY_BOTTOM, WHITE, ORANGE, SLATE = '#1B232E', '#263142', '#121821', '#FEFEFE', '#EE7D14', '#778796'
INK = '#1B232E'  # wordmark on light backgrounds

class Face:
    def __init__(self, name):
        self.path = os.path.join(FONTS, name)
        self.tt = TTFont(self.path)
        self.upm = self.tt['head'].unitsPerEm
        self.cap = self.tt['OS/2'].sCapHeight
        self.cmap = self.tt.getBestCmap()
        self.gs = self.tt.getGlyphSet()
        self.hmtx = self.tt['hmtx']

    def advance(self, ch, size):
        return self.hmtx[self.cmap[ord(ch)]][0] * size / self.upm

    def width(self, text, size, track):
        return sum(self.advance(c, size) for c in text) + track * (len(text) - 1)

    def svg(self, text, x, baseline, size, track, fill):
        """Each glyph as an outline path, placed at (x, baseline)."""
        k = size / self.upm
        parts = []
        for ch in text:
            g = self.cmap[ord(ch)]
            pen = SVGPathPen(self.gs)
            self.gs[g].draw(TransformPen(pen, (k, 0, 0, -k, x, baseline)))
            d = pen.getCommands()
            if d:
                parts.append(f'<path d="{d}"/>')
            x += self.advance(ch, size) + track
        return f'<g fill="{fill}">' + ''.join(parts) + '</g>'

    def pil(self, draw, text, x, baseline, size, track, fill):
        font = ImageFont.truetype(self.path, round(size))
        for ch in text:
            draw.text((x, baseline), ch, font=font, fill=fill, anchor='ls')
            x += self.advance(ch, size) + track

BOLD, REG = Face('Barlow-Bold.ttf'), Face('Barlow-Regular.ttf')

def wordmark(w, cx, top, cap_big, cap_small, gap, open_fill, wx_fill, sub_fill, subtitle=True):
    """Returns (svg_fragment, draw_fn) for OPENWX [+ BROADCASTING] centred on cx, cap top at `top`."""
    big = cap_big * BOLD.upm / BOLD.cap
    track_big = big * 0.01
    total = BOLD.width('OPENWX', big, track_big)
    x0 = cx - total / 2
    base = top + cap_big
    x_wx = x0 + BOLD.width('OPEN', big, track_big) + track_big
    frag = [BOLD.svg('OPEN', x0, base, big, track_big, open_fill), BOLD.svg('WX', x_wx, base, big, track_big, wx_fill)]
    small = cap_small * REG.upm / REG.cap
    # BROADCASTING spans about 76% of OPENWX, as in the artwork.
    natural = REG.width('BROADCASTING', small, 0)
    track_small = max(0, (total * 0.76 - natural) / 11)
    sub_w = REG.width('BROADCASTING', small, track_small)
    sub_base = base + gap + cap_small
    if subtitle:
        frag.append(REG.svg('BROADCASTING', cx - sub_w / 2, sub_base, small, track_small, sub_fill))
    def draw(d, scale=1.0):
        s = scale
        BOLD.pil(d, 'OPEN', x0 * s, base * s, big * s, track_big * s, open_fill)
        BOLD.pil(d, 'WX', x_wx * s, base * s, big * s, track_big * s, wx_fill)
        if subtitle:
            REG.pil(d, 'BROADCASTING', (cx - sub_w / 2) * s, sub_base * s, small * s, track_small * s, sub_fill)
    return ''.join(frag), draw

def svg_doc(w, h, body, title):
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" role="img" aria-label="{title}">'
            f'<title>{title}</title>{body}</svg>\n')

def grad(id_):
    return (f'<defs><linearGradient id="{id_}" x1="0" y1="0" x2="0" y2="1">'
            f'<stop offset="0" stop-color="{NAVY_TOP}"/><stop offset="1" stop-color="{NAVY_BOTTOM}"/></linearGradient></defs>')

def save(name, text):
    open(os.path.join(OUT, name), 'w', encoding='utf-8').write(text)

def png(name, w, h, bg, draw_fns, scale, radius=0):
    im = Image.new('RGBA', (round(w * scale), round(h * scale)), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    if bg:
        if radius:
            d.rounded_rectangle((0, 0, im.width - 1, im.height - 1), radius=radius * scale, fill=bg)
        else:
            d.rectangle((0, 0, im.width, im.height), fill=bg)
    for f in draw_fns:
        f(d, scale)
    im.save(os.path.join(OUT, name))
    return im

# 1. Logo on navy, 1024 x 572 (the artwork's canvas).
LW, LH = 1024, 572
frag, draw = wordmark(LW, LW / 2, 196, 130, 54, 34, WHITE, ORANGE, SLATE)
save('openwx-logo.svg', svg_doc(LW, LH, f'<rect width="{LW}" height="{LH}" fill="{NAVY}"/>' + frag, 'OpenWX Broadcasting'))
png('openwx-logo.png', LW, LH, NAVY, [draw], 1.0)
png('openwx-logo@2x.png', LW, LH, NAVY, [draw], 2.0)

# 2. Transparent wordmarks for placing on other backgrounds (tight canvas).
TW, TH = 1000, 330
frag_d, draw_d = wordmark(TW, TW / 2, 30, 150, 62, 40, WHITE, ORANGE, SLATE)
save('openwx-wordmark-on-dark.svg', svg_doc(TW, TH, frag_d, 'OpenWX Broadcasting'))
png('openwx-wordmark-on-dark.png', TW, TH, None, [draw_d], 1.0)
frag_l, draw_l = wordmark(TW, TW / 2, 30, 150, 62, 40, INK, ORANGE, '#5B6B7A')
save('openwx-wordmark-on-light.svg', svg_doc(TW, TH, frag_l, 'OpenWX Broadcasting'))
png('openwx-wordmark-on-light.png', TW, TH, None, [draw_l], 1.0)

# 3. App icon, 1024 squircle-ish rounded square, full wordmark (for 48 px and up).
IS, R = 1024, 200
frag_i, draw_i = wordmark(IS, IS / 2, 410, 145, 58, 32, WHITE, ORANGE, SLATE)
save('openwx-icon.svg', svg_doc(IS, IS, grad('g') + f'<rect width="{IS}" height="{IS}" rx="{R}" fill="url(#g)"/>' + frag_i, 'OpenWX Broadcasting'))
for size in (1024, 512, 256, 128, 64, 48):
    png(f'openwx-icon-{size}.png', IS, IS, NAVY, [draw_i], size / IS, R)

# 4. Small mark (16-32 px): just "WX", heavy and centred; a white bar keeps the OPEN/WX pairing.
MS, MR = 256, 52
cap = 118
big = cap * BOLD.upm / BOLD.cap
tr = big * 0.02
wx_w = BOLD.width('WX', big, tr)
base = (MS + cap) / 2 + 6
bar_y, bar_h = base + 22, 16
mark_body = (grad('m') + f'<rect width="{MS}" height="{MS}" rx="{MR}" fill="url(#m)"/>'
             + BOLD.svg('WX', (MS - wx_w) / 2, base, big, tr, ORANGE)
             + f'<rect x="{(MS - wx_w) / 2:.1f}" y="{bar_y:.1f}" width="{wx_w * 0.42:.1f}" height="{bar_h}" rx="4" fill="{WHITE}"/>')
save('openwx-mark.svg', svg_doc(MS, MS, mark_body, 'OpenWX'))
def draw_mark(d, s):
    BOLD.pil(d, 'WX', (MS - wx_w) / 2 * s, base * s, big * s, tr * s, ORANGE)
    d.rounded_rectangle(((MS - wx_w) / 2 * s, bar_y * s, ((MS - wx_w) / 2 + wx_w * 0.42) * s, (bar_y + bar_h) * s), radius=4 * s, fill=WHITE)
for size in (256, 64, 48, 32, 24, 20, 16):
    png(f'openwx-mark-{size}.png', MS, MS, NAVY, [draw_mark], size / MS, MR)

# 5. Windows .ico: small mark for 16-32, full icon from 48 up.
frames = [Image.open(os.path.join(OUT, f'openwx-mark-{s}.png')) for s in (16, 20, 24, 32, 48)]
frames += [Image.open(os.path.join(OUT, f'openwx-icon-{s}.png')) for s in (64, 128, 256)]
frames[-1].save(os.path.join(OUT, 'openwx.ico'), sizes=[f.size for f in frames], append_images=frames[:-1])
print('done')

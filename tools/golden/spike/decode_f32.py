# Spike: try float32 formulations of the decode to settle the .5 ties.
import sys, os, glob, itertools
import numpy as np
_a = sys.argv[:]
sys.argv = sys.argv[:2]
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "decode_final.py")).read().split("for b in sorted")[0])
f = np.float32

def dec32(blocks, w, h, norm, interp, divmode, outmode):
    bh, bw, _ = blocks.shape
    c0 = blocks[..., 8] | (blocks[..., 9] << 8); c1 = blocks[..., 10] | (blocks[..., 11] << 8)
    def ex(c):
        r = ((c >> 11) & 31).astype(f); g = ((c >> 5) & 63).astype(f); b = (c & 31).astype(f)
        if norm == "unit":
            return np.stack([r / f(31), g / f(63), b / f(31)], -1)
        return np.stack([r * (f(1) / f(31)), g * (f(1) / f(63)), b * (f(1) / f(31))], -1)
    e0, e1 = ex(c0), ex(c1)
    if interp == "div3":
        p2 = (f(2) * e0 + e1) / f(3); p3 = (e0 + f(2) * e1) / f(3)
    elif interp == "mul":
        p2 = e0 * f(2 / 3) + e1 * f(1 / 3); p3 = e0 * f(1 / 3) + e1 * f(2 / 3)
    else:  # lerp
        p2 = e0 + (e1 - e0) * f(1 / 3); p3 = e0 + (e1 - e0) * f(2 / 3)
    pal = np.stack([e0, e1, p2, p3], -2).astype(f)
    cbits = blocks[..., 12] | (blocks[..., 13] << 8) | (blocks[..., 14] << 16) | (blocks[..., 15] << 24)
    ci = np.stack([(cbits >> (2 * i)) & 3 for i in range(16)], -1)
    col = np.take_along_axis(pal, ci[..., None].repeat(3, -1), -2)
    a0 = (blocks[..., 0].astype(f) / f(255)); a1 = (blocks[..., 1].astype(f) / f(255))
    if interp == "div3":
        ap8 = [a0, a1] + [((f(7 - i)) * a0 + f(i) * a1) / f(7) for i in range(1, 7)]
        ap6 = [a0, a1] + [((f(5 - i)) * a0 + f(i) * a1) / f(5) for i in range(1, 5)]
    elif interp == "mul":
        ap8 = [a0, a1] + [a0 * f((7 - i) / 7) + a1 * f(i / 7) for i in range(1, 7)]
        ap6 = [a0, a1] + [a0 * f((5 - i) / 5) + a1 * f(i / 5) for i in range(1, 5)]
    else:
        ap8 = [a0, a1] + [a0 + (a1 - a0) * f(i / 7) for i in range(1, 7)]
        ap6 = [a0, a1] + [a0 + (a1 - a0) * f(i / 5) for i in range(1, 5)]
    ap6 = ap6 + [np.zeros_like(a0), np.ones_like(a0)]
    apal = np.where((blocks[..., 0] > blocks[..., 1])[..., None], np.stack(ap8, -1), np.stack(ap6, -1)).astype(f)
    abits = sum(blocks[..., 2 + k] << (8 * k) for k in range(6))
    ai = np.stack([(abits >> (3 * i)) & 7 for i in range(16)], -1)
    al = np.take_along_axis(apal, ai, -1)
    with np.errstate(divide="ignore", invalid="ignore"):
        if divmode == "div":
            q = col / al[..., None]
        else:
            q = col * (f(1) / al[..., None])
    q = np.where(al[..., None] > 0, q, f(0)).astype(f)
    q = np.minimum(q, f(1))
    rgba = np.concatenate([q, al[..., None]], -1).astype(f)
    if outmode == "add.5":
        o = np.floor((rgba * f(255) + f(0.5)).astype(f))
    elif outmode == "rint":
        o = np.rint((rgba * f(255)).astype(f))
    else:
        o = np.floor((rgba * f(255.999)).astype(f))
    img = o.reshape(bh, bw, 4, 4, 4).transpose(0, 2, 1, 3, 4).reshape(bh * 4, bw * 4, 4)
    return img[:h, :w].astype(np.int64)

bases = sorted(os.path.basename(p)[:-4] for p in glob.glob(os.path.join(sp, "dll", "*_c1.vid")))
data = {b: load2(b) for b in bases}
res = []
for combo in itertools.product(["unit", "recip"], ["div3", "mul", "lerp"], ["div", "recip"], ["add.5", "rint", "trunc"]):
    per = []
    for b in bases:
        w, h, blocks, dec, src, pitch, vlen = data[b]
        per.append(int((dec32(blocks, w, h, *combo) != dec).any(-1).sum()))
    res.append((sum(per), combo, per))
res.sort()
for r in res[:6]:
    print(r)

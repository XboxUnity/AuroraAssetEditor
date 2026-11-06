# Spike: characterise remaining alpha-divide mismatches.
import sys, os
import numpy as np
import glob; _a = sys.argv[:]; sys.argv = sys.argv[:2]
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "decode_variants2.py")).read().split("bases = sorted")[0])

def decode_raw(w, h, blocks):
    bh, bw, _ = blocks.shape
    c0 = blocks[..., 8] | (blocks[..., 9] << 8); c1 = blocks[..., 10] | (blocks[..., 11] << 8)
    e0, e1 = expand(c0, "float"), expand(c1, "float")
    pal = np.stack([e0, e1, (2 * e0 + e1) / 3, (e0 + 2 * e1) / 3], -2)
    cbits = blocks[..., 12] | (blocks[..., 13] << 8) | (blocks[..., 14] << 16) | (blocks[..., 15] << 24)
    ci = np.stack([(cbits >> (2 * i)) & 3 for i in range(16)], -1)
    col = np.take_along_axis(pal, ci[..., None].repeat(3, -1), -2)
    a0 = blocks[..., 0].astype(np.float64); a1 = blocks[..., 1].astype(np.float64)
    ap8 = np.stack([a0, a1] + [((7 - i) * a0 + i * a1) / 7 for i in range(1, 7)], -1)
    ap6 = np.stack([a0, a1] + [((5 - i) * a0 + i * a1) / 5 for i in range(1, 5)] + [np.zeros_like(a0), np.full_like(a0, 255)], -1)
    apal = np.where((a0 > a1)[..., None], ap8, ap6)
    abits = sum(blocks[..., 2 + k] << (8 * k) for k in range(6))
    ai = np.stack([(abits >> (3 * i)) & 7 for i in range(16)], -1)
    al = np.take_along_axis(apal, ai, -1)
    rgba = np.concatenate([col, al[..., None]], -1)
    return rgba.reshape(bh, bw, 4, 4, 4).transpose(0, 2, 1, 3, 4).reshape(bh * 4, bw * 4, 4)[:h, :w]

PITCH = {256: 1024, 1280: 5120, 420: 2048, 900: 4096, 64: int(_a[2]), 1000: 4096}
for b in sorted(os.path.basename(p)[:-4] for p in glob.glob(os.path.join(sp, "dll", "*_c1.vid"))):
    w, h, blocks, dec, src = load(os.path.join(b))
    raw = decode_raw(w, h, blocks)
    col, al = raw[..., :3], raw[..., 3]
    for name, f in {
        "div_clamp": lambda c, a: np.where(a[..., None] > 0, np.minimum(c * 255 / np.maximum(a[..., None], 1e-9), 255), c),
        "div_a/255": lambda c, a: np.where(a[..., None] > 0, np.minimum(c / (a[..., None] / 255), 255), c),
        "div_zero->0": lambda c, a: np.where(a[..., None] > 0, np.minimum(c * 255 / np.maximum(a[..., None], 1e-9), 255), 0),
    }.items():
        out = np.concatenate([np.round(f(col, al)), np.round(al)[..., None]], -1).astype(np.int64)
        bad = (out != dec).any(-1)
        print(f"{b:28} {name:12} bad={int(bad.sum())}")
    out = np.concatenate([np.round(np.where(al[..., None] > 0, np.minimum(col * 255 / np.maximum(al[..., None], 1e-9), 255), col)), np.round(al)[..., None]], -1).astype(np.int64)
    bad = np.argwhere((out != dec).any(-1))[:6]
    for y, x in bad:
        print(f"   ({y},{x}) raw={np.round(raw[y,x],3).tolist()} mine={out[y,x].tolist()} dll={dec[y,x].tolist()} src={src[y,x].tolist()}")



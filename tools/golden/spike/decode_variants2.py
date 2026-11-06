# Spike: find the exact BC3 decode arithmetic (565 expansion, interpolation rounding,
# alpha handling) that reproduces the DLL's ConvertAssetToImage output bit-for-bit.
import sys, os, glob
import numpy as np

sp = sys.argv[1]

def load(base):
    name, wh, _ = base.rsplit("_", 2)
    w, h = map(int, wh.split("x"))
    vid = np.frombuffer(open(os.path.join(sp, "dll", base + ".vid"), "rb").read(), np.uint8)
    vid = vid.reshape(-1, 2)[:, ::-1].reshape(-1)  # 8in16 swap
    bw, bh = (w + 3) // 4, (h + 3) // 4
    pitch = PITCH[w]
    blocks = np.stack([vid[by * pitch: by * pitch + bw * 16].reshape(bw, 16) for by in range(bh)])  # bh,bw,16
    dec = np.frombuffer(open(os.path.join(sp, "dll", base + ".dec"), "rb").read(), np.uint8).reshape(h, w, 4)[:, :, [2, 1, 0, 3]].astype(np.int64)
    src = np.frombuffer(open(os.path.join(sp, "in", f"{name}_{wh}.argb"), "rb").read(), np.uint8).reshape(h, w, 4)[:, :, [1, 2, 3, 0]].astype(np.int64)
    return w, h, blocks.astype(np.int64), dec, src

def expand(c, mode):
    r = (c >> 11) & 31; g = (c >> 5) & 63; b = c & 31
    if mode == "rep":
        return np.stack([(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)], -1).astype(np.float64)
    if mode == "float":
        return np.stack([r * 255 / 31, g * 255 / 63, b * 255 / 31], -1)

def rnd(x, mode):
    if mode == "round": return np.floor(x + 0.5)
    if mode == "trunc": return np.floor(x + 1e-9)
    if mode == "rne": return np.round(x)

def decode(w, h, blocks, exp_mode, interp_mode, alpha_mode, out_mode):
    bh, bw, _ = blocks.shape
    c0 = blocks[..., 8] | (blocks[..., 9] << 8)
    c1 = blocks[..., 10] | (blocks[..., 11] << 8)
    e0, e1 = expand(c0, exp_mode), expand(c1, exp_mode)
    pal = np.stack([e0, e1, (2 * e0 + e1) / 3, (e0 + 2 * e1) / 3], -2)  # bh,bw,4,3
    cbits = blocks[..., 12] | (blocks[..., 13] << 8) | (blocks[..., 14] << 16) | (blocks[..., 15] << 24)
    ci = np.stack([(cbits >> (2 * i)) & 3 for i in range(16)], -1)
    col = np.take_along_axis(pal, ci[..., None].repeat(3, -1), -2)  # bh,bw,16,3
    a0 = blocks[..., 0].astype(np.float64); a1 = blocks[..., 1].astype(np.float64)
    big = a0 > a1
    ap8 = np.stack([a0, a1] + [((7 - i) * a0 + i * a1) / 7 for i in range(1, 7)], -1)
    ap6 = np.stack([a0, a1] + [((5 - i) * a0 + i * a1) / 5 for i in range(1, 5)] + [np.zeros_like(a0), np.full_like(a0, 255)], -1)
    apal = np.where(big[..., None], ap8, ap6)
    abits = sum(blocks[..., 2 + k] << (8 * k) for k in range(6))
    ai = np.stack([(abits >> (3 * i)) & 7 for i in range(16)], -1)
    al = np.take_along_axis(apal, ai, -1)
    if alpha_mode == "q":  # quantize alpha before using it
        al = rnd(al, interp_mode)
    col = rnd(col, interp_mode) if out_mode == "int" else col
    if alpha_mode in ("mul", "q_mul"):
        aq = rnd(al, interp_mode) if alpha_mode == "q_mul" else al
        col = col * aq[..., None] / 255
    if alpha_mode in ("div",):
        col = np.where(al[..., None] > 0, np.minimum(col * 255 / np.maximum(al[..., None], 1), 255), col)
    rgba = np.concatenate([rnd(col, interp_mode), rnd(al, interp_mode)[..., None]], -1)
    img = rgba.reshape(bh, bw, 4, 4, 4).transpose(0, 2, 1, 3, 4).reshape(bh * 4, bw * 4, 4)
    return img[:h, :w].astype(np.int64)

bases = sorted(os.path.basename(p)[:-4] for p in glob.glob(os.path.join(sp, "dll", "*_c1.vid")))
data = {b: load(b) for b in bases}
results = []
for exp_mode in ("rep", "float"):
    for interp_mode in ("round", "trunc", "rne"):
        for alpha_mode in ("none", "mul", "q_mul", "div"):
            for out_mode in ("int", "float"):
                tot_bad = 0; worst = 0; per = []
                for b in bases:
                    w, h, blocks, dec, src = data[b]
                    img = decode(w, h, blocks, exp_mode, interp_mode, alpha_mode, out_mode)
                    bad = int((img != dec).any(-1).sum())
                    tot_bad += bad; worst = max(worst, int(np.abs(img - dec).max())); per.append(bad)
                results.append((tot_bad, worst, exp_mode, interp_mode, alpha_mode, out_mode, per))
results.sort()
print("files:", bases)
for r in results[:8]:
    print(f"badpx={r[0]:8} maxdiff={r[1]:3} exp={r[2]:5} interp={r[3]:5} alpha={r[4]:5} out={r[5]:5} per-file={r[6]}")

# Alpha channel alone, and a peek at how the DLL treats colour vs alpha in a semi-transparent block.
w, h, blocks, dec, src = data["ph_cover_900x600_c1"]
print("cover src rgba[0,0..3]:", src[0, :4].tolist())
print("cover dll rgba[0,0..3]:", dec[0, :4].tolist())
print("cover blk0:", blocks[0, 0].tolist())


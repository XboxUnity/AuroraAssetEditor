# Spike: decode the DLL's DXT4/5 video data with a plain-Python BC3 decoder and compare
# against the DLL's own ConvertAssetToImage output (.dec, BGRA). Also reports layout
# (row pitch, padded rows), block-mode stats and PSNR of the DLL encode vs. source.
import sys, os, glob, struct
import numpy as np

sp = sys.argv[1]

def swap16(b):
    a = np.frombuffer(b, np.uint8).reshape(-1, 2)[:, ::-1]
    return a.reshape(-1).tobytes()

def exp565(c):
    r = (c >> 11) & 31; g = (c >> 5) & 63; b = c & 31
    return np.array([(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)], np.int32)

def decode_block(blk, variant):
    a0, a1 = blk[0], blk[1]
    abits = int.from_bytes(blk[2:8], "little")
    if a0 > a1:
        av = [a0, a1] + [((7 - i) * a0 + i * a1 + variant["ar"]) // 7 for i in range(1, 7)]
    else:
        av = [a0, a1] + [((5 - i) * a0 + i * a1 + variant["ar"]) // 5 for i in range(1, 5)] + [0, 255]
    c0, c1 = struct.unpack("<HH", blk[8:12])
    e0, e1 = exp565(c0), exp565(c1)
    # BC3 colour is always 4-colour mode regardless of c0/c1 order
    cv = [e0, e1, (2 * e0 + e1 + variant["cr"]) // 3, (e0 + 2 * e1 + variant["cr"]) // 3]
    cbits = int.from_bytes(blk[12:16], "little")
    out = np.zeros((16, 4), np.int32)
    for i in range(16):
        out[i, :3] = cv[(cbits >> (2 * i)) & 3]
        out[i, 3] = av[(abits >> (3 * i)) & 7]
    return out, (c0 > c1), (a0 > a1)

variants = {"round(+1/+3)": {"cr": 1, "ar": 3}, "trunc": {"cr": 0, "ar": 0}}
for vid in sorted(glob.glob(os.path.join(sp, "dll", "*_c1.vid"))):
    base = os.path.basename(vid)[:-4]
    name, wh, _ = base.rsplit("_", 2)
    w, h = map(int, wh.split("x"))
    bw, bh = (w + 3) // 4, (h + 3) // 4
    raw = swap16(open(vid, "rb").read())
    dec = np.frombuffer(open(vid[:-4] + ".dec", "rb").read(), np.uint8).reshape(h, w, 4)  # BGRA
    src = np.frombuffer(open(os.path.join(sp, "in", f"{name}_{wh}.argb"), "rb").read(), np.uint8).reshape(h, w, 4)  # ARGB
    # Row pitch in bytes = total / padded block rows; try candidate paddings.
    pitch = None
    for pad_rows in range(bh, bh + 64):
        if len(raw) % pad_rows == 0 and len(raw) // pad_rows >= bw * 16 and (len(raw) // pad_rows) % 16 == 0:
            pitch = len(raw) // pad_rows
            if pad_rows % 8 == 0:
                break
    res = {}
    for vn, v in variants.items():
        img = np.zeros((bh * 4, bw * 4, 4), np.int32)
        modes = [0, 0]
        for by in range(bh):
            for bx in range(bw):
                off = by * pitch + bx * 16
                o, cm, am = decode_block(raw[off:off + 16], v)
                modes[0] += cm; modes[1] += am
                img[by * 4:by * 4 + 4, bx * 4:bx * 4 + 4] = o.reshape(4, 4, 4)
        img = img[:h, :w]
        mine_bgra = img[:, :, [2, 1, 0, 3]]
        res[vn] = int(np.abs(mine_bgra - dec.astype(np.int32)).max())
    src_rgba = src[:, :, [1, 2, 3, 0]].astype(np.float64)
    dec_rgba = dec[:, :, [2, 1, 0, 3]].astype(np.float64)
    mse_rgb = ((src_rgba[..., :3] - dec_rgba[..., :3]) ** 2).mean()
    psnr = 10 * np.log10(255 ** 2 / mse_rgb) if mse_rgb else float("inf")
    print(f"{base:28} pitch={pitch} rows={len(raw)//pitch} maxdiff vs DLL decode: {res}  c0>c1 blocks={modes[0]}/{bw*bh} a0>a1={modes[1]}  DLL PSNR(rgb)={psnr:.2f}")

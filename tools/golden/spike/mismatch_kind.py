# Spike: characterise blocks where DirectXTex (dither_rgb) differs from the DLL.
import sys, os, glob
import numpy as np
sp = sys.argv[1]

def dll_blocks(base, w, h):
    vid = np.frombuffer(open(os.path.join(sp, "dll", base + "_c1.vid"), "rb").read(), np.uint8)
    vid = vid.reshape(-1, 2)[:, ::-1].reshape(-1)
    bw, bh = (w + 3) // 4, (h + 3) // 4
    pitch = ((bw + 31) // 32) * 32 * 16
    return np.stack([vid[by * pitch: by * pitch + bw * 16].reshape(bw, 16) for by in range(bh)]).astype(np.int64)

def ep(blk):
    c0 = blk[..., 8] | (blk[..., 9] << 8); c1 = blk[..., 10] | (blk[..., 11] << 8)
    f = lambda c: np.stack([(c >> 11) & 31, (c >> 5) & 63, c & 31], -1)
    return f(c0), f(c1)

for argb in sorted(glob.glob(os.path.join(sp, "in", "*.argb"))):
    base = os.path.basename(argb)[:-5]
    w, h = map(int, base.rsplit("_", 1)[1].split("x"))
    bw, bh = w // 4, h // 4
    ref = dll_blocks(base, w, h)[:bh, :bw]
    for fl in ("dither_rgb", "dither_rgba"):
        mine = np.frombuffer(open(os.path.join(sp, "dxtex", f"{base}_{fl}.bc3"), "rb").read(), np.uint8).reshape((h + 3) // 4, (w + 3) // 4, 16)[:bh, :bw].astype(np.int64)
        cbad = ~(mine[..., 8:] == ref[..., 8:]).all(-1)
        abad = ~(mine[..., :8] == ref[..., :8]).all(-1)
        msg = f"{base:22} {fl:12} colour-bad={int(cbad.sum()):5}"
        if cbad.any():
            r0, r1 = ep(ref[cbad]); m0, m1 = ep(mine[cbad])
            same_ep = ((r0 == m0).all(-1) & (r1 == m1).all(-1)).mean() * 100
            swapped = ((r0 == m1).all(-1) & (r1 == m0).all(-1)).mean() * 100
            d = np.maximum(np.abs(r0 - m0).max(-1), np.abs(r1 - m1).max(-1))
            msg += f" [same endpoints, other indices {same_ep:.0f}% | swapped {swapped:.0f}% | endpoint maxdelta {np.bincount(d, minlength=4)[:4].tolist()}...]"
        msg += f"  alpha-bad={int(abad.sum()):5}"
        if abad.any():
            ra, ma = ref[abad][:, :2], mine[abad][:, :2]
            msg += f" [same a0/a1 {((ra == ma).all(-1)).mean() * 100:.0f}% | a-delta max {int(np.abs(ra - ma).max())}]"
        print(msg)

# Spike: candidate exact decoder (pitch = align(blocks_w, 32) * 16, 8in16 swap,
# float 565 expand + float interp, colour /= alpha (alpha==0 -> colour 0), round).
import sys, os, glob
import numpy as np
_a = sys.argv[:]
sys.argv = sys.argv[:2]
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "decode_alpha2.py")).read().split("PITCH = ")[0])
sp = _a[1]

def load2(base):
    name, wh, _ = base.rsplit("_", 2)
    w, h = map(int, wh.split("x"))
    vid = np.frombuffer(open(os.path.join(sp, "dll", base + ".vid"), "rb").read(), np.uint8)
    vid = vid.reshape(-1, 2)[:, ::-1].reshape(-1).astype(np.int64)
    bw, bh = (w + 3) // 4, (h + 3) // 4
    pitch = ((bw + 31) // 32) * 32 * 16
    blocks = np.stack([vid[by * pitch: by * pitch + bw * 16].reshape(bw, 16) for by in range(bh)])
    dec = np.frombuffer(open(os.path.join(sp, "dll", base + ".dec"), "rb").read(), np.uint8).reshape(h, w, 4)[:, :, [2, 1, 0, 3]].astype(np.int64)
    src = np.frombuffer(open(os.path.join(sp, "in", f"{name}_{wh}.argb"), "rb").read(), np.uint8).reshape(h, w, 4)[:, :, [1, 2, 3, 0]].astype(np.int64)
    return w, h, blocks, dec, src, pitch, len(vid)

for b in sorted(os.path.basename(p)[:-4] for p in glob.glob(os.path.join(sp, "dll", "*_c1.vid"))):
    w, h, blocks, dec, src, pitch, vlen = load2(b)
    raw = decode_raw(w, h, blocks)
    col, al = raw[..., :3], raw[..., 3]
    q = col * 255 / np.maximum(al[..., None], 1e-9)
    out = np.concatenate([np.round(np.where(al[..., None] > 0, np.minimum(q, 255), 0)), np.round(al)[..., None]], -1).astype(np.int64)
    bad = np.argwhere((out != dec).any(-1))
    print(f"{b:28} pitch={pitch} padded_rows={vlen // pitch} bad={len(bad)}")
    for y, x in bad[:4]:
        print(f"   ({y},{x}) raw={np.round(raw[y, x], 4).tolist()} q={np.round(q[y, x], 4).tolist()} mine={out[y, x].tolist()} dll={dec[y, x].tolist()}")

# Spike: locate where each 4x4 block of the 64x64 icon lives in the DLL's video data.
import sys, os, glob
import numpy as np
_a = sys.argv[:]
sys.argv = sys.argv[:2]
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "decode_alpha2.py")).read().split("PITCH = ")[0])
sp = _a[1]
base = "ph_icon_64x64_c1"
w = h = 64
vid = np.frombuffer(open(os.path.join(sp, "dll", base + ".vid"), "rb").read(), np.uint8)
vid = vid.reshape(-1, 2)[:, ::-1].reshape(-1).astype(np.int64)
chunks = vid.reshape(-1, 16)  # all 16-byte chunks
n = len(chunks)
blk = chunks.reshape(1, n, 16)
raw = decode_raw(4, 4 * n, blk.reshape(n, 1, 16).transpose(0, 1, 2)) if False else None
# decode each chunk as its own 4x4 block
dec_chunks = []
for i in range(n):
    r = decode_raw(4, 4, chunks[i].reshape(1, 1, 16))
    col, al = r[..., :3], r[..., 3]
    out = np.concatenate([np.round(np.where(al[..., None] > 0, np.minimum(col * 255 / np.maximum(al[..., None], 1e-9), 255), 0)), np.round(al)[..., None]], -1).astype(np.int64)
    dec_chunks.append(out)
dec = np.frombuffer(open(os.path.join(sp, "dll", base + ".dec"), "rb").read(), np.uint8).reshape(h, w, 4)[:, :, [2, 1, 0, 3]].astype(np.int64)
grid = np.full((16, 16), -1)
for by in range(16):
    for bx in range(16):
        tgt = dec[by * 4:by * 4 + 4, bx * 4:bx * 4 + 4]
        hits = [i for i in range(n) if np.array_equal(dec_chunks[i], tgt)]
        uniq = len(np.unique(tgt.reshape(-1, 4), axis=0))
        grid[by, bx] = hits[0] if len(hits) == 1 else (-2 if len(hits) > 1 else -1)
print("chunk index per block (-1 none, -2 ambiguous):")
for row in grid:
    print(" ".join(f"{v:4}" for v in row))
nz = [i for i in range(n) if chunks[i].any()]
print("non-zero chunks:", len(nz), "first/last:", nz[:5], nz[-5:])

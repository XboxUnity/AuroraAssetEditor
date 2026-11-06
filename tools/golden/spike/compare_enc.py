# Spike: compare DirectXTex D3DXEncodeBC3 output against AuroraAsset.dll DXT4/5 blocks.
# usage: compare_enc.py <spike dir> [bin subdir] [only flags csv]
import sys, os, glob, subprocess
import numpy as np

sp = sys.argv[1]
bindir = sys.argv[2] if len(sys.argv) > 2 else "bin64"
exe = os.path.join(sp, bindir, "dxtex_enc.exe")
outdir = os.path.join(sp, "dxtex" if bindir == "bin64" else "dxtex_" + bindir)
os.makedirs(outdir, exist_ok=True)
FLAGS = {"none": 0x0, "uniform": 0x40000, "dither_rgb": 0x10000, "dither_rgba": 0x30000, "uniform+dither": 0x70000}
if len(sys.argv) > 3:
    FLAGS = {k: v for k, v in FLAGS.items() if k in sys.argv[3].split(",")}

def dll_blocks(base, w, h):
    vid = np.frombuffer(open(os.path.join(sp, "dll", base + "_c1.vid"), "rb").read(), np.uint8)
    vid = vid.reshape(-1, 2)[:, ::-1].reshape(-1)
    bw, bh = (w + 3) // 4, (h + 3) // 4
    pitch = ((bw + 31) // 32) * 32 * 16
    return np.stack([vid[by * pitch: by * pitch + bw * 16].reshape(bw, 16) for by in range(bh)])

print(f"{'image':22} {'flags':15} {'full-blk':>9} {'colour':>8} {'alpha':>8}  (exact-match % over interior blocks)")
for argb in sorted(glob.glob(os.path.join(sp, "in", "*.argb"))):
    base = os.path.basename(argb)[:-5]
    name, wh = base.rsplit("_", 1)
    w, h = map(int, wh.split("x"))
    ref = dll_blocks(base, w, h)
    bw, bh = w // 4, h // 4  # interior (full) blocks only
    ref_i = ref[:bh, :bw]
    for fname, fl in FLAGS.items():
        out = os.path.join(outdir, f"{base}_{fname}.bc3")
        r = subprocess.run([exe, argb, str(w), str(h), f"{fl:x}", out], capture_output=True, text=True)
        if r.returncode != 0:
            print(f"{base:22} {fname:15} ENCODER FAILED rc={r.returncode} {r.stderr.strip()}")
            continue
        mine = np.frombuffer(open(out, "rb").read(), np.uint8).reshape((h + 3) // 4, (w + 3) // 4, 16)[:bh, :bw]
        full = (mine == ref_i).all(-1).mean() * 100
        col = (mine[..., 8:] == ref_i[..., 8:]).all(-1).mean() * 100
        alp = (mine[..., :8] == ref_i[..., :8]).all(-1).mean() * 100
        print(f"{base:22} {fname:15} {full:8.2f}% {col:7.2f}% {alp:7.2f}%")


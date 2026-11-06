# Spike: dump header fields and check DLL decode-of-own-output vs input.
import sys, os, struct, glob
import numpy as np
sp = sys.argv[1]
for hdr in sorted(glob.glob(os.path.join(sp, "dll", "*.hdr"))):
    pre = hdr[:-4]
    base = os.path.basename(pre)
    h = open(hdr, "rb").read()
    be = struct.unpack(">13I", h)
    fc = be[7:13]
    tiled = fc[0] & 1
    pitch = (fc[0] >> 1) & 0x1FF
    endian = (fc[1] >> 6) & 3
    fmt = fc[1] & 0x3F
    w = (fc[2] & 0x1FFF) + 1
    hh = ((fc[2] >> 13) & 0x1FFF) + 1
    vid = open(pre + ".vid", "rb").read()
    dec = np.frombuffer(open(pre + ".dec", "rb").read(), np.uint8)
    name, wh, c = base.rsplit("_", 2)
    src = np.frombuffer(open(os.path.join(sp, "in", f"{name}_{wh}.argb"), "rb").read(), np.uint8)
    same = np.array_equal(src, dec)
    diff = "" if same else f" maxdiff={np.abs(src.astype(int)-dec.astype(int)).max()} bytes0={src[:8].tolist()} dec0={dec[:8].tolist()}"
    print(f"{base:28} fmt=0x{fmt:02X} endian={endian} tiled={tiled} pitch={pitch} ({pitch*32}) size={w}x{hh} vid={len(vid)} dec==src:{same}{diff}")
    if base.startswith("grad_256x256"):
        print("   hdr dwords:", " ".join(f"{x:08X}" for x in be))
        print("   vid[0:32]:", vid[:32].hex(" "))

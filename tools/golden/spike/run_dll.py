# Spike: run the 32-bit harness on every <work>/in/<name>_<w>x<h>.argb, uncompressed (c0) and
# compressed (c1), writing <work>/dll/<name>_<w>x<h>_c<0|1>.hdr/.vid/.dec.
# usage: run_dll.py <work folder>
import sys, os, glob, subprocess

work = sys.argv[1]
exe = os.path.join(work, "bin32", "dllharness.exe")
out = os.path.join(work, "dll")
os.makedirs(out, exist_ok=True)
for argb in sorted(glob.glob(os.path.join(work, "in", "*.argb"))):
    base = os.path.basename(argb)[:-5]
    w, h = base.rsplit("_", 1)[1].split("x")
    for comp in ("0", "1"):
        subprocess.run([exe, argb, w, h, comp, os.path.join(out, f"{base}_c{comp}")], check=True)

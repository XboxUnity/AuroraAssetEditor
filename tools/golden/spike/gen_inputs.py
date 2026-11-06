# Spike: generate test images as raw ARGB (A,R,G,B byte order per pixel, exactly what
# AuroraAsset.cs ImageToRawArgb feeds the DLL). Writes <name>_<w>x<h>.argb into out dir.
import sys, os
import numpy as np
from PIL import Image

out = sys.argv[1]
placeholders = sys.argv[2]
os.makedirs(out, exist_ok=True)

def save(name, rgba):
    h, w, _ = rgba.shape
    argb = rgba[:, :, [3, 0, 1, 2]].astype(np.uint8)
    with open(os.path.join(out, f"{name}_{w}x{h}.argb"), "wb") as f:
        f.write(argb.tobytes())

# Real content: the app's own placeholder images, at the sizes the app resizes to.
sizes = {"cover": (900, 600), "background": (1280, 720), "banner": (420, 96), "icon": (64, 64), "screenshot": (1000, 562)}
for n, (w, h) in sizes.items():
    img = Image.open(os.path.join(placeholders, f"{n}.png")).convert("RGBA").resize((w, h), Image.LANCZOS)
    save(f"ph_{n}", np.array(img))

rng = np.random.default_rng(1234)
W = H = 256
y, x = np.mgrid[0:H, 0:W]
opaque = np.full((H, W), 255)

grad = np.stack([x, y, (x + y) // 2, opaque], -1)
save("grad", grad)
noise = np.concatenate([rng.integers(0, 256, (H, W, 3)), opaque[..., None]], -1)
save("noise", noise)
sines = np.stack([127 + 127 * np.sin(x / 9.0), 127 + 127 * np.sin(y / 13.0), 127 + 127 * np.sin((x + y) / 21.0), opaque], -1).astype(int)
save("sines", sines)
alpha = np.stack([x, y, 255 - x, (x + y) // 2], -1)
save("alphagrad", alpha)
alphanoise = rng.integers(0, 256, (H, W, 4))
save("alphanoise", alphanoise)
print("ok")

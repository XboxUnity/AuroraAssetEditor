# How AuroraAsset.dll encodes BC3 ("Use Compression")

Found by disassembling `tools/golden/dll/AuroraAsset.dll` (rizin 0.4.1, image base `0x10000000`)
and checking each finding against the golden corpus with
`cargo test -p aurora-asset --test golden_dxt_encode report_match -- --nocapture`. The DLL
statically links the XDK `XGCopySurface` compressor, which is the D3DX BC encoder that DirectXTex
later inherited. The port in `crates/aurora-asset/src/texture/bc3_encode` is DirectXTex `BC.cpp`
with the differences below. Addresses are virtual addresses in the DLL.

## Routines

| Address | What it does |
|---|---|
| `0x10010f98` | Builds the per-format codec object. For DXT4 and DXT5 it installs `0x10014577` as the block encoder. |
| `0x100113xx` (loop ending at `0x1001151d`) | Walks the surface in 4x4 blocks, copies each block's 16 float texels into a buffer, fills a partial block, and calls the block encoder with a dither flag. |
| `0x1000ed18` | Loads an A8R8G8B8 row into float texels. |
| `0x10014577` | DXT4/DXT5 entry: a global premultiply flag (`flags & 1` of the copy, set at `0x10004fe3`) picks the premultiplying path `0x100144eb` (colour times alpha first, `0x100123c4`) or the plain path `0x1001442a`. |
| `0x1001442a` | One BC3 block: the colour half (`0x1001355f`, threshold 0), then the alpha half (`0x10012a6e`), passing the dither flag to both. |
| `0x10012a6e` | Alpha half: quantize, choose 6-step or 8-step mode, call the endpoint optimizer, pick indices. |
| `0x100123fc` | Alpha endpoint optimizer (Newton iterations). Step count arrives in a register. |
| `0x1001355f` | Colour half (BC1 four-colour mode). |
| `0x10012f56` | Colour endpoint optimizer, called from the colour half. |

## Findings

### Dither flag

The block loop passes one flag to the block encoder: 1 when the copy dithers (it does for
Aurora's call), else 0. The alpha half reads the same flag as the colour half, so alpha is
dithered too. DirectXTex keeps `BC_FLAGS_DITHER_A` separate; the port turns it on.

- Measured: whole-block match on `alphagrad` went from 0% to 92.6% (equal to its colour rate),
  `alphanoise` 41% to 96%, `ph_banner` 94.4% to 96.6%, `ph_icon` 97.3% to 99.6%.

### Partial blocks

When fewer than 4 columns of a block lie inside the image, the loop fills column `c` (from the
first missing one up to 3) by copying column `[0, 0, 0, 1][c]` of the same block, which may itself
have been filled already. Missing rows are filled the same way from whole rows afterwards. So two
columns inside give `c0 c1 c0 c1` and three give `c0 c1 c2 c1`; it is not a clamp to the edge.

- Measured: `odd_63x65` whole-block 45% to 97.8%, `odd_130x70` 35% to 95.8%.

### Premultiply

The plain path is the one Aurora's call takes: the stored colour is straight (not premultiplied),
which the spike also observed. The port has no premultiply step.

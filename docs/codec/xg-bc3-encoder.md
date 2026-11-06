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
dithered too. DirectXTex keeps `BC_FLAGS_DITHER_A` separate; the port has one `dither`
option, and no uniform-weighting option, since the DLL's colour half always uses the
luminance weights (`0x1001c110`).

- Measured: whole-block match on `alphagrad` went from 0% to 92.6% (equal to its colour rate),
  `alphanoise` 41% to 96%, `ph_banner` 94.4% to 96.6%, `ph_icon` 97.3% to 99.6%.

### Partial blocks

When fewer than 4 columns of a block lie inside the image, the loop fills column `c` (from the
first missing one up to 3) by copying column `[0, 0, 0, 1][c]` of the same block, which may itself
have been filled already. Missing rows are filled the same way from whole rows afterwards. So two
columns inside give `c0 c1 c0 c1` and three give `c0 c1 c2 c1`; it is not a clamp to the edge.

- Measured: `odd_63x65` whole-block 45% to 97.8%, `odd_130x70` 35% to 95.8%.

### Floating-point precision and rounding control

The encoder is x87 code. Two properties of it change results against `f32` arithmetic:

1. **Double precision until a store.** The FPU runs at 53-bit precision (the default control word,
   `0x27F`), and constants are `f32` values widened to `double`. An expression such as
   `alpha * 255 + 0.5` is rounded once, when it is stored to an `f32` slot, not after every
   operation. A single operation on two `f32` values gives the same result either way.
2. **Truncating loops.** Before some loops the code sets the rounding-control bits (`or 0xc00`)
   and restores them after the loop, not just around the `fistp`. Every operation and every
   `f32` store inside such a loop rounds toward zero. That moved, for example, an alpha index
   position of exactly 5.0 to just below it, which selects code 1 instead of the reserved
   code 7 for an opaque texel in a six-step block (the commonest alpha mismatch before this).

The port models both with `bc3_encode/x87.rs`: `Fpu::load` starts an expression in a given
rounding mode, and `store()` rounds it to `f32` the way the DLL does.

Alpha regions (addresses in `0x10012a6e` unless noted):

| Code | Rounding |
|---|---|
| Quantize loop, 8-bit level, dither error (`0x10012aaf`..`0x10012bba`) | toward zero |
| Endpoint optimizer iterations (`0x100123fc`: `0x10012505`..`0x100126e6`) | toward zero |
| Endpoints to bytes (`0x10012c30`..`0x10012c6f`) | toward zero |
| Byte endpoints back to float, step interpolation `((n-i)*a + i*b) * (1/n)`, index scale | nearest |
| Index loop, position, dither error (`0x10012dc3`..`0x10012f3d`) | toward zero |

Colour regions (addresses in `0x1001355f` unless noted):

| Code | Rounding |
|---|---|
| Prepare loop: dither, 565 level `(c*31 + 0.5)` and back, luminance weighting (`0x1001362f`..`0x10013a7c`) | toward zero |
| Bounding box, axis length, diagonal choice (`0x10012f56` up to `0x10013241`) | nearest |
| Endpoint optimizer iterations (`0x1001324f`..`0x10013520`) | toward zero |
| Weighted endpoints back to RGB, palette (SSE, `f32`) | nearest |
| Endpoint to 565, `c*31 + 0.5` (`0x1001222f`) | toward zero |
| Index scale `3 / (axis . axis)` (one expression, not stored in between) | nearest |
| Index loop: weighting, dither, projection (`0x10013db1`..`0x1001412a`) | toward zero |

Expression shapes that matter, all evaluated before the store: projections are
`((p.g - o.g)*d.g + (p.r - o.r)*d.r) + (p.b - o.b)*d.b`; lengths are `(r*r + g*g) + b*b`; the
diagonal sums are `(g + r) ± b` and `(r - g) ± b`; palette steps in the optimizer are
`w_high*high + w_low*low`; error diffusion is `error + difference * weight`.

- Measured, alpha model alone (colour still `f32`): every fixture's whole-block match equals its
  colour-half match; placeholders 100% whole-block.
- Measured, colour model added: 100% whole-block on every fixture, including the two edge-block
  fixtures and the sub-block sizes (`odd_1x1`, `odd_3x5`). `dxt_encode_matches_dll_on_all_fixtures`
  passes and is no longer ignored.
- A unit test that fed alpha 0.5 expected endpoints `0x80, 0xff` (DirectXTex). The DLL itself
  gives `0x7f, 0x80` for that input (checked with the block probe below); the test now feeds
  the byte value 128 as the DLL loads it, for which the DLL gives `0x80, 0xff`.

Theories tested on the way:

- Double precision alone, rounding to nearest everywhere (alpha half): `alphanoise` whole-block
  96.1% to 96.5%, `odd_130x70` 95.8% to 96.3%. Real but small; it did not explain the reserved-code
  mismatches. Kept, because the truncation model needs it.
- Brute force with the probe over a 6-step block (opaque texels plus one alpha `lo`): the DLL
  picked code 1 over code 7 for 116 of 254 values of `lo`, round-to-nearest predicted 2. Every
  case is explained by the truncating loop.

### Byte loading (ruling R19)

The A8R8G8B8 loader multiplies each byte by the `f32` constant `1/255` (stored as a double,
`0x1001ac20`) and stores the product as `f32`. That is `n * (1.0f / 255.0f)`, not `n / 255`; the
two differ by one ulp on 126 of the 256 values. No lookup table and no extra precision is
involved.

- Measured with the alpha model: keeping `n / 255` instead drops `ph_cover` to 98.9% and
  `ph_screenshot` to 99.0% whole-block.

### Known deviation: colour optimizer projection below the palette

The DLL's colour endpoint optimizer has no `position <= 0` test before
`trunc(position + 0.5)`. For positions in (-1.5, 0] that gives step 0, the same as DirectXTex.
At -1.5 or below it indexes before its step and weight tables and reads neighbouring stack
values and constants. No golden block reaches that case (a check that panicked on it never
fired across the corpus), so the port keeps step 0 there. If a future golden fails on it, this
is the place.

### Checking a block against the DLL

`0x1001442a` is `stdcall (u8 *block, const float texels[16][4] /* r,g,b,a */, u32 dither)`. A
32-bit probe that loads the DLL and calls `base + 0x1442a` with float texels reproduces the
golden blocks exactly, which makes it an oracle for any float input.

### Premultiply

The plain path is the one Aurora's call takes: the stored colour is straight (not premultiplied),
which the spike also observed. The port has no premultiply step.

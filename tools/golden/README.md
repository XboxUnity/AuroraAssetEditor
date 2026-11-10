# Golden reference tooling

Test-only. Nothing in this folder is part of a release.

## `dll/`

The original `AuroraAsset.dll` (MaesterRowen, 32-bit x86, built against the Xbox 360 XDK) and the
`msvcr100.dll` it needs. They are the reference the Rust
codec is tested against: encode and decode goldens are generated with them, and
`cargo xtask cross-decode` runs them against the Rust output. That check is Windows-only and is run by
hand; CI does not run it.

## `spike/`

The black-box investigation of the old AuroraAsset.dll: what its texture header, block layout,
decode arithmetic and BC3 encoder do. The fixture generators and harnesses the tests use are
built from it under `tools/golden`. It is kept as a record and runs on Windows only.

Requirements:

- Visual Studio 2017 or later with the C++ x86/x64 build tools. The build scripts find it with
  `vswhere`.
- git, to fetch DirectXTex.
- Python with numpy (and Pillow for the inputs), or [uv](https://docs.astral.sh/uv/), which the
  commands below use.

Everything is written to a work folder: `target\golden-spike` by default, or the folder given as
the first argument to each build script and Python script. Run the steps from the repository root,
in order:

1. Generate the test images as raw A,R,G,B (`in\<name>_<w>x<h>.argb`), from the app's
   placeholders and a few synthetic patterns:

   ```
   uv run --with numpy --with pillow python tools/golden/spike/gen_inputs.py target/golden-spike/in crates/aurora-ui/ui/placeholders
   ```

2. Build the 32-bit DLL harness (`bin32`, with the DLL copied next to it), the 64-bit DirectXTex
   BC3 encoder harness (`bin64`) and its 32-bit x87 variants (`bin32x87_precise`,
   `bin32x87_fast`):

   ```
   tools\golden\spike\build32.bat
   tools\golden\spike\build64.bat
   tools\golden\spike\build32x87.bat
   ```

   The encoder builds clone DirectXTex into `ext\DirectXTex` when it is missing, at the tag set in
   `directxtex.bat`. The encoder comparison depends on DirectXTex's BC3 encoder, so the tag is
   pinned; delete `ext\DirectXTex` after changing it.

3. Run `ConvertImageToAsset` and `ConvertAssetToImage` on every image, uncompressed (`_c0`) and
   compressed (`_c1`), writing the header, video data and the DLL's own decode to `dll\`:

   ```
   uv run python tools/golden/spike/run_dll.py target/golden-spike
   ```

4. Analyse the decode. Each script takes the work folder:

   | Script | Prints |
   |---|---|
   | `dump_hdr.py` | Texture header fields and padded sizes |
   | `decode_check.py` | Row pitch, block modes and a first BC3 decode against the DLL's |
   | `decode_variants.py` | Mismatch counts for candidate decode arithmetic |
   | `decode_alpha.py` | Mismatches of candidate alpha divides |
   | `decode_alpha2.py` | The same over every image; takes the icon's row pitch, `512`, as a second argument |
   | `decode_final.py` | Mismatches of the final decoder candidate |
   | `decode_f32.py` | Best float32 formulations of that decoder |
   | `icon_layout.py` | Where each 4x4 block of the 64x64 icon lives in the video data |

   ```
   uv run --with numpy python tools/golden/spike/decode_final.py target/golden-spike
   uv run --with numpy python tools/golden/spike/decode_alpha2.py target/golden-spike 512
   ```

   `decode_variants2.py` is loaded by `decode_alpha2.py` and is not run on its own. The earlier
   scripts test hypotheses that later ones refine, so their non-zero mismatch counts are part of
   the record, not failures.

5. Compare DirectXTex's BC3 output with the DLL's, block by block. `compare_enc.py` takes the work
   folder, optionally the encoder's build folder (default `bin64`) and a comma-separated list of
   flag sets; `mismatch_kind.py` then characterises the blocks that differ in the `bin64` output:

   ```
   uv run --with numpy python tools/golden/spike/compare_enc.py target/golden-spike
   uv run --with numpy python tools/golden/spike/compare_enc.py target/golden-spike bin32x87_precise dither_rgb
   uv run --with numpy python tools/golden/spike/mismatch_kind.py target/golden-spike
   ```

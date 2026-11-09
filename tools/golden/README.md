# Golden reference tooling

Test-only. Nothing in this folder is part of a release.

## `dll/`

The original `AuroraAsset.dll` (MaesterRowen, 32-bit x86, built against the Xbox 360 XDK) and the
`msvcr100.dll` it needs, byte-identical to the copies on `master`. They are the reference the Rust
codec is tested against: encode and decode goldens are generated with them, and the Windows CI
cross-decode job runs them against the Rust output.

## `spike/`

The black-box investigation of the old AuroraAsset.dll, kept as it was run. The
scripts still carry the session scratchpad paths they were written with; they are a record, not
the maintained tooling. The fixture generators and harnesses the tests use are built from them
under `tools/golden`.

| File | Purpose |
|---|---|
| `gen_inputs.py` | Writes the test images as raw A,R,G,B (`<name>_<w>x<h>.argb`) |
| `dllharness.c`, `build32.bat` | 32-bit harness: `ConvertImageToAsset` / `ConvertAssetToImage` per image |
| `dump_hdr.py` | Dumps texture header fields and padded sizes |
| `decode_*.py`, `icon_layout.py` | Reverse-engineering of the decode math and block layout |
| `dxtex_enc.cpp`, `build64.bat`, `build32x87.bat` | DirectXTex BC3 encoder harness (SSE and x87 builds) |
| `compare_enc.py`, `mismatch_kind.py` | Block-level comparison of DirectXTex output with the DLL |

DirectXTex itself is not vendored; the spike used a clone of `microsoft/DirectXTex` at `c0cb977`.

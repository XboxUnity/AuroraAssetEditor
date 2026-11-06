# How AuroraAsset.dll decodes a texture header

Found by decoding crafted headers with the DLL (`cargo xtask golden generate`, `[[crafted]]` in
`fixtures/codec/manifest.toml`) and comparing each result with the port. The wrapper applies
`XGCOMPRESS_ALPHADIVIDE` only when `XGGetTextureDesc` reports exactly `D3DFMT_LIN_DXT4`.

## Header signatures

All values are the DLL's own header fields. "Divide" is the alpha divide; "Port" is what
`TextureHeader::format()` reports.

| Format | data_format | endian | swizzle | num_format | sign | tiled | DLL | Divide | Port |
|---|---|---|---|---|---|---|---|---|---|
| DXT4 (DLL's own header) | 0x14 | 1 (8in16) | R,G,B,A | 0 | unsigned | no | accepted | yes | `LinearDxt4` |
| DXT5 | 0x14 | 0..3 | R,G,B,A or B,G,R,A | any | unsigned or signed | no | accepted | no | `LinearDxt5` unless it is the DXT4 row |
| DXT1 | 0x12 | 0..3 | R,G,B,A or B,G,R,A | any | unsigned or signed | no | accepted | no | `LinearDxt1` |
| ARGB8888 (DLL's own header) | 0x06 | 2 (8in32) | B,G,R,A | 0 | unsigned | no | accepted | no | `LinearArgb8888` |
| DXT2_3 | 0x13 | 0..3 | R,G,B,A or B,G,R,A | any | any | no | accepted | no | `Unsupported` (explicit 4-bit alpha needs its own decoder) |
| any format, tiled bit set | any | any | any | any | any | yes | accepted, read as linear | per row above | `Unsupported { tiled: true }` |
| DXT1 or DXT5, other swizzle (A,B,G,R tried) | 0x12, 0x14 | 1 | A,B,G,R | 0 | unsigned | no | accepted | no | `Unsupported` |
| DXT1 or DXT5, gamma sign | 0x12, 0x14 | 1 | R,G,B,A | 0 | gamma | no | accepted, applies a transfer curve | no | `Unsupported` |

Each of the 0x14 rows above is a DXT5. Which header Aurora's own `D3DFMT_LIN_DXT5` produces is not
known; every 0x14 header the DLL decodes without the divide is read as DXT5.

What each field does to the decode:

- **endian** selects the byte order of the surface data (none, 8in16, 8in32, 16in32). The port
  converts the data to native order before reading blocks.
- **swizzle** B,G,R,A swaps red and blue in the output.
- **num_format** 1 and **sign** unsigned/signed do not change the decoded pixels, but any value other
  than the DXT4 row's switches the divide off.
- **tiled** is ignored by the DLL, which always reads a linear surface.
- A single-bit flip probe over the DXT4 header found that every sign bit (FETCH_0 bits 2 to 9),
  num_format and the tiled bit switch the divide off without changing anything else; flips of the
  pitch, format, endian, size and swizzle bits change the decode.

## BC1 and BC3 colour

The DLL decodes the colour of a BC3 block exactly as a BC1 block: when the first endpoint (as
a 16-bit value) is greater than the second there are four colours, otherwise the two endpoints, their
midpoint and black. BC1 additionally makes the fourth entry transparent black; BC3 takes alpha from
its own alpha block. Equal endpoints use the three-colour form.

## Layout and pitch

The pitch field counts 32 texels. A 4x4 block row of 32 texels is 8 blocks, so a pitch unit is 128
bytes of BC3 or ARGB and 64 bytes of DXT1. `row_pitch / 128` is therefore only the pitch for 16-byte
blocks and ARGB; for DXT1 it is `row_pitch / 64`.

The DLL reads rows at the stride the header's pitch gives, so the port does the same for block formats.
A pitch wider than the image needs decodes normally; a pitch too small for one row is refused by the
port (`DecodeError::RowPitchTooSmall`). The DLL accepts it but its output depends on memory outside the
surface, and it crashed the harness with heap corruption, so no golden case exists for it.

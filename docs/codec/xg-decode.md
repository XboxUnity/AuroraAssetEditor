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
| DXT5 | 0x14 | 0..3 | any selection of R,G,B,A | any | unsigned or signed | no | accepted | no | `LinearDxt5` unless it is the DXT4 row |
| DXT1 | 0x12 | 0..3 | any selection of R,G,B,A | any | unsigned or signed | no | accepted | no | `LinearDxt1` |
| ARGB8888 (DLL's own header) | 0x06 | 2 (8in32) | B,G,R,A | 0 | unsigned | no | accepted | no | `LinearArgb8888` |
| DXT2_3 | 0x13 | 0..3 | R,G,B,A or B,G,R,A | any | any | no | accepted | no | `Unsupported` (explicit 4-bit alpha needs its own decoder) |
| any format, tiled bit set | any | any | any | any | any | yes | accepted, read as linear | no |  `Unsupported { tiled: true }` |
| DXT1 or DXT5, a swizzle that selects constant 0 (R,G,B,0 tried) | 0x12, 0x14 | 1 | R,G,B,0 | 0 | unsigned | no | accepted, alpha forced | no | `Unsupported` |
| any readable format, FETCH_3 bit 31 (border) set | any | any | any | any | any | no | accepted, decodes 2 pixels wider and taller | no | `Unsupported` |
| DXT1 or DXT5, gamma sign | 0x12, 0x14 | 1 | R,G,B,A | 0 | gamma | no | accepted, applies a transfer curve | no | `Unsupported` |

Each of the 0x14 rows above is a DXT5. Which header Aurora's own `D3DFMT_LIN_DXT5` produces is not
known; every 0x14 header the DLL decodes without the divide is read as DXT5.

What each field does to the decode:

- **endian** selects the byte order of the surface data (none, 8in16, 8in32, 16in32). The port
  converts the data to native order before reading blocks.
- **swizzle** picks, for each presented channel, which stored channel it shows: output channel i is stored channel `selector[i]`. Checked for all 24 permutations and for the repeats R,R,R,A and A,A,A,A on DXT1 and DXT5; the port reads any selector up to A, B,G,R,A being the usual swap of red and blue. The ARGB8888 header must have exactly the DLL's B,G,R,A.
- **num_format** 1 and **sign** unsigned/signed do not change the decoded pixels, but any value other
  than the DXT4 row's switches the divide off.
- **tiled** is ignored by the DLL, which always reads a linear surface.
- A single-bit flip probe over the DXT4 header found that every sign bit (FETCH_0 bits 2 to 9),
  num_format and the tiled bit switch the divide off without changing anything else; flips of FETCH_3
  bit 31 (border size, two extra pixels each way), the pitch, format, endian, size and swizzle bits
  change the decode; flips of the first dword's low four bits crash the DLL. Every other bit was
  flipped one by one with no change.
- The `probe_*` cases in `[[crafted]]` reproduce this for DXT4, DXT1 and ARGB: each flips a whole
  group of bits the port does not read (common flags, GPU runtime dwords, misc FETCH_0 to FETCH_5 bits)
  and stores only the baseline it equals (`same_as`). The `probe_border` cases differ and are
  refused. The port requires the three fields the DLL depends on to match its own headers: the low
  four bits of the first dword, FETCH_3 bit 31 and the 2D dimension.

## BC1 and BC3 colour

The DLL decodes the colour of a BC3 block exactly as a BC1 block: when the first endpoint (as
a 16-bit value) is greater than the second there are four colours, otherwise the two endpoints, their
midpoint and black. BC1 additionally makes the fourth entry transparent black; BC3 takes alpha from
its own alpha block. Equal endpoints use the three-colour form.

## Layout and pitch

The pitch field counts 32 texels. A 4x4 block row of 32 texels is 8 blocks, so a pitch unit is 128
bytes of BC3 or ARGB and 64 bytes of DXT1. `row_pitch / 128` is therefore only the pitch for 16-byte
blocks and ARGB; for DXT1 it is `row_pitch / 64`.

The DLL reads rows at the stride the header's pitch gives, for ARGB as well (a crafted ARGB header with a
wider pitch decodes differently from the plain one), so the port does the same for every format.
A pitch wider than the image needs decodes normally; a pitch too small for one row is refused by the
port (`DecodeError::RowPitchTooSmall`). The DLL accepts it but its output depends on memory outside the
surface, and it crashed the harness with heap corruption, so no golden case exists for it.

The DLL does not check the length of the data. Data that ends right after the last byte the decode
reads (every row above the last visible block row at the header's pitch, then the visible blocks or
pixels of that row) decodes exactly like the padded surface; `golden generate` checks this for every
fixture and every crafted case the port decodes (`*decodes_at_minimum_len` in the manifest). The port
accepts such data. Below that length the DLL reads past the buffer and its output is not stable, so
the port refuses it (`DecodeError::TruncatedData`).

## Decode arithmetic

Found by disassembling `tools/golden/dll/AuroraAsset.dll` (rizin, image base `0x10000000`) and
confirmed by decoding every fixture and every accepted crafted case exactly. `ConvertAssetToImage`
does not decode through the codec objects the encoder uses (`0x10010f98`): a probe that hooked their
BC3 decoder (`0x1001454c`) and their BC1 colour decoder (`0x1001415e`) saw no calls. It takes a
separate SSE path instead, and every step there is `f32`, so the x87 control word plays no part.

| Address | What it does |
|---|---|
| `0x1000249c` | DXT4 block: decode as DXT5 (`0x100021ab`), then divide colour by alpha (`0x100021cd`). |
| `0x100021ab` | DXT5 block: colour half (`0x10001cb5`), alpha half (`0x10002018`). |
| `0x10001cb5` | Colour half; DXT1 blocks reach it too, through `0x10002479`. |
| `0x10001c0d` | RGB565 to `f32`: the fields times `{1/31, 1/63, 1/31}` (`0x1001a100`, `f32` constants). |
| `0x10002018` | Alpha half: endpoints times `1/255` (`0x1001a230`), then the palette from weight tables. |
| `0x10003d9e` | The row loop: calls the block decoder, rearranges each texel's channels, calls the writer. |
| `0x10002768` | Writer: clamps and scales by vectors the caller passes, rounds with `cvtps2dq`, packs. |

Each step, in `f32`:

1. **Colour endpoints:** `field * (1/31)` (or `1/63` for green), the reciprocal rounded to `f32`.
2. **Colour palette:** `first + (second - first) * weight`, with weights `1/3`, `2/3` (four colours) or
   `0.5` (three colours, the fourth entry all zero).
3. **Alpha endpoints:** `byte * (1/255)`.
4. **Alpha palette:** `first * w_first + second * w_second`, with the weights `f32` constants `(n-i)/n`
   and `i/n` (`n` is 7 when the first endpoint is greater, otherwise 5; the 5-step form ends in 0 and 1).
5. **DXT4 divide:** `min((1/alpha) * colour, 1)`, zero where alpha is zero; alpha itself is kept.
6. **To a byte:** `value * 255`, rounded to nearest with ties to even (`cvtps2dq` in the default
   MXCSR mode). The clamp bounds (0 to 1) and the scale (255) are the caller's arguments and were not
   traced; they are inferred from the exact match on the whole corpus.

Theories ruled out on the way:

- The A8R8G8B8 writer of the codec objects (`0x10009f4b`), which truncates (`fldcw` with `or 0xc00`)
  and adds a bias of a half: modelling it left 23 mismatching pixels on two fixtures and two
  crafted cases. That writer is not on the decode path.
- x87 truncation in the endpoint expansion: it fixed some of those and broke DXT1 cases that had
  matched.

## DDS files

`ConvertDDSToImage` decodes the DDS images found in FSD files (`[[dds]]` in the manifest, made by
`xtask/src/dds_sample.rs`). Found by decoding the samples and larger probe files with the DLL; not
disassembled. `dds::decode` returns the top mip level, as the DLL does. It refuses images wider or
taller than `MAX_DIMENSION` (8192), the most an `.asset` texture can hold, which bounds the memory a
decode allocates; the DLL's own limit was not probed.

`FsdFile::parse` treats an entry as DDS when it starts with `DDS `. The old app tested
`data[0] == 'D' || data[1] == 'D' || data[1] == 'S'`, which no PNG, JPEG or GIF file passes
either, so both send the same image files down each path.

What the DLL does with each format:

- **DXT1 and DXT5:** the same block decode as `ConvertAssetToImage` (the arithmetic above): DXT5
  colour uses the three-colour form too, and its alpha comes from the alpha block. The port calls
  the same block decoders; probes of 4096 random DXT1 and DXT5 blocks match every texel.
- **A8R8G8B8:** copied unchanged.
- **X8R8G8B8:** copied unchanged, so the unused byte becomes alpha. The port makes these pixels
  opaque instead: an image that is transparent only by accident would stay transparent in the
  `.asset` file Aurora reads.
- **DXT3, R8G8B8, R5G6B5, A1R5G5B5, A4R4G4B4 and L8:** accepted, but never decoded: the output is a
  raw copy of the pixel data followed by uninitialised memory, and two decodes of one file differ.
  There is nothing stable to match, so the port refuses them (`DdsError::Unsupported`) and the
  manifest marks them `dll_output_unstable` with no hash.

Defects of the DLL that the port does not reproduce:

- **Reported length.** The DLL reports the header's `dwPitchOrLinearSize` as the image length (128
  bytes for a 16x16 DXT1), but always writes `width * height * 4` bytes. The old app allocated the
  reported length, so the DLL overran its buffer; the harness allocates the larger of the two.
- **Byte order.** The DLL builds the image in ARGB byte order and swaps only the first reported
  length to BGRA. With a pitch below 4 nothing is swapped, which is why an A8R8G8B8 file with such a
  pitch came out byte-reversed. The port returns every pixel in one order. The golden hash is the
  DLL's buffer with the swap completed (`decoded_sha256`), and the region the DLL did swap must
  equal the port byte for byte (`verbatim_sha256`).
- **Widths not a multiple of 4 (block formats).** The DLL writes the partial block column at the
  right edge into the start of the next row, so the image is scrambled. The port crops the edge
  blocks in place. For those samples the manifest records `dll_layout_scrambled` and only the DLL's
  acceptance; a unit test in `dds.rs` pins the crop against the block decoder.

// Spike: encode raw ARGB (A,R,G,B bytes) to BC3 with DirectXTex's D3DX-derived encoder.
// usage: dxtex_enc <in.argb> <w> <h> <flags-hex> <out.bc3>
// Output: block-linear, (ceil(w/4) x ceil(h/4)) blocks, little-endian D3D layout, no row padding.
// Pixels outside the image are filled by clamping to the edge.
#include <cstdio>
#include <cstdlib>
#include <cstdint>
#include <vector>
#include "BC.h"
using namespace DirectX;

int main(int argc, char** argv) {
    if (argc != 6) { fprintf(stderr, "usage\n"); return 1; }
    FILE* f = fopen(argv[1], "rb"); if (!f) { fprintf(stderr, "open failed\n"); return 2; }
    int w = atoi(argv[2]), h = atoi(argv[3]);
    uint32_t flags = strtoul(argv[4], nullptr, 16);
    std::vector<uint8_t> px((size_t)w * h * 4);
    if (fread(px.data(), 1, px.size(), f) != px.size()) { fprintf(stderr, "short read\n"); return 2; }
    fclose(f);
    int bw = (w + 3) / 4, bh = (h + 3) / 4;
    std::vector<uint8_t> out((size_t)bw * bh * 16);
    for (int by = 0; by < bh; by++) {
        for (int bx = 0; bx < bw; bx++) {
            XMVECTOR c[16];
            for (int i = 0; i < 16; i++) {
                int x = bx * 4 + (i & 3), y = by * 4 + (i >> 2);
                if (x >= w) { x = w - 1; }
                if (y >= h) { y = h - 1; }
                const uint8_t* p = &px[((size_t)y * w + x) * 4];
                c[i] = XMVectorSet(p[1] / 255.f, p[2] / 255.f, p[3] / 255.f, p[0] / 255.f);
            }
            D3DXEncodeBC3(&out[((size_t)by * bw + bx) * 16], c, flags);
        }
    }
    f = fopen(argv[5], "wb"); if (!f) { fprintf(stderr, "write failed\n"); return 2; }
    fwrite(out.data(), 1, out.size(), f); fclose(f);
    return 0;
}

// Spike: 32-bit harness around AuroraAsset.dll.
// usage: dllharness <in.argb> <w> <h> <compress 0|1> <out_prefix>
// writes <out_prefix>.hdr (52-byte texture header), .vid (video data), .dec (DLL decode of its own output)
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>

typedef int (__cdecl *pI2A)(const BYTE*, int, int, int, int, BYTE*, int*, BYTE*, int*);
typedef int (__cdecl *pA2I)(const BYTE*, int, const BYTE*, int, BYTE*, int*, int*, int*);

static BYTE* readall(const char* p, int* len) {
    FILE* f = fopen(p, "rb"); if (!f) { fprintf(stderr, "open %s failed\n", p); exit(2); }
    fseek(f, 0, SEEK_END); *len = ftell(f); fseek(f, 0, SEEK_SET);
    BYTE* b = (BYTE*)malloc(*len); fread(b, 1, *len, f); fclose(f); return b;
}
static void writeall(const char* pre, const char* ext, const BYTE* b, int len) {
    char p[1024]; sprintf(p, "%s%s", pre, ext);
    FILE* f = fopen(p, "wb"); if (!f) { fprintf(stderr, "write %s failed\n", p); exit(2); }
    fwrite(b, 1, len, f); fclose(f);
}

int main(int argc, char** argv) {
    if (argc != 6) { fprintf(stderr, "usage\n"); return 1; }
    HMODULE m = LoadLibraryA("AuroraAsset.dll");
    if (!m) { fprintf(stderr, "LoadLibrary failed %lu\n", GetLastError()); return 3; }
    pI2A i2a = (pI2A)GetProcAddress(m, "ConvertImageToAsset");
    pA2I a2i = (pA2I)GetProcAddress(m, "ConvertAssetToImage");
    if (!i2a || !a2i) { fprintf(stderr, "GetProcAddress failed\n"); return 3; }
    int len; BYTE* img = readall(argv[1], &len);
    int w = atoi(argv[2]), h = atoi(argv[3]), comp = atoi(argv[4]);
    int hl = 0, vl = 0;
    if (!i2a(img, len, w, h, comp, NULL, &hl, NULL, &vl)) { fprintf(stderr, "size query failed\n"); return 4; }
    BYTE* hd = (BYTE*)malloc(hl); BYTE* vd = (BYTE*)malloc(vl);
    if (!i2a(img, len, w, h, comp, hd, &hl, vd, &vl)) { fprintf(stderr, "convert failed\n"); return 4; }
    writeall(argv[5], ".hdr", hd, hl);
    writeall(argv[5], ".vid", vd, vl);
    int dl = 0, dw = 0, dh = 0;
    // ConvertAssetToImage swaps the header in place and back; give it a copy anyway.
    BYTE* hc = (BYTE*)malloc(hl); memcpy(hc, hd, hl);
    if (!a2i(hc, hl, vd, vl, NULL, &dl, &dw, &dh)) { fprintf(stderr, "decode size failed\n"); return 5; }
    BYTE* dec = (BYTE*)malloc(dl);
    if (!a2i(hc, hl, vd, vl, dec, &dl, &dw, &dh)) { fprintf(stderr, "decode failed\n"); return 5; }
    writeall(argv[5], ".dec", dec, dl);
    printf("%s hdr=%d vid=%d dec=%d %dx%d\n", argv[5], hl, vl, dl, dw, dh);
    return 0;
}

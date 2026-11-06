/// The app's "Use Compression" toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Dxt,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    LinearArgb8888,
    LinearDxt1,
    LinearDxt4,
    LinearDxt5,
    Unsupported { data_format: u8, tiled: bool },
}

impl Compression {
    pub(super) fn fetch_format(self) -> FetchFormat {
        match self {
            Self::Dxt => FetchFormat::LINEAR_DXT4,
            Self::None => FetchFormat::LINEAR_ARGB8888,
        }
    }
}

/// How the GPU fetch constant describes a texture's data: format code, byte
/// order and the channel order it presents to shaders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FetchFormat {
    pub data_format: u32,
    pub endian: u32,
    pub swizzle: [u32; 4],
}

const GPU_ENDIAN_8IN16: u32 = 1;
const GPU_ENDIAN_8IN32: u32 = 2;

const GPU_FORMAT_8_8_8_8: u32 = 0x06;
const GPU_FORMAT_DXT4_5: u32 = 0x14;

const GPU_SWIZZLE_R: u32 = 0;
const GPU_SWIZZLE_G: u32 = 1;
const GPU_SWIZZLE_B: u32 = 2;
const GPU_SWIZZLE_A: u32 = 3;

impl FetchFormat {
    pub(super) const LINEAR_DXT4: Self = Self {
        data_format: GPU_FORMAT_DXT4_5,
        endian: GPU_ENDIAN_8IN16,
        swizzle: [GPU_SWIZZLE_R, GPU_SWIZZLE_G, GPU_SWIZZLE_B, GPU_SWIZZLE_A],
    };

    pub(super) const LINEAR_ARGB8888: Self = Self {
        data_format: GPU_FORMAT_8_8_8_8,
        endian: GPU_ENDIAN_8IN32,
        swizzle: [GPU_SWIZZLE_B, GPU_SWIZZLE_G, GPU_SWIZZLE_R, GPU_SWIZZLE_A],
    };
}

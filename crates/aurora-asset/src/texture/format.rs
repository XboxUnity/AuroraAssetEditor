use super::block_decode::BlockCodec;
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
const GPU_FORMAT_DXT1: u32 = 0x12;
const GPU_FORMAT_DXT4_5: u32 = 0x14;

const GPU_NUM_FORMAT_FRACTION: u32 = 0;
const GPU_SIGN_ALL_UNSIGNED: u32 = 0b0000_0000;
const GPU_SIGN_ALL_SIGNED: u32 = 0b0101_0101;

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

/// The order in which a block-compressed texture presents its channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChannelOrder {
    Rgba,
    /// Red and blue are swapped relative to the stored colour.
    Bgra,
}

impl ChannelOrder {
    pub(super) fn from_swizzle(swizzle: [u32; 4]) -> Self {
        if swizzle == FetchFormat::LINEAR_ARGB8888.swizzle {
            Self::Bgra
        } else {
            Self::Rgba
        }
    }

    /// Reorders decoded RGBA values to the presented channel order.
    pub(super) fn arrange(self, [red, green, blue, alpha]: [f64; 4]) -> [f64; 4] {
        match self {
            Self::Rgba => [red, green, blue, alpha],
            Self::Bgra => [blue, green, red, alpha],
        }
    }

    fn is_known_swizzle(swizzle: [u32; 4]) -> bool {
        swizzle == FetchFormat::LINEAR_DXT4.swizzle
            || swizzle == FetchFormat::LINEAR_ARGB8888.swizzle
    }
}

/// The fetch-constant fields that decide how AuroraAsset.dll reads a texture's data.
pub(super) struct FetchFields {
    pub tiled: bool,
    pub data_format: u32,
    pub endian: u32,
    pub swizzle: [u32; 4],
    pub num_format: u32,
    pub sign_bits: u32,
}

impl FetchFields {
    fn is(&self, known: FetchFormat) -> bool {
        self.data_format == known.data_format
            && self.endian == known.endian
            && self.swizzle == known.swizzle
    }

    /// The exact header the DLL writes for DXT4: the only one it decodes with the alpha divide.
    fn is_dxt4(&self) -> bool {
        self.is(FetchFormat::LINEAR_DXT4)
            && self.num_format == GPU_NUM_FORMAT_FRACTION
            && self.sign_bits == GPU_SIGN_ALL_UNSIGNED
    }

    /// Any byte order and either sign setting, with the two channel orders the DLL's headers use.
    /// Gamma and the other sign settings change the DLL's decode and are not supported.
    fn is_readable_block_format(&self, data_format: u32) -> bool {
        let known_sign = [GPU_SIGN_ALL_UNSIGNED, GPU_SIGN_ALL_SIGNED].contains(&self.sign_bits);
        self.data_format == data_format
            && known_sign
            && ChannelOrder::is_known_swizzle(self.swizzle)
    }
}

pub(super) fn classify(fields: &FetchFields) -> TextureFormat {
    let unsupported = TextureFormat::Unsupported {
        data_format: fields.data_format as u8,
        tiled: fields.tiled,
    };
    if fields.tiled {
        return unsupported;
    }
    if fields.data_format == GPU_FORMAT_8_8_8_8 && fields.endian == GPU_ENDIAN_8IN32 {
        TextureFormat::LinearArgb8888
    } else if fields.is_dxt4() {
        TextureFormat::LinearDxt4
    } else if fields.is_readable_block_format(GPU_FORMAT_DXT4_5) {
        TextureFormat::LinearDxt5
    } else if fields.is_readable_block_format(GPU_FORMAT_DXT1) {
        TextureFormat::LinearDxt1
    } else {
        unsupported
    }
}

impl TextureFormat {
    pub(super) fn block_codec(self) -> Option<BlockCodec> {
        match self {
            Self::LinearDxt1 => Some(BlockCodec::Bc1),
            Self::LinearDxt4 => Some(BlockCodec::Bc3 { divide_alpha: true }),
            Self::LinearDxt5 => Some(BlockCodec::Bc3 {
                divide_alpha: false,
            }),
            Self::LinearArgb8888 | Self::Unsupported { .. } => None,
        }
    }
}

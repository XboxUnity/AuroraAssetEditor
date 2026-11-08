use std::collections::HashMap;

use aurora_asset::RgbaImage;
use slint::Image;

use super::{PictureId, Placeholder, Previews, ShownPicture, slint_image};

/// The placeholder art, each decoded once, the first time an empty slot shows it.
#[derive(Default)]
pub struct PlaceholderArt {
    /// `None` for art that could not be decoded, which is logged once and not shown.
    decoded: HashMap<Placeholder, Option<DecodedArt>>,
}

struct DecodedArt {
    pixels: RgbaImage,
    image: Image,
}

impl PlaceholderArt {
    /// The art as its area shows it, resized like any picture; `None` when it could not be decoded.
    pub fn show(
        &mut self,
        placeholder: Placeholder,
        previews: &mut Previews,
    ) -> Option<ShownPicture> {
        let art = self
            .decoded
            .entry(placeholder)
            .or_insert_with(|| decode(placeholder))
            .as_ref()?;
        let id = PictureId::Placeholder(placeholder);
        Some(previews.show(id, placeholder.area(), &art.pixels, || art.image.clone()))
    }
}

fn decode(placeholder: Placeholder) -> Option<DecodedArt> {
    match aurora_image::decode_image(placeholder.png(), None) {
        Ok(pixels) => {
            let image = slint_image(&pixels);
            Some(DecodedArt { pixels, image })
        }
        Err(error) => {
            log::error!("The {placeholder:?} placeholder art could not be decoded: {error}");
            None
        }
    }
}

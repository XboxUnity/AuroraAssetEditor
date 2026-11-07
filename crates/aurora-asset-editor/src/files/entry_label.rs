use aurora_asset::AssetSlot;

/// How a dialog names an entry of a page.
pub fn slot_label(slot: AssetSlot) -> String {
    match slot {
        AssetSlot::Cover => "Cover".to_owned(),
        AssetSlot::Background => "Background".to_owned(),
        AssetSlot::Icon => "Icon".to_owned(),
        AssetSlot::Banner => "Banner".to_owned(),
        AssetSlot::Screenshot(number) => screenshot_label(usize::from(number.get())),
        AssetSlot::Slot => "Unused entry".to_owned(),
    }
}

/// `number` counts from 1.
pub fn screenshot_label(number: usize) -> String {
    format!("Screenshot {number}")
}

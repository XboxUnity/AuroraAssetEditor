use aurora_ui::PickerGameView;

use super::KeptRows;

/// The games of an ID dialog's picker. Its rows change in place, so a render keeps a click in progress on one.
pub type PickerRows = KeptRows<PickerGameView, PickerGameView>;

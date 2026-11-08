use aurora_ui::ToastView;

use super::KeptRows;

/// The toasts on screen. Their rows change in place, so a render keeps a click in progress on one.
pub type ToastRows = KeptRows<ToastView, ToastView>;

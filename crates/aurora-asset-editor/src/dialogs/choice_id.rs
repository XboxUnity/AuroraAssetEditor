/// A button of a choice dialog, besides Cancel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceId {
    Icon,
    Banner,
    Replace,
    AddToFreeSlots,
}

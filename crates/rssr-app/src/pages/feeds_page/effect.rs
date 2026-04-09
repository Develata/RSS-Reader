use crate::ui::UiCommand;

#[derive(Debug, Clone)]
pub(crate) enum FeedsPageEffect {
    LoadSnapshot,
    Dispatch(UiCommand),
}

use ratatui::{
    layout::Rect,
    buffer::Buffer,
    widgets::Widget,
};

/// A custom ratatui widget of a playlist selector menu.
#[derive(Default)]
pub struct TuiPicker {
    // TODO
}

/// A struct describing the complete state of a `TuiPicker`.
#[derive(Default)]
pub struct TuiPickerState {
    // TODO
}

impl TuiPicker {
    pub fn new(state: &TuiPickerState) -> Self {
        TuiPicker {}
    }
}

impl Widget for TuiPicker {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // TODO
    }
}

use ratatui::{
    layout::Rect,
    buffer::Buffer,
    widgets::Widget,
};

/// A custom ratatui widget of a playlist selector menu.
pub struct TuiPicker {
    // TODO
}

impl TuiPicker {
    pub fn new() -> Self {
        TuiPicker {}
    }
}

impl Widget for TuiPicker {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // TODO
    }
}

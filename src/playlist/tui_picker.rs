use crate::playlist::Playlist;
use ratatui::{
    text::{Line, Span},
    layout::Rect,
    buffer::Buffer,
    widgets::Widget,
    style::{Style, Stylize},
};
use std::collections::HashMap;

/// A custom ratatui widget of a playlist selector menu.
#[derive(Default)]
pub struct TuiPicker;

/// A custom ratatui widget of a playlist selector item. May be used independently of `TuiPicker`.
pub struct TuiPickerItem<'a> {
    line: Line<'a>,
}

/// A struct describing the complete state of a `TuiPicker`.
#[derive(Default)]
pub struct TuiPickerState {
    // TODO
}

/// A struct describing the complete state of a `TuiPickerItem`.
pub struct TuiPickerItemState {
    pub width: usize,
    pub playlist: Playlist,
    pub shortcut: String,
    pub states: Vec<u8>,
    pub state_styles: HashMap<u8, Style>,
    pub state_callback: Box<dyn Fn(&Self)>,
    pub state: usize,
}

impl TuiPicker {
    pub fn new(state: &TuiPickerState, input: &str) -> Self {
        Self {}
    }
}

impl<'a> TuiPickerItem<'a> {
    pub fn new(state: &'a TuiPickerItemState, input: &str) -> Self {
        let n_input_chars_hl = if state.shortcut.starts_with(input) { input.len() } else { 0 };
        let name_style = state.state_styles[&state.states[state.state]];
        let width = state.shortcut.len() + 1 + state.playlist.name.len();
        let line = Line::from(vec![
            Span::styled(&state.shortcut[..n_input_chars_hl], Style::new().bold().yellow()),
            Span::styled(&state.shortcut[n_input_chars_hl..], Style::new().bold().cyan()),
            Span::raw(" "),
            Span::styled(&state.playlist.name, name_style),
            Span::raw(" ".repeat(if width < state.width { state.width - width } else { 0 })),
        ]);
        Self { line }
    }
}

impl Widget for TuiPicker {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // TODO
    }
}

impl Widget for TuiPickerItem<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.line.render(area, buf);
    }
}

impl TuiPickerItemState {
    pub fn trigger(&mut self) {
        self.state = (self.state + 1) % self.states.len();
        self.state_callback.as_ref()(self);
    }
}

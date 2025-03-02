#![allow(clippy::type_complexity)]
use crate::{
    playlist::{Playlist, TracksFile},
    path_from,
};
use ratatui::{
    text::{Text, Line, Span},
    layout::Rect,
    buffer::Buffer,
    widgets::{Widget, StatefulWidget},
    style::{Style, Stylize},
};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use camino::{Utf8Path, Utf8PathBuf};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::OnceLock;

/// Returns the path to the playlists directory.
pub fn playlist_mappings_path() -> &'static Utf8Path {
    static PLAYLIST_MAPPINGS_PATH: OnceLock<Utf8PathBuf> = OnceLock::new();
    PLAYLIST_MAPPINGS_PATH.get_or_init(|| path_from(dirs::config_dir, "playlist-mappings.tsv"))
}

/// A custom ratatui widget of a playlist selector menu.
pub struct TuiPicker<'a> {
    input: &'a str,
}

/// A custom ratatui widget of a playlist selector item. May be used independently of `TuiPicker`.
pub struct TuiPickerItem<'a> {
    spans: Vec<Span<'a>>,
}

/// A struct describing the complete state of a `TuiPicker`.
pub struct TuiPickerState {
    pub scroll_amount: usize,
    /// A `None` value denotes the start of a new "paragraph" of items.
    items: Vec<Option<TuiPickerItemState>>,
    is_refreshing: bool,
    did_select: bool,
}

/// A struct describing the complete state of a `TuiPickerItem`.
pub struct TuiPickerItemState {
    pub playlist: Playlist,
    pub shortcut: String,
    width: usize,
    shortcut_rpad: usize,
    state_styles: HashMap<u8, Style>,
    on_refresh: Box<dyn Fn(u8, &mut Playlist) -> u8>,
    on_select: Box<dyn Fn(u8, &mut Playlist) -> u8>,
    state: u8,
    is_refreshing: bool,
}

impl<'a> TuiPicker<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input }
    }
}

impl<'a> TuiPickerItem<'a> {
    pub fn new(state: &'a TuiPickerItemState, input: &str) -> Self {
        let n_input_chars_hl = if state.shortcut.starts_with(input) { input.len() } else { 0 };
        let width = state.shortcut.len() + 1 + state.playlist.name().len();
        let mut name_style = state.state_styles[&state.state];
        let mut bg_style = Style::new();
        if n_input_chars_hl != 0 {
            bg_style = bg_style.on_dark_gray();
            name_style = name_style.on_dark_gray();
        };
        if state.is_refreshing {
            Self { spans: vec![
                Span::raw(" ".repeat(state.shortcut_rpad)),
                Span::styled(&state.shortcut, Style::new().bold().dark_gray()),
                Span::styled(" ", Style::new().dark_gray()),
                Span::styled(state.playlist.name(), name_style.dark_gray()),
                Span::raw(" ".repeat(
                    if width + state.shortcut_rpad < state.width {
                        state.width - width - state.shortcut_rpad
                    } else {
                        0
                    }
                )),
            ]}
        } else {
            Self { spans: vec![
                Span::raw(" ".repeat(state.shortcut_rpad)),
                Span::styled(&state.shortcut[..n_input_chars_hl], bg_style.bold().yellow()),
                Span::styled(&state.shortcut[n_input_chars_hl..], bg_style.bold().cyan()),
                Span::styled(" ", bg_style),
                Span::styled(state.playlist.name(), name_style),
                Span::raw(" ".repeat(
                    if width + state.shortcut_rpad < state.width {
                        state.width - width - state.shortcut_rpad
                    } else {
                        0
                    }
                )),
            ]}
        }
    }
}

impl StatefulWidget for TuiPicker<'_> {
    type State = TuiPickerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let items = &state.items;  // Shorthand
        let n_cols = state.compute_n_columns(area.width as usize);
        let par_ranges = state.compute_paragraph_ranges();

        // Compose the text to render
        let mut text = Text::default();
        for (par_begin, par_end) in par_ranges {
            let n_par_items = par_end - par_begin + 1;
            let n_par_lines = n_par_items.div_ceil(n_cols);
            let n_par_overflow = if n_par_items % n_cols != 0 { n_par_items % n_cols } else { n_cols };
            for i_offset in 0..n_par_lines {
                let mut i = par_begin + i_offset;
                let mut line = Line::default();
                let mut x = 1;  // Horizontal position (column index)
                while x <= n_cols && i <= par_end && i < items.len() {
                    // Within each paragraph it is guaranteed that all values will be Some
                    for span in TuiPickerItem::new(items[i].as_ref().unwrap(), self.input).spans {
                        line.push_span(span);
                    }
                    // This calculation is a bit more complex, but essentially, we want to
                    // skip all items that will be below us in a column. That number is always
                    // either n_par_lines, or n_par_lines-1 (if the final row is not full).
                    // In addition, we must always skip at least 1 item.
                    i += std::cmp::max(1, n_par_lines - if x > n_par_overflow { 1 } else { 0 });
                    if i_offset == n_par_lines - 1 && x >= n_par_overflow {
                        break;
                    } else {
                        x += 1;
                    }
                }
                text.push_line(line);
            }
            text.push_line(Line::default());  // Separator
        }
        assert_eq!(text.lines.len(), state.height(area.width as usize));

        // Apply scrolling
        let max_scroll = text.lines.len().saturating_sub(area.height as usize);
        state.scroll_amount = state.scroll_amount.clamp(0, max_scroll);
        text.lines.drain(0..state.scroll_amount);

        text.render(area, buf);
    }
}

impl Widget for TuiPickerItem<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Line::from(self.spans).render(area, buf);
    }
}

impl TuiPickerState {
    pub fn new<F, G>(state: u8, state_styles: &HashMap<u8, Style>, on_refresh: F, on_select: G) -> Result<Self>
    where
        F: Fn(u8, &mut Playlist) -> u8 + 'static + Clone,
        G: Fn(u8, &mut Playlist) -> u8 + 'static + Clone,
    {
        let mut items = vec![];
        let fpath = playlist_mappings_path();
        let file = BufReader::new(File::open(fpath)?);
        let mut width = 0usize;
        let mut shortcut_width = 0usize;
        for (i, line) in file.lines().enumerate() {
            let line = match line {
                Ok(str) => str,
                Err(e) => return Err(anyhow!("Failed to read line from '{}': {}", fpath, e)),
            };
            if line.is_empty() {
                items.push(None);
                continue;
            }
            let mut it = line.splitn(2, '\t');
            let name = match it.next() {
                Some(str) => str,
                None => return Err(anyhow!("Failed to extract playlist name from mappings line: {}", line)),
            };
            let shortcut = match it.next() {
                Some(str) => str.to_owned(),
                None => return Err(anyhow!("Failed to extract shortcut from mappings line: {}", line)),
            };
            for other_shortcut in items.iter().filter_map(|x| x.as_ref()).map(|x: &TuiPickerItemState| &x.shortcut) {
                if shortcut.starts_with(other_shortcut) || other_shortcut.starts_with(&shortcut) {
                    return Err(anyhow!("Colliding shortcuts: {}, {}", shortcut, other_shortcut));
                }
            }
            shortcut_width = std::cmp::max(shortcut_width, shortcut.len());
            let pl_path = Playlist::playlist_dir().join(name.to_owned() + ".m3u");
            let playlist = match Playlist::open(&pl_path) {
                Ok(pl) => pl,
                Err(e) => return Err(anyhow!("Failed to read playlist '{}' from mappings line {}: {}", pl_path, i + 1, e)),
            };
            width = std::cmp::max(width, shortcut.len() + 1 + playlist.name().len() + 2);
            items.push(Some(TuiPickerItemState::new(
                playlist,
                shortcut,
                0,  // width; will be updated later
                0,  // shortcut_rpad; will be updated later
                state,
                state_styles.clone(),
                on_refresh.clone(),
                on_select.clone(),
            )));
        }

        for item in items.iter_mut().filter_map(|x| x.as_mut()) {
            // Update the width of every item
            item.width = width;

            // Compute shortcut padding
            item.shortcut_rpad = shortcut_width - item.shortcut.len();
        }

        Ok(Self {
            items,
            scroll_amount: 0,
            is_refreshing: false,
            did_select: false,
        })
    }

    /// Returns whether a refresh is in progress. See `refresh()`.
    pub fn is_refreshing(&self) -> bool {
        self.is_refreshing
    }

    /// Returns whether the last call to `update_input()` caused an item selection.
    pub fn did_select(&self) -> bool {
        self.did_select
    }

    /// Calls `on_refresh` for every item. The first call to this method initiates a refresh. Then,
    /// subsequent calls must be made until `is_refreshing()` returns `false`.
    /// The reason is that it internally calls refresh for each item one by one. This design allows
    /// you to redraw the screen while the refresh is happening, and implement e.g. animations
    /// while keeping the program single-threaded.
    pub fn refresh(&mut self) -> bool {
        if !self.is_refreshing {
            for item in self.items.iter_mut().filter_map(|x| x.as_mut()) {
                // The first invocation must yield false
                assert!(!item.refresh());
            }
            self.is_refreshing = true;
            return false;
        }

        for item in self.items.iter_mut().filter_map(|x| x.as_mut()) {
            assert!(item.refresh());
        }
        self.is_refreshing = false;
        true
    }

    /// Updates the input string. Returns `true` if at least one item is matching the current
    /// input, `false` if input should be cleared and started from scratch.
    pub fn update_input(&mut self, input: &str) -> bool {
        self.did_select = false;
        for item in self.items.iter_mut().filter_map(|x| x.as_mut()) {
            if item.shortcut == input {
                item.select();
                self.did_select = true;
                return false;
            }
            if item.shortcut.starts_with(input) {
                return true;
            }
        }
        false
    }

    /// Computes the number of columns that the widget would take, given an area width.
    pub fn compute_n_columns(&self, area_width: usize) -> usize {
        if let Some(item) = self.items.iter().filter_map(|x| x.as_ref()).next() {
            (area_width / item.width).clamp(1, 5)
        } else {
            0
        }
    }

    /// Computes the width of the whole widget, given an area width.
    pub fn width(&self, area_width: usize) -> usize {
        if let Some(item) = self.items.iter().filter_map(|x| x.as_ref()).next() {
            let n_cols = self.compute_n_columns(area_width);
            item.width * n_cols
        } else {
            0
        }
    }

    /// Computes the height of the whole widget, given an area width.
    pub fn height(&self, area_width: usize) -> usize {
        let n_cols = self.compute_n_columns(area_width);
        if n_cols == 0 {
            return 0;
        }
        let par_ranges = self.compute_paragraph_ranges();
        let mut height = 0;
        for (par_begin, par_end) in par_ranges {
            let n_par_items = par_end - par_begin + 1;
            let n_par_lines = n_par_items.div_ceil(n_cols);
            height += n_par_lines + 1;  // +1 for separator between paragraphs
        }
        height
    }

    /// Computes index ranges of all paragraphs of items.
    ///
    /// E.g. `vec![(0, 3), (4, 4), (5, 13)]`
    fn compute_paragraph_ranges(&self) -> Vec<(usize, usize)> {
        if self.items.is_empty() {
            return vec![(0, 0)];
        }
        let mut par_ranges = vec![(0usize, usize::MAX)];
        for i in 0..self.items.len() {
            if self.items[i].is_none() {
                par_ranges.last_mut().unwrap().1 = i - 1;
                #[allow(clippy::int_plus_one)]
                if self.items.len() >= i + 1 {
                    par_ranges.push((i + 1, usize::MAX));
                }
            }
        }
        par_ranges.last_mut().unwrap().1 = self.items.len() - 1;
        par_ranges
    }
}

impl TuiPickerItemState {
    #[allow(clippy::too_many_arguments)]
    pub fn new<F, G>(playlist: Playlist, shortcut: String, width: usize, shortcut_rpad: usize, state: u8, state_styles: HashMap<u8, Style>, on_refresh: F, on_select: G) -> Self
    where
        F: Fn(u8, &mut Playlist) -> u8 + 'static + Clone,
        G: Fn(u8, &mut Playlist) -> u8 + 'static + Clone,
    {
        Self {
            playlist,
            shortcut,
            width,
            shortcut_rpad,
            state_styles,
            on_refresh: Box::new(on_refresh),
            on_select: Box::new(on_select),
            state,
            is_refreshing: false,
        }
    }

    pub fn is_refreshing(&self) -> bool {
        self.is_refreshing
    }

    pub fn refresh(&mut self) -> bool {
        if !self.is_refreshing {
            self.is_refreshing = true;
            return false;
        }
        self.state = (self.on_refresh)(self.state, &mut self.playlist);
        self.is_refreshing = false;
        true
    }

    pub fn select(&mut self) {
        self.state = (self.on_select)(self.state, &mut self.playlist);
    }
}

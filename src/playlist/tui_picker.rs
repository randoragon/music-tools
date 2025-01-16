use crate::{
    playlist::{Playlist, TracksFile},
    path_from,
};
use ratatui::{
    text::{Text, Line, Span},
    layout::Rect,
    buffer::Buffer,
    widgets::Widget,
    style::{Style, Stylize},
};
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use camino::{Utf8Path, Utf8PathBuf};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::OnceLock;
use std::rc::Rc;
use std::cell::RefCell;

/// Returns the path to the playlists directory.
pub fn playlist_mappings_path() -> &'static Utf8Path {
    static PLAYLIST_MAPPINGS_PATH: OnceLock<Utf8PathBuf> = OnceLock::new();
    PLAYLIST_MAPPINGS_PATH.get_or_init(|| path_from(dirs::config_dir, "playlist-mappings.tsv"))
}

/// A custom ratatui widget of a playlist selector menu.
pub struct TuiPicker<'a> {
    state_ref: &'a TuiPickerState,
    input: &'a str,
}

/// A custom ratatui widget of a playlist selector item. May be used independently of `TuiPicker`.
pub struct TuiPickerItem<'a> {
    spans: Vec<Span<'a>>,
}

/// A struct describing the complete state of a `TuiPicker`.
pub struct TuiPickerState {
    /// A `None` value denotes the start of a new "paragraph" of items.
    items: Vec<Option<TuiPickerItemState>>,
}

/// A struct describing the complete state of a `TuiPickerItem`.
pub struct TuiPickerItemState {
    pub width: usize,
    pub playlist: Playlist,
    pub shortcut: String,
    pub states: Vec<u8>,
    pub state_styles: HashMap<u8, Style>,
    pub state_callback: Box<dyn Fn(u8)>,
    pub state: Rc<RefCell<usize>>,
}

impl<'a> TuiPicker<'a> {
    pub fn new(state: &'a TuiPickerState, input: &'a str) -> Self {
        Self {
            state_ref: state,
            input,
        }
    }
}

impl<'a> TuiPickerItem<'a> {
    pub fn new(state: &'a TuiPickerItemState, input: &str) -> Self {
        let n_input_chars_hl = if state.shortcut.starts_with(input) { input.len() } else { 0 };
        let name_style = state.state_styles[&state.states[*state.state.borrow()]];
        let width = state.shortcut.len() + 1 + state.playlist.name.len();
        Self { spans: vec![
            Span::styled(&state.shortcut[..n_input_chars_hl], Style::new().bold().yellow()),
            Span::styled(&state.shortcut[n_input_chars_hl..], Style::new().bold().cyan()),
            Span::raw(" "),
            Span::styled(&state.playlist.name, name_style),
            Span::raw(" ".repeat(if width < state.width { state.width - width } else { 0 })),
        ]}
    }
}

impl Widget for TuiPicker<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let items = &self.state_ref.items;  // Shorthand

        // Compute the number of columns
        let item_width = match items.iter()
            .filter_map(|x| if x.is_some() { Some(x.as_ref().unwrap().width) } else { None })
            .next() {
            Some(w) => w,
            None => return,  // Nothing to render
        };
        let n_cols = std::cmp::max(1, std::cmp::min(area.width as usize / item_width, 5));

        // Compute the left-padding needed to center the whole text
        let lpad = 1 + (area.width as usize - (item_width * n_cols)) / 2;

        // Find index ranges for each "paragraph".
        // At this point it is guaranteed that there is at least one Some(Item).
        let mut par_ranges = vec![(0usize, usize::MAX)];
        for i in 0..items.len() {
            if items[i].is_none() {
                par_ranges.last_mut().unwrap().1 = i - 1;
                if items.len() >= i + 1 {
                    par_ranges.push((i + 1, usize::MAX));
                }
            }
        }
        par_ranges.last_mut().unwrap().1 = items.len() - 1;

        // Compose the text to render
        let mut text = Text::default();
        for (par_begin, par_end) in par_ranges {
            let n_par_items = par_end - par_begin + 1;
            let n_par_lines = (n_par_items + n_cols - 1) / n_cols;
            let n_par_overflow = n_par_items % n_cols;
            for i_offset in 0..n_par_lines {
                let mut i = par_begin + i_offset;
                let mut line = Line::default();
                let mut x = 1;  // Horizontal position (column index)
                line.push_span(Span::raw(" ".repeat(lpad)));
                while x <= n_cols && i <= par_end && i < items.len() {
                    // Within each paragraph it is guaranteed that all values will be Some
                    for span in TuiPickerItem::new(items[i].as_ref().unwrap(), self.input).spans {
                        line.push_span(span);
                    }
                    // This calculation is a bit more complex, but essentially, we want to
                    // skip all items that will be below us in a column. That number is always
                    // either n_par_lines, or n_par_lines-1 (if the final row is not full).
                    // In addition, we must always skip at least 1 item.
                    i += std::cmp::max(1, n_par_lines - if x <= n_par_overflow { 0 } else { 1 });
                    if i_offset == n_par_lines - 1 && n_par_overflow != 0 && x >= n_par_overflow {
                        break;
                    } else {
                        x += 1;
                    }
                }
                text.push_line(line);
            }
            text.push_line(Line::default());  // Separator
        }

        text.render(area, buf);
    }
}

impl Widget for TuiPickerItem<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Line::from(self.spans).render(area, buf);
    }
}

impl TuiPickerState {
    pub fn new<F>(states: &[u8], state_styles: &HashMap<u8, Style>, state_callback: F) -> Result<Self>
    where
        F: Fn(u8) + 'static + Clone,
    {
        let mut items = vec![];
        let fpath = playlist_mappings_path();
        let file = BufReader::new(File::open(fpath)?);
        let mut width = 0usize;
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
            let pl_path = Playlist::playlist_dir().join(name.to_owned() + ".m3u");
            let playlist = match Playlist::open(&pl_path) {
                Ok(pl) => pl,
                Err(e) => return Err(anyhow!("Failed to read playlist '{}' from mappings line {}: {}", pl_path, i + 1, e)),
            };
            width = std::cmp::max(width, shortcut.len() + 1 + playlist.name().len() + 2);
            items.push(Some(TuiPickerItemState {
                width: 0,  // Will be updated later
                playlist,
                shortcut,
                states: states.to_vec(),
                state_styles: state_styles.to_owned(),
                state_callback: Box::new(state_callback.clone()),
                state: Rc::new(RefCell::new(0)),
            }));
        }

        // Update the width of every item
        items.iter_mut().for_each(|x| if x.is_some() { x.as_mut().unwrap().width = width; });

        Ok(Self { items })
    }
}

impl TuiPickerItemState {
    pub fn trigger(&mut self) {
        let mut state = self.state.as_ref().borrow_mut();
        *state = (*state + 1) % self.states.len();
        (self.state_callback)(self.states[*state]);
    }
}

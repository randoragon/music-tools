use crate::mpd::mpd_connect;
use camino::{Utf8Path, Utf8PathBuf};
use ratatui::{
    text::{Text, Line, Span},
    layout::Rect,
    buffer::Buffer,
    widgets::Widget,
    style::{Style, Stylize},
};

/// A custom ratatui widget for displaying info about some selected track.
/// If `file` is `Some`, it contains the relative path to the track file.
/// If `file` is `None`, it means that no track is currently selected.
#[derive(Clone)]
pub struct TrackInfo {
    file: Option<Utf8PathBuf>,
    title: Option<String>,
    /// Duration in seconds.
    duration: Option<u64>,
    artist: Option<String>,
    album: Option<String>,
}

impl Widget for TrackInfo {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.file.is_none() {
            Line::raw("Nothing is currently selected.").render(area, buf);
            return;
        }

        let title = match self.title {
            Some(t) => Span::styled(format!("\"{t}\""), Style::new().bold()),
            None => Span::styled("???", Style::new().bold()),
        };

        let duration = match self.duration {
            Some(d) => Span::styled(
                format!("[{}:{}]", d / 60, d % 60),
                Style::new().bold().cyan()
            ),
            None => Span::styled("[??:??]", Style::new().bold().cyan()),
        };

        let artist = match self.artist {
            Some(a) => Span::styled(a, Style::new().bold().yellow()),
            None => Span::styled("???", Style::new().bold().yellow()),
        };

        let album = match self.album {
            Some(a) => Span::styled(a, Style::new().bold().cyan()),
            None => Span::styled("<no album>", Style::new().bold().cyan()),
        };

        Text::from(vec![
            Line::from(vec![title, Span::raw(" "), duration]),
            Line::from(vec![Span::styled("by ", Style::new().bold().green()), artist]),
            Line::from(vec![Span::styled("in ", Style::new().bold().green()), album]),
        ]).render(area, buf);
    }
}

impl Default for TrackInfo {
    /// Initializes TrackInfo with the current song playing in MPD.
    fn default() -> Self {
        let mut ret = Self {
            file: None,
            title: None,
            duration: None,
            artist: None,
            album: None,
        };

        let mut conn = match mpd_connect() {
            Ok(c) => c,
            Err(_) => return ret,
        };

        let song = match conn.currentsong() {
            Ok(s) => s,
            Err(_) => return ret,
        };

        let song = match song {
            Some(s) => s,
            None => return ret,
        };

        ret.file = Some(Utf8PathBuf::from(song.file));
        ret.title = song.title;
        ret.artist = song.artist;
        ret.duration = song.duration.map(|d| d.as_secs());

        ret.album = song.tags.into_iter().find(|x| x.0 == "Album").map(|(_, a)| a);
        ret
    }
}

impl TrackInfo {
    pub fn new<T: AsRef<Utf8Path>>(file: Option<T>, title: String, duration: u64, artist: String, album: Option<String>) -> Self {
        Self {
            file: file.map(|x| Utf8PathBuf::from(x.as_ref())),
            title: Some(title),
            duration: Some(duration),
            artist: Some(artist),
            album
        }
    }

    pub fn file(&self) -> Option<&Utf8PathBuf> {
        self.file.as_ref()
    }

    pub fn title(&self) -> Option<&String> {
        self.title.as_ref()
    }

    pub fn duration(&self) -> Option<u64> {
        self.duration
    }

    pub fn artist(&self) -> Option<&String> {
        self.artist.as_ref()
    }

    pub fn album(&self) -> Option<&String> {
        self.album.as_ref()
    }
}

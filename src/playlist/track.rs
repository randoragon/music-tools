use std::path::PathBuf;

/// A track in a playlist.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Track {
    /// The path to the audio file, relative to `MUSIC_DIR`.
    pub path: PathBuf,
}

impl Track {
    pub fn new(fpath: &str) -> Self {
        Track {
            path: PathBuf::from(fpath),
        }
    }
}

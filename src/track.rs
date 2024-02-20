use camino::{Utf8Path, Utf8PathBuf};

/// A track in a playlist.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Track {
    /// The path to the audio file, relative to `MUSIC_DIR`.
    pub path: Utf8PathBuf,
}

impl Track {
    pub fn new<T: AsRef<Utf8Path>>(fpath: T) -> Self {
        Track {
            path: Utf8PathBuf::from(fpath.as_ref()),
        }
    }
}

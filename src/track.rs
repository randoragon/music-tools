use crate::music_dir;
use camino::{Utf8Path, Utf8PathBuf};

/// A track in a playlist.
///
/// Note that this struct should only provide basic path information for unique identification, and
/// otherwise be fast to hash, clone and not take up a lot of memory. If more information is
/// needed, such as file metadata, ID3v2 tags, etc., it should be delegated to a separate place in
/// memory, to keep this lightweight.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Track {
    /// The path to the audio file, relative to `MUSIC_DIR`.
    pub path: Utf8PathBuf,
}

impl Track {
    /// If `fpath` begins with `MUSIC_DIR`, the prefix is truncated, leaving a relative path.
    pub fn new<T: AsRef<Utf8Path>>(fpath: T) -> Self {
        Track {
            path: Utf8PathBuf::from(
                // Strip music_dir prefix, if it exists
                if fpath.as_ref().starts_with(music_dir()) {
                    fpath.as_ref().strip_prefix(music_dir()).unwrap_or(fpath.as_ref())
                } else {
                    fpath.as_ref()
                }
            ),
        }
    }
}

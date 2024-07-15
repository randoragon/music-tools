pub mod track;
pub mod playlist;
pub mod playcount;

mod tracksfile;

use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use log::warn;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

/// Returns the path to the music directory.
pub fn music_dir() -> &'static Utf8Path {
    static MUSIC_DIR: OnceLock<Utf8PathBuf> = OnceLock::new();
    MUSIC_DIR.get_or_init(|| path_from(dirs::home_dir, "Music"))
}

/// Returns the number of tracks in the entire music library.
///
/// Note that this function only checks this number on its first call. Every subsequent call is
/// instantaneous due to the value being cached, but if there were any changes to the library in
/// the meantime, the reported value might be incorrect.
pub fn library_size() -> usize {
    static LIBRARY_SIZE: OnceLock<usize> = OnceLock::new();

    *LIBRARY_SIZE.get_or_init(|| {
        if let Ok(mut conn) = mpd_connect() {
            if let Ok(list) = conn.listall() {
                return list.len();
            }
        }

        // Fallback if MPD listing fails
        walkdir::WalkDir::new(music_dir())
            .follow_links(false)
            .into_iter()
            .filter_map(|x| x.ok())
            .filter(|x| x.file_name().to_string_lossy().ends_with(".mp3"))
            .count()
    })
}

/// Connects to MPD.
pub fn mpd_connect() -> Result<mpd::Client> {
    const MPD_SOCKET: &str = "127.0.0.1:6601";

    match mpd::Client::connect(MPD_SOCKET) {
        Ok(conn) => Ok(conn),
        Err(e) => Err(anyhow!("Could not connect to MPD: {}", e)),
    }
}

/// Constructs a path by concatenating a `dirs::*` function output and an arbitrary relative path.
///
/// # Examples
/// ```ignore
/// assert_eq!(path_from(dirs::home_dir, "my_file.txt"), "/home/user/my_file.txt");
/// ```
pub fn path_from<A: AsRef<Path>, B: AsRef<Path>>(base_dir: fn() -> Option<A>, rel_path: B) -> Utf8PathBuf {
    assert!(rel_path.as_ref().is_relative(), "rel_path must be relative");
    let path =  match base_dir() {
        Some(path) => path,
        None => panic!("Failed to locate home directory"),
    };
    assert!(path.as_ref().is_absolute(), "base_dir must yield an absolute path");
    let mut path = match path.as_ref().to_str() {
        Some(str) => Utf8PathBuf::from(str),
        None => panic!("Failed to convert base_dir to UTF-8 (other encodings not supported)"),
    };
    let rel_path = match rel_path.as_ref().to_str() {
        Some(path) => path,
        None => panic!("Failed to convert rel_path to UTF-8 (other encodings not supported)"),
    };
    path.push(rel_path);
    path
}

/// Returns an iterator over directory files, with a filtering function.
fn iter_paths<F: Fn(&Utf8Path) -> bool>(dir: &Utf8Path, f: F) -> Result<impl Iterator<Item = Utf8PathBuf>> {
    let mut path_strings = Vec::<Utf8PathBuf>::new();
    for result in fs::read_dir(dir)? {
        let entry = match result {
            Ok(entry) => entry,
            Err(e) => {
                warn!("Unexpected error when listing the '{}' directory: {}, skipping", dir, e);
                continue;
            },
        };
        let path = entry.path();
        let path_str = match path.to_str() {
            Some(str) => str,
            None => return Err(anyhow!("Failed to convert system path {:?} to UTF-8 (other encodings not supported)", path)),
        };
        let path = Utf8PathBuf::from(path_str);
        if f(&path) {
            path_strings.push(path);
        }
    }
    Ok(path_strings.into_iter())
}

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
pub fn dirname() -> &'static Utf8Path {
    static MUSIC_DIR: OnceLock<Utf8PathBuf> = OnceLock::new();
    MUSIC_DIR.get_or_init(|| path_from(dirs::home_dir, "Music"))
}

/// Constructs a path by concatenating a `dirs::*` function output and an arbitrary relative path.
///
/// # Examples
/// ```
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

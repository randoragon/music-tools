pub mod track;
pub mod playlist;
pub mod playcount;

use std::fs;
use camino::{Utf8Path, Utf8PathBuf};
use anyhow::{anyhow, Result};
use log::warn;

const MUSIC_DIR: &'static str = "~/Music";

/// Converts a string that might begin with `"~/"` into a path with home directory expanded.
/// Works by looking up the HOME environment variable. Panics if HOME is not found.
fn expand_tilde(str: String) -> Utf8PathBuf {
    if str.starts_with("~/") {
        let mut path = match std::env::var("HOME") {
            Ok(home) => home,
            Err(e) => panic!("Failed to read HOME: {}", e),
        };
        path.push_str(&str[1..]); // Note that '/' is guaranteed at str[1]
        return Utf8PathBuf::from(path);
    }
    Utf8PathBuf::from(str)
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

pub mod track;
pub mod playlist;
pub mod playcount;

mod tracksfile;

use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use log::warn;
use std::fs;
use std::sync::OnceLock;

/// Returns the path to the music directory.
pub fn dirname() -> &'static Utf8Path {
    static MUSIC_DIR: OnceLock<Utf8PathBuf> = OnceLock::new();
    MUSIC_DIR.get_or_init(|| shellexpand_or_panic("~/Music"))
}

/// Expands a path. Panics on any error encountered, including undefined variables.
fn shellexpand_or_panic(path: &str) -> Utf8PathBuf {
    match shellexpand::full(path) {
        Ok(cow) => Utf8PathBuf::from(cow.as_ref()),
        Err(e) => panic!("Failed to expand path '{}': {}", path, e),
    }
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

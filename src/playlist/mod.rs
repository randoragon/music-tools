pub mod track;

use anyhow::{anyhow, Result};
use log::{error, warn};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Write, BufRead, BufReader};
use std::path::{Path, PathBuf};
use track::Track;

/// Directory where all playlists are stored.
const PLAYLIST_DIR: &'static str = "~/Music/Playlists";

#[derive(Debug)]
pub struct Playlist {
    pub path: PathBuf,
    pub name: OsString,
    pub tracks: Vec<Track>,

    /// Cached index for `tracks`, to avoid linear search.
    tracks_map: HashMap<Track, Vec<usize>>,
}

impl Playlist {
    pub fn new<T: AsRef<Path>>(fpath: T) -> Result<Self> {
        let mut pl = Playlist {
            path: PathBuf::from(fpath.as_ref()),
            name: OsString::with_capacity(64),
            tracks: Vec::new(),
            tracks_map: HashMap::new(),
        };
        match pl.path.file_stem() {
            Some(name) => pl.name.push(name),
            None => return Err(anyhow!("Failed to extract filename from '{:?}'", pl.path)),
        }

        let file = BufReader::new(File::open(fpath)?);
        for line in file.lines() {
            match line {
                Ok(str) => {
                    let track = Track::new(&str);
                    if pl.tracks_map.contains_key(&track) {
                        pl.tracks_map.get_mut(&track)
                            .unwrap()
                            .push(pl.tracks.len());
                        pl.tracks.push(track);
                    } else {
                        let list = vec![pl.tracks.len()];
                        pl.tracks_map.insert(track.clone(), list);
                        pl.tracks.push(track);
                    }
                },
                _ => break,
            }
        }
        Ok(pl)
    }

    pub fn dirname() -> PathBuf {
        let str = PLAYLIST_DIR.to_string();
        if str.starts_with("~/") {
            let mut path = match std::env::var("HOME") {
                Ok(home) => home,
                Err(e) => panic!("Could not find $HOME: {}", e),
            };
            path.push_str(&str[1..]); // Note that '/' is guaranteed at str[1]
            return PathBuf::from(path);
        }
        PathBuf::from(str)
    }

    /// Returns an iterator over all playlist file paths.
    pub fn iter_paths() -> Result<impl Iterator<Item = PathBuf>> {
        fn path_filter(path: PathBuf) -> Option<PathBuf> {
            if path.is_file() && path.extension().is_some_and(|x| x == "m3u") {
                return Some(path);
            }
            None
        }
        let paths = fs::read_dir(Self::dirname())?
            .filter_map(|result| result.ok().and_then(|entry| path_filter(entry.path())));
        Ok(paths)
    }

    /// Returns an iterator over all playlists.
    ///
    /// Playlists are only loaded into memory when the iterator gets to them.
    pub fn iter_playlists() -> Option<impl Iterator<Item = Playlist>> {
        match Self::iter_paths() {
            Ok(paths) => {
                let iterator = paths.filter_map(|path|
                    match Playlist::new(&path) {
                        Ok(playlist) => Some(playlist),
                        Err(e) => {
                            warn!("Failed to read playlist '{:?}': {}, skipping", path, e);
                            None
                        },
                    }
                );
                Some(iterator)
            },
            Err(e) => {
                error!("Failed to list the playlists directory '{:?}': {}", Playlist::dirname(), e);
                None
            }
        }
    }

    /// Write the playlist file to disk (previous contents are lost).
    pub fn write(&self) -> Result<()> {
        // Theoretically, converting from PathBuf to String can fail if the underlying OsString
        // cannot be converted to UTF-8. Writing a playlist file must not "partially succeed";
        // in case of any difficulty, it should fail without doing anything to the external file.
        // As such, make sure all PathBufs can be converted first, and only then begin writing
        // to the file.
        let mut track_strings: Vec<String> = vec![];
        for track in &self.tracks {
            match track.path.clone().into_os_string().into_string() {
                Ok(str) => track_strings.push(str),
                Err(e) => return Err(anyhow!("Failed to convert track OsString to String: {:?}", e)),
            };
        }

        let mut file = File::create(&self.path)?;
        write!(file, "{}\n", track_strings.join("\n"))?;
        Ok(())
    }
}

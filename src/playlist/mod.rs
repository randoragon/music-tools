pub mod track;

use track::*;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};

/// Directory where all playlists are stored.
const PLAYLIST_DIR: &'static str = "~/Music/Playlists";

#[derive(Debug)]
pub struct Playlist {
    pub path: PathBuf,
    pub tracks: Vec<Track>,

    /// Cached index for `tracks`, to avoid linear search.
    tracks_map: HashMap<Track, Vec<usize>>,
}

impl Playlist {
    pub fn new<T: AsRef<Path>>(fpath: T) -> io::Result<Self> {
        let mut pl = Playlist{
            path: PathBuf::from(fpath.as_ref()),
            tracks: Vec::new(),
            tracks_map: HashMap::new(),
        };

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
            path.push_str(&str[1..]);  // Note that '/' is guaranteed at str[1]
            return PathBuf::from(path);
        }
        PathBuf::from(str)
    }

    pub fn iter_paths() -> io::Result<impl Iterator<Item = PathBuf>> {
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
}

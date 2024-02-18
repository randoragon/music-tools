pub mod track;

use track::*;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};

/// Directory where all playlists are stored.
pub const PLAYLIST_DIR: &'static str = "~/Music/Playlists";

#[derive(Debug)]
pub struct Playlist {
    pub tracks: Vec<Track>,

    /// Cached index for `tracks`, to avoid linear search.
    tracks_map: HashMap<Track, Vec<usize>>,
}

impl Playlist {
    pub fn new<T: AsRef<Path>>(fpath: T) -> io::Result<Self> {
        let mut pl = Playlist{
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

    pub fn iter_paths() -> io::Result<impl Iterator<Item = PathBuf>> {
        let paths = fs::read_dir(PLAYLIST_DIR)?
            .filter(|x| -> bool {
                let path = match x {
                    Ok(entry) => entry.path(),
                    _ => return false,
                };
                path.is_file() && path.ends_with(".m3u")
            })
            .map(|x| -> PathBuf { x.unwrap().path() });
        Ok(paths)
    }
}

pub use crate::tracksfile::TracksFile;

use crate::track::Track;
use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use log::{error, warn};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Write, BufRead, BufReader};

#[derive(Debug)]
pub struct Playlist {
    path: Utf8PathBuf,
    name: String,
    tracks: Vec<Track>,

    /// Cached index for `tracks`, to avoid linear search.
    tracks_map: HashMap<Track, Vec<usize>>,
}

impl Playlist {
    /// Directory where all playlists are stored.
    const DIR: &'static str = "~/Music/Playlists";

    /// Returns the path to the playlists directory.
    fn dirname() -> Utf8PathBuf {
        crate::expand_tilde(Self::DIR.to_string())
    }

    /// Returns an iterator over all playlist file paths.
    fn iter_paths() -> Result<impl Iterator<Item = Utf8PathBuf>> {
        crate::iter_paths(
            &Self::dirname(),
            |x| x.is_file() && x.extension().is_some_and(|y| y == "m3u")
        )
    }

    /// Returns the playlist name.
    pub fn name(&self) -> &String {
        &self.name
    }
}

impl TracksFile for Playlist {
    fn new<T: AsRef<Utf8Path>>(fpath: T) -> Result<Self> {
        let mut pl = Self {
            path: Utf8PathBuf::from(fpath.as_ref()),
            name: String::with_capacity(64),
            tracks: Vec::new(),
            tracks_map: HashMap::new(),
        };
        match pl.path.file_stem() {
            Some(name) => pl.name.push_str(name),
            None => return Err(anyhow!("Failed to extract filename from '{:?}'", pl.path)),
        }

        let file = BufReader::new(File::open(&pl.path)?);
        for line in file.lines() {
            let line = match line {
                Ok(str) => str,
                Err(e) => return Err(anyhow!("Failed to read line from '{}': {}", pl.path, e)),
            };
            let track = Track::new(&line);
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
        }
        Ok(pl)
    }

    fn iter() -> Option<impl Iterator<Item = Self>> {
        let it = match Self::iter_paths() {
            Ok(it) => it,
            Err(e) => {
                error!("Failed to list the playlists directory '{:?}': {}", Self::dirname(), e);
                return None;
            },
        };
        let it = it.filter_map(|path|
            match Self::new(&path) {
                Ok(playlist) => Some(playlist),
                Err(e) => {
                    warn!("Failed to read playlist '{:?}': {}, skipping", path, e);
                    None
                },
            }
        );
        Some(it)
    }

    fn path(&self) -> &Utf8PathBuf {
        &self.path
    }

    fn tracks(&self) -> impl Iterator<Item = &Track> {
        self.tracks.iter()
    }

    fn tracks_unique(&self) -> impl Iterator<Item = &Track> {
        self.tracks_map.keys()
    }

    fn track_positions(&self, track: &Track) -> Option<&Vec<usize>> {
        self.tracks_map.get(track)
    }

    fn write(&self) -> Result<()> {
        let mut file = File::create(&self.path)?;
        writeln!(file, "{}",
            self.tracks.iter()
                .map(|x| x.path.clone().into_string())
                .collect::<Vec<String>>()
                .join("\n")
        )?;
        Ok(())
    }

    fn remove_at(&mut self, index: usize) {
        if index >= self.tracks.len() {
            warn!("Out-of-bounds remove_at requested (index: {}, len: {})", index, self.tracks.len());
            return;
        }

        // Remove index pointing at the given track from `tracks_map`
        let track = &self.tracks[index];
        // If either unwrap here fails, it means `tracks_map` got corrupt somehow
        let map_index = self.tracks_map[track].iter().position(|&x| x == index).unwrap();
        self.tracks_map.get_mut(track).unwrap().remove(map_index);

        self.tracks.remove(index);
    }

    fn remove_all(&mut self, track: &Track) {
        if !self.tracks_map.contains_key(track) {
            warn!("Attempted to remove a track that does not exist (playlist: {:?}, track: {:?})", self.name, track);
            return;
        }
        let mut indices = self.tracks_map.remove(track).unwrap();
        indices.sort_unstable();
        for index in indices.iter().rev() {
            self.remove_at(*index);
        }
    }
}

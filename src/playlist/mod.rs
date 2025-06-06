pub use crate::tracksfile::TracksFile;

use crate::music_dir;
use crate::track::Track;
use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use log::warn;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Write, BufRead, BufReader};
use std::sync::OnceLock;

#[derive(Debug)]
pub struct Playlist {
    path: Utf8PathBuf,
    name: String,
    tracks: Vec<Track>,

    /// Cached index for `tracks`, to avoid linear search.
    tracks_map: HashMap<Track, Vec<usize>>,

    /// Whether the playlist was modified since the last `write`.
    is_modified: bool,
}

impl Playlist {
    /// Returns the path to the playlists directory.
    pub fn playlist_dir() -> &'static Utf8Path {
        static PLAYLISTS_DIR: OnceLock<Utf8PathBuf> = OnceLock::new();
        PLAYLISTS_DIR.get_or_init(|| music_dir().join("Playlists"))
    }

    /// Returns the path to the ignore playlist file. This is a meta-playlist that stores invalid
    /// paths that should not be erased from hist.* and playcount files, for historical reasons.
    pub fn ignore_file() -> &'static Utf8Path {
        static IGNORE_FILE: OnceLock<Utf8PathBuf> = OnceLock::new();
        IGNORE_FILE.get_or_init(|| music_dir().join(".ignore.m3u"))
    }

    /// Removes all duplicate tracks from the playlist, leaving only the first occurrence of each.
    /// Returns the number of tracks removed.
    pub fn remove_duplicates(&mut self) -> usize {
        // Build a list of all indices to remove
        let mut indices = Vec::new();
        for pos in self.tracks_map.values() {
            if pos.len() > 1 {
                indices.extend_from_slice(&pos[1..]);
            }
        }

        let n_duplicates = indices.len();

        // Remove the indices
        if !indices.is_empty() {
            indices.sort_unstable();
            indices.into_iter().rev().for_each(|x| self.remove_at(x));
            self.is_modified = true;
        }
        debug_assert!(self.verify_integrity());

        n_duplicates
    }

    /// Returns an iterator over all playlist file paths.
    fn iter_paths() -> Result<impl Iterator<Item = Utf8PathBuf>> {
        crate::iter_paths(
            Self::playlist_dir(),
            |x| x.is_file() && x.extension().is_some_and(|y| y == "m3u")
        )
    }

    /// Clears `track_map`, iterates through `tracks` and rebuilds it.
    fn rebuild_tracks_map(&mut self) {
        self.tracks_map.clear();
        for (i, track) in self.tracks.iter().enumerate() {
            if self.tracks_map.contains_key(track) {
                self.tracks_map.get_mut(track).unwrap().push(i);
            } else {
                self.tracks_map.insert(track.clone(), vec![i]);
            }
        }
        debug_assert!(self.verify_integrity());
    }

    /// Verifies the integrity of the struct. This is quite slow and intended for use with
    /// `debug_assert`.
    fn verify_integrity(&self) -> bool {
        for (i, track) in self.tracks.iter().enumerate() {
            if !self.tracks_map.contains_key(track) {
                return false;
            }
            if !self.tracks_map[track].contains(&i) {
                return false;
            }
        }
        for (track, indices) in self.tracks_map.iter() {
            if indices.is_empty() {
                return false;
            }
            if indices.iter().any(|&i| &self.tracks[i] != track) {
                return false;
            }
        }
        true
    }

    /// Returns the playlist name.
    pub fn name(&self) -> &String {
        &self.name
    }

    /// Same as `push()`, but for `Track` objects. For convenience and to avoid constructing
    /// unnecessary new `Track`s.
    pub fn push_track(&mut self, track: Track) -> Result<()> {
        if let Some(v) = self.tracks_map.get_mut(&track) {
            v.push(self.tracks.len());
        } else {
            self.tracks_map.insert(track.clone(), vec![self.tracks.len()]);
        }
        self.tracks.push(track);
        self.is_modified = true;
        debug_assert!(self.verify_integrity());
        Ok(())
    }
}

impl TracksFile for Playlist {
    fn open<T: AsRef<Utf8Path>>(fpath: T) -> Result<Self> {
        let mut pl = Self::new(fpath)?;
        pl.reload()?;
        Ok(pl)
    }

    fn new<T: AsRef<Utf8Path>>(fpath: T) -> Result<Self> where Self: Sized {
        let mut pl = Self {
            path: Utf8PathBuf::from(fpath.as_ref()),
            name: String::with_capacity(64),
            tracks: Vec::new(),
            tracks_map: HashMap::new(),
            is_modified: false,
        };
        match pl.path.file_stem() {
            Some(name) => pl.name.push_str(name),
            None => return Err(anyhow!("Failed to extract filename from '{:?}'", pl.path)),
        }
        Ok(pl)
    }

    fn len(&self) -> usize {
        self.tracks.len()
    }

    fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    fn open_or_new<T: AsRef<Utf8Path>>(fpath: T) -> Result<Self> where Self: Sized {
        match fpath.as_ref().exists() {
            true => Self::open(fpath),
            false => Self::new(fpath),
        }
    }

    fn reload(&mut self) -> Result<()> {
        let mut tracks_new = Vec::new();
        let mut tracks_map_new = HashMap::<Track, Vec<usize>>::new();

        let file = BufReader::new(File::open(&self.path)?);
        for line in file.lines() {
            let line = match line {
                Ok(str) => str,
                Err(e) => return Err(anyhow!("Failed to read line from '{}': {}", self.path, e)),
            };
            let track = Track::new(&line);
            if tracks_map_new.contains_key(&track) {
                tracks_map_new.get_mut(&track)
                    .unwrap()
                    .push(tracks_new.len());
                tracks_new.push(track);
            } else {
                let list = vec![tracks_new.len()];
                tracks_map_new.insert(track.clone(), list);
                tracks_new.push(track);
            }
        }

        // Don't represent empty files with a single empty track
        if tracks_new.len() == 1 && tracks_new[0].path.as_str().is_empty() {
            tracks_new.clear();
            tracks_map_new.clear();
        }

        self.tracks = tracks_new;
        self.tracks_map = tracks_map_new;
        self.is_modified = false;
        debug_assert!(self.verify_integrity());
        Ok(())
    }

    fn iter() -> Result<impl Iterator<Item = Self>> {
        let it = match Self::iter_paths() {
            Ok(it) => it,
            Err(e) => return Err(anyhow!("Failed to list the playlists directory '{:?}': {}", Self::playlist_dir(), e)),
        };
        let it = it.filter_map(|path|
            match Self::open(&path) {
                Ok(playlist) => Some(playlist),
                Err(e) => {
                    warn!("Failed to read playlist '{:?}': {}, skipping", path, e);
                    None
                },
            }
        );
        Ok(it)
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

    fn contains(&self, track: &Track) -> bool {
        self.tracks_map.contains_key(track)
    }

    fn track_positions(&self, track: &Track) -> Option<&Vec<usize>> {
        self.tracks_map.get(track)
    }

    fn is_modified(&self) -> bool {
        self.is_modified
    }

    fn write(&mut self) -> Result<()> {
        let mut file = File::create(&self.path)?;
        write!(file, "{}",
            self.tracks.iter()
                .map(|x| x.path.clone().into_string() + "\n")
                .collect::<Vec<String>>()
                .concat()
        )?;
        self.is_modified = false;
        Ok(())
    }

    fn push<T: AsRef<Utf8Path>>(&mut self, fpath: T) -> Result<()> {
        self.push_track(Track::new(fpath))
    }

    fn remove_last(&mut self, track: &Track) -> bool {
        if !self.tracks_map.contains_key(track) {
            return false;
        }
        let index = self.tracks_map[track].iter().max().unwrap();
        self.remove_at(*index);
        self.is_modified = true;
        true
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
        if self.tracks_map[track].is_empty() {
            self.tracks_map.remove(track);
        }

        self.tracks.remove(index);

        // Shift all higher indices down by one
        for indices in self.tracks_map.values_mut() {
            for i in indices.iter_mut() {
                assert!(*i != index);
                if *i > index {
                    *i -= 1;
                }
            }
        }
        self.is_modified = true;
        debug_assert!(self.verify_integrity());
    }

    fn remove_all(&mut self, track: &Track) -> usize {
        if !self.tracks_map.contains_key(track) {
            return 0;
        }
        let mut indices = self.tracks_map[track].clone();
        indices.sort_unstable();
        for index in indices.iter().rev() {
            self.remove_at(*index);
        }
        self.is_modified = true;
        indices.len()
    }

    fn bulk_rename(&mut self, edits: &HashMap<Track, Utf8PathBuf>) -> Result<usize> {
        let mut n_changed = 0usize;
        for (target_track, new_path) in edits {
            if !self.tracks_map.contains_key(target_track) {
                continue;
            }
            for &index in &self.tracks_map[target_track] {
                self.tracks[index].path = new_path.clone();
                n_changed += 1;
            }
            self.is_modified = true;
        }
        self.rebuild_tracks_map();
        Ok(n_changed)
    }
}

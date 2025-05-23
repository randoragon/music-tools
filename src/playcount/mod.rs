pub mod entry;

pub use entry::Entry;
pub use crate::tracksfile::TracksFile;

use crate::{music_dir, path_from};
use crate::track::Track;
use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use log::warn;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Write, BufRead, BufReader};
use std::sync::OnceLock;
use std::time::Duration;
use chrono::Local;

#[derive(Debug)]
pub struct Playcount {
    path: Utf8PathBuf,
    entries: Vec<Entry>,

    /// Cached index for `entries` which correspond to a given track.
    tracks_map: HashMap<Track, Vec<usize>>,

    /// Whether the playcount was modified since the last `write`.
    is_modified: bool,
}

impl Playcount {
    /// Returns the path to the playcount directory.
    pub fn playcount_dir() -> &'static Utf8Path {
        static PLAYCOUNTS_DIR: OnceLock<Utf8PathBuf> = OnceLock::new();
        PLAYCOUNTS_DIR.get_or_init(|| music_dir().join(".playcount"))
    }

    /// Returns an iterator over all playcount file paths.
    pub fn iter_paths() -> Result<impl Iterator<Item = Utf8PathBuf>> {
        crate::iter_paths(
            Self::playcount_dir(),
            |x| x.is_file() && x.extension().is_some_and(|y| y == "tsv")
        )
    }

    /// Clears `track_map`, iterates through `tracks` and rebuilds it.
    fn rebuild_tracks_map(&mut self) {
        self.tracks_map.clear();
        for (i, entry) in self.entries.iter().enumerate() {
            let track = &entry.track;
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
        for (i, entry) in self.entries.iter().enumerate() {
            let track = &entry.track;
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
            if indices.iter().any(|&i| &self.entries[i].track != track) {
                return false;
            }
        }
        true
    }

    /// Convenience function that works like `open_or_new` on the current playcount file, based on
    /// system time.
    pub fn current() -> Result<Self> {
        Self::open_or_new(Self::current_path())
    }

    /// Returns the path where the current playcount file should be located, based on system time.
    pub fn current_path() -> Utf8PathBuf {
        let playcount_fname = Local::now().format("%Y-%m.tsv").to_string();
        path_from(|| Some(Self::playcount_dir()), playcount_fname)
    }

    /// Returns an iterator to all entries in the playcount, in order of appearance.
    /// Note that several entries may refer to the same track.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }
}

impl TracksFile for Playcount {
    fn open<T: AsRef<Utf8Path>>(fpath: T) -> Result<Self> {
        let mut pc = Self::new(fpath)?;
        pc.reload()?;
        Ok(pc)
    }

    fn new<T: AsRef<Utf8Path>>(fpath: T) -> Result<Self> {
        Ok(Self {
            path: Utf8PathBuf::from(fpath.as_ref()),
            entries: Vec::new(),
            tracks_map: HashMap::new(),
            is_modified: false,
        })
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn open_or_new<T: AsRef<Utf8Path>>(fpath: T) -> Result<Self> where Self: Sized {
        match fpath.as_ref().exists() {
            true => Self::open(fpath),
            false => Self::new(fpath),
        }
    }

    fn reload(&mut self) -> Result<()> {
        let mut entries_new = Vec::new();
        let mut tracks_map_new = HashMap::<Track, Vec<usize>>::new();

        let file = BufReader::new(File::open(&self.path)?);
        for (i, line) in file.lines().enumerate() {
            let line = match line {
                Ok(str) => str,
                Err(e) => return Err(anyhow!("Failed to read line {} in '{}': {}", i+1, self.path, e)),
            };
            let entry = match line.parse::<Entry>() {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("Failed to parse line {} in '{}': {}, skipping", i+1, self.path, e);
                    continue;
                },
            };
            if tracks_map_new.contains_key(&entry.track) {
                tracks_map_new.get_mut(&entry.track)
                    .unwrap()
                    .push(entries_new.len());
                entries_new.push(entry);
            } else {
                let list = vec![entries_new.len()];
                tracks_map_new.insert(entry.track.clone(), list);
                entries_new.push(entry);
            }
        }

        self.entries = entries_new;
        self.tracks_map = tracks_map_new;
        self.is_modified = false;
        debug_assert!(self.verify_integrity());
        Ok(())
    }

    fn iter() -> Result<impl Iterator<Item = Self>> {
        let it = match Self::iter_paths() {
            Ok(it) => it,
            Err(e) => return Err(anyhow!("Failed to list the playcounts directory '{:?}': {}", Self::playcount_dir(), e)),
        };
        let it = it.filter_map(|path|
            match Self::open(&path) {
                Ok(playcount) => Some(playcount),
                Err(e) => {
                    warn!("Failed to read playcount '{:?}': {}, skipping", path, e);
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
        self.entries.iter().map(|x| &x.track)
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
            self.entries.iter()
                .map(|x| x.as_file_line() + "\n")
                .collect::<Vec<String>>()
                .concat())?;
        self.is_modified = false;
        Ok(())
    }

    fn push<T: AsRef<Utf8Path>>(&mut self, fpath: T) -> Result<()> {
        let entry = match Entry::new(&fpath, None, None, None, None, None) {
            Ok(entry) => entry,
            Err(e) => return Err(anyhow!("Failed to create an entry from '{}': {}", fpath.as_ref(), e)),
        };

        if let Some(v) = self.tracks_map.get_mut(&entry.track) {
            v.push(self.entries.len());
        } else {
            self.tracks_map.insert(entry.track.clone(), vec![self.entries.len()]);
        }
        self.entries.push(entry);
        self.is_modified = true;
        debug_assert!(self.verify_integrity());
        Ok(())
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
        if index >= self.entries.len() {
            warn!("Out-of-bounds remove_at requested (index: {}, len: {})", index, self.entries.len());
            return;
        }

        // Remove index pointing at the given track from `tracks_map`
        let track = &self.entries[index].track;
        // If either unwrap here fails, it means `tracks_map` got corrupt somehow
        let map_index = self.tracks_map[track].iter().position(|&x| x == index).unwrap();
        self.tracks_map.get_mut(track).unwrap().remove(map_index);
        if self.tracks_map[track].is_empty() {
            self.tracks_map.remove(track);
        }

        self.entries.remove(index);

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
        // Make sure all metadata can be pulled from target paths in advance
        // to guarantee either all or no renames get applied.
        let mut new_entries = HashMap::<&Utf8PathBuf, Entry>::new();
        for (target_track, new_path) in edits {
            if !self.tracks_map.contains_key(target_track) {
                continue;
            }

            // Create a new track from path
            let new_entry = match Entry::new(
                new_path,
                Some(Duration::new(0, 0)), // Will be changed to each of the old entries' values
                None, None, None, None, // Read artist, album artist, album and title from `new_path` ID3v2 tags
            ) {
                Ok(entry) => entry,
                Err(e) => return Err(anyhow!("Failed to construct a playcount entry for '{}': {}", new_path, e)),
            };

            new_entries.insert(new_path, new_entry);
        }

        // "Tricky scenarios" are avoided, because `self.tracks_map` is only updated at the end.
        let mut n_changed = 0usize;
        for (target_track, new_path) in edits {
            if !self.tracks_map.contains_key(target_track) {
                continue;
            }

            // Unwrap because this is guaranteed
            let new_entry = new_entries.get_mut(new_path).unwrap();

            for &index in &self.tracks_map[target_track] {
                new_entry.duration = self.entries[index].duration;
                self.entries[index] = new_entry.clone();
                n_changed += 1;
            }
            self.is_modified = true;
        }
        self.rebuild_tracks_map();
        Ok(n_changed)
    }
}

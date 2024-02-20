pub mod entry;

use crate::track::Track;
use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use log::{error, warn};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Write, BufRead, BufReader};
use entry::Entry;

/// Directory where all playcount files are stored.
const PLAYCOUNT_DIR: &'static str = "~/Music/.playcount";

#[derive(Debug)]
pub struct Playcount {
    path: Utf8PathBuf,
    entries: Vec<Entry>,

    /// Cached index for `entries` which correspond to a given track.
    tracks_map: HashMap<Track, Vec<usize>>,
}

impl Playcount {
    pub fn new<T: AsRef<Utf8Path>>(fpath: T) -> Result<Self> {
        let mut pc = Playcount {
            path: Utf8PathBuf::from(fpath.as_ref()),
            entries: Vec::new(),
            tracks_map: HashMap::new(),
        };

        let file = BufReader::new(File::open(&pc.path)?);
        for line in file.lines() {
            let line = match line {
                Ok(str) => str,
                Err(e) => return Err(anyhow!("Failed to read line from '{}': {}", pc.path, e)),
            };
            let entry = match line.parse::<Entry>() {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("Failed to parse line in '{}': {}, skipping", pc.path, e);
                    continue;
                },
            };
            if pc.tracks_map.contains_key(&entry.track) {
                pc.tracks_map.get_mut(&entry.track)
                    .unwrap()
                    .push(pc.entries.len());
                pc.entries.push(entry);
            } else {
                let list = vec![pc.entries.len()];
                pc.tracks_map.insert(entry.track.clone(), list);
                pc.entries.push(entry);
            }
        }
        Ok(pc)
    }

    /// Returns the path to the playcount directory.
    pub fn dirname() -> Utf8PathBuf {
        crate::expand_tilde(PLAYCOUNT_DIR.to_string())
    }

    /// Returns an iterator over all playcount file paths.
    pub fn iter_paths() -> Result<impl Iterator<Item = Utf8PathBuf>> {
        crate::iter_paths(
            &Self::dirname(),
            |x| x.is_file() && x.extension().is_some_and(|y| y == "tsv")
        )
    }

    /// Returns an iterator over all playcounts.
    ///
    /// Playcounts are only loaded into memory when the iterator gets to them.
    pub fn iter_playcounts() -> Option<impl Iterator<Item = Playcount>> {
        let it = match Self::iter_paths() {
            Ok(it) => it,
            Err(e) => {
                error!("Failed to list the playcounts directory '{:?}': {}", Playcount::dirname(), e);
                return None;
            },
        };
        let it = it.filter_map(|path|
            match Playcount::new(&path) {
                Ok(playcount) => Some(playcount),
                Err(e) => {
                    warn!("Failed to read playcount '{:?}': {}, skipping", path, e);
                    None
                },
            }
        );
        Some(it)
    }

    /// Returns the playcount path.
    pub fn path(&self) -> &Utf8PathBuf {
        &self.path
    }

    /// Returns an iterator to all entries in the playcount, in order of appearance.
    /// Note that several entries may refer to the same track.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    /// Returns an iterator to all unique tracks in the playcount.
    /// The order is undefined and arbitrary.
    pub fn tracks_unique(&self) -> impl Iterator<Item = &Track> {
        self.tracks_map.iter().map(|(k, _)| k)
    }

    /// Returns a vector of playcount indices at which entries corresponding to a given track
    /// occur. The indices are sorted in ascending order, i.e. the order in which they appear on
    /// the playcount.
    pub fn entry_positions(&self, track: &Track) -> Option<&Vec<usize>> {
        self.tracks_map.get(track)
    }

    /// Write the playcount file to disk (previous contents are lost).
    pub fn write(&self) -> Result<()> {
        let mut file = File::create(&self.path)?;
        write!(file, "{}\n",
            self.entries.iter()
                .map(|x| format!("{}\t{}", x.count, x.track.path))
                .collect::<Vec<String>>()
                .join("\n")
        )?;
        Ok(())
    }

    /// Remove a track from the playcount, by index.
    pub fn remove_at(&mut self, index: usize) {
        if index >= self.entries.len() {
            warn!("Out-of-bounds remove_at requested (index: {}, len: {})", index, self.entries.len());
            return;
        }

        // Remove index pointing at the given track from `tracks_map`
        let track = &self.entries[index].track;
        // If either unwrap here fails, it means `tracks_map` got corrupt somehow
        let map_index = self.tracks_map[track].iter().position(|&x| x == index).unwrap();
        self.tracks_map.get_mut(track).unwrap().remove(map_index);

        self.entries.remove(index);
    }

    /// Remove all occurrences of a track from the playcount.
    pub fn remove_all(&mut self, track: &Track) {
        if !self.tracks_map.contains_key(track) {
            warn!("Attempted to remove a track that does not exist (playcount: {:?}, track: {:?})", self.path, track);
            return;
        }
        let mut indices = self.tracks_map.remove(track).unwrap();
        indices.sort_unstable();
        for index in indices.iter().rev() {
            self.remove_at(*index);
        }
    }
}


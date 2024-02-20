use crate::track::Track;
use camino::{Utf8Path, Utf8PathBuf};
use anyhow::{anyhow, Result, Error};

/// Representation of a single line in a playcount file.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Entry {
    /// The track that was played.
    pub track: Track,

    /// The number of times `track` was played.
    /// This number may be smaller than the total number of plays of `track` within the entire
    /// playcount file, because multiple entries for the same `track` may exist, in which case
    /// all their `count`s should be summed up.
    pub count: usize,
}

impl Entry {
    pub fn new<T: AsRef<Utf8Path>>(fpath: T, count: usize) -> Self {
        Entry {
            track: Track::new(fpath),
            count,
        }
    }
}

impl std::str::FromStr for Entry {
    type Err = Error;

    fn from_str(line: &str) -> Result<Self> {
        let mut it = line.splitn(2, '\t');
        let count_str = match it.next() {
            Some(split) => split,
            None => return Err(anyhow!("Failed to extract count substring from playcount line '{}'", line)),
        };
        let path = match it.next() {
            Some(split) => Utf8PathBuf::from(split),
            None => return Err(anyhow!("Failed to extract path substring from playcount line '{}'", line)),
        };

        let count = match count_str.parse::<usize>() {
            Ok(num) => num,
            Err(e) => return Err(anyhow!("Failed to convert count substring '{}' to number: {}", count_str, e)),
        };

        Ok(Entry::new(path, count))
    }
}

use crate::track::Track;
use camino::{Utf8Path, Utf8PathBuf};
use anyhow::{anyhow, Result, Error};
use std::time::Duration;
use id3::{Tag, TagLike};
use metadata::media_file::MediaFileMetadata;

/// Representation of a single line in a playcount file.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Entry {
    /// The track that was played.
    pub track: Track,

    /// The playtime length of the track. This is the only value that is allowed to vary across
    /// entries for the same `track` and should never be changed in playcount files.
    ///
    /// The variability of `duration` can happen when the audio file is modified in some way, e.g.
    /// a leading or trailing silence is trimmed, leading to earlier playcount entries having a
    /// longer `duration`, and later ones having a shorter `duration`.
    pub duration: Duration,

    /// Artist name.
    pub artist: String,

    /// Album title, if any.
    pub album: Option<String>,

    /// Track title.
    pub title: String,
}

impl Entry {
    /// Create a new playcount entry. Only `fpath` is required, the rest can be inferred
    /// automatically if passed as `None`, or explicitly stated.
    pub fn new<T: AsRef<Utf8Path>>(fpath: T, duration: Option<Duration>, artist: Option<String>, album: Option<Option<String>>, title: Option<String>) -> Result<Self> {
        let duration = match duration {
            Some(duration) => duration,
            None => {
                let metadata = match MediaFileMetadata::new(&fpath.as_ref()) {
                    Ok(data) => data,
                    Err(e) => return Err(anyhow!("Failed to extract metadata from media file '{}': {}", fpath.as_ref(), e)),
                };

                let duration = match metadata._duration {
                    Some(val) => val,
                    None => 0.0, // return Err(anyhow!("File '{}' has no duration metadata", path)),
                };
                Duration::new(duration as u64, ((duration - duration.floor()) * 1e9) as u32)
            }
        };

        let mut tag: Option<Tag> = None;
        if artist.is_none() || album.is_none() || title.is_none() {
            tag = match Tag::read_from_path(&fpath.as_ref()) {
                Ok(tag) => Some(tag),
                Err(e) => return Err(anyhow!("Failed to read ID3v2 tag from '{}': {}", fpath.as_ref(), e)),
            };
        }

        let artist = match artist {
            Some(artist) => artist,
            None => match tag.as_ref().unwrap().artist() {
                Some(val) => val.to_string(),
                None => return Err(anyhow!("Artist ID3v2 frame missing from '{}'", fpath.as_ref())),
            }
        };

        let album = match album {
            Some(album) => album,
            None => match tag.as_ref().unwrap().album() {
                Some(str) => Some(str.to_string()),
                None => None,
            },
        };

        let title = match title {
            Some(title) => title,
            None => match tag.as_ref().unwrap().title() {
                Some(val) => val.to_string(),
                None => return Err(anyhow!("Title ID3v2 frame missing from '{}'", fpath.as_ref())),
            },
        };

        Ok(Entry {
            track: Track::new(fpath),
            duration,
            artist,
            album,
            title,
        })
    }

    pub fn as_file_line(&self) -> String {
        format!("{}\t{}\t{}\t{}\t{}",
            self.duration.as_secs_f32(),
            self.artist,
            self.album.as_ref().unwrap_or(&String::new()),
            self.title,
            self.track.path)
    }
}

impl std::str::FromStr for Entry {
    type Err = Error;

    fn from_str(line: &str) -> Result<Self, anyhow::Error> {
        let mut it = line.splitn(5, '\t');
        let duration_str = match it.next() {
            Some(split) => split,
            None => return Err(anyhow!("Failed to extract duration substring from playcount line '{}'", line)),
        };
        let artist = match it.next() {
            Some(split) => split.to_string(),
            None => return Err(anyhow!("Failed to extract artist substring from playcount line '{}'", line)),
        };
        let album = match it.next() {
            Some(split) => split,
            None => return Err(anyhow!("Failed to extract album substring from playcount line '{}'", line)),
        };
        let title = match it.next() {
            Some(split) => split.to_string(),
            None => return Err(anyhow!("Failed to extract title substring from playcount line '{}'", line)),
        };
        let path = match it.next() {
            Some(split) => Utf8PathBuf::from(split),
            None => return Err(anyhow!("Failed to extract path substring from playcount line '{}'", line)),
        };

        let duration = match duration_str.parse::<f32>() {
            Ok(num) => num,
            Err(e) => return Err(anyhow!("Failed to convert count substring '{}' to number: {}", duration_str, e)),
        };
        let duration = Duration::new(duration as u64, ((duration - duration.floor()) * 1e9) as u32);

        Ok(Self::new(
            path,
            Some(duration),
            Some(artist),
            Some(if album.is_empty() { None } else { Some(album.to_string()) }),
            Some(title),
        )?)
    }
}

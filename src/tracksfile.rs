use crate::track::Track;
use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashMap;

/// A trait for dealing with text files containing a list of tracks.
/// This description fits m3u playlists, but also more esoteric custom formats.
///
/// An important requirement is that the number of possible objects (and thus text files) is finite
/// and possible to iterate over in a quick fashion. In practice this means that all the text files
/// reside in known locations in the filesystem. This is an implementation detail though, as the
/// source of the objects is not exposed in any way; what matters is the ability to iterate.
pub trait TracksFile {
    /// Creates a new object from file contents.
    fn new<T: AsRef<Utf8Path>>(fpath: T) -> Result<Self> where Self: Sized;

    /// Returns an iterator over all objects.
    /// The objects are not all loaded into memory at once; they are created on-demand only.
    fn iter() -> Option<impl Iterator<Item = Self>> where Self: Sized;

    /// Returns the path to the text file from which the object was created.
    fn path(&self) -> &Utf8PathBuf;

    /// Returns an iterator to all tracks in the object, in order of appearance.
    /// Note that tracks can repeat. For a unique iterator, see `tracks_unique()`.
    fn tracks(&self) -> impl Iterator<Item = &Track>;

    /// Returns an iterator to all unique tracks in the object.
    /// The order is undefined and arbitrary. For a defined order, see `tracks()`.
    fn tracks_unique(&self) -> impl Iterator<Item = &Track>;

    /// Returns whether a track appears in the object.
    fn contains(&self, track: &Track) -> bool;

    /// Returns a vector of indices at which the given track occurs.
    /// The indices are sorted in ascending order, i.e. the order in which they appear in the
    /// object.
    fn track_positions(&self, track: &Track) -> Option<&Vec<usize>>;

    /// Overwrites the text file to reflect the current object state.
    fn write(&self) -> Result<()>;

    /// Removes a track from the object, by index.
    fn remove_at(&mut self, index: usize);

    /// Removes all (if any) occurrences of a track from the object.
    fn remove_all(&mut self, track: &Track);

    /// Modify the path of a subset of tracks at the same time.
    ///
    /// Ensures safe handling of tricky scenarios like renaming A to B and B to A, or renaming A to
    /// B and then B to C, which in a naive implementation might cause A to end up as C.
    fn repath(&mut self, edits: &HashMap<Track, Utf8PathBuf>) -> Result<()>;
}

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
    /// Creates a new object from existing file contents.
    fn open<T: AsRef<Utf8Path>>(fpath: T) -> Result<Self> where Self: Sized;

    /// Creates a new empty object tied to a given path. This is the same as `open()`, except
    /// no reading or initialization from an external file takes place. `fpath` is only given
    /// for a potential future call to `write()`. Be careful not to overwrite an existing file!
    fn new<T: AsRef<Utf8Path>>(fpath: T) -> Result<Self> where Self: Sized;

    /// Returns the length of the number of tracks inside the object.
    fn len(&self) -> usize;

    /// Returns whether the object is empty (contains no tracks).
    fn is_empty(&self) -> bool;

    /// Works like `open()` if the file exists, and like `new()` if it doesn't.
    fn open_or_new<T: AsRef<Utf8Path>>(fpath: T) -> Result<Self> where Self: Sized;

    /// Reads file contents from disk and loads them, discarding what's currently in memory.
    /// In case of failures (parsing error, missing file, etc.), the memory contents remain
    /// in-tact.
    fn reload(&mut self) -> Result<()>;

    /// Returns an iterator over all objects.
    /// The objects are not all loaded into memory at once; they are created on-demand only.
    fn iter() -> Result<impl Iterator<Item = Self>> where Self: Sized;

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

    /// Returns whether the object has been modified since the last `write`.
    fn is_modified(&self) -> bool;

    /// Overwrites the text file to reflect the current object state.
    fn write(&mut self) -> Result<()>;

    /// Creates a new track from `fpath` and appends at the end of the object.
    fn push<T: AsRef<Utf8Path>>(&mut self, fpath: T) -> Result<()>;

    /// Removes the last occurrence of a track from the object.
    /// Returns whether or not `track` was found.
    fn remove_last(&mut self, track: &Track) -> bool;

    /// Removes a track from the object, by index.
    fn remove_at(&mut self, index: usize);

    /// Removes all (if any) occurrences of a track from the object.
    /// Returns the number of tracks removed.
    fn remove_all(&mut self, track: &Track) -> usize;

    /// Modify the path of a subset of tracks at the same time.
    ///
    /// Ensures safe handling of tricky scenarios like renaming A to B and B to A, or renaming A to
    /// B and then B to C, which in a naive implementation might cause A to end up as C.
    ///
    /// Every `Utf8PathBuf` in the `edits` hashmap must be a valid path to an audio file.
    ///
    /// Returns the number of changed tracks (duplicate paths are counted).
    fn bulk_rename(&mut self, edits: &HashMap<Track, Utf8PathBuf>) -> Result<usize>;
}

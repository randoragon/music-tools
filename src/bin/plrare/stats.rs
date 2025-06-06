use music_tools::{
    music_dir,
    playlist::*,
    playcount::*,
};
use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};
use log::{warn, error};
use std::collections::HashMap;
use colored::Colorize;

/// The minimum duration (in seconds) for an album to be considered an "album".
/// This prevents single-track albums which were played many times from appearing
/// in the top albums ranking.
const MIN_ALBUM_DURATION: f64 = 25.0 * 60.0;

// Types for readability
type ArtistName = String;
type TrackTitle = String;
type AlbumTitle = String;
type TrackRecord = (usize, f64);  // Number of plays and total duration
type TrackRecordTitle = (usize, f64, TrackTitle);
type TrackRecordArtistTitle = (usize, f64, ArtistName, TrackTitle);
type AlbumPath = Utf8PathBuf;
type TrackPath = Utf8PathBuf;

/// Returns a vector of **absolute** paths to each playcount file.
pub fn get_playcount_paths(playcounts: Vec<String>) -> Result<Vec<Utf8PathBuf>> {
    let mut n_months = i32::MIN;
    if playcounts.is_empty() {
        n_months = 1;
        println!("-- MONTHLY STATS --");
    }
    if playcounts.len() == 1 {
        if let Ok(val) = playcounts[0].parse::<i32>() {
            if val < 0 {
                return Err(anyhow!("The number of months cannot be negative"));
            }
            n_months = val;
            println!("-- {val}-MONTHLY STATS --");
        } else if playcounts[0] == "." {
            n_months = i32::MAX;
            println!("-- GLOBAL STATS --");
        }
    }

    if n_months != i32::MIN {
        let mut fpaths = Playcount::iter_paths()?
            .map(|x| Playcount::playcount_dir().join(x))
            .collect::<Vec<_>>();
        fpaths.sort_unstable();
        fpaths.reverse();
        if (n_months as usize) < fpaths.len() {
            fpaths.resize(n_months as usize, Utf8PathBuf::default());
        }
        Ok(fpaths)
    } else {
        // Interpret each `playcounts` element as a file path
        if let Ok(pwd) = std::env::current_dir() {
            Ok(playcounts.into_iter()
                .map(Utf8PathBuf::from)
                .map(|x| if x.is_absolute() { x } else { Utf8PathBuf::from(pwd.to_str().unwrap()).join(x) })
                .collect())
        } else {
            Err(anyhow!("Failed to read current directory"))
        }
    }
}

#[allow(clippy::map_entry)]
pub fn print_summary<'a>(fpaths: impl Iterator<Item = &'a Utf8PathBuf>, n_artists: usize, n_albums: usize, n_tracks: usize, reverse: bool) -> Result<()> {
    // Change directory to music_dir to make path validation easier
    if let Err(e) = std::env::set_current_dir(music_dir()) {
        return Err(anyhow!("Failed to change directory to {}: {}", music_dir(), e));
    }

    let mut n_seconds = 0.0f64;
    let mut n_plays = 0usize;

    let mut artists = HashMap::<ArtistName, TrackRecord>::new();
    let mut albums = HashMap::<AlbumPath, (ArtistName, AlbumTitle, HashMap<TrackPath, TrackRecordTitle>)>::new();
    let mut tracks = HashMap::<TrackPath, TrackRecordArtistTitle>::new();
    let mut fnames = Vec::<String>::new();

    // Tally up the stats
    for fpath in fpaths {
        let playcount = match Playcount::open(fpath) {
            Ok(playcount) => {
                fnames.push(String::from(fpath.file_name().unwrap_or(fpath.as_str())));
                playcount
            },
            Err(e) => {
                error!("Failed to open '{}': {}, skipping", fpath, e);
                continue;
            }
        };
        for entry in playcount.entries() {
            let dur = entry.duration.as_secs_f64();
            n_seconds += dur;
            n_plays += 1;
            if !artists.contains_key(&entry.artist) {
                artists.insert(entry.artist.to_owned(), (1, dur));
            } else {
                let rec = artists.get_mut(&entry.artist).unwrap();
                rec.0 += 1;
                rec.1 += dur;
            }
            if let Some(album) = &entry.album {
                if !albums.contains_key(entry.album_path()) {
                    let artist = if entry.album_artist.is_some() {
                        entry.album_artist.clone().unwrap()
                    } else {
                        warn!("Missing album_artist field in {}, using artist as fallback", entry.track.path);
                        entry.artist.to_owned()
                    };
                    albums.insert(entry.album_path().to_owned(), (
                        artist,
                        album.to_owned(),
                        HashMap::from([(entry.track.path.to_owned(), (1, dur, entry.title.to_owned()))]),
                    ));
                } else {
                    let album_tracks = &mut albums.get_mut(entry.album_path()).unwrap().2;
                    if !album_tracks.contains_key(&entry.track.path) {
                        album_tracks.insert(entry.track.path.to_owned(), (1, dur, entry.title.to_owned()));
                    } else {
                        let rec = album_tracks.get_mut(&entry.track.path).unwrap();
                        rec.0 += 1;
                        rec.1 += dur;
                    }
                }
            }
            {
                if !tracks.contains_key(&entry.track.path) {
                    tracks.insert(entry.track.path.to_owned(), (1, dur, entry.artist.to_owned(), entry.title.to_owned()));
                } else {
                    let tuple = tracks.get_mut(&entry.track.path).unwrap();
                    tuple.0 += 1;
                    tuple.1 += dur;
                }
            }
        }
    }

    if tracks.is_empty() {
        println!("No playcount data found.");
        return Ok(());
    }

    print_summary_general(&fnames, n_plays, n_seconds);
    if n_artists != 0 {
        println!();
        print_summary_artists(n_artists, n_plays, n_seconds, &artists, reverse);
    }
    if n_albums != 0 {
        println!();
        floor_album_listens_to_at_least_half(&mut albums);
        print_summary_albums(n_albums, n_plays, n_seconds, &albums, reverse);
    }
    if n_tracks != 0 {
        println!();
        print_summary_tracks(n_tracks, n_plays, n_seconds, &tracks, reverse);
    }

    Ok(())
}

pub fn print_summary_general(fnames: &[String], n_plays: usize, n_seconds: f64) {
    let days = (n_seconds as usize) / 86400;
    let hrs = ((n_seconds as usize) % 86400) / 3600;
    let mins = ((n_seconds as usize) % 3600) / 60;
    let secs = (n_seconds % 60.0).round() as usize;
    println!("Inputs ({}): {}\n", fnames.len(),
        fnames.iter().map(|x| x.underline().to_string()).collect::<Vec<String>>().join(", "));
    println!("Total listen time:   {}",
        format!("{days}d, {hrs}h, {mins}m, {secs}s").bright_yellow());
    println!("Total no. plays:     {}", format!("{n_plays}").bright_yellow());
    println!("Avg track duration:  {}",
        format!("{:02}:{:02}",
            ((n_seconds as usize) / n_plays) / 60,
            ((n_seconds as usize) / n_plays) % 60).bright_yellow()
    );
}

fn print_summary_artists(n_top: usize, n_plays: usize, n_seconds: f64, artists: &HashMap<ArtistName, TrackRecord>, reverse: bool) {
    println!("No. artists:       {}", format!("{}", artists.len()).bright_yellow());
    let mut artists_order = artists.keys().collect::<Vec<_>>();
    artists_order.sort_unstable_by_key(|&k| -artists[k].1 as i32);
    if reverse {
        artists_order.reverse();
    }
    let top_plays = artists_order.iter()
        .take(n_top)
        .map(|&x| artists[x].0)
        .sum::<usize>();
    let top_coverage = artists_order.iter()
        .take(n_top)
        .map(|&x| artists[x].1)
        .sum::<f64>();
    println!("Top {} {} listened artists ({} of plays, {} of listen time):",
        n_top,
        if !reverse { "most" } else { "least" },
        format!("{:.2}%", (top_plays as f64) / (n_plays as f64) * 100.0).purple(),
        format!("{:.2}%", top_coverage / n_seconds * 100.0).purple());
    for artist in artists_order.into_iter().take(n_top) {
        let duration = artists[artist].1 as usize;
        println!("{}{}{}  {}",
            format!("{:>4}:{:02}:{:02}",
                format!("{:>02}", duration / 3600),
                (duration % 3600) / 60,
                duration % 60
            ).blue(),
            "│".dimmed(),
            format!("{:<5}", artists[artist].0).cyan(),
            artist);
    }
}

/// Round down each album to the number of times AT LEAST HALF of all tracks on it
/// were listened to. This mechanism aims to prevent albums with popular singles
/// from appearing higher in the ranking.
fn floor_album_listens_to_at_least_half(albums: &mut HashMap<AlbumPath, (ArtistName, AlbumTitle, HashMap<TrackPath, TrackRecordTitle>)>) {
    // Filter out tracks whose files no longer exist (see explanation below)
    for (_, (_, _, tracks)) in albums.iter_mut() {
        tracks.retain(|k, _| k.exists());
    }

    // Initialize `new_albums` with every track count set to 0
    let mut new_albums = albums.clone();
    for tracks in new_albums.values_mut() {
        for (n_plays, duration, _) in tracks.2.values_mut() {
            *n_plays = 0;
            *duration = 0.0;
        }
    }

    // Transfer counts from `albums` to `new_albums` until no at-least-halves are left
    for (album_path, album_info) in albums.iter_mut() {
        // Compute the number of tracks on the album
        // EXPLANATION
        // This problem is tricky and does not have a clear perfect solution without trade-offs.
        // Playcounts are retained in history, but the files on the disk can get added or deleted,
        // which means the first listen through the full album can include 10 tracks, then 3 tracks
        // get deleted, and suddenly the next 5 listens are on 7 tracks. Then a track gets added
        // back in and the next 2 listens are on 8 tracks. You get the point. There is no way of
        // determining what the "true" number of tracks on an album is.
        //
        // After thinking about it a lot, I decided the following things:
        // 1) Assume the album does not change, or changes very rarely.
        // 2) Consider the full album length to be the number of files present on the disk.
        // 3) Ignore album tracks which are present only in the playcount, i.e. they were deleted.
        //
        // Rationale: For an album listen to count, at least half of its tracks must be played.
        // What happens commonly is that I will import a full album, listen through it, delete some
        // songs I don't like. With the above assumptions, deleting anything off the album does not
        // change the fact that it was listened through once. Moreover, any subsequent listens to
        // this trimmed-down album will be counted as full listens as well, because the deleted
        // tracks are no longer considered as part of the full album length.
        // The only trouble could arise if a large number of deleted tracks was brought back.
        // For example, if I did 10 listens of a 5-track album, and suddenly the album becomes
        // 11-track-long, all 5 of those listens will not count as album listens anymore. However,
        // these kinds of situations are rather extreme and should be very rare. It is more likely
        // that only a few tracks would be brought back, which would not affect the previous album
        // listens from losing their relevance.
        let tracks = &mut album_info.2;
        if tracks.is_empty() {
            continue;
        }
        let album_n_tracks = match get_album_n_tracks(album_path) {
            Ok(n) => n,
            Err(e) => {
                error!("{} (skipping, results may be inaccurate)", e);
                continue;
            },
        };

        // Convert TOTAL duration for each track on the album to AVERAGE duration.
        // For simplicity it will be assumed that each play of each track is worth that average
        // duration. This simplification is made because accounting for individual durations of
        // each play of each track on the album amidst the already complicated counting logic would
        // be hell. And the error, if any, should be negligible.
        for (n_plays, duration, _) in tracks.values_mut() {
            *duration /= *n_plays as f64;
        }

        // This loop will in each iteration create a batch of as many tracks from `album` as
        // possible, but only 1 listen per track. If the batch is equal to or exceeds the total
        // number of tracks on the album (`album_n_tracks`), then it will be transferred from
        // `albums` to `new_albums`. Otherwise, the loop will end.
        // Consequently, the number of times this loop runs will be equal to the number of times
        // the album was counted as played (minus 1, because the final loop doesn't count).
        loop {
            let mut batch = HashMap::<&Utf8Path, f64>::new();

            // Populate the batch
            for (track_path, trt) in tracks.iter_mut() {
                debug_assert!(!batch.contains_key(track_path.as_path()));
                if trt.0 > 0 {
                    batch.insert(track_path, trt.1);
                    trt.0 -= 1;
                }
            }

            if batch.len() >= album_n_tracks.div_ceil(2) {
                for (title, duration) in batch {
                    let trt = new_albums.get_mut(album_path).unwrap().2.get_mut(title).unwrap();
                    trt.0 += 1;
                    trt.1 += duration;
                }
            } else {
                break;
            }
        }
    }

    *albums = new_albums;
}

/// Computes the number of tracks on an album by listing directory files.
fn get_album_n_tracks(album_path: &Utf8Path) -> Result<usize> {
    match std::fs::read_dir(album_path) {
        Ok(dir) => {
            Ok(dir.filter(|x|
                x.as_ref().is_ok_and(|y|
                    y.file_name().to_str().unwrap().ends_with(".mp3") && y.path().is_file()
                ))
                .count())
        },
        Err(e) => Err(anyhow!("Failed to list directory '{}': {}", album_path, e)),
    }
}

fn print_summary_albums(n_top: usize, n_plays: usize, n_seconds: f64, albums: &HashMap<AlbumPath, (ArtistName, AlbumTitle, HashMap<TrackPath, TrackRecordTitle>)>, reverse: bool) {
    /// Estimates how many times the entire album was played
    fn album_estimate_n_plays(album_path: &Utf8PathBuf, album: &HashMap<TrackPath, TrackRecordTitle>) -> f64 {
        let n_plays = album.values().map(|x| x.0).sum::<usize>() as f64;
        match get_album_n_tracks(album_path) {
            Ok(n) => n_plays / (n as f64),
            Err(e) => {
                error!("{} (skipping, results may be inaccurate)", e);
                0.0
            },
        }
    }
    let mut albums_order = albums.keys()
        .filter(|&k| albums[k].2.values().filter(|x| x.0 != 0).map(|x| x.1 / (x.0 as f64)).sum::<f64>() >= MIN_ALBUM_DURATION)
        .collect::<Vec<_>>();
    println!("No. albums:       {}", format!("{}", albums_order.len()).bright_yellow());
    albums_order.sort_unstable_by_key(|&k| -albums[k].2.values().map(|x| x.1).sum::<f64>() as i32);
    albums_order.sort_by_key(|&k| -(album_estimate_n_plays(k, &albums[k].2) * 1e3) as i32);
    if reverse {
        albums_order.reverse();
    }
    let top_plays = albums_order.iter()
        .take(n_top)
        .map(|&x| albums[x].2.values().map(|y| y.0).sum::<usize>())
        .sum::<usize>();
    let top_coverage = albums_order.iter()
        .take(n_top)
        .map(|&x| albums[x].2.values().map(|y| y.1).sum::<f64>())
        .sum::<f64>();
    println!("Top {} {} listened albums ({} of plays, {} of listen time):",
        n_top,
        if !reverse { "most" } else { "least" },
        format!("{:.2}%", (top_plays as f64) / (n_plays as f64) * 100.0).purple(),
        format!("{:.2}%", top_coverage / n_seconds * 100.0).purple());
    for k in albums_order.into_iter().take(n_top) {
        let duration = albums[k].2.values().map(|x| x.1).sum::<f64>() as usize;
        println!("  {}{}{}  {}  {}",
            format!("{:02}:{:02}:{:02}",
                duration / 3600,
                (duration % 3600) / 60,
                duration % 60
            ).blue(),
            "│".dimmed(),
            format!("{:<5.1}", album_estimate_n_plays(k, &albums[k].2)).cyan(),
            albums[k].1, albums[k].0.dimmed());
    }
}

fn print_summary_tracks(n_top: usize, n_plays: usize, n_seconds: f64, tracks: &HashMap<TrackPath, TrackRecordArtistTitle>, reverse: bool) {
    println!("No. tracks:       {}", format!("{}", tracks.len()).bright_yellow());
    let mut tracks_order = tracks.keys().collect::<Vec<_>>();
    tracks_order.sort_unstable_by_key(|&k| -(tracks[k].1 as i32));
    tracks_order.sort_by_key(|&k| -(tracks[k].0 as i32));
    if reverse {
        tracks_order.reverse();
    }
    let top_plays = tracks_order.iter()
        .take(n_top)
        .map(|&x| tracks[x].0)
        .sum::<usize>();
    let top_coverage = tracks_order.iter()
        .take(n_top)
        .map(|&x| tracks[x].1)
        .sum::<f64>();
    println!("Top {} {} replayed tracks ({} of plays, {} of listen time):",
        n_top,
        if !reverse { "most" } else { "least" },
        format!("{:.2}%", (top_plays as f64) / (n_plays as f64) * 100.0).purple(),
        format!("{:.2}%", top_coverage / n_seconds * 100.0).purple());
    for track in tracks_order.into_iter().take(n_top) {
        let duration = tracks[track].1 as usize;
        println!("  {}{}{}  {}  {}",
            format!("{:02}:{:02}:{:02}",
                duration / 3600,
                (duration % 3600) / 60,
                duration % 60
            ).blue(),
            "│".dimmed(),
            format!("{:<5}", tracks[track].0).cyan(),
            tracks[track].3, tracks[track].2.dimmed());
    }
}

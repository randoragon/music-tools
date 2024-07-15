use music_tools::{
    library_size,
    playlist::*,
    playcount::*,
};
use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use log::error;
use std::collections::HashMap;
use colored::Colorize;

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
        let mut fpaths = Playcount::iter_paths()?.collect::<Vec<_>>();
        fpaths.sort_unstable();
        fpaths.reverse();
        if (n_months as usize) < fpaths.len() {
            fpaths.resize(n_months as usize, Utf8PathBuf::default());
        }
        Ok(fpaths)
    } else {
        // Interpret each `playcounts` element as a file path
        Ok(playcounts.into_iter()
            .map(Utf8PathBuf::from)
            .collect())
    }
}

pub fn print_summary<'a>(fpaths: impl Iterator<Item = &'a Utf8PathBuf>, n_artists: usize, n_albums: usize, n_tracks: usize, reverse: bool) -> Result<()> {
    let mut n_seconds = 0.0f64;
    let mut n_plays = 0usize;

    // Types for readability
    type ArtistName = String;
    type AlbumArtistName = String;
    type TrackTitle = String;
    type AlbumTitle = String;
    type TrackRecord = (usize, f64);  // Number of plays and total duration
    type AlbumKey = (AlbumArtistName, AlbumTitle);
    type TrackKey = (ArtistName, TrackTitle);

    let mut artists = HashMap::<ArtistName, TrackRecord>::new();
    let mut albums = HashMap::<AlbumKey, HashMap<TrackTitle, TrackRecord>>::new();
    let mut tracks = HashMap::<TrackKey, TrackRecord>::new();
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
                let album_artist = if entry.album_artist.is_some() {
                    entry.album_artist.clone().unwrap()
                } else {
                    entry.artist.clone()
                };
                let key = (album_artist, album.to_owned());
                if !albums.contains_key(&key) {
                    albums.insert(key, HashMap::from([(entry.title.to_owned(), (1, dur))]));
                } else {
                    let album_tracks = albums.get_mut(&key).unwrap();
                    if !album_tracks.contains_key(&entry.title) {
                        album_tracks.insert(entry.title.to_owned(), (1, dur));
                    } else {
                        let rec = album_tracks.get_mut(&entry.title).unwrap();
                        rec.0 += 1;
                        rec.1 += dur;
                    }
                }
            }
            {
                let key = (entry.artist.to_owned(), entry.title.to_owned());
                if !tracks.contains_key(&key) {
                    tracks.insert(key, (1, dur));
                } else {
                    let tuple = tracks.get_mut(&key).unwrap();
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

    print_summary_general(&fnames, n_plays, n_seconds, &tracks);
    if n_artists != 0 {
        println!();
        print_summary_artists(n_artists, n_plays, n_seconds, &artists, reverse);
    }
    if n_albums != 0 {
        println!();
        print_summary_albums(n_albums, n_plays, n_seconds, &albums, reverse);
    }
    if n_tracks != 0 {
        println!();
        print_summary_tracks(n_tracks, n_plays, n_seconds, &tracks, reverse);
    }

    Ok(())
}

pub fn print_summary_general(fnames: &Vec<String>, n_plays: usize, n_seconds: f64, tracks: &HashMap<(String, String), (usize, f64)>) {
    let days = (n_seconds as usize) / 86400;
    let hrs = ((n_seconds as usize) % 86400) / 3600;
    let mins = ((n_seconds as usize) % 3600) / 60;
    let secs = (n_seconds % 60.0).round() as usize;
    println!("Inputs ({}): {}\n", fnames.len(), fnames.join(", "));
    println!("Total listen time:   {days}d, {hrs}h, {mins}m, {secs}s");
    println!("Total no. plays:     {n_plays}");
    println!("Library coverage:    {:.2}% of all tracks",
        (tracks.len() as f64) / (library_size() as f64) * 100.0
    );
    println!("Avg track duration:  {:02}:{:02}",
        ((n_seconds as usize) / tracks.len()) / 60,
        ((n_seconds as usize) / tracks.len()) % 60,
    );
}

fn print_summary_artists(n_top: usize, n_plays: usize, n_seconds: f64, artists: &HashMap<String, (usize, f64)>, reverse: bool) {
    println!("No. artists:       {}", artists.len());
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
    println!("Top {} {} listened artists ({:.2}% of plays, {:.2}% of listen time):",
        n_top,
        if !reverse { "most" } else { "least" },
        (top_plays as f64) / (n_plays as f64) * 100.0,
        top_coverage / n_seconds * 100.0);
    for artist in artists_order.into_iter().take(n_top) {
        let duration = artists[artist].1 as usize;
        println!("  {:02}:{:02}:{:02}│{}\t{}",
            duration / 3600,
            (duration % 3600) / 60,
            duration % 60,
            artists[artist].0,
            artist);
    }
}

fn print_summary_albums(n_top: usize, n_plays: usize, n_seconds: f64, albums: &HashMap<(String, String), HashMap<String, (usize, f64)>>, reverse: bool) {
    println!("No. albums:       {}", albums.len());
    let mut albums_order = albums.keys().collect::<Vec<_>>();
    albums_order.sort_unstable_by_key(|&k| -albums[k].values().map(|x| x.1).sum::<f64>() as i32);
    if reverse {
        albums_order.reverse();
    }
    let top_plays = albums_order.iter()
        .take(n_top)
        .map(|&x| albums[x].values().map(|y| y.0).sum::<usize>())
        .sum::<usize>();
    let top_coverage = albums_order.iter()
        .take(n_top)
        .map(|&x| albums[x].values().map(|y| y.1).sum::<f64>())
        .sum::<f64>();
    println!("Top {} {} listened albums ({:.2}% of plays, {:.2}% of listen time):",
        n_top,
        if !reverse { "most" } else { "least" },
        (top_plays as f64) / (n_plays as f64) * 100.0,
        top_coverage / n_seconds * 100.0);
    for album in albums_order.into_iter().take(n_top) {
        let duration = albums[album].values().map(|x| x.1).sum::<f64>() as usize;
        println!("  {:02}:{:02}:{:02}│{}\t{}",
            duration / 3600,
            (duration % 3600) / 60,
            duration % 60,
            '?',
            format!("{}  {}", album.1, album.0.dimmed()));
    }
}

fn print_summary_tracks(n_top: usize, n_plays: usize, n_seconds: f64, tracks: &HashMap<(String, String), (usize, f64)>, reverse: bool) {
    println!("No. tracks:       {}", tracks.len());
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
    println!("Top {} {} replayed tracks ({:.2}% of plays, {:.2}% of listen time):",
        n_top,
        if !reverse { "most" } else { "least" },
        (top_plays as f64) / (n_plays as f64) * 100.0,
        top_coverage / n_seconds * 100.0);
    for track in tracks_order.into_iter().take(n_top) {
        let duration = tracks[track].1 as usize;
        println!("  {:02}:{:02}:{:02}│{}\t{}",
            duration / 3600,
            (duration % 3600) / 60,
            duration % 60,
            tracks[track].0,
            format!("{}  {}", track.1, track.0.dimmed()));
    }
}

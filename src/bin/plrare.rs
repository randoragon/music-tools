use music_tools::{
    music_dir,
    playlist::*,
    playcount::*,
    track::*,
};
use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use log::{warn, error};
use std::fs::File;
use std::io::{BufReader, BufRead};
use std::process::ExitCode;
use std::collections::HashMap;
use colored::Colorize;

const MPD_SOCKET: &str = "127.0.0.1:6601";

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Bump the count for one or more tracks
    Bump {
        /// Path to a file, playlist or "^" to target the current MPD queue
        item: String,
        /// How many times to append <ITEM>. Can be negative for removal. Default 1.
        n: Option<i32>
    },
    /// Print a listening report.
    Stats {
        /// A number of past months, or a list of paths to playcount files. Default 1.
        playcounts: Vec<String>,

        #[arg(short, long, help = "List this many most listened artists")]
        artists: Option<usize>,

        #[arg(short = 'b', long, help = "List this many most listened albums")]
        albums: Option<usize>,

        #[arg(short, long, help = "List this many most listened tracks")]
        tracks: Option<usize>,
    },
}

fn get_bump_fpaths_from_item(item: &str) -> Result<Vec<Utf8PathBuf>> {
    match item {
        // Bump the current contents of the MPD queue
        "^" => {
            let mut conn = match mpd::Client::connect(MPD_SOCKET) {
                Ok(conn) => conn,
                Err(e) => {
                    println!("Could not connect to MPD: {}", e);
                    return Ok(vec![]);
                },
            };

            let queue = match conn.queue() {
                Ok(queue) => queue,
                Err(e) => {
                    warn!("Connection to MPD established, but could not fetch the queue: {}", e);
                    return Ok(vec![]);
                },
            };

            Ok(queue.iter()
                .map(|song| [music_dir().as_str(), song.file.as_str()].iter().collect())
                .collect())
        },

        // Bump a playlist
        x if x.ends_with(".m3u") => {
            let playlist = match File::open(item) {
                Ok(file) => file,
                Err(e) => return Err(anyhow!("Failed to open playlist '{}': {}", item, e)),
            };

            Ok(BufReader::new(playlist)
                .lines()
                .flatten()
                .map(|x| [music_dir().as_str(), x.as_str()].iter().collect())
                .collect())
        },

        // Bump a track
        _ => {
            Ok(vec![Utf8PathBuf::from(item)])
        }
    }
}

fn get_stats_entries(playcounts: Vec<String>) -> Result<Vec<Entry>> {
    let mut entries = vec![];

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
        for fpath in fpaths.iter().rev().take(n_months as usize) {
            let playcount = Playcount::open(fpath)?;
            entries.extend(playcount.entries().cloned());
        }
    } else {
        // Interpret each `playcounts` element as a file path
        for fpath in &playcounts {
            let playcount = Playcount::open(fpath)?;
            entries.extend(playcount.entries().cloned());
        }
    }

    Ok(entries)
}

fn music_library_size() -> usize {
    if let Ok(mut conn) = mpd::Client::connect(MPD_SOCKET) {
        if let Ok(list) = conn.listall() {
            return list.len();
        }
    }

    // Fallback if MPD listing fails
    walkdir::WalkDir::new(music_dir())
        .follow_links(false)
        .into_iter()
        .filter_map(|x| x.ok())
        .filter(|x| x.file_name().to_string_lossy().ends_with(".mp3"))
        .count()
}

fn print_stats_summary<'a>(entries: impl Iterator<Item = &'a Entry>, n_artists: usize, n_albums: usize, n_tracks: usize) -> Result<()> {
    let mut n_seconds = 0.0f64;
    let mut n_plays = 0usize;
    let mut artists = HashMap::<String, f64>::new();
    let mut albums = HashMap::<(String, String), f64>::new(); // key: (artist, album title)
    let mut tracks = HashMap::<(String, String), f64>::new(); // key: (artist, track title)

    // Tally up the stats
    for entry in entries {
        let dur = entry.duration.as_secs_f64();
        n_seconds += dur;
        n_plays += 1;
        if !artists.contains_key(&entry.artist) {
            artists.insert(entry.artist.to_owned(), dur);
        } else {
            *artists.get_mut(&entry.artist).unwrap() += dur;
        }
        if let Some(album) = &entry.album {
            let key = (entry.artist.to_owned(), album.to_owned());
            if !albums.contains_key(&key) {
                albums.insert(key, dur);
            } else {
                *albums.get_mut(&key).unwrap() += dur;
            }
        }
        {
            let key = (entry.artist.to_owned(), entry.title.to_owned());
            if !tracks.contains_key(&key) {
                tracks.insert(key, dur);
            } else {
                *tracks.get_mut(&key).unwrap() += dur;
            }
        }
    }

    // Print general summary
    if tracks.is_empty() {
        println!("No playcount data found.");
        return Ok(());
    }
    let days = (n_seconds as usize) / 86400;
    let hrs = ((n_seconds as usize) % 86400) / 3600;
    let mins = ((n_seconds as usize) % 3600) / 60;
    let secs = (n_seconds % 60.0).round() as usize;
    println!("Total listen time:   {days}d, {hrs}h, {mins}m, {secs}s");
    println!("Total no. plays:     {n_plays}");
    println!("No. tracks:          {} ({:.2}% of plays, {:.2}% of all)",
        tracks.len(),
        (tracks.len() as f64) / (n_plays as f64) * 100.0,
        (tracks.len() as f64) / (music_library_size() as f64) * 100.0
    );
    println!("Avg track duration:  {:02}:{:02}",
        ((n_seconds as usize) / tracks.len()) / 60,
        ((n_seconds as usize) / tracks.len()) % 60,
    );

    // Print artists summary
    if n_artists != 0 {
        println!("\nNo. artists:       {}", artists.len());
        let mut artists_order = artists.keys().collect::<Vec<_>>();
        artists_order.sort_by_key(|&k| artists[k] as usize);
        let top_coverage = artists_order.iter().rev()
            .take(n_artists)
            .map(|&x| artists[x])
            .sum::<f64>();
        println!("Top {} listened artists ({:.2}% of all listen time):",
            n_artists, top_coverage / n_seconds * 100.0);
        for artist in artists_order.into_iter().rev().take(n_artists) {
            let duration = artists[artist] as usize;
            println!("  {:02}:{:02}:{:02}\t{}",
                duration / 3600,
                (duration % 3600) / 60,
                duration % 60,
                artist);
        }
    }

    // Print albums summary
    if n_albums != 0 {
        println!("\nNo. albums:       {}", albums.len());
        let mut albums_order = albums.keys().collect::<Vec<_>>();
        albums_order.sort_by_key(|&k| albums[k] as usize);
        let top_coverage = albums_order.iter().rev()
            .take(n_albums)
            .map(|&x| albums[x])
            .sum::<f64>();
        println!("Top {} listened albums ({:.2}% of all listen time):",
            n_albums, top_coverage / n_seconds * 100.0);
        for album in albums_order.into_iter().rev().take(n_albums) {
            let duration = albums[album] as usize;
            println!("  {:02}:{:02}:{:02}\t{}",
                duration / 3600,
                (duration % 3600) / 60,
                duration % 60,
                format!("{}  {}", album.1, album.0.dimmed()));
        }
    }

    // Print tracks summary
    if n_tracks != 0 {
        println!("\nNo. tracks:       {}", tracks.len());
        let mut tracks_order = tracks.keys().collect::<Vec<_>>();
        tracks_order.sort_by_key(|&k| tracks[k] as usize);
        let top_coverage = tracks_order.iter().rev()
            .take(n_tracks)
            .map(|&x| tracks[x])
            .sum::<f64>();
        println!("Top {} listened tracks ({:.2}% of all listen time):",
            n_tracks, top_coverage / n_seconds * 100.0);
        for track in tracks_order.into_iter().rev().take(n_tracks) {
            let duration = tracks[track] as usize;
            println!("  {:02}:{:02}:{:02}\t{}",
                duration / 3600,
                (duration % 3600) / 60,
                duration % 60,
                format!("{}  {}", track.1, track.0.dimmed()));
        }
    }

    Ok(())
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    stderrlog::new()
        .module(module_path!())
        .module("music_tools")
        .verbosity(2)
        .init()
        .unwrap();

    // Create the current playcount file, if it does not exist
    match Playcount::current() {
        Ok(mut playcount) => {
            if !playcount.path().exists() {
                if let Err(e) = playcount.write() {
                    warn!("Failed to create the current playcount file: {}", e);
                }
            }
        },
        Err(e) => {
            warn!("Failed to open the current playcount file: {}", e);
        },
    }

    match cli.command {
        Commands::Bump { item, n } => {
            // Open the playcount file
            let mut playcount = match Playcount::current() {
                Ok(playcount) => playcount,
                Err(e) => {
                    error!("Failed to open the current playcount file: {}", e);
                    return ExitCode::FAILURE;
                },
            };

            // Parse item and get the list of paths to append/remove
            let fpaths = match get_bump_fpaths_from_item(&item) {
                Ok(fpaths) => fpaths,
                Err(e) => {
                    error!("Failed to infer paths to bump from '{}': {}", item, e);
                    return ExitCode::FAILURE;
                },
            };

            // Append/remove paths
            let n = n.unwrap_or(1);
            if n > 0 {
                for _ in 0..n {
                    for fpath in &fpaths {
                        if let Err(e) = playcount.push(fpath) {
                            error!("Failed to bump '{}': {}, skipping", fpath, e);
                        }
                    }
                }
            } else {
                for fpath in &fpaths {
                    let track = Track::new(fpath);
                    for _ in n..0 {
                        playcount.remove_last(&track);
                    }
                }
            }

            // Write the playcount file
            if playcount.is_modified() {
                if let Err(e) = playcount.write() {
                    error!("Failed to write to '{}': {}", playcount.path(), e);
                    return ExitCode::FAILURE;
                }
            }
        }

        Commands::Stats { playcounts, artists, albums, tracks } => {
            let entries = match get_stats_entries(playcounts) {
                Ok(entries) => entries,
                Err(e) => {
                    error!("Failed to obtain a list of entries: {}", e);
                    return ExitCode::FAILURE;
                }
            };
            if let Err(e) = if artists.is_none() && albums.is_none() && tracks.is_none() {
                print_stats_summary(entries.iter(), 10, 10, 10)
            } else {
                print_stats_summary(entries.iter(), artists.unwrap_or(0), albums.unwrap_or(0), tracks.unwrap_or(0))
            } {
                error!("{}", e);
                return ExitCode::FAILURE;
            }
        }
    }

    ExitCode::SUCCESS
}

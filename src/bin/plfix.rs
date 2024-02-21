use clap::Parser;
use log::error;
use music_tools::playlist::*;
use music_tools::playcount::*;
use std::process::ExitCode;

#[derive(Parser)]
struct Cli {
    #[arg(short, long, help = "Show what would be fixed, but do not apply any changes")]
    pretend: bool,
}

/// Returns the total number of duplicate entries found.
fn remove_playlist_duplicates(playlists: impl Iterator<Item = Playlist>, pretend: bool) -> usize {
    let mut n_duplicates = 0usize;
    for mut playlist in playlists {
        // Duplicates are allowed in history playlists
        if playlist.name().starts_with("hist.") {
            continue;
        }

        // Build a list of all indices to remove
        let mut indices = Vec::new();
        for track in playlist.tracks_unique() {
            if let Some(pos) = playlist.track_positions(track) {
                if pos.len() > 1 {
                    indices.extend_from_slice(&pos[1..]);
                    n_duplicates += pos.len() - 1;
                }
            }
        }

        // Remove the indices
        if !pretend && !indices.is_empty() {
            indices.sort_unstable();
            indices.into_iter().rev().for_each(|x| playlist.remove_at(x));
            if let Err(e) = playlist.write() {
                error!("Failed to write to '{}': {}", playlist.path(), e);
            }
        }
    }
    n_duplicates
}

/// Returns the total number of duplicate entries merged.
fn merge_playcount_duplicates(playcounts: impl Iterator<Item = Playcount>, pretend: bool) -> usize {
    let mut n_dupes_total = 0usize;
    for mut playcount in playcounts {
        let n_dupes = playcount.merge_duplicates(pretend);
        if !pretend && n_dupes != 0 {
            if let Err(e) = playcount.write() {
                error!("Failed to write to '{}': {}", playcount.path(), e);
            }
        }
        n_dupes_total += n_dupes;
    }

    n_dupes_total
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    stderrlog::new()
        .module(module_path!())
        .module("music_tools")
        .verbosity(2)
        .init()
        .unwrap();

    println!("-- PLAYLISTS --");
    let playlists = match Playlist::iter() {
        Some(it) => it,
        None => return ExitCode::FAILURE,
    };
    match remove_playlist_duplicates(playlists, cli.pretend) {
        0 => println!("No duplicate paths found"),
        n => println!("{} {} duplicate paths",
            if cli.pretend { "Detected" } else { "Removed" },
            n),
    };

    println!("\n-- PLAYCOUNT --");
    let playcounts = match Playcount::iter() {
        Some(it) => it,
        None => return ExitCode::FAILURE,
    };
    match merge_playcount_duplicates(playcounts, cli.pretend) {
        0 => println!("No duplicate entries found"),
        n => println!("{} {} duplicate entries",
            if cli.pretend { "Detected" } else { "Merged" },
            n),
    };

    ExitCode::SUCCESS
}

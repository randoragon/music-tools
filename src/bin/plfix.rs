use clap::Parser;
use log::error;
use music_tools::playlist::*;
use std::process::ExitCode;

#[derive(Parser)]
struct Cli {
    #[arg(short, long, help = "Show what would be fixed, but do not apply any changes")]
    pretend: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    stderrlog::new()
        .module(module_path!())
        .module("music_tools")
        .verbosity(2)
        .init()
        .unwrap();

    // Read all playlists
    let mut playlists: Vec<Playlist> = match Playlist::iter() {
        Some(it) => it.collect(),
        None => return ExitCode::FAILURE,
    };

    // Remove duplicates
    println!("-- PLAYLISTS --");
    let mut n_duplicates = 0usize;
    for playlist in playlists.iter_mut() {
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

        if !cli.pretend && indices.len() != 0 {
            // Remove the indices
            indices.sort_unstable();
            indices.into_iter().rev().for_each(|x| playlist.remove_at(x));
            if let Err(e) = playlist.write() {
                error!("Failed to write to '{}': {}", playlist.path(), e);
            }
        }
    }
    match n_duplicates {
        0 => println!("No duplicates paths found"),
        _ => println!("{} {} duplicate paths",
            if cli.pretend { "Detected" } else { "Removed" },
            n_duplicates),
    };

    ExitCode::SUCCESS
}

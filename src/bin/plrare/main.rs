mod bump;
mod stats;
mod gen;
use music_tools::{
    playlist::*,
    playcount::*,
    track::*,
};
use clap::{Parser, Subcommand};
use log::error;
use std::process::ExitCode;

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

        /// List this many most listened artists.
        #[arg(short, long)]
        artists: Option<usize>,

        /// List this many most listened albums.
        #[arg(short = 'b', long)]
        albums: Option<usize>,

        /// List this many most listened tracks.
        #[arg(short, long)]
        tracks: Option<usize>,

        /// Print which music was played THE LEAST.
        #[arg(short, long)]
        reverse: bool,
    },

    /// Generate a playlist of the least listened to tracks.
    /// By default, any track can be included with a probability inversely proportional to its
    /// global playcount.
    Gen {
        /// A number of tracks or playlist duration ([HH:]MM:[SS]).
        content: String,

        /// List tracks played THE MOST.
        #[arg(short, long)]
        reverse: bool,

        /// Rank strictly based on playcount, no probabilities involved.
        #[arg(short, long)]
        strict: bool,
    }
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
    if !Playcount::current_path().exists() {
        match Playcount::current() {
            Ok(mut playcount) => {
                if let Err(e) = playcount.write() {
                    error!("Failed to create the current playcount file: {}", e);
                }
            },
            Err(e) => {
                error!("Failed to open the current playcount file: {}", e);
            },
        }
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
            let fpaths = match bump::get_fpaths_from_item(&item) {
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
        },

        Commands::Stats { playcounts, artists, albums, tracks, reverse } => {
            let fpaths = match stats::get_playcount_paths(playcounts) {
                Ok(fpaths) => fpaths,
                Err(e) => {
                    error!("Failed to obtain a list of entries: {}", e);
                    return ExitCode::FAILURE;
                }
            };
            if let Err(e) = if artists.is_none() && albums.is_none() && tracks.is_none() {
                stats::print_summary(fpaths.iter(), 10, 10, 10, reverse)
            } else {
                stats::print_summary(fpaths.iter(), artists.unwrap_or(0), albums.unwrap_or(0), tracks.unwrap_or(0), reverse)
            } {
                error!("{}", e);
                return ExitCode::FAILURE;
            }
        },

        Commands::Gen { content, reverse, strict } => {
            if let Err(e) = gen::generate(&content, reverse, strict) {
                error!("{}", e);
                return ExitCode::FAILURE;
            }
        },
    }

    ExitCode::SUCCESS
}

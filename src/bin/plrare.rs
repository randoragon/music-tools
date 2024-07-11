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

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
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
        #[arg(short, long, help = "A number of past months to include")]
        months: Option<u16>,

        #[arg(short, long, num_args = 1.., help = "Playcount files to include")]
        files: Option<Vec<String>>,
    },
}

fn get_bump_fpaths_from_item<'a>(item: &'a str) -> Result<Vec<Utf8PathBuf>> {
    match item {
        // Bump the current contents of the MPD queue
        "^" => {
            let mpd_host = std::env::var("MPD_HOST").unwrap_or("127.0.0.1".to_string());
            let mpd_port = std::env::var("MPD_PORT").unwrap_or("6600".to_string());
            let mut conn = match mpd::Client::connect(format!("{mpd_host}:{mpd_port}")) {
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

fn stats(months: Option<u16>, files: Option<Vec<String>>) -> Result<()> {
    todo!();
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    stderrlog::new()
        .module(module_path!())
        .module("music_tools")
        .verbosity(2)
        .init()
        .unwrap();

    println!("{:?}", cli);

    match cli.command {
        Commands::Bump { item: item, n } => {
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
                        if let Err(e) = playcount.push(&fpath) {
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
        Commands::Stats { months, files } => {
            if let Err(e) = stats(months, files) {
                error!("{e}");
            }
        }
    }

    ExitCode::SUCCESS
}

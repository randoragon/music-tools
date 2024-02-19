use music_tools::playlist::*;
use std::process::ExitCode;
use std::io;
use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[arg(short, long, help = "Show what would be fixed, but do not apply any changes")]
    pretend: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Read all playlists
    let mut playlists = Vec::<Playlist>::new();
    match Playlist::iter_paths() {
        Ok(paths) => {
            for path in paths {
                match Playlist::new(&path) {
                    Ok(playlist) => playlists.push(playlist),
                    Err(e) => eprintln!("Failed to read playlist '{:?}': {}, skipping", path, e),
                }
            }
        },
        Err(e) => {
            eprintln!("Failed to list the playlists directory '{:?}': {}", Playlist::dirname(), e);
            return ExitCode::FAILURE;
        }
    }

    println!("{:?}", playlists);
    ExitCode::SUCCESS
}

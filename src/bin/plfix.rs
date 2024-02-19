use music_tools::playlist::*;
use std::process::ExitCode;
use clap::Parser;

#[derive(Parser)]
struct Cli {
    #[arg(short, long, help = "Show what would be fixed, but do not apply any changes")]
    pretend: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    // Read all playlists
    let playlists: Vec<Playlist> = match Playlist::iter_playlists() {
        Some(it) => it.collect(),
        None => return ExitCode::FAILURE,
    };

    println!("{:#?}", playlists);
    ExitCode::SUCCESS
}

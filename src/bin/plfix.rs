use music_tools::{
    path_from,
    dirname as music_dir,
    track::*,
    playlist::*,
    playcount::*,
};
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;
use log::{warn, error};
use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::{BufRead, Write};
use std::mem::drop;
use std::process::{ExitCode, Command};

#[derive(Parser)]
struct Cli {
    #[arg(short, long, help = "Show what would be fixed, but do not apply any changes")]
    pretend: bool,
}

/// Removes duplicate tracks from playlists. Returns the number of removed tracks.
fn remove_playlist_duplicates(playlists: &mut Vec<Playlist>) -> usize {
    let mut n_duplicates = 0usize;
    for playlist in playlists {
        // Duplicates are allowed in history playlists
        if playlist.name().starts_with("hist.") {
            continue;
        }
        n_duplicates += playlist.remove_duplicates();
    }
    n_duplicates
}

/// Merges duplicate entries in playcounts. The indices of affected playcounts are added to a set.
/// Returnes the number of removed entries.
fn merge_playcount_duplicates(playcounts: &mut Vec<Playcount>) -> usize {
    let mut n_duplicates = 0usize;
    for playcount in playcounts {
        n_duplicates += playcount.merge_duplicates();
    }
    n_duplicates
}

/// Finds invalid tracks in a tracks file. Found tracks are inserted into `set`.
/// Invalid paths can be ignored with a custom `ignore` closure.
/// A summary of all found paths is written to a log file.
/// Returns the total number of invalid tracks found across all files.
fn find_invalid_tracks<T: TracksFile, F: Fn(&Track) -> bool>(
    files: &[T],
    set: &mut HashSet<Track>,
    ignore: F,
    log_file: &mut File,
) -> usize {
    let mut invalid_count = 0usize;
    for file in files {
        let mut printed_header = false;
        let it = file.tracks_unique().filter(|&x| !x.path.exists() && !ignore(x));
        for invalid_track in it {
            set.insert(invalid_track.clone());
            invalid_count += 1;

            // Write to log file
            if !printed_header {
                let header = file.path().strip_prefix(music_dir()).unwrap_or(file.path());
                if let Err(e) = writeln!(log_file, "{}", header) {
                    error!("Failed to append line to log file: {}", e);
                }
                printed_header = true;
            }
            if let Err(e) = writeln!(log_file, "\t{}", invalid_track.path) {
                error!("Failed to append line to log file: {}", e);
            }
        }
    }
    invalid_count
}

/// Interactively asks user what to do with each invalid path.
/// Returns a hashmap of new track paths, and a hashset of tracks to delete.
fn ask_resolve_invalid_paths(
    invalid_tracks: &HashSet<Track>,
    playlists: &[Playlist],
    playcounts: &[Playcount],
) -> Result<(HashMap<Track, Utf8PathBuf>, HashSet::<Track>)> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    // For remembering user decisions
    let mut edits = HashMap::<Track, Utf8PathBuf>::new();
    let mut deletes = HashSet::<Track>::new();

    for (i, track) in invalid_tracks.iter().enumerate() {
        // Aux buffer for storing user input - will automatically grow if needed
        let mut ans = String::with_capacity(64);

        // Print track information and menu
        println!("\n({}/{})  {:?}", i+1, invalid_tracks.len(), track.path);
        print!("Appears in: ");
        let mut appearances: Vec<String> = vec![];
        appearances.extend(playlists.iter()
            .filter(|&x| x.contains(track))
            .map(|x| x.name())
            .cloned());
        appearances.extend(playcounts.iter()
            .filter(|&x| x.contains(track))
            .map(|x| x.path().file_name().unwrap_or(x.path().as_str()).to_string()));
        println!("{}", appearances.join(", "));
        print!("[s]kip, [e]dit, [d]elete/ignore, [q]uit, a[b]ort  (default: skip): ");
        stdout.flush()?;

        // Let user choose action
        ans.clear();
        while ans.is_empty() {
            stdin.lock().read_line(&mut ans)?;
            match ans.trim_end() {
                "" | "s" | "e" | "d" | "q" | "b" => (),
                _ => {
                    print!("Please choose one of: s, e, d, q, b: ");
                    stdout.flush()?;
                    ans.clear();
                },
            };
        }

        // Execute action
        match ans.trim_end() {
            "s" | "" => println!("Skipping."),
            "e" => {
                print!("New path (leave empty to skip): {}/", music_dir());
                stdout.flush()?;
                ans.clear();
                let mut new_path: Option<Utf8PathBuf> = None;
                while ans.is_empty() {
                    stdin.lock().read_line(&mut ans)?;
                    let path = Utf8PathBuf::from(ans.trim_end());
                    if path.exists() && path.is_file() && path.is_relative() {
                        new_path = Some(path);
                    } else {
                        print!("Invalid path. Try again: {}/", music_dir());
                        stdout.flush()?;
                        ans.clear();
                    }
                }
                edits.insert(track.clone(), new_path.unwrap());
                println!("Path accepted.");
            },
            "d" => {
                deletes.insert(track.clone());
                println!("Marking for deletion in regular playlists, ignore in playcount/history");
            },
            "q" => {
                println!("Skipping all remaining tracks.");
                break;
            },
            "b" => {
                println!("Abort - discarding all changes.");
                edits.clear();
                deletes.clear();
                break;
            },
            _ => unreachable!(),
        }
    }
    assert!(edits.keys().all(|x| !deletes.contains(x)), "edits and deletes must be disjoint");
    Ok((edits, deletes))
}

/// Removes all given tracks from all given playlists. In the case of history playlists, tracks are
/// not removed, but instead appended to the external ignore playlist.
///
/// If `ignore_playlist` is `None`, only the non-history playlists will be considered.
fn remove_tracks_from_playlists(
    playlists: &mut [Playlist],
    tracks: &HashSet<Track>,
    ignore_playlist: &mut Playlist,
) {
    for playlist in playlists {
        // History playlists never get deleted from; instead, append outdated tracks
        // to the ignored meta-playlist.
        if playlist.name().starts_with("hist.") {
            for track in tracks.iter().filter(|&x| playlist.contains(x)) {
                ignore_playlist.push(track.clone());
            }
            continue;
        }

        // Delete normally from all other playlists
        for track in tracks {
            playlist.remove_all(track);
        }
    }
}

fn remove_tracks_from_playcounts(
    playcounts: &mut [Playcount],
    tracks: &HashSet<Track>,
    ignore_playlist: &mut Playlist,
) {
    for playcount in playcounts {
        for track in tracks.iter().filter(|&x| playcount.contains(x)) {
            ignore_playlist.push(track.clone());
        }
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

    if let Err(e) = std::env::set_current_dir(music_dir()) {
        println!("Failed to change directory to '{}': {}", music_dir(), e);
        return ExitCode::FAILURE;
    }
    let log_path = path_from(dirs::data_dir, "plfix-latest.log");

    // For storing invalid path information to be written to the log file.
    let mut invalid_tracks = HashSet::<Track>::new();

    // Open log file for writing invalid paths summary to
    let mut log_file = match File::create(&log_path) {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open '{}' for writing: {}", log_path, e);
            return ExitCode::FAILURE;
        }
    };

    // Load the ignore playlist
    let mut ignore_playlist = match Playlist::open_or_new(Playlist::ignore_file()) {
        Ok(pl) => pl,
        Err(e) => {
            error!("Failed to read '{}': {}", Playlist::ignore_file(), e);
            return ExitCode::FAILURE;
        },
    };


    println!("-- PLAYLISTS --");

    // Remove playlist duplicates
    let mut playlists = match Playlist::iter() {
        Some(it) => it.collect::<Vec<Playlist>>(),
        None => return ExitCode::FAILURE,
    };
    match remove_playlist_duplicates(&mut playlists) {
        0 => println!("No duplicate paths found"),
        n => println!("{} {} duplicate paths",
            if cli.pretend { "Detected" } else { "Removed" },
            n),
    };

    // Find invalid playlist tracks
    match find_invalid_tracks(
        &playlists,
        &mut invalid_tracks,
        |x| ignore_playlist.contains(x),
        &mut log_file
    ) {
        0 => println!("No invalid paths found"),
        n => println!("Detected {} invalid paths", n),
    };


    println!("\n-- PLAYCOUNT --");

    // Remove playcount duplicates
    let mut playcounts = match Playcount::iter() {
        Some(it) => it.collect::<Vec<Playcount>>(),
        None => return ExitCode::FAILURE,
    };
    match merge_playcount_duplicates(&mut playcounts) {
        0 => println!("No duplicate entries found"),
        n => println!("{} {} duplicate entries",
            if cli.pretend { "Detected" } else { "Merged" },
            n),
    };

    // Find playcount entries with invalid paths
    match find_invalid_tracks(
        &playcounts,
        &mut invalid_tracks,
        |x| ignore_playlist.contains(x),
        &mut log_file
    ) {
        0 => println!("No invalid paths found"),
        n => println!("Detected {} invalid paths", n),
    };

    // Close the log file
    drop(log_file);

    if !invalid_tracks.is_empty() {
        // Figure out which pager command to use
        let pager = match std::env::var("PAGER") {
            Ok(cmd) => cmd,
            _ => "less".to_string(),
        };

        // Open the log file in the pager for showcase
        match Command::new(pager).arg("--").arg(log_path).spawn() {
            Ok(mut proc) => { let _ = proc.wait(); },
            Err(e) => warn!("Failed to run the pager: {}", e),
        };

        if !cli.pretend {
            // Interactively decide how to fix the paths
            println!("\nFixing {} paths:", invalid_tracks.len());
            let (edits, deletes) = match ask_resolve_invalid_paths(
                &invalid_tracks, &playlists, &playcounts) {
                Ok(tuple) => tuple,
                Err(e) => {
                    error!("{}", e);
                    return ExitCode::FAILURE;
                },
            };

            // Remove tracks marked for deletion
            remove_tracks_from_playlists(&mut playlists, &deletes, &mut ignore_playlist);
            remove_tracks_from_playcounts(&mut playcounts, &deletes, &mut ignore_playlist);

            // Update the ignore playlist
            if ignore_playlist.is_modified() {
                if let Err(e) = ignore_playlist.write() {
                    error!("Failed to write to '{}': {}", ignore_playlist.path(), e);
                }
            }

            // Apply path edits
            playlists.iter_mut().for_each(|x| { x.repath(&edits); });
            playcounts.iter_mut().for_each(|x| { x.repath(&edits); });

            // Write all modified files
            for mut playlist in playlists.into_iter().filter(|x| x.is_modified()) {
                if let Err(e) = playlist.write() {
                    error!("Failed to write to '{}': {}", playlist.path(), e);
                }
            }
            for mut playcount in playcounts.into_iter().filter(|x| x.is_modified()) {
                if let Err(e) = playcount.write() {
                    error!("Failed to write to '{}': {}", playcount.path(), e);
                }
            }
        }
    }

    ExitCode::SUCCESS
}

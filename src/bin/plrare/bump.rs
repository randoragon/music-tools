use music_tools::{music_dir, mpd_connect};
use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use log::warn;
use std::fs::File;
use std::io::{BufReader, BufRead};

/// Convert command-line argument `plrare bump <ITEM>` into a list of file paths to bump.
pub fn get_fpaths_from_item(item: &str) -> Result<Vec<Utf8PathBuf>> {
    match item {
        // `item` denotes the current contents of the MPD queue
        "^" => {
            let mut conn = match mpd_connect() {
                Ok(conn) => conn,
                Err(e) => {
                    println!("{}", e);
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

        // `item` is a path to a playlist
        x if x.ends_with(".m3u") => {
            let playlist = match File::open(item) {
                Ok(file) => file,
                Err(e) => return Err(anyhow!("Failed to open playlist '{}': {}", item, e)),
            };

            Ok(BufReader::new(playlist)
                .lines()
                .map_while(Result::ok)
                .map(|x| [music_dir().as_str(), x.as_str()].iter().collect())
                .collect())
        },

        // `item` is a path to an audio file
        _ => {
            Ok(vec![Utf8PathBuf::from(item)])
        }
    }
}

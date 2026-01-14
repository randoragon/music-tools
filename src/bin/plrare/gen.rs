use music_tools::{
    music_dir,
    library_songs,
    path_from,
    compute_duration,
    playcount::*,
    playlist::*,
};
use regex::Regex;
use std::sync::OnceLock;
use anyhow::{Result, anyhow};
use camino::Utf8PathBuf;
use std::collections::HashMap;
use rand::rng;
use rand::distr::Distribution;
use rand::distr::weighted::WeightedIndex;
use rand::seq::SliceRandom;
use std::time::Duration;

enum Content {
    Number(usize),
    Duration(Duration),
}

fn parse_content(content: &str) -> Result<Content> {
    fn re_duration() -> &'static Regex {
        static RE_DURATION: OnceLock<Regex> = OnceLock::new();
        RE_DURATION.get_or_init(|| {
            Regex::new(r"((\d+):)?(\d+):(\d+)?").expect("Failed to compile RE_DURATION regex")
        })
    }
    if let Ok(n) = content.parse::<usize>() {
        Ok(Content::Number(n))
    } else {
        let captures = match re_duration().captures(content) {
            Some(v) => v,
            None => return Err(anyhow!("Failed to parse CONTENT")),
        };
        let hrs = captures.get(2).map_or(0, |x| x.as_str().parse::<u64>().unwrap());
        let mins = captures.get(3).unwrap().as_str().parse::<u64>().unwrap();
        let secs = captures.get(4).map_or(0, |x| x.as_str().parse::<u64>().unwrap());
        Ok(Content::Duration(Duration::new(hrs * 3600 + mins * 60 + secs, 0)))
    }
}

pub fn generate(content: &str, reverse: bool, strict: bool) -> Result<()> {
    // Change directory to music_dir to make path validation easier
    if let Err(e) = std::env::set_current_dir(music_dir()) {
        return Err(anyhow!("Failed to change directory to {}: {}", music_dir(), e));
    }

    let content = parse_content(content)?;

    // Build a map of all tracks (including unplayed) to their play counts
    let mut tracks = HashMap::<Utf8PathBuf, usize>::new();
    let mut track_durations = HashMap::<Utf8PathBuf, Option<Duration>>::new();
    for fpath in library_songs() {
        tracks.insert(fpath.clone(), 0);
        track_durations.insert(fpath.clone(), None);
    }
    for playcount in Playcount::iter()? {
        for entry in playcount.entries() {
            match tracks.get_mut(&entry.track.path) {
                Some(n) => {
                    *n += 1;
                    *track_durations.get_mut(&entry.track.path).unwrap() = Some(entry.duration);
                },
                None => continue, // Playcount path no longer exists in the library
            }
        }
    }

    let tracks = tracks.into_iter().collect::<Vec<_>>();

    let picks = if strict {
        generate_strict(content, tracks, track_durations, reverse)
    } else {
        generate_prob(content, tracks, track_durations, reverse)
    }?;

    // Write the result into a playlist
    let mut playlist = Playlist::new(path_from(|| Some(Playlist::playlist_dir()), ".Rare.m3u"))?;
    for fpath in picks {
        playlist.push(fpath)?;
    }
    playlist.write()?;

    Ok(())
}

fn generate_prob(content: Content, tracks: Vec<(Utf8PathBuf, usize)>, track_durations: HashMap<Utf8PathBuf, Option<Duration>>, reverse: bool) -> Result<Vec<Utf8PathBuf>> {
    let (tracks, track_playcounts): (Vec<_>, Vec<_>) = tracks.into_iter().unzip();

    // Raw playcount values don't work well as probability weights; pass them through
    // experimentally chosen "activation functions" for better results.
    let mut weights = track_playcounts.iter()
        .map(|&x| x as f64)
        .collect::<Vec<_>>();
    if !reverse {
        // Invert `weights` to give higher values to unpopular tracks.
        // Inverting changes the majority of small values to become big, and the small number of
        // high values to become small. To increase the chances of the few low values for better
        // diversity, bring all values closer together with logarithm smoothing.
        let max = *track_playcounts.iter().max().unwrap() as f64;
        for (idx, w) in weights.iter_mut().enumerate() {
            *w = f64::log2(2.0 + max - *w);

            // Give extra advantage to tracks with 0 listens
            if track_playcounts[idx] == 0 {
                *w += 100.0;
            }
        }
    } else {
        for w in weights.iter_mut() {
            // Higher = more radical to pick values above PIVOT. Probably best kept [0.1; 0.2].
            const STEEPNESS: f64 = 0.13;
            // The point of maximum slope (left = fast shrinkage, right = fast growth).
            // Should be kept at a number that is considered a high number of plays for a track.
            const PIVOT: f64 = 15.0;
            *w = 1.0 + f64::tanh(STEEPNESS * (*w - PIVOT));
        }
    }
    assert!(weights.iter().all(|&x| x != 0.0));

    // Set up WeightedIndex and rng for weighted distribution sampling
    let mut dist = WeightedIndex::new(&weights)?;
    let mut rng = rng();
    let mut last_idx: Option<usize> = None;

    let mut ret = Vec::<Utf8PathBuf>::new();
    let mut add_next = || -> Result<Duration> {
        // Choose a random value within the cumulative sum range
        if let Some(last_idx) = last_idx {
            // Set the last added track's probability to 0.
            // If the sum of the distribution becomes 0, update_weights will throw an error,
            // thus allowing to detect when we've run out of tracks.
            if let Err(e) = dist.update_weights(&[(last_idx, &0.0)]) {
                return Err(anyhow!("No tracks left to choose from: {e}"));
            }
        }
        let idx = dist.sample(&mut rng);
        let track = &tracks[idx];
        let track_duration = match track_durations.get(track).unwrap() {
            Some(v) => *v,
            None => match compute_duration(track) {
                Ok(val) => val,
                Err(e) => return Err(anyhow!("Failed to measure the duration of '{}': {}", track, e)),
            },
        };
        println!("{}\t{}", track_playcounts[idx], track);
        ret.push(track.clone());
        last_idx = Some(idx);
        Ok(track_duration)
    };


    match content {
        Content::Number(n) => {
            for _ in 0..n {
                add_next()?;
            }
        },
        Content::Duration(d) => {
            let mut pl_duration = Duration::new(0, 0);
            while pl_duration < d {
                pl_duration += add_next()?;
            }
        },
    }
    Ok(ret)
}

fn generate_strict(content: Content, mut tracks: Vec<(Utf8PathBuf, usize)>, track_durations: HashMap<Utf8PathBuf, Option<Duration>>, reverse: bool) -> Result<Vec<Utf8PathBuf>> {
    let mut rng = rng();
    tracks.shuffle(&mut rng);
    tracks.sort_by_key(|x| if reverse { x.1 as i64 } else { -(x.1 as i64) });

    let mut ret = Vec::<Utf8PathBuf>::new();
    let mut add_next = || -> Result<Duration> {
        // Choose a random value within the cumulative sum range
        let (track, n_plays) = match tracks.pop() {
            Some(v) => v,
            None => return Err(anyhow!("No tracks left to choose from")),
        };
        let track_duration = match track_durations.get(&track).unwrap() {
            Some(v) => *v,
            None => match compute_duration(&track) {
                Ok(val) => val,
                Err(e) => return Err(anyhow!("Failed to measure the duration of '{}': {}", &track, e)),
            },
        };
        println!("{n_plays}\t{track}");
        ret.push(track.clone());
        Ok(track_duration)
    };


    match content {
        Content::Number(n) => {
            for _ in 0..n {
                add_next()?;
            }
        },
        Content::Duration(d) => {
            let mut pl_duration = Duration::new(0, 0);
            while pl_duration < d {
                pl_duration += add_next()?;
            }
        },
    }
    Ok(ret)
}

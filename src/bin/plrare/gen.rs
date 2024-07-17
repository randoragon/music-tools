use music_tools::{
    mpd_connect,
    playcount::*,
};
use std::time::Duration;
use regex::Regex;
use std::sync::OnceLock;
use anyhow::{Result, anyhow};

enum Content {
    Number(usize),
    Duration(Duration),
}

fn parse_content(content: &String) -> Result<Content> {
    fn re_duration() -> &'static Regex {
        static RE_DURATION: OnceLock<Regex> = OnceLock::new();
        RE_DURATION.get_or_init(|| {
            Regex::new(r"((%d+):)?(%d+):(%d+)?").expect("Failed to compile RE_DURATION regex")
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

pub fn generate(content: &String, reverse: bool, strict: bool) -> Result<()> {
    let mut len = 0;
    let mut dur = Duration::new(0, 0);

    let playcount = Playcount::iter()

    let mut add_next = || -> Result<()> {
        todo!();
        Ok(())
    };

    match parse_content(content)? {
        Content::Number(n) => {
            for _ in 0..n {
                add_next()?;
            }
        },
        Content::Duration(d) => {
            while dur < d {
                add_next()?;
            }
        },
    }
    Ok(())
}

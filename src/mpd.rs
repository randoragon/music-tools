pub use mpd::Client;
use std::sync::OnceLock;
use std::env;
use anyhow::{Result, anyhow};

static MPD_DEFAULT_HOST: &str = "127.0.0.1";
static MPD_DEFAULT_PORT: &str = "6600";

pub fn mpd_connect() -> Result<Client> {
    match Client::connect(address()) {
        Ok(c) => Ok(c),
        Err(e) => Err(anyhow!("Failed to connect to MPD: {e}")),
    }
}

fn address() -> &'static str {
    static MPD_ADDRESS: OnceLock<String> = OnceLock::new();
    MPD_ADDRESS.get_or_init(|| {
        let mut addr = String::with_capacity(64);
        // It seems the mpd crate does not support connecting via Linux socket files, and that's
        // what I use. So, ignore MPD_HOST and just use localhost always.
        // if false let Ok(str) = env::var("MPD_HOST") {
        //     addr.push_str(&str);
        // } else {
        //     addr.push_str(MPD_DEFAULT_HOST);
        // }
        addr.push_str(MPD_DEFAULT_HOST);
        addr.push(':');
        if let Ok(str) = env::var("MPD_PORT") {
            addr.push_str(&str);
        } else {
            addr.push_str(MPD_DEFAULT_PORT);
        }
        addr
    })
}

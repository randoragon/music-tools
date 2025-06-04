use std::fs;
use midly::{Smf, TrackEventKind, MidiMessage};
use std::process::ExitCode;
use clap::Parser;
use camino::Utf8PathBuf;
use std::collections::HashMap;

#[derive(Parser)]
struct Cli {
    file: Utf8PathBuf,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let bytes = fs::read(&cli.file).unwrap();
    let smf = Smf::parse(&bytes).unwrap();

    println!("{}\n", cli.file);
    println!("no. tracks: {}", smf.tracks.len());

    // Count up all events in all channels
    let mut channels = HashMap::<u8, HashMap<String, u64>>::new();
    for track_events in smf.tracks {
        for event in track_events {
            match event.kind {
                TrackEventKind::Midi { channel, message } => {
                    let channel = channel.as_int();
                    let msg_name = match message {
                        MidiMessage::NoteOn { key: _, vel: _ } => "note-on",
                        MidiMessage::NoteOff { key: _, vel: _ }  => "note-off",
                        MidiMessage::Controller { controller: _, value: _ } => "controller",
                        MidiMessage::Aftertouch { key: _, vel: _ } => "aftertouch",
                        MidiMessage::ChannelAftertouch { vel: _ }  => "channel-aftertouch",
                        MidiMessage::ProgramChange { program: _ }  => "program-change",
                        MidiMessage::PitchBend { bend: _ }  => "pitch-bend",
                    };
                    channels.entry(channel).or_default();
                    if !channels[&channel].contains_key(msg_name) {
                        channels.get_mut(&channel).unwrap().insert(msg_name.to_owned(), 0);
                    }
                    *channels.get_mut(&channel).unwrap().get_mut(msg_name).unwrap() += 1;
                },
                _ => continue,
            }
        }
    }

    println!("no. channels: {}\n", channels.keys().count());

    let mut sorted_channels = channels.keys().cloned().collect::<Vec<u8>>();
    sorted_channels.sort();

    for channel in sorted_channels {
        println!("channel #{}", channel + 1);
        for (msg_name, count) in &channels[&channel] {
            println!("  - {}: {}", msg_name, count);
        }
    }

    ExitCode::SUCCESS
}

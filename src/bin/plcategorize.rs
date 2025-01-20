use music_tools::{
    mpd::*,
    track::Track,
    playlist::{Playlist, TracksFile},
    playlist::tui_picker::*,
};
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    text::{Span, Line, Text},
    widgets::Clear,
    Frame,
    style::{Style, Stylize},
    layout::{Layout, Constraint, Direction, Alignment},
};
use log::error;
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::{LazyLock, Mutex};

static CURRENT_TRACK: LazyLock<Mutex<Option<Track>>> = LazyLock::new(|| {
    Mutex::new(fetch_current_track().ok())
});

struct App {
    title: String,
    picker_state: TuiPickerState,
}

fn fetch_current_track() -> Result<Track> {
    let mut conn = mpd_connect()?;
    let song = conn.currentsong()?;
    match song {
        Some(info) => Ok(Track::new(info.file)),
        None => Err(anyhow!("No track playing")),
    }
}

fn on_refresh(_state: u8, playlist: &mut Playlist) -> u8 {
    if let Err(_) = playlist.reload() {
        return 2;
    }

    if let Some(track) = CURRENT_TRACK.lock().unwrap().as_ref() {
        if playlist.contains(track) {
            return 1;
        }
    }

    0
}

fn on_select(state: u8, playlist: &mut Playlist) -> u8 {
    if let Err(_) = playlist.reload() {
        return 2;
    }

    if let Some(track) = CURRENT_TRACK.lock().unwrap().as_ref() {
        return match state {
            0 => {
                // Add to playlist
                if playlist.contains(track) {
                    1
                } else if let Ok(_) = playlist.push(track.path.clone()) {
                    if let Ok(_) = playlist.write() {
                        1
                    } else {
                        0
                    }
                } else {
                    0
                }
            },
            1 => {
                // Remove from playlist
                playlist.remove_all(track);
                if let Ok(_) = playlist.write() {
                    0
                } else {
                    1
                }
            },
            2 => 2,
            _ => panic!("unknown state {state}, internal error!"),
        };
    }

    0
}

fn app_init() -> Result<App> {
    let state_styles = HashMap::from([
        (0, Style::new().red()),
        (1, Style::new().bold().green()),
        (2, Style::new().gray()),
    ]);
    let picker_state = TuiPickerState::new(0, &state_styles, on_refresh, on_select)?;
    Ok(App {
        title: String::from(" plcategorize "),
        picker_state,
    })
}

fn draw(app: &App, frame: &mut Frame, input: &str) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(frame.area());

    let title_bar = Line::from(vec![
        Span::styled(&app.title, Style::new().bold().reversed()),
        Span::raw(" "),
        Span::styled("q", Style::new().bold().blue()),
        Span::raw(" exit  "),
        Span::styled("ESC", Style::new().bold().blue()),
        Span::raw(if input.is_empty() { " refresh" } else { " cancel" }),
    ]);
    frame.render_widget(title_bar, layout[0]);
    frame.render_widget(Clear, layout[1]);
    frame.render_widget(Clear, layout[2]);
    frame.render_widget(Clear, layout[3]);
    frame.render_widget(TuiPicker::new(&app.picker_state, input), layout[4]);
}

enum Action {
    Quit,
    NewChar,
    Refresh,
    ClearInput,
    Ignore,
}

/// Handles a crossterm event.
///
/// Return values:
/// - 0: quit application
/// - 1: default (add to input buffer)
/// - 2: refresh UI
/// - 3: clear input
fn handle_event(ev: Event, input: &mut String) -> Action {
    match ev {
        Event::Key(kev) => handle_key_event(kev, input),
        _ => Action::Ignore,
    }
}

fn handle_key_event(kev: event::KeyEvent, input: &mut String) -> Action {
    if kev.code == KeyCode::Char('q') && input.is_empty() {
        return Action::Quit;
    }

    if kev.code == KeyCode::Esc {
        if !input.is_empty() {
            return Action::ClearInput;
        } else {
            return Action::Refresh;
        }
    }

    match kev.code {
        KeyCode::Char(c) => {
            input.push(c);
            Action::NewChar
        },
        _ => Action::Ignore,
    }
}

fn main() -> ExitCode {
    stderrlog::new()
        .module(module_path!())
        .module("music_tools")
        .verbosity(2)
        .init()
        .unwrap();

    let mut app = match app_init() {
        Ok(app) => app,
        Err(e) => {
            error!("Failed to initialize application: {e}");
            return ExitCode::FAILURE;
        },
    };
    let mut input = String::with_capacity(32);
    let mut terminal = ratatui::init();
    app.picker_state.refresh();
    loop {
        if let Err(e) = terminal.draw(|x| draw(&app, x, &input)) {
            error!("Failed to draw frame: {e}");
            return ExitCode::FAILURE;
        }

        // Event handling
        let ev = match event::read() {
            Ok(ev) => ev,
            Err(e) => {
                error!("Failed to read event: {e}");
                return ExitCode::FAILURE;
            }
        };

        match handle_event(ev, &mut input) {
            Action::Ignore => {},
            Action::Quit => break,
            Action::NewChar => {
                if !app.picker_state.update_input(&input) {
                    input.clear();
                }
            },
            Action::Refresh => {
                *CURRENT_TRACK.lock().unwrap() = fetch_current_track().ok();
                app.picker_state.refresh();
                // TODO: visual feedback
            }
            Action::ClearInput => {
                input.clear();
            },
        }
    }

    // Exit
    ratatui::restore();
    ExitCode::SUCCESS
}

use music_tools::{
    path_from,
    playlist::*,
    track::*,
    widgets::tui_picker::*,
    widgets::track_info::*,
};
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEventKind};
use ratatui::{
    text::{Span, Line},
    Frame,
    widgets::{Scrollbar, ScrollbarState, ScrollbarOrientation},
    style::{Style, Stylize},
    layout::{Layout, Constraint, Direction},
};
use log::error;
use anyhow::Result;
use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::{LazyLock, Mutex};

const DELETE_PLAYLIST: &str = ".Delete.m3u";

static CURRENT_TRACK: LazyLock<Mutex<TrackInfo>> = LazyLock::new(|| {
    Mutex::new(TrackInfo::default())
});

struct App {
    title: String,
    picker_state: TuiPickerState,
    delete_item_state: TuiPickerItemState,
    scroll_state: ScrollbarState,
}

fn on_refresh(_state: u8, playlist: &mut Playlist) -> u8 {
    if playlist.reload().is_err() {
        return 2;
    }

    let track_info = CURRENT_TRACK.lock().unwrap();
    if let Some(file) = track_info.file() {
        if playlist.contains(&Track::new(file)) {
            return 1;
        }
    }

    0
}

fn on_select(state: u8, playlist: &mut Playlist) -> u8 {
    if playlist.reload().is_err() {
        return 2;
    }

    let track_info = CURRENT_TRACK.lock().unwrap();
    if let Some(file) = track_info.file() {
        let track = Track::new(file);
        return match state {
            0 => {
                // Add to playlist
                if playlist.contains(&track) {
                    1
                } else if playlist.push(track.path).is_ok() {
                    if playlist.write().is_ok() {
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
                playlist.remove_all(&track);
                if playlist.write().is_ok() {
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
        (2, Style::new().dark_gray().crossed_out()),
    ]);
    let picker_state = TuiPickerState::new(0, &state_styles, on_refresh, on_select)?;

    let delete_playlist = Playlist::open(path_from(|| Some(Playlist::playlist_dir()), DELETE_PLAYLIST)).unwrap();
    let delete_item_state = TuiPickerItemState::new(
        delete_playlist,
        String::from("DEL"),
        0,  // width
        0,  // shortcut_rpad
        0,  // state
        HashMap::from([
            (0, Style::new().red()),
            (1, Style::new().bold().white().on_light_red()),
            (2, Style::new().dark_gray().crossed_out()),
        ]),
        on_refresh,
        on_select,
    );

    Ok(App {
        title: String::from(" plcategorize "),
        picker_state,
        delete_item_state,
        scroll_state: ScrollbarState::default(),
    })
}

fn draw(app: &mut App, frame: &mut Frame, input: &str) {
    let title_bar = Line::from(vec![
        Span::styled(&app.title, Style::new().bold().reversed()),
        Span::raw(" "),
        Span::styled("q", Style::new().bold().blue()),
        Span::raw(" exit  "),
        Span::styled("ESC", Style::new().bold().blue()),
        Span::raw(if input.is_empty() { " refresh" } else { " cancel" }),
    ]);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(frame.area());

    let layout_title_delete = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![
            Constraint::Length(title_bar.width() as u16 + 2),
            Constraint::Min(0),
        ])
        .split(layout[0]);

    let layout_indent = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .split(layout[2]);

    let layout_song_picker = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .split(layout_indent[1]);

    let layout_picker_scroll = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(layout_song_picker[2]);

    let current_track = CURRENT_TRACK.lock().unwrap().clone();
    frame.render_widget(title_bar, layout_title_delete[0]);
    if current_track.file().is_some() {
        let tui_picker = TuiPicker::new(input);

        frame.render_widget(TuiPickerItem::new(&app.delete_item_state, input), layout_title_delete[1]);
        frame.render_stateful_widget(tui_picker, layout_picker_scroll[0], &mut app.picker_state);

        // Compute scroll. This must be done after rendering tui_picker, because tui_picker
        // may clamp app.picker_state.scroll_amount inside its render code.
        let tui_picker_area_w = layout_picker_scroll[0].width;
        let tui_picker_area_h = layout_picker_scroll[0].height;
        let tui_picker_h = app.picker_state.height(tui_picker_area_w as usize);
        let mut scroll_state = app.scroll_state
            .content_length(tui_picker_h.saturating_sub(tui_picker_area_h as usize))
            .position(app.picker_state.scroll_amount);

        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            layout_picker_scroll[1],
            &mut scroll_state,
        );
    }
    if app.picker_state.is_refreshing() {
        frame.render_widget(Line::styled("Refreshing ...", Style::new().dark_gray()), layout_song_picker[0]);
    } else {
        frame.render_widget(current_track, layout_song_picker[0]);
    }
}

enum Action {
    Quit,
    NewChar,
    DelChar,
    ToggleDelete,
    Refresh,
    ClearInput,
    Ignore,
    ScrollUp,
    ScrollDown,
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
        Event::Mouse(mev) => handle_mouse_event(mev),
        _ => Action::Ignore,
    }
}

fn handle_key_event(kev: event::KeyEvent, input: &mut String) -> Action {
    let has_selection = CURRENT_TRACK.lock().unwrap().file().is_some();
    if kev.code == KeyCode::Esc {
        if !input.is_empty() {
            return Action::ClearInput;
        } else {
            return Action::Refresh;
        }
    }

    // Scrolling
    if kev.code == KeyCode::Up {
        return Action::ScrollUp;
    }
    if kev.code == KeyCode::Down {
        return Action::ScrollDown;
    }
    if !kev.modifiers.intersection(KeyModifiers::CONTROL | KeyModifiers::ALT).is_empty() {
        if kev.code == KeyCode::Char('k') {
            return Action::ScrollUp;
        }
        if kev.code == KeyCode::Char('j') {
            return Action::ScrollDown;
        }
    }

    if kev.code == KeyCode::Char('q') && input.is_empty() {
        return Action::Quit;
    }

    if has_selection {
        if kev.code == KeyCode::Backspace && !input.is_empty() {
            return Action::DelChar;
        }

        if kev.code == KeyCode::Delete && input.is_empty() {
            return Action::ToggleDelete;
        }

        if let KeyCode::Char(c) = kev.code {
            input.push(c);
            return Action::NewChar;
        }
    }

    Action::Ignore
}

fn handle_mouse_event(mev: event::MouseEvent) -> Action {
    match mev.kind {
        MouseEventKind::ScrollUp => Action::ScrollUp,
        MouseEventKind::ScrollDown => Action::ScrollDown,
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
    app.delete_item_state.refresh();
    loop {
        if let Err(e) = terminal.draw(|x| draw(&mut app, x, &input)) {
            error!("Failed to draw frame: {e}");
            return ExitCode::FAILURE;
        }
        if app.picker_state.is_refreshing() {
            app.picker_state.refresh();
            app.delete_item_state.refresh();
        } else {
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
                Action::DelChar => {
                    input.remove(input.len() - 1);
                }
                Action::ToggleDelete => {
                    app.delete_item_state.select();
                    input.clear();
                }
                Action::Refresh => {
                    *CURRENT_TRACK.lock().unwrap() = TrackInfo::default();
                    app.picker_state.refresh();
                    app.delete_item_state.refresh();
                }
                Action::ClearInput => {
                    input.clear();
                },
                Action::ScrollUp => {
                    let scroll_amount = &mut app.picker_state.scroll_amount;
                    *scroll_amount = scroll_amount.saturating_sub(1);
                }
                Action::ScrollDown => {
                    let scroll_amount = &mut app.picker_state.scroll_amount;
                    *scroll_amount = scroll_amount.saturating_add(1);
                }
            }
        }
    }

    // Exit
    ratatui::restore();
    ExitCode::SUCCESS
}

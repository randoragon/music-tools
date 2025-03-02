use music_tools::{
    path_from,
    mpd::*,
    library_songs,
    playlist::*,
    track::*,
    widgets::tui_picker::*,
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
use std::collections::{HashMap, HashSet};
use std::process::ExitCode;

struct App {
    title: String,
    picker_state: TuiPickerState,
    mpd_item_state: TuiPickerItemState,
    scroll_state: ScrollbarState,
}

fn on_refresh(state: u8, playlist: &mut Playlist) -> u8 {
    *playlist = match Playlist::open(playlist.path()) {
        Ok(pl) => pl,
        Err(_) => return 3,
    };
    state
}

fn on_select(mut state: u8, _playlist: &mut Playlist) -> u8 {
    if state == 3 {
        return state
    } else {
        state = (state + 1) % 3;
    }
    state
}

fn app_init() -> Result<App> {
    let state_styles = HashMap::from([
        (0, Style::new().gray()),
        (1, Style::new().bold().green()),
        (2, Style::new().bold().red()),
        (3, Style::new().dark_gray().crossed_out()),
    ]);
    let picker_state = TuiPickerState::new(0, &state_styles, on_refresh, on_select)?;
    let mpd_playlist = Playlist::new("mpd").unwrap();  // File name is display-only
    let mpd_item_state = TuiPickerItemState::new(
        mpd_playlist,
        String::from("."),
        0,  // width
        0,  // shortcut_rpad
        0,  // state
        HashMap::from([
            (0, Style::new().gray()),
            (1, Style::new().bold().green()),
            (2, Style::new().bold().red()),
            (3, Style::new().dark_gray().crossed_out()),
        ]),
        on_refresh,
        on_select,
    );

    Ok(App {
        title: String::from(" plfilter "),
        picker_state,
        mpd_item_state,
        scroll_state: ScrollbarState::default(),
    })
}

fn draw(app: &mut App, frame: &mut Frame, input: &str) {
    let title_bar = Line::from(vec![
        Span::styled(&app.title, Style::new().bold().reversed()),
        Span::raw(" "),
        Span::styled("q", Style::new().bold().blue()),
        Span::raw(" exit  "),
    ]);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(frame.area());

    let layout_title_mpd_filtered = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![
            Constraint::Length(title_bar.width() as u16 + 2),
            Constraint::Min(0), // TODO: fix alignment?
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

    let layout_picker_scroll = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(layout_indent[1]);

    frame.render_widget(title_bar, layout_title_mpd_filtered[0]);
    frame.render_widget(TuiPickerItem::new(&app.mpd_item_state, input), layout_title_mpd_filtered[1]);
    // TODO: Render n_filtered at layout_title_filtered[2]

    frame.render_stateful_widget(
        TuiPicker::new(input),
        layout_picker_scroll[0],
        &mut app.picker_state
    );

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

enum Action {
    Quit,
    NewChar,
    DelChar,
    ToggleMPD,
    Refresh,
    ClearInput,
    Ignore,
    ScrollUp,
    ScrollDown,
    ScrollUpMore,
    ScrollDownMore,
}

fn handle_event(ev: Event, input: &mut String) -> Action {
    match ev {
        Event::Key(kev) => handle_key_event(kev, input),
        Event::Mouse(mev) => handle_mouse_event(mev),
        _ => Action::Ignore,
    }
}

fn handle_key_event(kev: event::KeyEvent, input: &mut String) -> Action {
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
        if kev.code == KeyCode::Char('u') {
            return Action::ScrollUpMore;
        }
        if kev.code == KeyCode::Char('d') {
            return Action::ScrollDownMore;
        }
    }

    if kev.code == KeyCode::Char('q') && input.is_empty() {
        return Action::Quit;
    }
    if kev.modifiers == KeyModifiers::CONTROL && kev.code == KeyCode::Char('c') {
        return Action::Quit;
    }

    if kev.code == KeyCode::Backspace && !input.is_empty() {
        return Action::DelChar;
    }

    if kev.code == KeyCode::Char('.') && input.is_empty() {
        return Action::ToggleMPD;
    }

    if let KeyCode::Char(c) = kev.code {
        input.push(c);
        return Action::NewChar;
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

fn generate_filtered_playlist(picker_state: &TuiPickerState, mpd_item_state: &TuiPickerItemState) -> Result<()> {
    let mut playlist = Playlist::new(path_from(|| Some(Playlist::playlist_dir()), ".Filtered.m3u"))?;
    // TODO: optimize -- we do not need to start with all songs if at least one item is green
    let mut tracks: HashSet<Track> = library_songs().iter().map(Track::new).into_iter().collect();
    let mpd_tracks = match mpd_item_state.state() {
        1 | 2 => {
            let mut conn = mpd_connect()?;
            conn.queue()?.into_iter().map(|x| Track::new(x.file)).collect()
        },
        _ => vec![],
    };
    for pl in picker_state.get_playlists_with_state(1) {
        tracks = tracks.into_iter().filter(|x| pl.contains(x)).collect();
    }
    if mpd_item_state.state() == 1 {
        tracks = tracks.into_iter().filter(|x| mpd_tracks.contains(x)).collect();
    }
    for pl in picker_state.get_playlists_with_state(2) {
        tracks = tracks.into_iter().filter(|x| !pl.contains(x)).collect();
    }
    if mpd_item_state.state() == 2 {
        tracks = tracks.into_iter().filter(|x| !mpd_tracks.contains(x)).collect();
    }
    for track in tracks {
        playlist.push_track(track)?;
    }
    playlist.write()?;
    Ok(())
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
        if let Err(e) = terminal.draw(|x| draw(&mut app, x, &input)) {
            error!("Failed to draw frame: {e}");
            return ExitCode::FAILURE;
        }
        if app.picker_state.is_refreshing() {
            app.picker_state.refresh();
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
                    if app.picker_state.did_select() {
                        if let Err(e) = generate_filtered_playlist(&app.picker_state, &app.mpd_item_state) {
                            error!("Failed to generated .Filtered.m3u: {e}");
                            return ExitCode::FAILURE;
                        }
                    }
                },
                Action::DelChar => {
                    input.remove(input.len() - 1);
                }
                Action::ToggleMPD => {
                    app.mpd_item_state.select();
                    input.clear();
                    if let Err(e) = generate_filtered_playlist(&app.picker_state, &app.mpd_item_state) {
                        error!("Failed to generated .Filtered.m3u: {e}");
                        return ExitCode::FAILURE;
                    }
                },
                Action::Refresh => {
                    app.picker_state.refresh();
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
                Action::ScrollUpMore => {
                    let scroll_amount = &mut app.picker_state.scroll_amount;
                    *scroll_amount = scroll_amount.saturating_sub(10);
                }
                Action::ScrollDownMore => {
                    let scroll_amount = &mut app.picker_state.scroll_amount;
                    *scroll_amount = scroll_amount.saturating_add(10);
                }
            }
        }
    }

    // Exit
    ratatui::restore();
    ExitCode::SUCCESS
}

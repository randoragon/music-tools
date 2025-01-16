use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    text::Text,
    widgets::Clear,
    Frame,
    style::{Style, Stylize},
    layout::{Layout, Constraint, Direction, Alignment},
};
use log::error;
use anyhow::Result;
use std::collections::HashMap;
use music_tools::playlist::tui_picker::*;
use std::process::ExitCode;

struct App {
    title: String,
    picker_state: TuiPickerState,
}

fn app_init() -> Result<App> {
    let states = vec![0, 1];
    let state_styles = HashMap::from([
        (0, Style::new().red()),
        (1, Style::new().green()),
    ]);
    // let state_callback = |_| {

    // };
    let picker_state = TuiPickerState::new(
        &states,
        &state_styles,
        |s| { println!("state: {s}"); },
    )?;
    Ok(App {
        title: String::from(" plcategorize "),
        picker_state,
    })
}

fn update(app: App) -> App {
    app
}

fn draw(app: &App, frame: &mut Frame) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(frame.area());

    frame.render_widget(
        Text::styled(&app.title,
        Style::new().bold().reversed()).alignment(Alignment::Center),
        layout[0]
    );
    frame.render_widget(Clear, layout[1]);
    frame.render_widget(TuiPicker::new(&app.picker_state, ""), layout[2]);
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
    loop {
        app = update(app);
        if let Err(e) = terminal.draw(|x| draw(&app, x)) {
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
                // Check input, if it matches any playlist, toggle that playlist and clear input.
                // TODO

                // If input does not match the beginning of any playlist shortcut, clear it.
                // TODO
            },
            Action::Refresh => {
                // TODO
            }
            Action::ClearInput => {
                input.clear();
                // TODO: erase shortcut highlights
            },
        }
    }

    // Exit
    ratatui::restore();
    ExitCode::SUCCESS
}

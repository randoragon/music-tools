use crossterm::event::{self, Event};
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
use std::rc::Rc;
use music_tools::playlist::tui_picker::*;
use std::process::ExitCode;

struct App {
    title: String,
    picker_state: TuiPickerState,
}

fn state_callback(state: &TuiPickerItemState) {

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
        Rc::new(state_callback),
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
        ].into_iter())
        .split(frame.area());

    frame.render_widget(
        Text::styled(&app.title,
        Style::new().bold().reversed()).alignment(Alignment::Center),
        layout[0]
    );
    frame.render_widget(Clear::default(), layout[1]);
    frame.render_widget(TuiPicker::new(&app.picker_state, ""), layout[2]);
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
    let mut terminal = ratatui::init();
    loop {
        app = update(app);
        terminal.draw(|x| draw(&app, x)).expect("failed to draw frame");
        if matches!(event::read().expect("failed to read event"), Event::Key(_)) {
            break;
        }
    }
    ratatui::restore();
    ExitCode::SUCCESS
}

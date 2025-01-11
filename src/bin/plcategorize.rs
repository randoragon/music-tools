use crossterm::event::{self, Event};
use ratatui::{
    text::Text,
    widgets::Clear,
    Frame,
    style::{Style, Stylize},
    layout::{Layout, Constraint, Direction, Alignment},
};
use music_tools::playlist::tui_picker::*;
use std::process::ExitCode;

struct App {
    title: String,
    picker_state: TuiPickerState,
}

fn init() -> App {
    App {
        title: String::from(" plcategorize "),
        picker_state: TuiPickerState::default(),
    }
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
    let mut terminal = ratatui::init();
    let mut app = init();
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

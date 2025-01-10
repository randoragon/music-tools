use crossterm::event::{self, Event};
use ratatui::{text::Text, Frame};
use music_tools::playlist::tui_picker::*;
use std::process::ExitCode;

struct App {
    picker: TuiPicker,
}

fn init() -> App {
    App {
        picker: TuiPicker::new(),
    }
}

fn update(app: App) -> App {
    app
}

fn draw(app: &App, frame: &mut Frame) {
    let text = Text::raw("Hello World!");
    frame.render_widget(text, frame.area());
    // TODO
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

use crossterm::event::{KeyCode, KeyEvent};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineAction {
    None,
    Quit,
}

pub fn key_event_to_action(event: KeyEvent) -> EngineAction {
    match event.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => EngineAction::Quit,
        _ => EngineAction::None,
    }
}

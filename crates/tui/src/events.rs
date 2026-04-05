use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};

#[derive(Debug, Clone)]
pub enum TuiEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Paste(String),
    AgentEvent(onicode_core::agent::AgentEvent),
}

impl TuiEvent {
    pub fn is_quit(&self) -> bool {
        if let TuiEvent::Key(key) = self {
            if key.modifiers == KeyModifiers::CONTROL {
                return matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'));
            }
            if key.code == KeyCode::Esc {
                return true;
            }
        }
        false
    }

    pub fn is_submit(&self) -> bool {
        if let TuiEvent::Key(key) = self {
            return matches!(key.code, KeyCode::Enter);
        }
        false
    }

    pub fn is_backspace(&self) -> bool {
        if let TuiEvent::Key(key) = self {
            return matches!(key.code, KeyCode::Backspace);
        }
        false
    }

    pub fn is_tab(&self) -> bool {
        if let TuiEvent::Key(key) = self {
            return matches!(key.code, KeyCode::Tab);
        }
        false
    }

    pub fn char_input(&self) -> Option<char> {
        if let TuiEvent::Key(KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
            ..
        }) = self
        {
            return Some(*c);
        }
        None
    }
}

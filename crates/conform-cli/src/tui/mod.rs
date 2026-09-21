//! The interactive console.
//!
//! Three panes — the catalogue, what was examined against the selected
//! specification, and one finding in full — driven entirely from the keyboard.
//!
//! # A view, in the strict sense
//!
//! The console computes nothing. [`crate::engine::run`] does the work *before*
//! the terminal is touched, and what arrives here is the same [`Run`] the
//! human report and the `--json` envelope are rendered from. So the console
//! cannot tell a reader a contract is conformant while `--json` says it is
//! not, and a rule added to an adapter appears in all three renderings at once
//! without anybody remembering to add it here.
//!
//! The split across the three modules is the same idea one level down:
//!
//! | Module | Holds | Knows about a terminal |
//! |---|---|---|
//! | [`app`] | what is selected, and what a keystroke does to it | no |
//! | [`view`] | how that looks | only how to draw |
//! | this one | raw mode, the event loop, and putting the terminal back | yes, and nothing else |
//!
//! Which is why [`app`] is testable with no terminal at all, and [`view`] is
//! testable against `ratatui`'s `TestBackend` — including the escaping, which
//! is the one thing here that a screenshot could not prove.
//!
//! # Keyboard only
//!
//! There is no mouse handling anywhere in this console, deliberately. A
//! terminal tool that needs a mouse cannot be used over `ssh` on a bad link,
//! from a text console, or by somebody driving a screen reader.

pub mod app;
pub mod view;

use std::io;
use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::model::Run;
use app::{App, Key};

/// How long to wait for a key before redrawing anyway.
///
/// A timeout rather than a blocking read, so a resize is picked up promptly
/// and the loop has somewhere to notice it should stop.
const TICK: Duration = Duration::from_millis(250);

/// Open the console on a run, and return when the user closes it.
///
/// Raw mode and the alternate screen are entered here and left here, including
/// on a panic — `ratatui`'s initialisation installs a hook for that, which
/// matters because a tool that panics out of raw mode leaves a terminal
/// nobody can type into.
///
/// # Errors
///
/// Any I/O failure setting up the terminal, drawing, or reading an event.
pub fn run(run: Run, initial_spec: Option<&str>) -> io::Result<()> {
    let mut app = App::new(run, initial_spec);
    let mut terminal = ratatui::try_init()?;

    let outcome = loop {
        if let Err(error) = terminal.draw(|frame| view::draw(frame, &app)) {
            break Err(error);
        }
        match pump(&mut app) {
            Ok(()) if app.should_quit() => break Ok(()),
            Ok(()) => {}
            Err(error) => break Err(error),
        }
    };

    // Restore before propagating: an error on the way out is still no excuse
    // for handing back a terminal in raw mode.
    let restored = ratatui::try_restore();
    outcome.and(restored)
}

/// Wait briefly for one event and apply it.
fn pump(app: &mut App) -> io::Result<()> {
    if !event::poll(TICK)? {
        return Ok(());
    }
    if let Event::Key(key) = event::read()?
        && let Some(action) = translate(key)
    {
        app.on_key(action);
    }
    Ok(())
}

/// Turn a key press into an intent.
///
/// The one place `crossterm` meets the state machine. `None` for a key the
/// console does not bind — which is most of them, and pressing one must do
/// nothing rather than something surprising.
#[must_use]
pub fn translate(key: KeyEvent) -> Option<Key> {
    // Windows reports both press and release; acting on both would move the
    // selection two rows for one keystroke.
    if key.kind == KeyEventKind::Release {
        return None;
    }

    // Ctrl-C is not a binding, it is the thing every terminal user expects to
    // get them out of anything.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        return Some(Key::Quit);
    }

    Some(match key.code {
        KeyCode::Char('q') => Key::Quit,
        KeyCode::Esc => Key::Escape,
        KeyCode::Tab => Key::NextPane,
        KeyCode::BackTab => Key::PreviousPane,
        KeyCode::Left | KeyCode::Char('h') => Key::Left,
        KeyCode::Right | KeyCode::Char('l') => Key::Right,
        KeyCode::Down | KeyCode::Char('j') => Key::Down,
        KeyCode::Up | KeyCode::Char('k') => Key::Up,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::Home | KeyCode::Char('g') => Key::First,
        KeyCode::End | KeyCode::Char('G') => Key::Last,
        KeyCode::Char('u') => Key::Upstream,
        KeyCode::Char('?') => Key::Help,
        _ => return None,
    })
}

//! What the console is showing, and what a keystroke does to it.
//!
//! Deliberately free of `ratatui`, of `crossterm`'s terminal handling, and of
//! any I/O: this is the state machine, and it is a plain type that a test can
//! construct, drive with key presses and assert on. The drawing lives in
//! [`crate::tui::view`], which reads this and never writes it.
//!
//! # It holds a [`Run`], and it never makes one
//!
//! Every fact on screen was computed by [`crate::engine::run`] before the
//! terminal was touched. The console navigates and displays; it does not
//! validate. That is what makes it a view rather than a third implementation
//! of the same idea, and it is why `--json`, the human report and this pane
//! cannot disagree about whether a contract is conformant.

use conform_core::{Diagnostic, Severity};

use crate::model::{DocumentOutcome, Run, SpecSummary};

/// Which pane has the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Pane {
    /// The catalogue: every specification in scope.
    Specs,
    /// What was examined against the selected specification, and what was
    /// found in it.
    Documents,
    /// One diagnostic, in full.
    Detail,
}

impl Pane {
    /// The panes, left to right.
    pub const ALL: [Self; 3] = [Self::Specs, Self::Documents, Self::Detail];

    /// The pane's title.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Specs => "Specs",
            Self::Documents => "Documents",
            Self::Detail => "Diagnostic",
        }
    }

    /// The pane to the right, wrapping.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Specs => Self::Documents,
            Self::Documents => Self::Detail,
            Self::Detail => Self::Specs,
        }
    }

    /// The pane to the left, wrapping.
    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::Specs => Self::Detail,
            Self::Documents => Self::Specs,
            Self::Detail => Self::Documents,
        }
    }
}

/// A row in the specs pane.
///
/// Not simply an index, because one row is not a specification: findings about
/// the *invocation* — a path that was not there, a registry that would not
/// load — belong to no standard, and hiding them because they do not fit the
/// shape would be the console quietly dropping findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// One catalogued specification, by index into [`Run::specs`].
    Spec(usize),
    /// Everything attributable to no specification.
    Run,
}

/// A row in the documents pane.
///
/// Documents and their diagnostics share one list, so `↑`/`↓` walks straight
/// through a document's findings and into the next document without a mode
/// change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// A document header, by index into the scope's document list.
    Document(usize),
    /// One diagnostic within that document.
    Diagnostic(usize, usize),
}

/// What is covering the panes, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// The full provenance record for the selected specification.
    Upstream,
    /// The key bindings.
    Help,
}

/// The console.
#[derive(Debug)]
pub struct App {
    run: Run,
    scopes: Vec<Scope>,
    scope: usize,
    rows: Vec<Row>,
    row: usize,
    documents: Vec<usize>,
    pane: Pane,
    overlay: Option<Overlay>,
    overlay_scroll: u16,
    detail_scroll: u16,
    quit: bool,
}

impl App {
    /// Open the console on a run.
    ///
    /// `initial_spec` is `--spec`: the console opens on that entry rather than
    /// on the first, so `conform --spec okf` lands where it was asked to.
    #[must_use]
    pub fn new(run: Run, initial_spec: Option<&str>) -> Self {
        let mut scopes: Vec<Scope> = (0..run.specs.len()).map(Scope::Spec).collect();
        if run.documents.iter().any(|d| d.spec_id.is_none()) {
            scopes.push(Scope::Run);
        }

        let scope = initial_spec
            .and_then(|id| run.specs.iter().position(|spec| spec.id == id))
            .unwrap_or(0)
            .min(scopes.len().saturating_sub(1));

        let mut app = Self {
            run,
            scopes,
            scope,
            rows: Vec::new(),
            row: 0,
            documents: Vec::new(),
            pane: Pane::Specs,
            overlay: None,
            overlay_scroll: 0,
            detail_scroll: 0,
            quit: false,
        };
        app.rebuild_rows();
        app
    }

    /// The run being displayed.
    #[must_use]
    pub const fn run(&self) -> &Run {
        &self.run
    }

    /// The rows of the specs pane.
    #[must_use]
    pub fn scopes(&self) -> &[Scope] {
        &self.scopes
    }

    /// Which specs-pane row is selected.
    #[must_use]
    pub const fn scope_index(&self) -> usize {
        self.scope
    }

    /// The rows of the documents pane.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Which documents-pane row is selected.
    #[must_use]
    pub const fn row_index(&self) -> usize {
        self.row
    }

    /// Which pane has the keyboard.
    #[must_use]
    pub const fn pane(&self) -> Pane {
        self.pane
    }

    /// What is covering the panes, if anything.
    #[must_use]
    pub const fn overlay(&self) -> Option<Overlay> {
        self.overlay
    }

    /// How far the overlay has been scrolled.
    ///
    /// The upstream record ends in the registry's `notes` — which is where the
    /// evidence for every field, and the reason for every absence, is written
    /// down. It is the longest text in the whole console and routinely taller
    /// than the overlay, so it has to be scrollable or the evidence is
    /// unreadable.
    #[must_use]
    pub const fn overlay_scroll(&self) -> u16 {
        self.overlay_scroll
    }

    /// How far the detail pane has been scrolled.
    #[must_use]
    pub const fn detail_scroll(&self) -> u16 {
        self.detail_scroll
    }

    /// Whether the console has been asked to close.
    #[must_use]
    pub const fn should_quit(&self) -> bool {
        self.quit
    }

    /// The selected specification, if the selected row is one.
    ///
    /// `None` on the run-level row, which has no upstream and no pin — and
    /// saying so is better than showing the previous entry's.
    #[must_use]
    pub fn selected_spec(&self) -> Option<&SpecSummary> {
        match self.scopes.get(self.scope)? {
            Scope::Spec(index) => self.run.specs.get(*index),
            Scope::Run => None,
        }
    }

    /// The document the selected row belongs to.
    #[must_use]
    pub fn selected_document(&self) -> Option<&DocumentOutcome> {
        let index = match self.rows.get(self.row)? {
            Row::Document(index) | Row::Diagnostic(index, _) => *index,
        };
        self.run.documents.get(*self.documents.get(index)?)
    }

    /// The selected diagnostic, when a diagnostic row is selected.
    #[must_use]
    pub fn selected_diagnostic(&self) -> Option<&Diagnostic> {
        match self.rows.get(self.row)? {
            Row::Document(_) => None,
            Row::Diagnostic(document, diagnostic) => self
                .run
                .documents
                .get(*self.documents.get(*document)?)?
                .report
                .diagnostics()
                .get(*diagnostic),
        }
    }

    /// The document at a documents-pane row, for the view.
    #[must_use]
    pub fn document_at(&self, index: usize) -> Option<&DocumentOutcome> {
        self.run.documents.get(*self.documents.get(index)?)
    }

    /// The diagnostic at a documents-pane row, for the view.
    #[must_use]
    pub fn diagnostic_at(&self, document: usize, diagnostic: usize) -> Option<&Diagnostic> {
        self.document_at(document)?
            .report
            .diagnostics()
            .get(diagnostic)
    }

    /// The worst severity in the selected scope, for the pane title.
    #[must_use]
    pub fn scope_worst(&self, scope: Scope) -> Option<Severity> {
        self.documents_of(scope)
            .into_iter()
            .filter_map(|index| self.run.documents.get(index))
            .filter_map(DocumentOutcome::worst)
            .max()
    }

    /// How many documents a scope holds.
    #[must_use]
    pub fn scope_documents(&self, scope: Scope) -> usize {
        self.documents_of(scope).len()
    }

    /// Handle one key press.
    ///
    /// Everything is a key: there is no mouse handling anywhere in this
    /// console, because a terminal tool that needs one is a terminal tool that
    /// cannot be used over `ssh` or by anybody driving a screen reader.
    pub fn on_key(&mut self, key: Key) {
        if let Some(overlay) = self.overlay {
            self.overlay_key(key, overlay);
            return;
        }

        match key {
            Key::Quit => self.quit = true,
            Key::Escape => {}
            Key::NextPane | Key::Right => self.pane = self.pane.next(),
            Key::PreviousPane | Key::Left => self.pane = self.pane.previous(),
            Key::Down => self.step(1),
            Key::Up => self.step(-1),
            Key::PageDown => self.step(10),
            Key::PageUp => self.step(-10),
            Key::First => self.jump(0),
            Key::Last => self.jump(usize::MAX),
            Key::Upstream => self.open_overlay(Overlay::Upstream),
            Key::Help => self.open_overlay(Overlay::Help),
        }
    }

    /// Handle a key while an overlay is up.
    ///
    /// Anything that is not a navigation key closes it, because an overlay a
    /// keystroke cannot dismiss is a trap.
    fn overlay_key(&mut self, key: Key, overlay: Overlay) {
        match key {
            Key::Quit => self.quit = true,
            Key::Upstream if overlay == Overlay::Upstream => self.close_overlay(),
            Key::Help if overlay == Overlay::Help => self.close_overlay(),
            Key::Upstream => self.open_overlay(Overlay::Upstream),
            Key::Help => self.open_overlay(Overlay::Help),
            // The movement keys scroll the overlay rather than dismissing it.
            // An overlay that closed on `↓` would make the registry's `notes`
            // — the longest and most load-bearing text in the console —
            // readable only as far as its first screenful.
            Key::Down => self.scroll_overlay(1),
            Key::Up => self.scroll_overlay(-1),
            Key::PageDown => self.scroll_overlay(10),
            Key::PageUp => self.scroll_overlay(-10),
            Key::First => self.overlay_scroll = 0,
            _ => self.close_overlay(),
        }
    }

    /// Open an overlay at the top.
    fn open_overlay(&mut self, overlay: Overlay) {
        self.overlay = Some(overlay);
        self.overlay_scroll = 0;
    }

    /// Dismiss the overlay.
    fn close_overlay(&mut self) {
        self.overlay = None;
        self.overlay_scroll = 0;
    }

    /// Scroll the overlay, clamped at the top.
    fn scroll_overlay(&mut self, delta: isize) {
        let scrolled = isize::try_from(self.overlay_scroll).unwrap_or(0) + delta;
        self.overlay_scroll = u16::try_from(scrolled.max(0)).unwrap_or(u16::MAX);
    }

    /// Move the selection in the focused pane.
    fn step(&mut self, delta: isize) {
        match self.pane {
            Pane::Specs => {
                self.scope = shift(self.scope, delta, self.scopes.len());
                self.rebuild_rows();
            }
            Pane::Documents => {
                self.row = shift(self.row, delta, self.rows.len());
                self.detail_scroll = 0;
            }
            Pane::Detail => {
                let scrolled = isize::try_from(self.detail_scroll).unwrap_or(0) + delta;
                self.detail_scroll = u16::try_from(scrolled.max(0)).unwrap_or(u16::MAX);
            }
        }
    }

    /// Jump to the first or last row of the focused pane.
    fn jump(&mut self, to: usize) {
        match self.pane {
            Pane::Specs => {
                self.scope = to.min(self.scopes.len().saturating_sub(1));
                self.rebuild_rows();
            }
            Pane::Documents => {
                self.row = to.min(self.rows.len().saturating_sub(1));
                self.detail_scroll = 0;
            }
            Pane::Detail => self.detail_scroll = 0,
        }
    }

    /// Which of the run's documents belong to a scope.
    fn documents_of(&self, scope: Scope) -> Vec<usize> {
        let wanted = match scope {
            Scope::Spec(index) => self.run.specs.get(index).map(|spec| spec.id.clone()),
            Scope::Run => None,
        };
        self.run
            .documents
            .iter()
            .enumerate()
            .filter(|(_, document)| match scope {
                Scope::Spec(_) => document.spec_id == wanted,
                Scope::Run => document.spec_id.is_none(),
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Recompute the documents pane for the selected scope.
    fn rebuild_rows(&mut self) {
        self.documents = self
            .scopes
            .get(self.scope)
            .copied()
            .map(|scope| self.documents_of(scope))
            .unwrap_or_default();

        self.rows = self
            .documents
            .iter()
            .enumerate()
            .flat_map(|(position, index)| {
                let count = self
                    .run
                    .documents
                    .get(*index)
                    .map_or(0, |document| document.report.len());
                std::iter::once(Row::Document(position))
                    .chain((0..count).map(move |d| Row::Diagnostic(position, d)))
            })
            .collect();

        self.row = 0;
        self.detail_scroll = 0;
    }
}

/// Move an index by a signed delta, clamped to a list.
///
/// Clamped rather than wrapping: a list that jumps from its end to its start
/// under `↓` makes "have I seen everything" unanswerable.
fn shift(current: usize, delta: isize, length: usize) -> usize {
    if length == 0 {
        return 0;
    }
    let moved = isize::try_from(current)
        .unwrap_or(isize::MAX)
        .saturating_add(delta);
    let last = isize::try_from(length - 1).unwrap_or(isize::MAX);
    usize::try_from(moved.clamp(0, last)).unwrap_or(0)
}

/// What a key press means, named by intent rather than by key.
///
/// The translation from `crossterm` lives in [`crate::tui`], which keeps this
/// module — and every test of it — free of a terminal library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// Close the console.
    Quit,
    /// Dismiss an overlay.
    Escape,
    /// Focus the pane to the right.
    NextPane,
    /// Focus the pane to the left.
    PreviousPane,
    /// Focus the pane to the left. Distinct from [`Key::PreviousPane`] only in
    /// which physical key produced it.
    Left,
    /// Focus the pane to the right.
    Right,
    /// Next row.
    Down,
    /// Previous row.
    Up,
    /// Ten rows down.
    PageDown,
    /// Ten rows up.
    PageUp,
    /// The first row.
    First,
    /// The last row.
    Last,
    /// Show the selected specification's full provenance record.
    Upstream,
    /// Show the key bindings.
    Help,
}

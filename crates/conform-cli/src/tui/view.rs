//! Drawing the console.
//!
//! Reads an [`App`] and writes a frame. It holds no state of its own and
//! changes none of the app's, so what is on screen is always a function of
//! what the state machine says — which is what makes a rendering test a real
//! test rather than a screenshot.
//!
//! # Everything dynamic is escaped on its way in
//!
//! A `Span` written into a `ratatui` buffer reaches the terminal very nearly
//! verbatim, so this module is as much "the renderer" as the plain-text one
//! is. Every string that came from a document, a bundle or the registry passes
//! through [`escape::for_terminal`] exactly once, here, at the point it
//! becomes a `Span`. Nothing else in the console may write a dynamic string.
//!
//! # No colour-only encoding
//!
//! Severity is a glyph *and* a word, and the drift status is a glyph *and* a
//! word, everywhere both appear. Colour is added on top for the people it
//! helps and carries nothing on its own — plan §4.2 — so the console reads
//! the same on a monochrome terminal, through a screen reader, or for a
//! reader who cannot distinguish red from green.

use conform_core::Severity;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap};

use crate::escape::for_terminal;
use crate::human::{TOOL_NAME, TOOL_VERSION};
use crate::model::{DocumentOutcome, SpecSummary};
use crate::tui::app::{App, Overlay, Pane, Row, Scope};

/// The key bindings: the key, the word the footer has room for, and the full
/// sentence the help overlay shows.
///
/// One list rather than two, so the footer and the overlay cannot drift apart
/// and tell a reader different things about the same keyboard. The short form
/// exists because the long one does not fit: spelled out in full these run to
/// something over two hundred columns, and a footer that is truncated at the
/// right-hand edge loses `[q] quit` — which is the one binding a person who is
/// stuck needs most.
pub const BINDINGS: [(&str, &str, &str); 8] = [
    ("tab", "pane", "move between panes; ← → and h l do the same"),
    ("↑ ↓", "move", "move within a pane; j k do the same"),
    ("pgup/pgdn", "", "ten rows at a time"),
    ("g / G", "", "first / last row"),
    (
        "u",
        "upstream",
        "the selected spec's full upstream record, and its notes",
    ),
    ("?", "keys", "this list"),
    ("esc", "", "close an overlay"),
    ("q", "quit", "quit; ctrl-c does the same"),
];

/// Draw one frame.
pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let screen = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .split(frame.area());
    header(frame, app, screen[0]);
    footer(frame, screen[2]);

    let panes = Layout::horizontal([
        Constraint::Percentage(28),
        Constraint::Percentage(34),
        Constraint::Percentage(38),
    ])
    .split(screen[1]);

    let left = Layout::vertical([Constraint::Min(4), Constraint::Length(9)]).split(panes[0]);
    specs(frame, app, left[0]);
    upstream(frame, app, left[1]);
    documents(frame, app, panes[1]);
    detail(frame, app, panes[2]);

    match app.overlay() {
        Some(Overlay::Upstream) => overlay(
            frame,
            "Upstream record  [↑ ↓] scroll  [u] close",
            upstream_lines(app),
            app.overlay_scroll(),
        ),
        Some(Overlay::Help) => overlay(frame, "Keys  [?] close", help_lines(), 0),
        None => {}
    }
}

/// The title bar: what ran, against which registry, and what it found.
fn header(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let run = app.run();
    let line = Line::from(vec![
        Span::styled(
            format!(" {TOOL_NAME} {TOOL_VERSION} "),
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "{}  ·  {} error / {} warning / {} info  ·  registry ",
            run.command.as_str(),
            run.count(Severity::Error),
            run.count(Severity::Warning),
            run.count(Severity::Info),
        )),
        Span::raw(for_terminal(&run.registry_path).into_owned()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

/// The key hints, always on screen.
fn footer(frame: &mut Frame<'_>, area: Rect) {
    let hints: Vec<String> = BINDINGS
        .iter()
        .filter(|(_, short, _)| !short.is_empty())
        .map(|(key, short, _)| format!("[{key}] {short}"))
        .collect();
    frame.render_widget(
        Paragraph::new(Line::from(format!(" {}", hints.join("  ")))).style(Style::new().dim()),
        area,
    );
}

/// The catalogue.
fn specs(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items: Vec<ListItem<'_>> = app
        .scopes()
        .iter()
        .map(|scope| match scope {
            Scope::Spec(index) => match app.run().specs.get(*index) {
                Some(spec) => spec_item(app, *scope, spec),
                None => ListItem::new("—"),
            },
            Scope::Run => ListItem::new(Line::from(vec![
                Span::raw("· "),
                Span::raw("(the invocation)"),
            ])),
        })
        .collect();

    let mut state = ListState::default().with_selected(Some(app.scope_index()));
    frame.render_stateful_widget(
        List::new(items)
            .block(pane_block(app, Pane::Specs))
            .highlight_symbol("▸ ")
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED)),
        area,
        &mut state,
    );
}

/// One catalogue row: identity, version, drift status, and what was found.
fn spec_item<'a>(app: &App, scope: Scope, spec: &'a SpecSummary) -> ListItem<'a> {
    let worst = app.scope_worst(scope);
    ListItem::new(Line::from(vec![
        Span::styled(
            format!("{} ", spec.verify.glyph()),
            Style::new().fg(verify_colour(spec)),
        ),
        Span::styled(
            format!("{:<6}", for_terminal(&spec.id)),
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "{:<8}",
            for_terminal(spec.version.as_deref().unwrap_or("—"))
        )),
        Span::styled(
            severity_label(worst),
            Style::new().fg(severity_colour(worst)),
        ),
    ]))
}

/// The upstream summary, on screen without a keystroke.
///
/// This block is the "never more than two keystrokes away" requirement, met at
/// zero: the link, the pin and the drift status for whatever is selected are
/// simply always there. `u` then opens the full record, including the
/// registry's `notes` — which is where the evidence for every one of these
/// fields, and the reason for every absence, is written down.
fn upstream(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let block = Block::bordered().title(" Upstream  [u] full record ");
    let Some(spec) = app.selected_spec() else {
        frame.render_widget(
            Paragraph::new("findings about the invocation, not about a specification")
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    };

    let lines = vec![
        labelled("pinned", spec.pinned_ref.as_deref()),
        Line::from(vec![
            Span::styled("bytes    ", Style::new().dim()),
            Span::styled(
                format!(
                    "{} {} ({})",
                    spec.verify.glyph(),
                    spec.verify.as_str(),
                    spec.verify_diagnostic.code.as_str()
                ),
                Style::new().fg(verify_colour(spec)),
            ),
        ]),
        labelled("steward", spec.steward.as_deref()),
        labelled("licence", spec.licence.as_deref()),
        Line::raw(""),
        Line::from(vec![
            Span::raw("▸ "),
            Span::styled(
                for_terminal(spec.upstream_link().unwrap_or("(no link recorded)")).into_owned(),
                Style::new().add_modifier(Modifier::UNDERLINED),
            ),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// What was examined, and what was found in it.
fn documents(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items: Vec<ListItem<'_>> = app
        .rows()
        .iter()
        .map(|row| match row {
            Row::Document(index) => match app.document_at(*index) {
                Some(document) => document_item(document),
                None => ListItem::new("—"),
            },
            Row::Diagnostic(document, diagnostic) => {
                match app.diagnostic_at(*document, *diagnostic) {
                    Some(found) => ListItem::new(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(
                            format!("{} {:<8}", glyph(found.severity), found.severity.as_str()),
                            Style::new().fg(severity_colour(Some(found.severity))),
                        ),
                        Span::raw(for_terminal(found.code.as_str()).into_owned()),
                    ])),
                    None => ListItem::new("—"),
                }
            }
        })
        .collect();

    let empty = items.is_empty();
    let mut state = ListState::default().with_selected(Some(app.row_index()));
    if empty {
        frame.render_widget(
            Paragraph::new("nothing was examined against this specification")
                .block(pane_block(app, Pane::Documents))
                .wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    frame.render_stateful_widget(
        List::new(items)
            .block(pane_block(app, Pane::Documents))
            .highlight_symbol("▸ ")
            .highlight_style(Style::new().add_modifier(Modifier::REVERSED)),
        area,
        &mut state,
    );
}

/// One document header row.
fn document_item(document: &DocumentOutcome) -> ListItem<'_> {
    ListItem::new(Line::from(vec![
        Span::styled(
            format!("{} ", document.glyph()),
            Style::new().fg(severity_colour(document.worst())),
        ),
        Span::styled(
            for_terminal(&document.id).into_owned(),
            Style::new().add_modifier(Modifier::BOLD),
        ),
    ]))
}

/// One diagnostic, in full.
fn detail(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let block = pane_block(app, Pane::Detail);
    let mut lines: Vec<Line<'_>> = Vec::new();

    if let Some(found) = app.selected_diagnostic() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} {} ", glyph(found.severity), found.severity.as_str()),
                Style::new()
                    .fg(severity_colour(Some(found.severity)))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                for_terminal(found.code.as_str()).into_owned(),
                Style::new().add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::raw(""));
        lines.push(Line::raw(for_terminal(&found.message).into_owned()));
        if let Some(help) = &found.help {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                format!("help: {}", for_terminal(help)),
                Style::new().dim(),
            ));
        }
        lines.push(Line::raw(""));
        lines.push(labelled(
            "document",
            Some(for_terminal(found.location.document.as_str()).as_ref()),
        ));
        lines.push(labelled(
            "line",
            found.location.line.map(|l| l.to_string()).as_deref(),
        ));
        lines.push(labelled(
            "column",
            found.location.column.map(|c| c.to_string()).as_deref(),
        ));
        lines.push(labelled(
            "pointer",
            found
                .location
                .pointer
                .as_ref()
                .map(|p| for_terminal(p.as_str()).into_owned())
                .as_deref(),
        ));
        lines.push(labelled(
            "spec",
            found
                .spec_ref
                .as_ref()
                .map(|s| for_terminal(&s.to_string()).into_owned())
                .as_deref(),
        ));
    } else if let Some(document) = app.selected_document() {
        lines.push(Line::styled(
            for_terminal(&document.id).into_owned(),
            Style::new().add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::raw(""));
        for severity in [Severity::Error, Severity::Warning, Severity::Info] {
            lines.push(labelled(
                severity.as_str(),
                Some(&document.report.count(severity).to_string()),
            ));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "select a finding below this document to see it in full",
            Style::new().dim(),
        ));
    } else {
        lines.push(Line::styled("nothing selected", Style::new().dim()));
    }

    if let Some(spec) = app.selected_spec() {
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("▸ ", Style::new().dim()),
            Span::styled(
                for_terminal(spec.upstream_link().unwrap_or("(no link recorded)")).into_owned(),
                Style::new().add_modifier(Modifier::UNDERLINED),
            ),
        ]));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((app.detail_scroll(), 0)),
        area,
    );
}

/// The full provenance record for the selected specification.
fn upstream_lines(app: &App) -> Vec<Line<'static>> {
    let Some(spec) = app.selected_spec() else {
        return vec![Line::raw(
            "these findings are about the invocation, not about a specification",
        )];
    };

    let mut lines = vec![
        Line::styled(
            format!("{}  {}", for_terminal(&spec.id), for_terminal(&spec.name)),
            Style::new().add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        labelled("version", spec.version.as_deref()),
        labelled("pinned_ref", spec.pinned_ref.as_deref()),
        labelled("homepage", spec.homepage.as_deref()),
        labelled("repository", spec.repository.as_deref()),
        labelled("steward", spec.steward.as_deref()),
        labelled("licence", spec.licence.as_deref()),
        Line::raw(""),
        labelled("vendored", Some(&spec.vendored_path)),
        labelled("sha256", Some(&spec.sha256)),
        labelled("fetched_at", Some(&spec.fetched_at)),
        labelled(
            "bytes",
            Some(&format!(
                "{} ({})",
                spec.verify.as_str(),
                spec.verify_diagnostic.code.as_str()
            )),
        ),
    ];

    if !spec.provenance_gaps.is_empty() {
        lines.push(labelled("gaps", Some(&spec.provenance_gaps.join(", "))));
    }

    if let Some(notes) = &spec.notes {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "notes — what was looked at, and what it did not say",
            Style::new().add_modifier(Modifier::BOLD),
        ));
        for paragraph in notes.split('\n') {
            lines.push(Line::raw(for_terminal(paragraph).into_owned()));
        }
    }
    lines
}

/// The key bindings, as the overlay lists them.
fn help_lines() -> Vec<Line<'static>> {
    BINDINGS
        .iter()
        .map(|(key, _, what)| {
            Line::from(vec![
                Span::styled(
                    format!("{key:<12}"),
                    Style::new().add_modifier(Modifier::BOLD),
                ),
                Span::raw(*what),
            ])
        })
        .collect()
}

/// Draw a centred overlay over the panes.
fn overlay(frame: &mut Frame<'_>, title: &str, lines: Vec<Line<'static>>, scroll: u16) {
    let area = centred(frame.area(), 80, 80);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered().title(format!(" {title} ")))
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
}

/// A rectangle covering a percentage of another, centred.
fn centred(area: Rect, width: u16, height: u16) -> Rect {
    let rows = Layout::vertical([
        Constraint::Percentage((100 - height) / 2),
        Constraint::Percentage(height),
        Constraint::Percentage((100 - height) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - width) / 2),
        Constraint::Percentage(width),
        Constraint::Percentage((100 - width) / 2),
    ])
    .split(rows[1])[1]
}

/// A pane's border, titled, and marked when it has the keyboard.
fn pane_block(app: &App, pane: Pane) -> Block<'static> {
    let focused = app.pane() == pane;
    let title = if focused {
        format!(" ▸ {} ", pane.title())
    } else {
        format!(" {} ", pane.title())
    };
    let block = Block::bordered().title(title);
    if focused {
        block.border_style(Style::new().add_modifier(Modifier::BOLD))
    } else {
        block.border_style(Style::new().dim())
    }
}

/// A `label  value` line, where an absent value says so.
///
/// `(not recorded)` rather than a blank, for the same reason the plain-text
/// catalogue says it: a blank reads as a rendering accident, and the words are
/// the registry's actual answer.
fn labelled(label: &str, value: Option<&str>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<11}"), Style::new().dim()),
        match value {
            Some(value) => Span::raw(for_terminal(value).into_owned()),
            None => Span::styled("(not recorded)", Style::new().dim()),
        },
    ])
}

/// A severity's glyph. Always shown beside the word, never instead of it.
const fn glyph(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "✗",
        Severity::Warning => "⚠",
        Severity::Info => "·",
    }
}

/// A scope's worst severity, as a word.
fn severity_label(worst: Option<Severity>) -> String {
    worst.map_or_else(|| "clean".to_owned(), |s| format!("{} {}", glyph(s), s))
}

/// Colour, carrying nothing a glyph and a word do not already carry.
const fn severity_colour(worst: Option<Severity>) -> Color {
    match worst {
        Some(Severity::Error) => Color::Red,
        Some(Severity::Warning) => Color::Yellow,
        Some(Severity::Info) => Color::Cyan,
        None => Color::Green,
    }
}

/// Likewise for drift.
const fn verify_colour(spec: &SpecSummary) -> Color {
    match spec.verify {
        crate::model::VerifyStatus::Matched => Color::Green,
        crate::model::VerifyStatus::Drifted => Color::Red,
        crate::model::VerifyStatus::Missing | crate::model::VerifyStatus::Unreadable => {
            Color::Yellow
        }
    }
}

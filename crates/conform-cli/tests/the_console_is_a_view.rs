//! The console shows the run, and only the run — and escapes what it shows.
//!
//! Two claims, and both of them are the kind that quietly stops being true:
//!
//! 1. **The TUI computes nothing.** It renders the same [`Run`] the human
//!    report and the `--json` envelope render. A console that validated for
//!    itself could tell an operator a contract is conformant while `--json`
//!    says it is not, and the operator would have no way to know which to
//!    believe.
//! 2. **The TUI escapes.** It is a terminal, so the obligation the adapters
//!    hand to "the renderer" lands on it exactly as it lands on the plain-text
//!    path. A hostile bundle must not be able to recolour, retitle or reorder
//!    a pane any more than it can a line of stdout.
//!
//! Both are held here against `ratatui`'s `TestBackend`, which renders into an
//! in-memory buffer — so this reads the real cells the real widgets produced,
//! not a paraphrase of what they were asked to draw.

use std::fmt::Write as _;

use conform_cli::engine::{self, Request};
use conform_cli::model::{Command, Run};
use conform_cli::tui::app::{App, Key, Overlay, Pane};
use conform_cli::tui::view;
use conform_core::GatePolicy;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

mod support;

use support::{scratch, specs_toml, write};

/// U+202E, an ANSI colour sequence, an OSC window-title sequence and a
/// zero-width space — the same payload the plain-text test uses.
const HOSTILE: &str = "act\u{202e}evi\u{1b}[31mtcani\u{1b}]0;pwned\u{7}ve\u{200b}d";

/// A run over the given paths, built exactly as the binary builds it.
fn run_over(paths: Vec<String>) -> Run {
    run_over_spec(paths, None)
}

/// The same, with `--spec` set.
fn run_over_spec(paths: Vec<String>, spec: Option<&str>) -> Run {
    engine::run(&Request {
        command: Command::Validate,
        paths: paths.into_iter().map(Into::into).collect(),
        spec: spec.map(str::to_owned),
        policy: GatePolicy::report_only(),
        registry: Some(specs_toml()),
    })
}

/// A run with no documents at all — the console's own default, which opens on
/// the catalogue and what re-hashing the vendored bytes just said.
fn catalogue_run() -> Run {
    engine::run(&Request {
        command: Command::RegistryVerify,
        paths: Vec::new(),
        spec: None,
        policy: GatePolicy::report_only(),
        registry: Some(specs_toml()),
    })
}

/// Render one frame and return every cell as text, row by row.
fn frame(app: &App) -> String {
    let mut terminal =
        Terminal::new(TestBackend::new(200, 60)).expect("a test backend always constructs");
    terminal
        .draw(|frame| view::draw(frame, app))
        .expect("drawing into memory cannot fail");

    let buffer = terminal.backend().buffer().clone();
    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        let _ = writeln!(text);
    }
    text
}

/// Every frame reachable by walking the documents pane from top to bottom.
fn every_frame(app: &mut App) -> Vec<String> {
    app.on_key(Key::NextPane);
    assert_eq!(app.pane(), Pane::Documents);

    let mut frames = vec![frame(app)];
    for _ in 0..app.rows().len() {
        app.on_key(Key::Down);
        frames.push(frame(app));
    }
    frames
}

#[test]
fn a_hostile_document_cannot_reach_the_panes() {
    let directory = scratch("tui-hostile");
    let path = write(
        &directory,
        "orders.yaml",
        &format!(
            "version: 1.0.0\napiVersion: v3.1.0\nkind: DataContract\n\
             id: 53581432-6c55-4ba2-a65f-72344a91553a\nstatus: {}\n",
            yaml_quoted(HOSTILE)
        ),
    );

    let mut app = App::new(run_over(vec![path]), None);
    let frames = every_frame(&mut app);

    for rendered in &frames {
        for character in ['\u{202e}', '\u{1b}', '\u{7}', '\u{200b}'] {
            assert!(
                !rendered.contains(character),
                "U+{:04X} reached the console's buffer",
                character as u32
            );
        }
    }

    // And it is shown rather than dropped: somewhere in the walk, the escaped
    // form is on screen. Otherwise this test would pass against a console that
    // simply never displayed the finding.
    assert!(
        frames.iter().any(|f| f.contains("<U+202E>")),
        "the payload was never displayed at all, escaped or otherwise"
    );
    assert!(frames.iter().any(|f| f.contains("<U+001B>")));
}

#[test]
fn the_upstream_link_and_the_pin_are_on_screen_without_a_keystroke() {
    // The requirement is "never more than two keystrokes away". This is the
    // stronger thing: for the specification selected when the console opens,
    // the link, the pin and the drift status are simply there.
    let app = App::new(catalogue_run(), None);
    let rendered = frame(&app);

    let spec = app.selected_spec().expect("the console opens on a spec");
    assert!(
        rendered.contains(spec.upstream_link().expect("odcs records a homepage")),
        "the upstream link is not on the opening frame:\n{rendered}"
    );
    assert!(rendered.contains(spec.pinned_ref.as_deref().expect("odcs is pinned")));
    assert!(rendered.contains("matched"));
}

#[test]
fn one_keystroke_opens_the_full_provenance_record() {
    let mut app = App::new(catalogue_run(), None);
    app.on_key(Key::Upstream);
    assert_eq!(app.overlay(), Some(Overlay::Upstream));

    let rendered = frame(&app);
    let spec = app.selected_spec().expect("a spec is selected").clone();
    assert!(rendered.contains(&spec.sha256[..16]), "{rendered}");
    assert!(rendered.contains(&spec.vendored_path));
    assert!(rendered.contains(&spec.fetched_at));

    // The absence, spelled out. `odcs` records no licence, and the console
    // must say so rather than leave a blank that reads as a drawing bug.
    assert!(
        spec.licence.is_none(),
        "this test assumes odcs has no licence"
    );
    assert!(rendered.contains("(not recorded)"));

    // And the evidence for every field, which is what `notes` is for. It runs
    // to more than one screenful, so it has to be scrollable — an overlay that
    // showed only its first page would hide the reason for every absence
    // above.
    assert!(rendered.contains("Version evidence"), "{rendered}");
    assert_eq!(app.overlay_scroll(), 0);
    app.on_key(Key::PageDown);
    assert_eq!(app.overlay_scroll(), 10, "`↓` must scroll, not dismiss");
    assert_eq!(app.overlay(), Some(Overlay::Upstream));
    assert_ne!(frame(&app), rendered, "scrolling changed nothing on screen");

    // An overlay a keystroke cannot dismiss is a trap.
    app.on_key(Key::Escape);
    assert_eq!(app.overlay(), None);
    assert_eq!(app.overlay_scroll(), 0);
}

#[test]
fn every_specification_is_reachable_and_carries_its_link() {
    let mut app = App::new(catalogue_run(), None);
    let count = app.run().specs.len();
    assert_eq!(count, 5);

    for _ in 0..count {
        let spec = app.selected_spec().expect("a spec row").clone();
        let rendered = frame(&app);
        assert!(
            rendered.contains(spec.upstream_link().expect("every entry records one")),
            "`{}` shows no upstream link",
            spec.id
        );
        app.on_key(Key::Down);
    }

    // Clamped, not wrapping: pressing `↓` at the bottom must not silently
    // return to the top, or "have I seen them all" becomes unanswerable.
    app.on_key(Key::Down);
    app.on_key(Key::Down);
    assert_eq!(app.scope_index(), count - 1);
}

#[test]
fn navigation_needs_no_mouse() {
    let mut app = App::new(catalogue_run(), None);
    assert_eq!(app.pane(), Pane::Specs);

    app.on_key(Key::NextPane);
    assert_eq!(app.pane(), Pane::Documents);
    app.on_key(Key::NextPane);
    assert_eq!(app.pane(), Pane::Detail);
    app.on_key(Key::NextPane);
    assert_eq!(app.pane(), Pane::Specs, "the panes cycle");

    app.on_key(Key::PreviousPane);
    assert_eq!(app.pane(), Pane::Detail);

    // Arrows and the vi keys reach the same intents, so neither habit is
    // second class. (`h`/`l`/`j`/`k` are translated in `tui::translate`.)
    app.on_key(Key::Left);
    assert_eq!(app.pane(), Pane::Documents);
    app.on_key(Key::Right);
    assert_eq!(app.pane(), Pane::Detail);

    app.on_key(Key::Help);
    assert_eq!(app.overlay(), Some(Overlay::Help));
    assert!(frame(&app).contains("ctrl-c does the same"));
    app.on_key(Key::Help);
    assert_eq!(app.overlay(), None);

    app.on_key(Key::Quit);
    assert!(app.should_quit());
}

#[test]
fn the_console_shows_the_run_and_never_a_second_opinion() {
    // Every diagnostic in the run is reachable in the console, and every
    // diagnostic reachable in the console is in the run. A console that
    // derived its own findings would fail this in one direction or the other.
    let contracts = support::in_workspace("crates/conform-odcs/tests/fixtures");
    let products = support::in_workspace("crates/conform-odps/tests/fixtures");
    let lexicon = support::in_workspace("crates/conform-lexicon/tests/fixtures");
    let bundle =
        support::in_workspace("crates/conform-okf/tests/fixtures/okf-upstream/acme_retail");
    let run = run_over(vec![contracts, products, lexicon, bundle]);

    let expected: Vec<String> = run
        .documents
        .iter()
        .flat_map(|document| document.report.iter())
        .map(|found| format!("{} {} {}", found.severity, found.code, found.location))
        .collect();
    assert!(
        expected.len() > 100,
        "only {} diagnostics across four standards; the corpus has collapsed",
        expected.len()
    );
    assert!(
        expected.iter().any(|found| found.contains(" ODCL")),
        "the corpus carries no ODCL diagnostic, so this test would not notice the console \
         dropping every one of them"
    );

    let mut app = App::new(run, None);
    let mut reached: Vec<String> = Vec::new();
    loop {
        // Walk every scope, and every row within it.
        app.on_key(Key::NextPane);
        for _ in 0..=app.rows().len() {
            if let Some(found) = app.selected_diagnostic() {
                reached.push(format!(
                    "{} {} {}",
                    found.severity, found.code, found.location
                ));
            }
            app.on_key(Key::Down);
        }
        app.on_key(Key::PreviousPane);

        let before = app.scope_index();
        app.on_key(Key::Down);
        if app.scope_index() == before {
            break;
        }
    }

    let mut expected_sorted = expected;
    let mut reached_sorted = reached;
    expected_sorted.sort();
    reached_sorted.sort();
    reached_sorted.dedup();
    expected_sorted.dedup();
    assert_eq!(reached_sorted, expected_sorted);
}

/// The payload as a YAML double-quoted scalar. See the plain-text test for why
/// it is written through YAML's own escapes.
fn yaml_quoted(text: &str) -> String {
    let mut quoted = String::from("\"");
    for character in text.chars() {
        if character.is_ascii_graphic() && character != '"' && character != '\\' {
            quoted.push(character);
        } else {
            let _ = write!(quoted, "\\u{:04X}", character as u32);
        }
    }
    quoted.push('"');
    quoted
}

#[test]
fn the_odcl_scope_is_reachable_and_carries_its_documents() {
    // `--spec odcl` opens the console on that entry, and walking the documents
    // pane reaches the findings the adapter raised. Before the adapter was
    // wired in the same walk reached a scope with nothing in it, which is the
    // shape of a specification the console shows and cannot verify.
    let lexicon = support::in_workspace("crates/conform-lexicon/tests/fixtures");
    let run = run_over_spec(vec![lexicon], Some("odcl"));

    let mut app = App::new(run, Some("odcl"));
    let spec = app
        .selected_spec()
        .expect("the console opens on odcl")
        .clone();
    assert_eq!(spec.id, "odcl");
    assert!(spec.has_adapter, "odcl must now report a validator");

    // The provenance the spec pane promises is on the opening frame, and the
    // three fields `specs.toml` leaves out say so rather than showing a blank.
    let opening = frame(&app);
    assert!(
        opening.contains(spec.upstream_link().expect("odcl records a repository")),
        "{opening}"
    );
    assert!(opening.contains(spec.pinned_ref.as_deref().expect("odcl is pinned")));
    assert!(opening.contains("matched"));

    app.on_key(Key::Upstream);
    let record = frame(&app);
    assert!(spec.licence.is_none() && spec.homepage.is_none() && spec.steward.is_none());
    assert!(record.contains("(not recorded)"), "{record}");
    app.on_key(Key::Escape);

    // And the documents are there, with ODCL codes on them.
    app.on_key(Key::NextPane);
    assert_eq!(app.pane(), Pane::Documents);
    let mut codes: Vec<String> = Vec::new();
    for _ in 0..=app.rows().len() {
        if let Some(found) = app.selected_diagnostic() {
            codes.push(found.code.to_string());
        }
        app.on_key(Key::Down);
    }
    assert!(
        codes.iter().any(|code| code.starts_with("ODCL")),
        "the odcl scope holds no ODCL diagnostic at all"
    );
    assert!(
        codes.iter().any(|code| code == "ODCL101"),
        "expected the required-property errors from the faulty fixtures, got {codes:?}"
    );
}

#[test]
fn a_hostile_odcl_document_cannot_reach_the_panes() {
    // The TUI's half of the escaping obligation, for the adapter that has just
    // been wired in. `conform-lexicon` states in as many words that it does
    // not escape; the cells are where that has to be made good.
    let directory = scratch("tui-hostile-lexicon");
    let path = write(
        &directory,
        "legacy.yaml",
        &format!(
            "dataContractSpecification: 1.2.1\n\
             id: urn:datacontract:checkout:orders\n\
             info:\n  title: Orders\n  version: 1.0.0\n  status: {}\n",
            yaml_quoted(HOSTILE)
        ),
    );

    // No `--spec`, so the console opens on the first entry and the operator
    // walks to `odcl` — which is the navigation this test is also checking.
    let mut app = App::new(run_over(vec![path]), None);
    while app.selected_spec().is_some_and(|spec| spec.id != "odcl") {
        let before = app.scope_index();
        app.on_key(Key::Down);
        assert_ne!(app.scope_index(), before, "the `odcl` scope is unreachable");
    }
    assert_eq!(app.selected_spec().expect("a spec row").id, "odcl");

    let frames = every_frame(&mut app);

    for rendered in &frames {
        for character in ['\u{202e}', '\u{1b}', '\u{7}', '\u{200b}'] {
            assert!(
                !rendered.contains(character),
                "U+{:04X} reached the console's buffer from an ODCL document",
                character as u32
            );
        }
    }

    assert!(
        frames.iter().any(|f| f.contains("<U+202E>")),
        "the payload was never displayed at all, escaped or otherwise"
    );
    assert!(frames.iter().any(|f| f.contains("<U+001B>")));
}

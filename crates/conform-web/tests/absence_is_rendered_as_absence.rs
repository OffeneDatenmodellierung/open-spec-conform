//! A field nobody could determine is shown as an answer, not as a blank.
//!
//! # The rule, and why a test rather than care
//!
//! `specs.toml` omits `licence` on three of its seven entries and `homepage` on
//! three, and each entry's notes say what was looked at and what it did not
//! say. That is the registry's central idea: a recorded unknown is usable
//! evidence, and a plausible-looking value nobody checked is worse than
//! silence because it reads as verified.
//!
//! A renderer undoes that in one line. `unwrap_or_default()` turns every gap
//! into an empty cell, and an empty cell reads as a rendering accident rather
//! than as the registry's answer. The console prints `(not recorded)` and this
//! page must say the same words, because a reader who has seen them in
//! `conform registry list` must not have to work out that a blank here means
//! the same thing.

use conform_registry::Registry;
use conform_web::{Site, render};

/// The console's words, which this page must use.
///
/// Read from `conform-cli`'s own rendering rather than typed here, so that the
/// two cannot drift apart: if the console ever changes its wording, this test
/// fails and the page is changed with it.
fn not_recorded() -> String {
    let registry = Registry::load_str(ALL_ABSENT, "<absent>").expect("the fixture is a registry");
    let entry = &registry.entries()[0];
    let summary = conform_cli::model::SpecSummary::new(entry, registry.verify_entry(0, entry));

    let mut rendered = Vec::new();
    let run = conform_cli::model::Run {
        command: conform_cli::model::Command::RegistryList,
        policy: conform_core::GatePolicy::default(),
        registry_path: "<absent>".to_owned(),
        registry_origin: conform_cli::model::RegistryOrigin::Explicit("<absent>".into()),
        specs: vec![summary],
        documents: Vec::new(),
        unusable: false,
    };
    conform_cli::human::render(&run, &mut rendered).expect("the console renders to a buffer");
    let text = String::from_utf8(rendered).expect("the console writes UTF-8");

    // The console prints the phrase for each absent field; lift it back out so
    // this test never transcribes it.
    let start = text
        .find("(not")
        .expect("the console reports an absence for an entry with absent fields");
    let end = text[start..]
        .find(')')
        .expect("the console's absence marker is parenthesised")
        + start
        + 1;
    text[start..end].to_owned()
}

/// A registry entry with every optional provenance field left out.
const ALL_ABSENT: &str = r#"
schema_version = 1

[[spec]]
id            = "bare"
name          = "Bare Entry"
vendored_path = "nowhere/bare.json"
sha256        = "6666666666666666666666666666666666666666666666666666666666666666"
fetched_at    = "2001-02-03"
notes         = "Nothing optional is recorded for this entry, on purpose."
"#;

/// The catalogue section of a page, and nothing else.
///
/// Scoping matters more here than it looks. The page's opening section
/// *explains* the convention by showing the marker, and the stylesheet names a
/// colour `--unknown`; both are the renderer's own words about absence rather
/// than a rendered absence, and an assertion that counted them would be
/// counting the explanation as though it were the thing explained. Every
/// assertion below is therefore made against the cards.
fn catalogue(page: &str) -> &str {
    let start = page
        .find("<section id=\"specs\">")
        .expect("the page has a catalogue section");
    let rest = &page[start..];
    let end = rest
        .find("</section>")
        .expect("the catalogue section closes");
    &rest[..end]
}

/// Render the page from a registry in which nothing optional is recorded.
fn bare_page() -> String {
    let registry = Registry::load_str(ALL_ABSENT, "<absent>").expect("the fixture is a registry");
    let site = Site::from_parts(&registry, Vec::new(), "absent.toml".to_owned());
    render::page(&site)
}

#[test]
fn the_page_uses_the_consoles_words_for_an_absence() {
    let phrase = not_recorded();
    assert_eq!(
        phrase, "(not recorded)",
        "the console's absence marker changed; this page must follow it",
    );
    assert!(
        catalogue(&bare_page()).contains(&phrase),
        "the catalogue does not use the console's words for an absent field",
    );
}

#[test]
fn every_absent_field_is_marked_rather_than_blank() {
    let page = bare_page();
    let phrase = not_recorded();

    // Six optional fields on the card are absent in this fixture — version,
    // steward, licence, pinned ref, homepage, repository — and each must
    // produce a visible marker. An empty `<dd></dd>` or an empty cell is the
    // failure this guards against.
    let markers = catalogue(&page).matches(&phrase).count();
    assert_eq!(
        markers, 6,
        "the card has six absent fields and marks {markers} of them",
    );

    assert!(
        !page.contains("<dd></dd>"),
        "an empty definition cell reached the page; a blank reads as a rendering accident",
    );
    assert!(
        !page.contains("<td></td>"),
        "an empty table cell reached the page",
    );
    assert!(
        !page.contains("<code></code>"),
        "an empty code element reached the page",
    );
}

#[test]
fn an_absence_is_never_a_plausible_looking_default() {
    let page = bare_page();
    let cards = catalogue(&page);
    // The failure this rule exists to prevent is not a blank — it is a guess.
    // Nothing in the fixture records a licence, a steward, a homepage or a
    // pin, so none of these may appear.
    for invented in [
        "MIT",
        "Apache-2.0",
        "unknown",
        "N/A",
        "n/a",
        "TBD",
        "main",
        "HEAD",
    ] {
        assert!(
            !cards.contains(invented),
            "{invented:?} appears on a page rendered from an entry that records none of it",
        );
    }
}

#[test]
fn the_gap_list_names_every_absent_field() {
    let cards = catalogue(&bare_page()).to_owned();
    // `SpecEntry::provenance_gaps` is the registry's own answer to "what is
    // missing", and the page shows it rather than recomputing it.
    for field in ["homepage", "repository", "steward", "licence", "pinned_ref"] {
        assert!(
            cards.contains(field),
            "the gap list does not name {field:?}, so a reader cannot see what is missing",
        );
    }
}

#[test]
fn the_real_registrys_gaps_all_reach_the_page() {
    // The control that keeps this from being a fixture-only claim: the
    // repository's own registry has gaps, and every one of them is on the
    // page.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/conform-web is two levels below the root");
    let site = Site::gather(root).expect("this repository's registry reads");
    let page = render::page(&site);

    // Exactly, not "at least". The card renders six optional values, five of
    // which `provenance_gaps` counts — `version` is not a provenance field, so
    // the registry does not count it and this test adds it — plus the notes.
    // An exact number is what catches an absence quietly rendered as a blank;
    // `>=` would pass while one went missing.
    let expected: usize = site
        .specs
        .iter()
        .map(|card| {
            card.summary.provenance_gaps.len()
                + usize::from(card.summary.version.is_none())
                + usize::from(card.summary.notes.is_none())
        })
        .sum();
    assert!(
        expected >= 5,
        "the real registry records {expected} absences; this test assumes it has several",
    );

    let marked = catalogue(&page).matches(&not_recorded()).count();
    assert_eq!(
        marked, expected,
        "the registry records {expected} absences in its catalogue and the page marks \
         {marked}; an absence was rendered as a blank, or a present value as absent",
    );
}

#[test]
fn a_present_field_is_not_marked_absent() {
    // The other direction, which a test that only looked for the phrase would
    // miss entirely: a renderer that printed `(not recorded)` for everything
    // would pass every assertion above.
    let present = r#"
schema_version = 1

[[spec]]
id            = "full"
name          = "Fully Recorded Entry"
version       = "7.7.7"
homepage      = "https://full.example.invalid/docs"
repository    = "https://full.example.invalid/src"
steward       = "The Full Board"
licence       = "Zlib"
pinned_ref    = "v7.7.7"
vendored_path = "nowhere/full.json"
sha256        = "7777777777777777777777777777777777777777777777777777777777777777"
fetched_at    = "2002-03-04"
notes         = "Everything is recorded for this entry."

  [spec.poll]
  endpoint = "https://full.example.invalid/poll"
  method   = "full-release"
  cadence  = "daily"
"#;
    let registry = Registry::load_str(present, "<full>").expect("the fixture is a registry");
    let site = Site::from_parts(&registry, Vec::new(), "full.toml".to_owned());
    let page = render::page(&site);

    assert!(
        !catalogue(&page).contains(&not_recorded()),
        "an entry that records everything still shows an absence marker",
    );
    for value in [
        "The Full Board",
        "Zlib",
        "v7.7.7",
        "https://full.example.invalid/docs",
    ] {
        assert!(page.contains(value), "the page does not show {value:?}");
    }
}

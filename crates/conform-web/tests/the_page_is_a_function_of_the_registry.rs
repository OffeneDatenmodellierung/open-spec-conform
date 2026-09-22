//! The page is a function of the registry, and this is the proof.
//!
//! # What this test is for
//!
//! The central claim of this crate is that **nothing about a specification is
//! hand-typed into the site**. A test that scanned the source for a URL would
//! be evidence for it, and `no_spec_facts_are_written_in_the_source.rs` is
//! that test — but scanning is a negative check, and a negative check can be
//! satisfied by a renderer that hard-codes a *name* instead of a URL, or by a
//! scanner looking in the wrong directory.
//!
//! This test is the positive one, and it cannot be satisfied by accident.
//! It renders the page from a registry in which **every value is fabricated**
//! and asserts two things:
//!
//! - every fabricated value appears in the rendered page, so the renderer
//!   really did read the registry it was handed; and
//! - **no** value from the real `specs.toml` appears anywhere, so the renderer
//!   has no second source it is quietly falling back to.
//!
//! Anything transcribed into the source survives the substitution and shows up
//! in the second assertion. There is no way to write a renderer with a link
//! typed into it that passes this.

use conform_registry::Registry;
use conform_web::manifests::CrateEntry;
use conform_web::{Site, render};

/// A registry with nothing real in it.
///
/// Deliberately not a copy of `specs.toml` with the values edited: the shape
/// exercises the branches that matter — an entry with every field present, an
/// entry with three of them absent, a moving-target-free pin, a poll and a
/// missing poll — while sharing not one byte with the real file.
const FABRICATED: &str = r#"
schema_version = 1

[[spec]]
id            = "zzfirst"
name          = "Fabricated Standard For Testing"
version       = "9.9.9"
homepage      = "https://fabricated.example.invalid/docs/"
repository    = "https://fabricated.example.invalid/source"
steward       = "The Fabrication Board"
licence       = "BSD-2-Clause"
pinned_ref    = "v9.9.9-fabricated"
vendored_path = "nowhere/fabricated-first.json"
sha256        = "1111111111111111111111111111111111111111111111111111111111111111"
fetched_at    = "1999-12-31"
notes         = "Every value in this entry is fabricated."

  [spec.poll]
  endpoint = "https://fabricated.example.invalid/api/releases"
  method   = "fabricated-release"
  field    = "tag_name"
  cadence  = "fortnightly"

[[spec]]
id            = "zzsecond"
name          = "Second Fabricated Standard"
version       = "0.0.1"
repository    = "https://second.example.invalid/source"
pinned_ref    = "2222222222222222222222222222222222222222"
vendored_path = "nowhere/fabricated-second.json"
sha256        = "3333333333333333333333333333333333333333333333333333333333333333"
fetched_at    = "1999-01-01"
notes         = "homepage, steward and licence are absent on purpose."
"#;

/// Every fabricated fact, as it should appear on the page.
const FABRICATED_FACTS: &[&str] = &[
    "zzfirst",
    "Fabricated Standard For Testing",
    "9.9.9",
    "https://fabricated.example.invalid/docs/",
    "https://fabricated.example.invalid/source",
    "The Fabrication Board",
    "BSD-2-Clause",
    "v9.9.9-fabricated",
    "nowhere/fabricated-first.json",
    "1111111111111111111111111111111111111111111111111111111111111111",
    "1999-12-31",
    "https://fabricated.example.invalid/api/releases",
    "fabricated-release",
    "fortnightly",
    "zzsecond",
    "Second Fabricated Standard",
    "https://second.example.invalid/source",
    "2222222222222222222222222222222222222222",
    "3333333333333333333333333333333333333333333333333333333333333333",
    "1999-01-01",
];

/// Render the page from the fabricated registry.
fn fabricated_page() -> String {
    let registry =
        Registry::load_str(FABRICATED, "<fabricated>").expect("the fixture is a registry");
    let site = Site::from_parts(&registry, fabricated_crates(), "fabricated.toml".to_owned());
    render::page(&site)
}

/// A crate family with nothing real in it either.
fn fabricated_crates() -> Vec<CrateEntry> {
    vec![CrateEntry {
        name: "fabricated-crate".to_owned(),
        version: Some("42.0.0".to_owned()),
        description: Some("A crate that does not exist.".to_owned()),
        published: false,
        manifest_path: "nowhere/Cargo.toml".to_owned(),
    }]
}

#[test]
fn every_fabricated_fact_reaches_the_page() {
    let page = fabricated_page();
    for fact in FABRICATED_FACTS {
        assert!(
            page.contains(fact),
            "the page does not show {fact:?}, so the renderer is not reading the registry it \
             was handed",
        );
    }
}

#[test]
fn the_crate_table_is_read_rather_than_written() {
    let page = fabricated_page();
    for fact in ["fabricated-crate", "42.0.0", "A crate that does not exist."] {
        assert!(page.contains(fact), "the page does not show {fact:?}");
    }
}

#[test]
fn no_fact_from_the_real_registry_survives_the_substitution() {
    // The assertion that makes this test worth having. Every one of these is a
    // value in this repository's own `specs.toml`; a renderer with any of them
    // written into it renders them whatever registry it is handed, and is
    // caught here.
    let page = fabricated_page();
    let real = real_registry();

    let mut leaked = Vec::new();
    for (index, entry) in real.entries().iter().enumerate() {
        let mut facts = vec![entry.id.clone(), entry.name.clone()];
        facts.extend(
            [
                entry.homepage.clone(),
                entry.repository.clone(),
                entry.steward.clone(),
                entry.licence.clone(),
            ]
            .into_iter()
            .flatten(),
        );
        for version in &entry.versions {
            facts.push(version.sha256.clone());
            facts.push(version.vendored_path.clone());
            facts.extend(version.version.clone());
            facts.extend(version.pinned_ref.clone());
            if let Some(poll) = &version.poll {
                facts.push(poll.endpoint.clone());
            }
        }

        for fact in facts {
            // `MIT` and `1.0` are real values in the real registry and are
            // also too short to mean anything as substrings — `MIT` matches
            // inside a licence header, `1.0` inside a version of something
            // else. Anything this short is checked by the source scan in
            // `no_spec_facts_are_written_in_the_source.rs` instead, which
            // looks at whole tokens rather than substrings.
            if fact.len() < 8 {
                continue;
            }
            if page.contains(&fact) {
                leaked.push(format!("spec[{index}] ({}) leaked {fact:?}", entry.id));
            }
        }
    }

    assert!(
        leaked.is_empty(),
        "the page shows facts from the real registry when rendered from a fabricated one, so \
         they are written into the source rather than read:\n  {}",
        leaked.join("\n  "),
    );
}

#[test]
fn the_substitution_check_is_not_vacuous() {
    // The control. If the scan above can pass because the real registry has no
    // facts in it, or because `contains` is looking at an empty page, it is
    // worth nothing. So: the real registry has entries, those entries have
    // long facts, and the *real* page does contain them.
    let real = real_registry();
    assert!(
        real.entries().len() >= 2,
        "the real registry must hold entries for the leak check to mean anything",
    );

    let real_site = Site::gather(repository_root()).expect("the repository's own registry reads");
    let real_page = render::page(&real_site);

    let mut checked = 0_usize;
    for entry in real.entries() {
        for fact in [entry.homepage.as_deref(), entry.repository.as_deref()]
            .into_iter()
            .flatten()
        {
            assert!(
                real_page.contains(fact),
                "the real page should show {fact:?}; if it does not, the leak check above is \
                 comparing against facts that never reach a page and can never fail",
            );
            checked += 1;
        }
        for version in &entry.versions {
            assert!(
                real_page.contains(&version.sha256),
                "the real page should show the digest for `{}`",
                entry.id,
            );
            checked += 1;
        }
    }

    assert!(
        checked >= 8,
        "only {checked} facts were confirmed to reach the real page; the control is too weak \
         to license the leak check",
    );

    // And the two pages really are different documents, which rules out the
    // degenerate case where `Site::from_parts` quietly ignored its argument.
    assert_ne!(
        real_page,
        fabricated_page(),
        "the fabricated page is byte-identical to the real one",
    );
}

/// This repository's own registry.
fn real_registry() -> Registry {
    Registry::load_path(repository_root().join("specs.toml"))
        .expect("this repository's registry reads")
}

/// The repository root, found relative to this crate rather than to the
/// directory cargo was invoked from.
fn repository_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/conform-web is two levels below the root")
        .to_path_buf()
}

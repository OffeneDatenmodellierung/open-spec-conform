//! A hostile registry cannot put markup on the page.
//!
//! # Why the threat is real rather than theoretical
//!
//! Every adapter in this family quotes the document it found a fault in, and
//! none of them sanitises what it quotes — `conform-okf` says so in as many
//! words, and is right to: escaping is not idempotent, so it must happen once,
//! at the point of display. The registry's own fields are quoted the same way,
//! and several of them are transcribed by hand from third-party READMEs.
//!
//! The console's answer to that is `conform_cli::escape::for_terminal`. This
//! crate's sink is a browser, where the failure is not a recoloured screen but
//! script running with the page's origin. This test drives the **whole
//! renderer** — not the escaping function, which has its own unit tests — with
//! a registry built to break out, and checks what actually comes out the far
//! end.
//!
//! # What these tests learned from failing
//!
//! The first shape of this file counted closing tags and searched the whole
//! page for `onmouseover=`. Both failed, and neither failure was an escaping
//! bug:
//!
//! - Two registries that differ only in *legitimate* ways render different
//!   numbers of `<code>` elements — a `javascript:` URL is correctly rendered
//!   as non-clickable text inside one — so the count compared nothing.
//! - `onmouseover=` genuinely is on the page, as **text**, inside a value
//!   whose quotation marks have become `&quot;`. It cannot be an attribute
//!   there, and an assertion that could not tell those apart was asserting the
//!   wrong property.
//!
//! What replaced them asks the questions that actually matter: which elements
//! exist, and what is inside a tag.

use conform_registry::Registry;
use conform_web::{Demo, Module, Site, render};

/// A registry whose every field is trying to become markup.
///
/// Valid TOML and a valid registry: the point is that a registry can be
/// perfectly well-formed and still hostile, so nothing upstream of the
/// renderer will have rejected it.
const HOSTILE_TEMPLATE: &str = r#"
schema_version = 1

[[spec]]
id            = "hostile"
name          = "</h3><script>alert('name')</script>"
version       = "\" onmouseover=\"alert('version')"
homepage      = "javascript:alert('homepage')"
repository    = "https://example.invalid/a?b=1&c=2</a><script>alert('repo')</script>"
steward       = "</dd><img src=x onerror=alert('steward')>"
licence       = "MIT' onclick='alert(1)"
pinned_ref    = "v1</code><svg onload=alert('pin')>"
vendored_path = "nowhere/hostile.json"
sha256        = "4444444444444444444444444444444444444444444444444444444444444444"
fetched_at    = "2000-01-01"
notes         = """
A note that tries to close the page: </details></article></main><script>\
alert('notes')</script>

And one that tries to reorder itself: @RLO@derotser eb tonnac sihT

And one with an invisible run in it: zero@ZWSP@width
"""

  [spec.poll]
  endpoint = "https://example.invalid/poll\"><script>alert('poll')</script>"
  method   = "</span><script>alert('method')</script>"
  cadence  = "weekly"
"#;

/// The fixture, with its invisible characters put back.
///
/// They cannot be written literally above: `rustc`'s own
/// `text_direction_codepoint_in_literal` lint refuses a source file containing
/// a bidirectional override — the same Trojan Source defence this renderer
/// implements for HTML, one layer down — so the two characters are spliced in
/// here instead.
fn hostile() -> String {
    HOSTILE_TEMPLATE
        .replace("@RLO@", "\u{202e}")
        .replace("@ZWSP@", "\u{200b}")
}

/// Render the page from the hostile registry.
fn hostile_page() -> String {
    let registry = Registry::load_str(&hostile(), "<hostile>").expect("the fixture is a registry");
    let site = Site::from_parts(&registry, Vec::new(), "hostile.toml".to_owned());
    render::page(&site)
}

/// The same page, with the in-browser validator wired up.
///
/// A second demo state means a second set of elements — a `select`, a
/// `textarea`, a `button`, a second `script` — and a scan that only ever saw
/// the other state would be a scan with a hole in it exactly where the newest
/// markup is. The registry is the same hostile one, because what is being
/// tested is still whether a registry value can become markup.
fn hostile_wired_page() -> String {
    let registry = Registry::load_str(&hostile(), "<hostile>").expect("the fixture is a registry");
    let site = Site::from_parts(&registry, Vec::new(), "hostile.toml".to_owned()).with_demo(
        Demo::Wired(Module {
            script_href: "./wasm/conform_ffi.js".to_owned(),
            script_bytes: 19_322,
            wasm_href: "./wasm/conform_ffi_bg.wasm".to_owned(),
            wasm_bytes: 3_112_486,
        }),
    );
    render::page(&site)
}

/// Both demo states, named, so a failure says which page it was looking at.
fn hostile_pages() -> [(&'static str, String); 2] {
    [
        ("demo not wired", hostile_page()),
        ("demo wired", hostile_wired_page()),
    ]
}

/// Every tag in a page, as raw `<…>` slices.
///
/// Sound because escaping guarantees it is: a registry value can no longer
/// hold a raw `<` or `>` by the time it reaches the document — both are
/// entity-encoded — so every `<` here opens a tag the renderer wrote, and the
/// boundaries are unambiguous. That is worth stating, because the test would
/// be circular if it were not true, and `every_hostile_value_reaches_the_page_escaped_and_never_raw`
/// is what establishes it independently.
fn tags(page: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut index = 0;
    while let Some(open) = page[index..].find('<') {
        let open = index + open;
        let Some(close) = page[open..].find('>') else {
            break;
        };
        let close = open + close;
        found.push(&page[open..=close]);
        index = close + 1;
    }
    found
}

/// The element name each tag opens or closes, lower-cased.
fn element_names(page: &str) -> Vec<String> {
    tags(page)
        .into_iter()
        .filter_map(|tag| {
            let inner = tag.trim_start_matches('<').trim_end_matches('>');
            let inner = inner.strip_prefix('/').unwrap_or(inner);
            if inner.starts_with('!') {
                return None;
            }
            let name: String = inner
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect::<String>()
                .to_ascii_lowercase();
            (!name.is_empty()).then_some(name)
        })
        .collect()
}

/// The elements this renderer emits, listed once.
///
/// Anything else in the document came from the registry, which is the failure
/// this file exists to catch. `script` and `style` are on the list because the
/// renderer writes exactly one of each — its own — and
/// `no_element_the_renderer_never_writes_appears_on_the_page` is paired with a
/// count for that reason.
const WRITTEN_BY_THE_RENDERER: &[&str] = &[
    "html", "head", "meta", "title", "style", "body", "header", "h1", "h2", "h3", "h4", "p", "dl",
    "dt", "dd", "div", "nav", "ul", "ol", "li", "a", "main", "section", "table", "thead", "tbody",
    "tr", "th", "td", "code", "span", "br", "article", "details", "summary", "em", "strong", "pre",
    "footer", "input", "label", "script", "colgroup", "col", "select", "button", "textarea",
    "noscript",
];

/// `option` is deliberately absent from the list above.
///
/// The `select` the wired demo writes is **empty**: its options are the spec
/// ids the WebAssembly module reports it carries a verified schema for, added
/// by the browser at run time. A page that wrote them would be a page that can
/// offer a standard the artefact beside it cannot validate, which is the exact
/// class of claim `Demo` exists to prevent — so an `option` in the generated
/// bytes is a regression, and the scan above is what would catch it.
const DELIBERATELY_NOT_WRITTEN: &[&str] = &["option"];

#[test]
fn the_fixture_really_is_hostile() {
    // The control. If the fixture were inert — a typo in the TOML, a field
    // that never reaches the page — every assertion below would pass for
    // nothing. So: the dangerous substrings must be present in the *input*.
    let hostile = hostile();
    for probe in [
        "<script>",
        "onerror=",
        "onload=",
        "javascript:",
        "\u{202e}",
        "\u{200b}",
    ] {
        assert!(
            hostile.contains(probe),
            "the fixture does not contain {probe:?}, so this test proves nothing",
        );
    }

    // And it must actually load, or the renderer is never reached.
    let registry = Registry::load_str(&hostile, "<hostile>").expect("the fixture is a registry");
    assert_eq!(registry.entries().len(), 1);
}

#[test]
fn no_element_the_renderer_never_writes_appears_on_the_page() {
    for (which, page) in hostile_pages() {
        let found: Vec<String> = element_names(&page)
            .into_iter()
            .filter(|name| !WRITTEN_BY_THE_RENDERER.contains(&name.as_str()))
            .collect();

        assert!(
            found.is_empty(),
            "{which}: elements the renderer never writes appear on the page, so the registry \
             created them: {found:?}",
        );

        for never in DELIBERATELY_NOT_WRITTEN {
            assert!(
                !element_names(&page).iter().any(|name| name == never),
                "{which}: `{never}` is on the page, and the renderer must not write one",
            );
        }

        // `script` is on the allow-list because the renderer writes its own.
        // It must write exactly the ones it means to, or a further one could
        // be the registry's: one when the demo is not wired, two when it is —
        // the page's own behaviour, and the module loader.
        let tags = element_names(&page)
            .iter()
            .filter(|n| *n == "script")
            .count();
        let expected = if which == "demo wired" { 4 } else { 2 };
        assert_eq!(
            tags,
            expected,
            "{which}: the page should hold {} script element(s) — an open tag and a close tag \
             each — and holds {tags} tags",
            expected / 2,
        );
    }
}

#[test]
fn the_element_scan_is_not_vacuous() {
    // A scanner that found no elements would pass the test above no matter
    // what the registry did.
    let names = element_names(&hostile_page());
    assert!(
        names.len() > 100,
        "only {} element tags were found in the page; the scanner is not reading it",
        names.len(),
    );
    assert!(
        names.contains(&"article".to_owned()),
        "the cards are missing"
    );

    // And it must be able to *see* a smuggled element when one is there: the
    // same scan over the fixture's raw text finds exactly what the renderer
    // refused to emit.
    let raw = element_names(&hostile());
    for smuggled in ["script", "svg", "img"] {
        assert!(
            raw.contains(&smuggled.to_owned()),
            "the fixture's raw text should contain a {smuggled:?} element for the scan to have \
             something to miss",
        );
        assert!(
            !WRITTEN_BY_THE_RENDERER.contains(&smuggled) || smuggled == "script",
            "{smuggled:?} is on the allow-list, so smuggling it would not be caught",
        );
    }
}

/// The parts of a tag that are attribute *names* rather than attribute
/// *values*: everything outside a double-quoted run.
///
/// The distinction is the whole test. `data-filter="… &quot; onmouseover=…"`
/// contains the letters `onmouseover=` and is completely inert, because the
/// quotation marks the registry supplied became `&quot;` and the attribute
/// value is one string. A scan that could not tell an attribute name from
/// text inside an attribute value would flag that, and flagging safe output is
/// how a security test gets switched off.
///
/// Splitting on `"` is sound for the same reason `tags` is: escaping
/// guarantees no registry value holds a raw `"` by the time it reaches the
/// document, so every `"` in a tag is a delimiter the renderer wrote. Even
/// indices are therefore outside values, odd indices inside them.
fn attribute_name_regions(tag: &str) -> Vec<&str> {
    tag.split('"').step_by(2).collect()
}

#[test]
fn no_tag_on_the_page_carries_an_event_handler() {
    for (which, page) in hostile_pages() {
        let mut inspected = 0_usize;

        for tag in tags(&page) {
            for region in attribute_name_regions(tag) {
                let lower = region.to_ascii_lowercase();
                for handler in ["onmouseover", "onerror", "onload", "onclick", "onfocus"] {
                    assert!(
                        !lower.contains(handler),
                        "{which}: a tag declares the attribute {handler:?}, so an attribute \
                         value was broken out of: {tag:?}",
                    );
                }
                inspected += 1;
            }
        }

        assert!(
            inspected > 100,
            "{which}: only {inspected} attribute regions were inspected; the scan is not \
             reading the page",
        );
    }
}

#[test]
fn the_attribute_scan_can_see_a_handler_when_there_is_one() {
    // The control for the scan above, which would otherwise pass by looking
    // only inside quoted values. A tag with a genuine handler must be caught.
    let planted = "<img src=x onerror=\"alert(1)\">";
    let regions = attribute_name_regions(planted);
    assert!(
        regions.iter().any(|r| r.contains("onerror")),
        "the scan cannot see a handler that is really declared: {regions:?}",
    );

    // And the inert case must not be caught, or the scan flags everything.
    let safe = "<article data-filter=\"x &quot; onerror=y\">";
    assert!(
        !attribute_name_regions(safe)
            .iter()
            .any(|r| r.contains("onerror")),
        "the scan flags letters inside an attribute value, which is safe output",
    );
}

#[test]
fn no_tag_on_the_page_carries_a_javascript_url() {
    for (which, page) in hostile_pages() {
        for tag in tags(&page) {
            for region in attribute_name_regions(tag) {
                assert!(
                    !region.to_ascii_lowercase().contains("javascript:"),
                    "{which}: a javascript: URL escaped its attribute value: {tag:?}",
                );
            }
            // An href is the one attribute where the *value* matters too, and
            // the scheme allow-list is what guards it.
            assert!(
                !tag.to_ascii_lowercase().contains("href=\"javascript:"),
                "{which}: a javascript: URL was emitted as an href: {tag:?}",
            );
        }
    }
}

#[test]
fn every_hostile_value_reaches_the_page_escaped_and_never_raw() {
    // The property, stated directly rather than inferred from tag counts: each
    // value the registry holds appears on the page in its escaped spelling and
    // never in the spelling that would have been markup. This is also what
    // licenses `tags` and `attribute_name_regions` above to treat every `<`
    // and every `"` as one the renderer wrote.
    //
    // The values are read back out of the *parsed* registry rather than
    // matched against the fixture's text: TOML has its own escaping, so
    // `\"` in the file is one `"` in the value, and comparing against the
    // file would be comparing against the wrong string.
    let page = hostile_page();
    let registry = Registry::load_str(&hostile(), "<hostile>").expect("the fixture is a registry");
    let entry = &registry.entries()[0];

    let version = entry.default_version();
    let values: Vec<&str> = [
        Some(entry.name.as_str()),
        version.version.as_deref(),
        entry.homepage.as_deref(),
        entry.repository.as_deref(),
        entry.steward.as_deref(),
        entry.licence.as_deref(),
        version.pinned_ref.as_deref(),
        entry.notes.as_deref(),
        version.poll.as_ref().map(|p| p.endpoint.as_str()),
        version.poll.as_ref().and_then(|p| p.method.as_deref()),
    ]
    .into_iter()
    .flatten()
    .collect();

    let mut needed_escaping = 0_usize;
    for value in values {
        // `notes` is rendered one paragraph per element, so the whole field
        // never appears on the page contiguously and checking for it would
        // fail for a reason that is not an escaping failure. Splitting here
        // the way the renderer splits is not the test conceding a point: each
        // paragraph is still required to appear escaped and never raw, which
        // is the property. What it concedes is only that the renderer is
        // allowed to choose where the paragraph breaks go.
        for raw in value.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
            let escaped = conform_web::escape::for_html(raw);
            if escaped.as_ref() == raw {
                continue;
            }
            needed_escaping += 1;

            assert!(!page.contains(raw), "{raw:?} reached the page unescaped");
            assert!(
                page.contains(escaped.as_ref()),
                "{raw:?} appears in neither its raw nor its escaped form, so it was silently \
                 dropped — which hides from a reader what the registry holds",
            );
        }
    }

    assert!(
        needed_escaping >= 8,
        "only {needed_escaping} of the fixture's values needed escaping; the fixture is not \
         hostile enough for this to prove anything",
    );
}

#[test]
fn a_javascript_url_is_shown_as_text_rather_than_dropped() {
    // Escaping is the wrong tool for this one and this is the test that says
    // so: `javascript:alert(1)` contains not one markup-significant character,
    // so a perfectly correct HTML escaper passes it through untouched and the
    // link still runs code. What stops it is the scheme allow-list — and what
    // this test adds is the other half of that decision. The unusable URL is
    // still shown, as text, because hiding that the registry holds something
    // unexpected is the one thing a provenance page must not do.
    let page = hostile_page();
    assert!(
        page.contains("javascript:alert(&#x27;homepage&#x27;)"),
        "the unusable URL should still be visible to a reader, as escaped text",
    );
}

#[test]
fn the_trojan_source_family_is_neutralised() {
    let page = hostile_page();
    assert!(
        !page.contains('\u{202e}'),
        "a right-to-left override reached the page and can reorder what a reader sees",
    );
    assert!(
        !page.contains('\u{200b}'),
        "a zero-width space reached the page",
    );
    assert!(
        page.contains("&lt;U+202E&gt;") && page.contains("&lt;U+200B&gt;"),
        "the neutralised characters should be spelled out in the console's notation, not \
         silently deleted",
    );
}

#[test]
fn the_ampersand_in_a_legitimate_url_is_encoded_exactly_once() {
    // The double-encoding bug, which is its own kind of dishonesty: a reader
    // told the registry holds `&amp;c=2` when it holds `&c=2`.
    let page = hostile_page();
    assert!(
        page.contains("b=1&amp;c=2"),
        "a query-string ampersand should be encoded once",
    );
    assert!(
        !page.contains("b=1&amp;amp;c=2"),
        "a query-string ampersand was encoded twice",
    );
}

#[test]
fn the_real_registry_needs_no_escaping_and_still_renders() {
    // The reassurance that the escaping is not mangling the actual content.
    // The registry's notes are full of backticks, quotation marks and em
    // dashes, and they must survive.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/conform-web is two levels below the root");
    let site = Site::gather(root).expect("this repository's registry reads");
    let page = render::page(&site);

    assert!(page.contains("&quot;"), "the notes contain quotation marks");
    assert!(
        !page.contains("&amp;quot;"),
        "the page double-encodes, so it is telling a reader the registry holds `&quot;`",
    );
    assert!(
        !page.contains('\u{202e}'),
        "the real registry should contain no bidirectional override, and does not",
    );

    // And the real page passes the same structural check the hostile one does.
    let unexpected: Vec<String> = element_names(&page)
        .into_iter()
        .filter(|name| !WRITTEN_BY_THE_RENDERER.contains(&name.as_str()))
        .collect();
    assert!(
        unexpected.is_empty(),
        "the real page holds elements the renderer never writes: {unexpected:?}",
    );
}

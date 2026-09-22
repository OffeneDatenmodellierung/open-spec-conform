//! The page.
//!
//! # It computes nothing and reads nothing
//!
//! Every value here comes out of a [`Site`]. This module opens no file, hashes
//! no bytes and counts nothing a second time — the rule `conform-cli`'s
//! `human.rs` states for the console, for the same reason: two renderers that
//! each derive their own facts are two renderers that can disagree, and the
//! cheapest way to make them agree forever is to leave them nothing to
//! disagree about.
//!
//! The consequence worth stating is the one the whole phase turns on. Nothing
//! about a specification — no URL, no name, no identifier, no version, no
//! digest — is written in this file. Each is a field read from `specs.toml`
//! through [`Site`], which is why
//! `tests/the_page_is_a_function_of_the_registry.rs` can hand this module a
//! registry of fabricated values and get a page of fabricated values back. A
//! renderer with a link typed into it would fail that test, and there is no
//! way to write one that passes it by accident.
//!
//! # Escaping happens here, exactly once
//!
//! Every value that came from the registry or from a manifest goes through
//! [`escape::for_html`] on its way into the document, at the point it is
//! written and nowhere else. The stylesheet and the script below are this
//! module's own constants rather than anybody's data, so they are not escaped
//! and must not be — escaping a stylesheet would produce a stylesheet that
//! does not parse.
//!
//! # Nothing is interpolated into a script, and there is no `innerHTML`
//!
//! Two scripts. [`SCRIPT`] filters the catalogue and highlights the
//! navigation, and touches no data at all: it toggles the `hidden` attribute
//! on elements the generator already wrote. [`DEMO_SCRIPT`] drives the
//! in-browser validator and does build elements at run time — from diagnostics
//! the WebAssembly module produced about a document the reader pasted in,
//! which is the most hostile text on the page. It builds every one of them
//! with `createElement` and writes it with `textContent`.
//!
//! Neither script has a value from this generator interpolated into it. Both
//! are constants; the one thing [`DEMO_SCRIPT`] needs — where the module is —
//! it reads from a `data-` attribute that went through [`for_html`] like
//! everything else. That is a design decision rather than a coincidence: a
//! generator that escapes perfectly for HTML and then hands the same value to
//! a script literal or to `innerHTML` has escaped for the wrong grammar, and
//! the second boundary is the one people forget. There is no second boundary
//! here.

use std::fmt::Write as _;

use conform_cli::model::{SpecSummary, VerifyStatus};

use crate::escape::for_html;
use crate::manifests::CrateEntry;
use crate::site::{Demo, Module, Site, SpecCard};

/// What the page calls a field nobody could determine.
///
/// The console's words, deliberately. `conform-cli` prints `(not recorded)` in
/// its catalogue and its console, and a reader who has seen it there must not
/// have to work out that "unknown", "n/a" and "—" mean the same thing here.
const NOT_RECORDED: &str = "(not recorded)";

/// Render the whole page.
///
/// One self-contained HTML document: the stylesheet and both scripts are
/// inline, nothing is loaded from a network, and every fact on it is written
/// into the file. A test can therefore assert on exactly the bytes a reader
/// will see.
///
/// One thing is fetched, and only when there is one to fetch: a
/// [`Demo::Wired`] page imports the WebAssembly module from a path beside
/// itself. That is the single dependency on being *served* rather than opened
/// — an ES module and a `.wasm` are both unreachable from `file://` — and the
/// page handles it the only honest way, by showing the browser's own error in
/// the demo panel. Everything else on the page, catalogue included, works from
/// a `file://` URL exactly as it always did.
#[must_use]
pub fn page(site: &Site) -> String {
    let mut out = String::with_capacity(64 * 1024);

    out.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
    out.push_str("<meta charset=\"utf-8\">\n");
    out.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str("<title>Open Spec Conform</title>\n");
    out.push_str("<meta name=\"description\" content=\"");
    out.push_str(
        "Conformance checking for vendored data specifications, and the provenance of the \
         bytes it checks against.",
    );
    out.push_str("\">\n<style>\n");
    out.push_str(STYLESHEET);
    out.push_str("</style>\n</head>\n<body>\n");

    masthead(&mut out, site);
    nav(&mut out);
    out.push_str("<main>\n");
    what_this_is(&mut out, site);
    upstream_watch(&mut out, site);
    catalogue(&mut out, site);
    family(&mut out, site);
    demo(&mut out, site);
    out.push_str("</main>\n");
    colophon(&mut out, site);

    out.push_str("<script>\n");
    out.push_str(SCRIPT);
    out.push_str("</script>\n</body>\n</html>\n");
    out
}

/// The title block.
fn masthead(out: &mut String, site: &Site) {
    out.push_str("<header class=\"masthead\">\n<h1>Open Spec Conform</h1>\n");
    out.push_str(
        "<p class=\"tagline\">Conformance checking for vendored data specifications — and a \
         machine-readable record of where every vendored byte came from.</p>\n",
    );
    out.push_str("<dl class=\"at-a-glance\">\n");
    stat(out, "specifications", &site.specs.len().to_string());
    stat(out, "crates", &site.crates.len().to_string());
    stat(
        out,
        "entries with a recorded gap",
        &site.with_gaps().to_string(),
    );
    stat(
        out,
        "provenance fields not recorded",
        &site.recorded_gaps().to_string(),
    );
    stat(
        out,
        "entries we could poll upstream for",
        &site.pollable().to_string(),
    );
    out.push_str("</dl>\n</header>\n");
}

/// One figure in the masthead.
fn stat(out: &mut String, label: &str, value: &str) {
    let _ = writeln!(
        out,
        "<div class=\"stat\"><dt>{}</dt><dd>{}</dd></div>",
        for_html(label),
        for_html(value),
    );
}

/// The in-page navigation.
///
/// Anchors, not routes. This is one document — a reader with JavaScript turned
/// off gets every section, in order, and the links still work; the script only
/// marks which one is in view.
fn nav(out: &mut String) {
    out.push_str("<nav aria-label=\"Sections\"><ul>\n");
    for (anchor, label) in [
        ("what", "What this is"),
        ("watch", "Upstream watch"),
        ("specs", "The catalogue"),
        ("crates", "The crates"),
        ("try", "Validate in the browser"),
    ] {
        let _ = writeln!(
            out,
            "<li><a href=\"#{0}\">{1}</a></li>",
            for_html(anchor),
            for_html(label),
        );
    }
    out.push_str("</ul></nav>\n");
}

/// What the tool is, and the defect it was built around.
///
/// The one section on the page that is prose rather than data, because "what
/// is this for" is not a field in any file. It names no specification, no
/// version and no URL — the filename it quotes is the *former* name of an
/// artefact and is the subject of the sentence, not a link to anywhere.
fn what_this_is(out: &mut String, site: &Site) {
    out.push_str("<section id=\"what\">\n<h2>What this is</h2>\n");
    out.push_str(
        "<p>A repository that validates documents against a published standard ends up holding \
         a copy of that standard's schema. The copy is the problem: it is a snapshot of a moving \
         document, and unless something records <em>which moment</em> it is a snapshot of, nobody \
         can answer the only question that matters about it — is this still what upstream \
         says?</p>\n",
    );
    out.push_str(
        "<p>This project is that something. A registry records, for every vendored artefact, \
         where it came from, who stewards it, what licence it carries, which immutable upstream \
         revision it was taken at, and the SHA-256 of the exact bytes on disk. A family of \
         validators resolves its schemas <em>through</em> that registry, so a verdict is never \
         issued against bytes whose provenance has not just been checked.</p>\n",
    );
    out.push_str("<h3>Two rules, and why each exists</h3>\n<dl class=\"rules\">\n");
    out.push_str(
        "<dt>Nothing may be pinned to a moving target.</dt><dd>A schema vendored as \
         <code>…-latest.json</code>, with no version, no source URL and no fetch date, is the \
         defect this project was built around. A copy pinned to whatever upstream holds today \
         cannot be checked against anything, ever. The registry refuses such a pin and the build \
         fails if one reappears.</dd>\n",
    );
    let _ = write!(
        out,
        "<dt>A recorded unknown beats a plausible guess.</dt><dd>Every provenance field is \
         optional, because upstream sometimes genuinely does not publish a licence or a \
         documentation site. A field nobody could determine is <em>left out</em>, and the entry \
         says what was looked at and what it did not say. {} at least one such gap, and this \
         page shows every one of them as <span class=\"absent\">{}</span> rather than as a \
         blank — because a blank looks like a rendering accident, and these are \
         answers.</dd>\n</dl>\n</section>\n",
        for_html(&gap_tally(site)),
        for_html(NOT_RECORDED),
    );
}

/// How many entries have a gap, phrased so that it reads as English.
///
/// "5 of the 5 entries below have" is what a naive interpolation produces, and
/// it reads as though somebody forgot to special-case it — which is precisely
/// the impression a page about honest reporting should not give. The numbers
/// are still the registry's; only the sentence around them changes.
fn gap_tally(site: &Site) -> String {
    let with_gaps = site.with_gaps();
    let total = site.specs.len();
    match with_gaps {
        0 => "No entry below has".to_owned(),
        n if n == total && total == 1 => "The single entry below has".to_owned(),
        n if n == total => format!("Every one of the {total} entries below has"),
        1 => format!("One of the {total} entries below has"),
        n => format!("{n} of the {total} entries below have"),
    }
}

/// The upstream watch — the table this page exists for.
fn upstream_watch(out: &mut String, site: &Site) {
    out.push_str("<section id=\"watch\">\n<h2>Upstream watch</h2>\n");
    let _ = writeln!(
        out,
        "<p>The canonical source for each vendored artefact, the immutable revision it was taken \
         at, and the digest of the bytes in the tree. Together these are what makes \
         &ldquo;has upstream changed?&rdquo; a question with an answer. Every value in this \
         table is read from <code>{}</code>; none of it is written into this page.</p>",
        for_html(&site.registry_path),
    );
    out.push_str(
        "<p class=\"caveat\">The <em>bytes</em> column is measured, not transcribed: each \
         artefact was re-hashed when this page was generated and compared with the digest the \
         registry records. It answers &ldquo;have <em>our</em> bytes changed&rdquo;. It does not \
         answer &ldquo;has upstream moved&rdquo; — nothing in this project performs network \
         access, which is what the last column is for.</p>\n",
    );

    out.push_str("<div class=\"scroller\">\n<table class=\"watch\">\n");
    out.push_str(
        "<colgroup><col class=\"c-spec\"><col class=\"c-upstream\"><col class=\"c-pin\">\
         <col class=\"c-digest\"><col class=\"c-poll\"></colgroup>\n",
    );
    out.push_str("<thead><tr>");
    for heading in [
        "Specification",
        "Canonical upstream",
        "Pinned to",
        "Vendored digest",
        "How we would notice",
    ] {
        let _ = write!(out, "<th scope=\"col\">{}</th>", for_html(heading));
    }
    out.push_str("</tr></thead>\n<tbody>\n");
    for card in &site.specs {
        watch_row(out, card);
    }
    out.push_str("</tbody>\n</table>\n</div>\n</section>\n");
}

/// One row of the upstream watch.
///
/// Five columns rather than the seven this started with. Seven put a 64-digit
/// digest and a date beside five other cells and pushed the drift status off
/// the right-hand edge of a 1280-pixel window — found by screenshotting the
/// rendered page, which is the only way that kind of defect is ever found.
/// `fetched_at` now sits under the pin and the drift badge under the digest,
/// each beside the thing it qualifies; nothing was dropped.
fn watch_row(out: &mut String, card: &SpecCard) {
    let spec = &card.summary;
    out.push_str("<tr>");

    let _ = write!(
        out,
        "<th scope=\"row\"><code>{}</code><br><span class=\"muted\">{}</span></th>",
        for_html(&spec.id),
        for_html(&spec.name),
    );

    out.push_str("<td>");
    upstream_cell(out, spec);
    out.push_str("</td>");

    let _ = write!(
        out,
        "<td>{}<br><span class=\"muted\">in tree since {}</span></td>",
        code_or_absence(spec.pinned_ref.as_deref()),
        for_html(&spec.fetched_at),
    );

    out.push_str("<td><code class=\"digest\">");
    out.push_str(&for_html(&spec.sha256));
    out.push_str("</code><br>");
    verify_badge(out, spec.verify);
    out.push_str("</td>");

    out.push_str("<td>");
    match &card.poll {
        Some(poll) => {
            let _ = write!(
                out,
                "<span class=\"muted\">{}, {}</span><br>",
                for_html(poll.method.as_deref().unwrap_or(NOT_RECORDED)),
                for_html(poll.cadence.as_deref().unwrap_or(NOT_RECORDED)),
            );
            link_or_absence(out, Some(&poll.endpoint));
        }
        None => absence(out),
    }
    out.push_str("</td></tr>\n");
}

/// The canonical-upstream cell, saying which field the link came from.
///
/// [`SpecSummary::upstream_link`] answers "the best link for this entry" by
/// falling back from the documentation site to the repository, and three of
/// the seven entries in this repository's registry take that fallback because
/// no documentation site is recorded for them. A cell that printed the URL and
/// stopped would be hiding exactly that: a reader would see a link under
/// *canonical upstream* and have no way to tell a steward's published
/// documentation from a repository this page settled for. The absence is the
/// finding, so the absence is labelled.
fn upstream_cell(out: &mut String, spec: &SpecSummary) {
    let Some(url) = spec.upstream_link() else {
        absence(out);
        return;
    };
    link_or_absence(out, Some(url));
    let _ = write!(
        out,
        "<br><span class=\"muted\">{}</span>",
        for_html(if spec.homepage.is_some() {
            "documentation site"
        } else {
            "repository — no documentation site is recorded"
        }),
    );
}

/// The badge for what re-hashing one artefact said.
///
/// The glyph is never the only encoding — plan §4.2 rules out colour-only, and
/// `conform-cli` notes that glyph-only is the same mistake in a different
/// font. The word is always beside it, and the colour is a third channel
/// rather than the first.
fn verify_badge(out: &mut String, status: VerifyStatus) {
    let _ = write!(
        out,
        "<span class=\"badge badge-{}\"><span aria-hidden=\"true\">{}</span> {}</span>",
        for_html(status.as_str()),
        for_html(status.glyph()),
        for_html(status.as_str()),
    );
}

/// The catalogue: one card per entry, with everything the registry records.
fn catalogue(out: &mut String, site: &Site) {
    out.push_str("<section id=\"specs\">\n<h2>The catalogue</h2>\n");
    let _ = writeln!(
        out,
        "<p>Every entry in <code>{}</code>, in the order the file writes them. The \
         <em>notes</em> on each card are the registry's own words, shown in full: they record \
         which evidence dated the artefact, why a field is absent, and what is known to be \
         uncertain. They are the most useful thing on this page.</p>",
        for_html(&site.registry_path),
    );

    out.push_str(
        "<p class=\"filter\"><label for=\"spec-filter\">Filter</label>\
         <input id=\"spec-filter\" type=\"search\" placeholder=\"name, steward, licence…\" \
         autocomplete=\"off\"></p>\n",
    );
    out.push_str("<p id=\"filter-status\" class=\"muted\" role=\"status\"></p>\n");

    for card in &site.specs {
        spec_card(out, card);
    }
    out.push_str("</section>\n");
}

/// One entry, in full.
fn spec_card(out: &mut String, card: &SpecCard) {
    let spec = &card.summary;
    let _ = writeln!(
        out,
        "<article class=\"card\" id=\"spec-{}\" data-filter=\"{}\">",
        for_html(&spec.id),
        for_html(&filter_key(spec)),
    );

    let _ = writeln!(
        out,
        "<h3><code>{}</code> <span class=\"card-name\">{}</span></h3>",
        for_html(&spec.id),
        for_html(&spec.name),
    );

    out.push_str("<p class=\"card-status\">");
    verify_badge(out, spec.verify);
    let _ = writeln!(
        out,
        " <span class=\"badge badge-{}\">{}</span></p>",
        if spec.has_adapter {
            "adapter"
        } else {
            "no-adapter"
        },
        for_html(if spec.has_adapter {
            "a validator in this family checks documents against this"
        } else {
            "catalogued and verified; no validator for it yet"
        }),
    );

    out.push_str("<dl class=\"provenance\">\n");
    for (label, value) in [
        ("version", spec.version.as_deref()),
        ("steward", spec.steward.as_deref()),
        ("licence", spec.licence.as_deref()),
        ("pinned ref", spec.pinned_ref.as_deref()),
    ] {
        field(out, label, value);
    }
    linked_field(out, "homepage", spec.homepage.as_deref());
    linked_field(out, "repository", spec.repository.as_deref());
    field(out, "vendored path", Some(&spec.vendored_path));
    field(out, "in tree since", Some(&spec.fetched_at));

    let _ = writeln!(
        out,
        "<div class=\"row\"><dt>sha256</dt><dd><code class=\"digest\">{}</code></dd></div>",
        for_html(&spec.sha256),
    );
    out.push_str("</dl>\n");

    // The registry's own diagnostic for the re-hash, verbatim. A paraphrase
    // here would be a second opinion about bytes this page did not hash.
    let _ = writeln!(
        out,
        "<p class=\"diagnostic\"><code>{}</code> {}</p>",
        for_html(spec.verify_diagnostic.code.as_str()),
        for_html(&spec.verify_diagnostic.message),
    );

    gaps(out, spec);
    notes(out, spec.notes.as_deref());
    out.push_str("</article>\n");
}

/// The provenance fields this entry does not record.
fn gaps(out: &mut String, spec: &SpecSummary) {
    if spec.provenance_gaps.is_empty() {
        out.push_str(
            "<p class=\"gaps none\">Every provenance field is recorded for this entry.</p>\n",
        );
        return;
    }
    out.push_str("<p class=\"gaps\">Not recorded for this entry: ");
    for (index, gap) in spec.provenance_gaps.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        let _ = write!(out, "<code>{}</code>", for_html(gap));
    }
    out.push_str(". The notes below say what was looked at in each case.</p>\n");
}

/// The entry's notes, verbatim.
///
/// Split into paragraphs on blank lines and otherwise **not interpreted**. The
/// registry's notes are written with backticks and asterisks in them, and
/// rendering those as markup would mean running a markdown parser over
/// third-party-derived text and then having to reason about what its output
/// does to this page's escaping. They are evidence; evidence is quoted, not
/// reformatted.
fn notes(out: &mut String, notes: Option<&str>) {
    let Some(notes) = notes else {
        out.push_str("<div class=\"notes\"><h4>Notes</h4><p class=\"absent\">");
        out.push_str(NOT_RECORDED);
        out.push_str("</p></div>\n");
        return;
    };

    out.push_str(
        "<details class=\"notes\" open><summary>Notes — the registry's own words</summary>\n",
    );
    for paragraph in notes.split("\n\n") {
        let paragraph = paragraph.trim();
        if !paragraph.is_empty() {
            let _ = writeln!(out, "<p>{}</p>", for_html(paragraph));
        }
    }
    out.push_str("</details>\n");
}

/// The crate family.
fn family(out: &mut String, site: &Site) {
    out.push_str("<section id=\"crates\">\n<h2>The crates</h2>\n");
    out.push_str(
        "<p>Every crate carries a <strong>literal version in its own manifest</strong>. There is \
         no workspace version to inherit and no <code>version.workspace = true</code> anywhere, \
         because these are not one product cut into pieces: each tracks a different upstream on a \
         different release cycle, and a consumer must be able to pin one without pinning the \
         rest. The versions below are read from the manifests when this page is generated.</p>\n",
    );

    out.push_str("<div class=\"scroller\">\n<table class=\"crates\">\n");
    out.push_str(
        "<colgroup><col class=\"c-name\"><col class=\"c-version\"><col class=\"c-published\">\
         <col class=\"c-what\"><col class=\"c-manifest\"></colgroup>\n",
    );
    out.push_str("<thead><tr>");
    for heading in ["Crate", "Version", "Published", "What it is", "Manifest"] {
        let _ = write!(out, "<th scope=\"col\">{}</th>", for_html(heading));
    }
    out.push_str("</tr></thead>\n<tbody>\n");
    for member in &site.crates {
        crate_row(out, member);
    }
    out.push_str("</tbody>\n</table>\n</div>\n</section>\n");
}

/// One crate.
fn crate_row(out: &mut String, member: &CrateEntry) {
    let _ = write!(
        out,
        "<tr><th scope=\"row\"><code>{}</code></th><td>{}</td>",
        for_html(&member.name),
        code_or_absence(member.version.as_deref()),
    );
    let _ = write!(
        out,
        "<td><span class=\"badge badge-{}\">{}</span></td>",
        if member.published {
            "adapter"
        } else {
            "no-adapter"
        },
        for_html(if member.published {
            "to crates.io"
        } else {
            "not published"
        }),
    );
    let _ = writeln!(
        out,
        "<td>{}</td><td><code>{}</code></td></tr>",
        match member.description.as_deref() {
            Some(description) => for_html(description).into_owned(),
            None => absent_span(),
        },
        for_html(&member.manifest_path),
    );
}

/// Whether a document can be validated in this page.
fn demo(out: &mut String, site: &Site) {
    out.push_str("<section id=\"try\">\n<h2>Validate in the browser</h2>\n");
    match &site.demo {
        Demo::Wired(module) => wired_demo(out, module),
        Demo::NotWired { because, evidence } => {
            let _ = writeln!(
                out,
                "<p class=\"not-wired\"><strong>Not in this build.</strong> {}</p>",
                for_html(because),
            );

            out.push_str("<h3>What was built and run to find that out</h3>\n");
            out.push_str(
                "<p>A claim about what a build does is worth what was run to establish it. \
                 Every row below is a measurement taken against this repository, not an \
                 inference from reading the code.</p>\n",
            );
            out.push_str("<div class=\"scroller\">\n<table class=\"evidence\">\n<thead><tr>");
            for heading in ["Probe", "Result"] {
                let _ = write!(out, "<th scope=\"col\">{}</th>", for_html(heading));
            }
            out.push_str("</tr></thead>\n<tbody>\n");
            for (probe, result) in evidence {
                let _ = writeln!(
                    out,
                    "<tr><th scope=\"row\"><code>{}</code></th><td>{}</td></tr>",
                    for_html(probe),
                    for_html(result),
                );
            }
            out.push_str("</tbody>\n</table>\n</div>\n");

            out.push_str(
                "<p>Until it is wired, the same check runs from a terminal against a checkout, \
                 where the registry and the vendored bytes are both on disk and the digest is \
                 verified before any verdict is issued.</p>\n",
            );
            out.push_str(
                "<pre><code>cargo run -p conform-cli -- registry list\n\
                 cargo run -p conform-cli -- validate path/to/contract.yaml\n\
                 cargo run -p conform-cli -- registry verify --check</code></pre>\n",
            );
        }
    }
    out.push_str("</section>\n");
}

/// The demo, when a module was found next to the page.
///
/// # Every claim on this panel comes out of the module
///
/// The standards on offer, the library version and — the one that matters —
/// the sentence about what a panic does are all read from the compiled
/// artefact at run time, by the script below. None of them is written here.
/// That is the same rule the catalogue follows for `specs.toml`, applied to
/// the one fact a page would be most tempted to soften: `conform-ffi` cannot
/// catch a panic on `wasm32-unknown-unknown`, and the sentence saying so is
/// the module's own, so it cannot be edited into something friendlier without
/// editing the thing that behaves that way.
///
/// # What this section writes, and what it refuses to
///
/// Static markup and two measured byte counts. The `href` of the module goes
/// into a `data-` attribute rather than into a script literal, so nothing in
/// this file is ever interpolated into JavaScript — see this module's opening
/// note, which that would otherwise have made untrue.
fn wired_demo(out: &mut String, module: &Module) {
    let _ = writeln!(
        out,
        "<div id=\"demo\" data-module=\"{}\">",
        for_html(&module.script_href),
    );

    out.push_str(
        "<p>This page carries the validator itself, compiled to WebAssembly. The document \
         below never leaves the browser: there is no upload, no request and no server \
         involved in the verdict.</p>\n",
    );

    out.push_str("<p id=\"demo-status\" class=\"not-wired\">Loading the validator…</p>\n");

    out.push_str("<div id=\"demo-ui\" hidden>\n");
    out.push_str("<div class=\"demo-controls\">\n");
    out.push_str("<label for=\"demo-spec\">Standard</label>\n");
    // Deliberately empty: the options are the spec ids the module reports it
    // carries a verified schema for. A list written here could offer a
    // standard this artefact cannot validate.
    out.push_str("<select id=\"demo-spec\"></select>\n");
    out.push_str("<button id=\"demo-run\" type=\"button\">Validate</button>\n");
    out.push_str("</div>\n");
    out.push_str(
        "<label for=\"demo-document\" class=\"visually-hidden\">The document to check</label>\n",
    );
    out.push_str(
        "<textarea id=\"demo-document\" rows=\"14\" spellcheck=\"false\" \
         aria-describedby=\"demo-status\" placeholder=\"Paste a document here, or press \
         Validate on an empty one to see what the harness says about that.\"></textarea>\n",
    );
    out.push_str("<div id=\"demo-verdict\" hidden></div>\n");
    out.push_str("<ol id=\"demo-findings\" class=\"findings\"></ol>\n");
    out.push_str("<p id=\"demo-provenance\" class=\"provenance\"></p>\n");
    out.push_str("</div>\n");

    out.push_str(
        "<noscript><p class=\"not-wired\"><strong>This panel needs JavaScript.</strong> \
         The validator is a WebAssembly module and a script is what loads it. Everything else \
         on this page works without one.</p></noscript>\n",
    );

    out.push_str("<h3>What a verdict here is, and is not</h3>\n");
    out.push_str("<ul>\n");
    out.push_str(
        "<li><strong>The schema is the one this repository vendored</strong>, compiled into \
         the module and re-hashed against the digest <code>specs.toml</code> records for it \
         before any verdict is issued. There is no way to hand it a schema of your own — that \
         would move the provenance decision to whoever opened the page, and this project \
         exists because of a file that was vendored with no version, no source and no \
         date.</li>\n",
    );
    out.push_str(
        "<li><strong>It cannot speak for the files on disk today.</strong> The registry and \
         the schema are frozen into the module together, so the check here catches a registry \
         edited without re-hashing and nothing else. Drift in a checkout is what \
         <code>conform registry verify</code> is for, in CI.</li>\n",
    );
    out.push_str(
        "<li id=\"demo-panic\"><strong>A panic is fatal to this validator.</strong> The \
         sentence that appears here is read from the module rather than written on this \
         page.</li>\n",
    );
    out.push_str("</ul>\n");

    out.push_str("<h3>What is being loaded</h3>\n");
    out.push_str("<div class=\"scroller\">\n<table class=\"evidence\">\n<thead><tr>");
    for heading in ["Artefact", "Bytes"] {
        let _ = write!(out, "<th scope=\"col\">{}</th>", for_html(heading));
    }
    out.push_str("</tr></thead>\n<tbody>\n");
    for (href, bytes) in [
        (&module.script_href, module.script_bytes),
        (&module.wasm_href, module.wasm_bytes),
    ] {
        let _ = writeln!(
            out,
            "<tr><th scope=\"row\"><code>{}</code></th><td>{}</td></tr>",
            for_html(href),
            for_html(&bytes.to_string()),
        );
    }
    out.push_str("</tbody>\n</table>\n</div>\n");
    out.push_str(
        "<p>Measured when this page was generated, uncompressed. A static host serves both \
         compressed; the module is mostly the JSON Schema implementation and the Unicode \
         tables it reaches through.</p>\n",
    );

    out.push_str(
        "<p>The same check runs from a terminal against a checkout, where the registry and \
         the vendored bytes are both on disk and the digest is verified against them rather \
         than against a copy compiled in alongside.</p>\n",
    );
    out.push_str(
        "<pre><code>cargo run -p conform-cli -- validate path/to/contract.yaml\n\
         cargo run -p conform-cli -- registry verify --check</code></pre>\n",
    );

    out.push_str("</div>\n");
    out.push_str("<script type=\"module\">\n");
    out.push_str(DEMO_SCRIPT);
    out.push_str("</script>\n");
}

/// Where the page came from.
fn colophon(out: &mut String, site: &Site) {
    out.push_str("<footer>\n");
    let _ = writeln!(
        out,
        "<p>Generated by <code>{}</code> {} from <code>{}</code> (registry format \
         {}) and the manifests under <code>crates/</code>. Every specification fact above is \
         read from those files at build time; none of it is written into the generator.</p>",
        for_html(env!("CARGO_PKG_NAME")),
        for_html(env!("CARGO_PKG_VERSION")),
        for_html(&site.registry_path),
        for_html(&site.registry_schema_version.to_string()),
    );
    out.push_str("</footer>\n");
}

/// One provenance row, present or honestly absent.
fn field(out: &mut String, label: &str, value: Option<&str>) {
    let _ = writeln!(
        out,
        "<div class=\"row\"><dt>{}</dt><dd>{}</dd></div>",
        for_html(label),
        match value {
            Some(value) => format!("<code>{}</code>", for_html(value)),
            None => absent_span(),
        },
    );
}

/// One provenance row whose value is a URL.
fn linked_field(out: &mut String, label: &str, value: Option<&str>) {
    let _ = write!(out, "<div class=\"row\"><dt>{}</dt><dd>", for_html(label));
    link_or_absence(out, value);
    out.push_str("</dd></div>\n");
}

/// A link, if the value is one this page is willing to make clickable;
/// otherwise the value as text; otherwise the absence.
///
/// # Why a scheme check, on data from a file in this repository
///
/// Because `href` is not just text. `javascript:` and `data:` URLs execute
/// with the page's origin, and HTML-escaping does nothing to them — the
/// quoting is perfectly correct and the link still runs code. The registry is
/// trusted in the sense that this repository owns it, and trusting it *here*
/// would still be the wrong shape: the value is transcribed from a
/// third-party README by a human, this page is the one place it becomes
/// executable, and an allow-list of two schemes costs nothing.
///
/// A value that is not a link is shown as text rather than dropped, because
/// dropping it would hide from a reader that the registry holds something
/// unexpected — which is the one thing a provenance page must never do.
fn link_or_absence(out: &mut String, value: Option<&str>) {
    let Some(value) = value else {
        absence(out);
        return;
    };

    if is_web_url(value) {
        let _ = write!(
            out,
            "<a href=\"{}\" rel=\"noopener noreferrer\">{}</a>",
            for_html(value),
            for_html(value),
        );
    } else {
        let _ = write!(
            out,
            "<code class=\"not-a-link\" title=\"not an http(s) URL; shown as text\">{}</code>",
            for_html(value),
        );
    }
}

/// Whether a recorded URL is one this page will emit as an `href`.
///
/// An allow-list, not a deny-list. A deny-list of `javascript:` and `data:`
/// would have to keep up with every scheme a browser ever adds; two schemes
/// are what a canonical upstream link is actually written in.
fn is_web_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// The marked-up absence, for a cell.
fn absence(out: &mut String) {
    out.push_str(&absent_span());
}

/// The marked-up absence, as a string.
fn absent_span() -> String {
    format!("<span class=\"absent\">{}</span>", for_html(NOT_RECORDED))
}

/// A value in a `<code>`, or the absence.
fn code_or_absence(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("<code>{}</code>", for_html(value)),
        None => absent_span(),
    }
}

/// The lower-cased haystack the in-page filter matches against.
///
/// Built here rather than in the script, so that the script never has to read
/// text out of the document and can restrict itself to setting an attribute.
/// Absent fields contribute nothing, which is what makes a search for a
/// licence find only the entries that record one.
fn filter_key(spec: &SpecSummary) -> String {
    let mut key = format!("{} {}", spec.id, spec.name);
    for value in [
        spec.version.as_deref(),
        spec.steward.as_deref(),
        spec.licence.as_deref(),
        spec.pinned_ref.as_deref(),
        spec.homepage.as_deref(),
        spec.repository.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        key.push(' ');
        key.push_str(value);
    }
    key.to_lowercase()
}

/// The page's stylesheet.
///
/// Inline, like everything else: one file that renders from `file://` is
/// easier to deploy, easier to archive and easier to assert on than a page
/// plus an asset directory.
/// The script that drives the in-browser validator.
///
/// # No value from this generator is interpolated into it
///
/// It is a constant. The one thing it needs from the page — where the module
/// is — it reads from a `data-` attribute that went through
/// [`escape::for_html`] like every other value on the page. That keeps the
/// rule this module opens with intact: there is one escaping boundary here,
/// HTML, and nothing is ever escaped for a second grammar.
///
/// # No `innerHTML`, and this is the section where that matters most
///
/// A diagnostic message quotes the document that caused it, verbatim — the
/// adapters deliberately do not sanitise what they quote, because escaping is
/// not idempotent and belongs at the point of display. Here the document is
/// whatever the reader pasted in. Every finding is therefore built with
/// `createElement` and written with `textContent`, which is the browser's own
/// escaping and the only kind that cannot be got wrong by forgetting a call.
/// A single `innerHTML` in this function would make the page a
/// self-XSS machine.
///
/// # A failure to load is shown, not swallowed
///
/// A page opened from `file://` cannot import an ES module or fetch a `.wasm`,
/// and neither can one served without the right media type. The import is
/// therefore in a `try`, and what a reader gets is the browser's own error in
/// the status line rather than a validate button that silently does nothing.
const DEMO_SCRIPT: &str = r#"
const panel = document.getElementById("demo");
const status = document.getElementById("demo-status");
const ui = document.getElementById("demo-ui");

function say(text) {
  // textContent, never innerHTML: see this constant's documentation.
  status.textContent = text;
}

try {
  const wasm = await import(panel.dataset.module);
  await wasm.default();

  const specs = document.getElementById("demo-spec");
  for (const id of wasm.reachableSpecIds()) {
    const option = document.createElement("option");
    option.value = id;
    option.textContent = id;
    specs.append(option);
  }

  // The panic contract, in the module's own words rather than this page's.
  const panic = document.getElementById("demo-panic");
  const sentence = document.createElement("span");
  sentence.textContent = " " + wasm.panicContract();
  panic.append(sentence);

  const document_field = document.getElementById("demo-document");
  const verdict = document.getElementById("demo-verdict");
  const findings = document.getElementById("demo-findings");
  const provenance = document.getElementById("demo-provenance");

  function render(report) {
    findings.replaceChildren();
    const counts = report.document;
    verdict.hidden = false;
    verdict.className = "verdict verdict-" + (counts.worst_severity || "none");
    verdict.textContent = counts.diagnostics === 0
      ? "no findings"
      : counts.error + " error(s), " + counts.warning + " warning(s), " + counts.info + " note(s)";

    for (const finding of report.diagnostics) {
      const item = document.createElement("li");
      item.className = "finding finding-" + finding.severity;

      const code = document.createElement("code");
      code.textContent = finding.code;
      item.append(code);

      const message = document.createElement("span");
      message.className = "finding-message";
      message.textContent = " " + finding.message;
      item.append(message);

      if (finding.location && finding.location.pointer) {
        const where = document.createElement("code");
        where.className = "finding-where";
        where.textContent = finding.location.pointer;
        item.append(" ", where);
      }

      if (finding.help) {
        const help = document.createElement("p");
        help.className = "finding-help";
        help.textContent = finding.help;
        item.append(help);
      }

      findings.append(item);
    }
  }

  document.getElementById("demo-run").addEventListener("click", function () {
    let validator = null;
    try {
      validator = new wasm.ConformValidator(specs.value);
      provenance.textContent = validator.provenance;
      render(JSON.parse(validator.validate("pasted-document", document_field.value)));
      say("Validated in this tab, against the schema compiled into the module.");
    } catch (error) {
      // Either a refusal — an unknown spec id, a schema that failed its
      // digest — or a trap, which is fatal to the instance whatever this
      // catch does with it. Both are shown; neither is treated as a verdict.
      verdict.hidden = true;
      findings.replaceChildren();
      say("The validator could not produce a report: " + error);
    } finally {
      if (validator) { validator.free(); }
    }
  });

  ui.hidden = false;
  say("Validator " + wasm.conformVersion() + " loaded. Paste a contract and press Validate.");
} catch (error) {
  // A page opened from file://, a host serving the wrong media type, a build
  // that wrote a truncated module: all of them land here, and all of them are
  // shown rather than swallowed.
  say("The validator could not be loaded, so nothing on this panel can validate anything: "
      + error);
}
"#;

const STYLESHEET: &str = r#"
:root {
  --bg: #fbfbfa; --fg: #16161d; --muted: #5d5d6b; --rule: #dcdcd6;
  --card: #ffffff; --accent: #1f4e79; --absent: #8a6a1f; --absent-bg: #fdf6e3;
  --ok: #1d6b3f; --bad: #9b1c1c; --unknown: #6b5b1d; --code-bg: #f2f2ee;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #14151a; --fg: #e8e8ea; --muted: #9a9aa8; --rule: #2c2d36;
    --card: #1b1c23; --accent: #8ab4dd; --absent: #d6b45e; --absent-bg: #2a2413;
    --ok: #5fc98a; --bad: #f08a8a; --unknown: #cbb668; --code-bg: #23242c;
  }
}
* { box-sizing: border-box; }
body {
  margin: 0; background: var(--bg); color: var(--fg);
  font: 16px/1.6 ui-sans-serif, -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
}
code, pre, .digest { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }
code { background: var(--code-bg); padding: 0.1em 0.35em; border-radius: 3px; font-size: 0.9em; }
pre { background: var(--code-bg); padding: 1rem; border-radius: 6px; overflow-x: auto; }
pre code { background: none; padding: 0; }
a { color: var(--accent); }
h1, h2, h3, h4 { line-height: 1.25; }
.masthead, nav, main, footer { max-width: 62rem; margin: 0 auto; padding: 0 1.25rem; }
.masthead { padding-top: 3rem; }
.masthead h1 { font-size: 2.4rem; margin: 0 0 0.5rem; }
.tagline { font-size: 1.15rem; color: var(--muted); margin-top: 0; }
.at-a-glance { display: flex; flex-wrap: wrap; gap: 1.5rem; margin: 2rem 0 0; padding: 0; }
.stat dt { font-size: 0.8rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); }
.stat dd { margin: 0; font-size: 1.9rem; font-variant-numeric: tabular-nums; }
nav { position: sticky; top: 0; background: var(--bg); border-bottom: 1px solid var(--rule); z-index: 5; }
nav ul { display: flex; flex-wrap: wrap; gap: 1.25rem; list-style: none; margin: 0; padding: 0.9rem 0; }
nav a { text-decoration: none; font-size: 0.95rem; }
nav a[aria-current="true"] { font-weight: 700; text-decoration: underline; }
section { padding: 3rem 0 1rem; border-bottom: 1px solid var(--rule); scroll-margin-top: 4rem; }
section > h2 { font-size: 1.8rem; margin-top: 0; }
.caveat { color: var(--muted); font-size: 0.95rem; border-left: 3px solid var(--rule); padding-left: 1rem; }
.rules dt { font-weight: 700; margin-top: 1rem; }
.rules dd { margin: 0.25rem 0 0; color: var(--muted); }
.scroller { overflow-x: auto; }
table { border-collapse: collapse; width: 100%; font-size: 0.92rem; }
th, td { text-align: left; vertical-align: top; padding: 0.7rem 0.9rem 0.7rem 0; border-bottom: 1px solid var(--rule); }
thead th { font-size: 0.78rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--muted); white-space: nowrap; }
.digest { font-size: 0.74rem; word-break: break-all; line-height: 1.35; display: inline-block; }

/* The watch table carries a 64-digit digest and a 40-character commit SHA in
   the same row. Left to size itself it grows past the text column and pushes
   its last column out of sight — which is what it did, and which only a
   screenshot showed. `table-layout: fixed` with explicit shares makes the
   table exactly as wide as the column and wraps the long values instead, and
   the `min-width` keeps it scrollable rather than crushed on a narrow screen. */
.watch { table-layout: fixed; min-width: 44rem; }
.watch th, .watch td { overflow-wrap: anywhere; }
/* `thead th` is `nowrap` so a heading does not wrap in an auto-sized table.
   In these fixed-layout ones the column width is decided in advance, so a
   heading too long for its share overlaps the next instead of wrapping — it
   did, and the screenshot is what showed it. */
.watch thead th, .crates thead th { white-space: normal; }
.watch col.c-spec { width: 13%; }
.watch col.c-upstream { width: 28%; }
.watch col.c-pin { width: 21%; }
.watch col.c-digest { width: 17%; }
.watch col.c-poll { width: 21%; }
.crates { table-layout: fixed; min-width: 42rem; }
.crates th, .crates td { overflow-wrap: anywhere; }
.crates col.c-name { width: 17%; }
.crates col.c-version { width: 10%; }
.crates col.c-published { width: 14%; }
.crates col.c-what { width: 41%; }
.crates col.c-manifest { width: 18%; }
.muted { color: var(--muted); font-size: 0.85rem; }
.absent {
  color: var(--absent); background: var(--absent-bg); border: 1px dashed currentColor;
  border-radius: 3px; padding: 0.05em 0.4em; font-size: 0.85em; white-space: nowrap;
}
.badge { display: inline-block; border-radius: 999px; padding: 0.15em 0.7em; font-size: 0.78rem; border: 1px solid currentColor; white-space: nowrap; }
.badge-matched { color: var(--ok); }
.badge-drifted { color: var(--bad); }
.badge-missing, .badge-unreadable { color: var(--unknown); }
.badge-adapter { color: var(--ok); }
.badge-no-adapter { color: var(--muted); }
.card { background: var(--card); border: 1px solid var(--rule); border-radius: 10px; padding: 1.5rem; margin: 1.5rem 0; }
.card h3 { margin-top: 0; }
.card-name { font-weight: 400; color: var(--muted); }
.provenance { margin: 1rem 0; }
.provenance .row { display: grid; grid-template-columns: 11rem 1fr; gap: 0.5rem; padding: 0.3rem 0; border-top: 1px solid var(--rule); }
.provenance dt { color: var(--muted); font-size: 0.85rem; }
.provenance dd { margin: 0; overflow-wrap: anywhere; }
.diagnostic { font-size: 0.88rem; color: var(--muted); }
.gaps { background: var(--absent-bg); border-left: 3px solid var(--absent); padding: 0.7rem 1rem; font-size: 0.92rem; }
.gaps.none { background: none; border-left-color: var(--ok); color: var(--muted); }
.notes { margin-top: 1.25rem; border-top: 1px solid var(--rule); padding-top: 0.75rem; }
.notes summary { cursor: pointer; font-weight: 600; font-size: 0.95rem; }
.notes p { white-space: pre-wrap; font-size: 0.93rem; color: var(--muted); }
.filter { display: flex; align-items: center; gap: 0.75rem; }
.filter input { flex: 1; max-width: 26rem; padding: 0.5rem 0.75rem; font: inherit; border: 1px solid var(--rule); border-radius: 6px; background: var(--card); color: var(--fg); }
.not-wired { background: var(--absent-bg); border-left: 3px solid var(--absent); padding: 1rem 1.25rem; }
.visually-hidden {
  position: absolute; width: 1px; height: 1px; margin: -1px; padding: 0;
  overflow: hidden; clip-path: inset(50%); white-space: nowrap;
}
.demo-controls { display: flex; gap: 0.75rem; align-items: center; flex-wrap: wrap; margin: 1rem 0 0.5rem; }
.demo-controls select, .demo-controls button {
  font: inherit; padding: 0.4rem 0.7rem; border: 1px solid var(--rule);
  border-radius: 6px; background: var(--card); color: var(--fg);
}
.demo-controls button { cursor: pointer; border-color: var(--accent); color: var(--accent); }
#demo-document {
  width: 100%; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 0.88rem; line-height: 1.5; padding: 0.75rem; border: 1px solid var(--rule);
  border-radius: 6px; background: var(--code-bg); color: var(--fg); resize: vertical;
}
.verdict { margin: 0.75rem 0; font-weight: 600; }
.verdict-error { color: var(--bad); }
.verdict-warning { color: var(--unknown); }
.verdict-info, .verdict-none { color: var(--ok); }
.findings { list-style: none; margin: 0.5rem 0; padding: 0; }
.finding { border-left: 3px solid var(--rule); padding: 0.4rem 0 0.4rem 0.75rem; margin: 0.4rem 0; }
.finding-error { border-left-color: var(--bad); }
.finding-warning { border-left-color: var(--unknown); }
.finding-info { border-left-color: var(--ok); }
.finding-message { overflow-wrap: anywhere; }
.finding-where { font-size: 0.8rem; color: var(--muted); }
.finding-help { margin: 0.25rem 0 0; font-size: 0.9rem; color: var(--muted); }
.provenance { font-size: 0.85rem; color: var(--muted); overflow-wrap: anywhere; }
.evidence th[scope="row"] { white-space: nowrap; padding-right: 1.25rem; }
.evidence th[scope="row"] code { font-size: 0.8rem; }
.not-a-link { border: 1px dashed var(--absent); }
footer { padding: 2rem 1.25rem 4rem; color: var(--muted); font-size: 0.88rem; }
@media (max-width: 40rem) {
  .provenance .row { grid-template-columns: 1fr; }
  .digest { max-width: none; }
}
"#;

/// The page's script.
///
/// Two behaviours, both of which the page works without: a filter that hides
/// cards, and a navigation link that marks itself current. Neither reads text
/// out of the document, neither builds an element from a string, and there is
/// no `innerHTML` — see this module's documentation for why that is a rule and
/// not a preference.
const SCRIPT: &str = r#"
(function () {
  "use strict";

  var input = document.getElementById("spec-filter");
  var status = document.getElementById("filter-status");
  var cards = Array.prototype.slice.call(document.querySelectorAll(".card[data-filter]"));

  if (input && status) {
    input.addEventListener("input", function () {
      var needle = input.value.trim().toLowerCase();
      var shown = 0;
      cards.forEach(function (card) {
        var hit = needle === "" || card.getAttribute("data-filter").indexOf(needle) !== -1;
        card.hidden = !hit;
        if (hit) { shown += 1; }
      });
      // textContent, never innerHTML: the only text set at runtime is two
      // numbers this script counted itself.
      status.textContent = needle === ""
        ? ""
        : shown + " of " + cards.length + " shown";
    });
  }

  var links = Array.prototype.slice.call(document.querySelectorAll("nav a[href^='#']"));
  var sections = links
    .map(function (a) { return document.getElementById(a.getAttribute("href").slice(1)); })
    .filter(Boolean);

  if ("IntersectionObserver" in window && sections.length) {
    var seen = new Set();
    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        if (entry.isIntersecting) { seen.add(entry.target.id); } else { seen.delete(entry.target.id); }
      });
      links.forEach(function (a) {
        var id = a.getAttribute("href").slice(1);
        if (seen.has(id)) { a.setAttribute("aria-current", "true"); }
        else { a.removeAttribute("aria-current"); }
      });
    }, { rootMargin: "-20% 0px -70% 0px" });
    sections.forEach(function (section) { observer.observe(section); });
  }
})();
"#;

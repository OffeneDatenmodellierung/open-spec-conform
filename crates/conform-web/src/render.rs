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
//! # The script touches no data
//!
//! The page carries about forty lines of JavaScript, and what they do is
//! toggle the `hidden` attribute on elements the generator already wrote.
//! Nothing is interpolated into a script literal, no element is built from a
//! string at runtime, and there is no `innerHTML` anywhere. That is a design
//! decision rather than a coincidence: a generator that escapes perfectly for
//! HTML and then hands the same value to `innerHTML` has escaped for the wrong
//! grammar, and the second boundary is the one people forget. There is no
//! second boundary here.

use std::fmt::Write as _;

use conform_cli::model::{SpecSummary, VerifyStatus};

use crate::escape::for_html;
use crate::manifests::CrateEntry;
use crate::site::{Demo, Site, SpecCard};

/// What the page calls a field nobody could determine.
///
/// The console's words, deliberately. `conform-cli` prints `(not recorded)` in
/// its catalogue and its console, and a reader who has seen it there must not
/// have to work out that "unknown", "n/a" and "—" mean the same thing here.
const NOT_RECORDED: &str = "(not recorded)";

/// Render the whole page.
///
/// One self-contained HTML document: the stylesheet and the script are inline,
/// no asset is fetched, and nothing is loaded from a network. It therefore
/// renders identically from `file://`, from a static host and from an archive,
/// which is also what makes it possible for a test to assert on exactly the
/// bytes a reader will see.
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
         says what was looked at and what it did not say. {} of the {} entries below have at \
         least one such gap, and this page shows every one of them as \
         <span class=\"absent\">{}</span> rather than as a blank — because a blank looks like a \
         rendering accident, and these are answers.</dd>\n</dl>\n</section>\n",
        for_html(&site.with_gaps().to_string()),
        for_html(&site.specs.len().to_string()),
        for_html(NOT_RECORDED),
    );
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

    out.push_str("<div class=\"scroller\">\n<table class=\"watch\">\n<thead><tr>");
    for heading in [
        "Specification",
        "Canonical upstream",
        "Pinned to",
        "Vendored digest",
        "Bytes",
        "In tree since",
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
fn watch_row(out: &mut String, card: &SpecCard) {
    let spec = &card.summary;
    out.push_str("<tr>");

    let _ = write!(
        out,
        "<th scope=\"row\"><a href=\"#spec-{}\"><code>{}</code></a><br><span class=\"muted\">{}</span></th>",
        for_html(&spec.id),
        for_html(&spec.id),
        for_html(&spec.name),
    );

    out.push_str("<td>");
    upstream_cell(out, spec);
    out.push_str("</td>");

    let _ = write!(
        out,
        "<td>{}</td>",
        code_or_absence(spec.pinned_ref.as_deref())
    );
    let _ = write!(
        out,
        "<td><code class=\"digest\">{}</code></td>",
        for_html(&spec.sha256),
    );
    out.push_str("<td>");
    verify_badge(out, spec.verify);
    out.push_str("</td>");
    let _ = write!(out, "<td>{}</td>", code_or_absence(Some(&spec.fetched_at)));

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
/// the five entries in this repository's registry take that fallback because
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

    out.push_str("<div class=\"scroller\">\n<table class=\"crates\">\n<thead><tr>");
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
.digest { font-size: 0.74rem; word-break: break-all; line-height: 1.35; display: inline-block; max-width: 20ch; }
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

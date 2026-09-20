//! Is this *good* OKF?
//!
//! A different question from [`conformance`](crate::conformance), and the
//! split is the whole reason this is a second type rather than a flag.
//! Conformance asks whether the bundle **is** OKF; hygiene asks whether it is
//! **good** OKF. Nothing here is a conformance failure, which is why no rule
//! below raises an error and why
//! [`OkfHygiene::gate_policy`] is [`GatePolicy::report_only`].

use std::collections::{BTreeMap, BTreeSet};

use conform_core::{ConformanceReport, GatePolicy, SpecRef, Validator};
use okf_core::{Bundle, Concept, Document, Frontmatter, PREFERRED_KEY_ORDER, Status, TrustTier};

use crate::codes;
use crate::context::Cx;
use crate::index::indexed_concepts;
use crate::spec_ref;

/// The hygiene rules: upstream's `L1`..`L12`, and one of ours.
///
/// ```
/// use conform_core::{GatePolicy, Severity, Validator};
/// use conform_okf::{Bundle, OkfHygiene};
///
/// let bundle = Bundle::load(
///     concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/okf-upstream/acme_retail"),
/// )?;
///
/// let report = OkfHygiene.validate(&bundle);
///
/// // Hygiene reports; it never gates. Both halves of that are checked: the
/// // rules cannot raise an error, and the policy would not fail on one.
/// assert_eq!(report.count(Severity::Error), 0);
/// assert_eq!(OkfHygiene.gate_policy(), GatePolicy::report_only());
/// assert!(!report.should_gate(OkfHygiene.gate_policy()));
/// # Ok::<(), okf_core::BundleError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OkfHygiene;

impl OkfHygiene {
    /// The policy this check is meant to be gated under: none.
    ///
    /// Hygiene is a set of opinions about a bundle that already conforms. A
    /// consumer is free to pass
    /// [`GatePolicy::warnings_as_errors`](conform_core::GatePolicy::warnings_as_errors)
    /// to [`Validator::check`] and fail their own build on it — that is their
    /// call, and the reason the policy is a parameter rather than a constant.
    /// What this says is what the *check* asks for, which is nothing.
    #[must_use]
    pub const fn gate_policy(self) -> GatePolicy {
        GatePolicy::report_only()
    }
}

impl Validator for OkfHygiene {
    type Document = Bundle;

    fn spec(&self) -> SpecRef {
        spec_ref()
    }

    fn validate(&self, bundle: &Bundle) -> ConformanceReport {
        let mut cx = Cx::new(bundle.root());
        let indexed = indexed_concepts(bundle);

        for concept in bundle.concepts() {
            cx.at(concept);
            let doc = &concept.document;
            let fm = &doc.frontmatter;

            lint_headings(&mut cx, doc);
            lint_key_order(&mut cx, fm);
            lint_unused_sources(&mut cx, concept, doc);
            lint_actor_convention(&mut cx, concept);
            lint_computation_block(&mut cx, doc);
            lint_whitespace(&mut cx, doc);
            lint_orphan(&mut cx, concept, &indexed);
            lint_portable_id(&mut cx, concept);
            lint_self_link(&mut cx, bundle, concept);
            lint_unverified(&mut cx, concept);
            lint_draft(&mut cx, concept);
        }

        cx.finish().into_iter().collect()
    }
}

/// `L1` (no top-level heading), `L3` (more than one, or a skipped level) and
/// `L4` (a heading with nothing under it).
///
/// One traversal, because all three are questions about the same heading list
/// and splitting them would walk the body three times to no purpose.
fn lint_headings(cx: &mut Cx<'_>, doc: &Document) {
    let headings = okf_core::markdown::extract_headings(&doc.body);
    if headings.is_empty() {
        cx.warn(
            codes::NO_TOP_LEVEL_HEADING,
            None,
            "body has no top-level `#` heading; OKF docs conventionally open with one",
        );
        return;
    }

    let mut top = 0usize;
    let mut previous = 0usize;
    for (i, heading) in headings.iter().enumerate() {
        if heading.level == 1 {
            top += 1;
            if top > 1 {
                cx.warn(
                    codes::HEADING_STRUCTURE,
                    None,
                    format!(
                        "multiple top-level `#` headings found (heading `{}` at line {})",
                        heading.text, heading.line_num
                    ),
                );
            }
        }
        if previous > 0 && heading.level > previous + 1 {
            cx.warn(
                codes::HEADING_STRUCTURE,
                None,
                format!(
                    "heading level skipped: `{}` jumps from h{previous} to h{}",
                    heading.text, heading.level
                ),
            );
        }
        previous = heading.level;

        // Empty when nothing but blank lines follows, *and* nothing is nested
        // beneath. A heading whose next sibling is deeper is a container — the
        // content is under its subheadings, not missing.
        let starts = heading.line_index + 1;
        let ends = headings
            .get(i + 1)
            .map_or_else(|| doc.body.lines().count(), |h| h.line_index);
        let contains_a_deeper_heading = headings
            .get(i + 1)
            .is_some_and(|next| next.level > heading.level);
        let empty = !contains_a_deeper_heading
            && doc
                .body
                .lines()
                .skip(starts)
                .take(ends.saturating_sub(starts))
                .all(|l| l.trim().is_empty());
        if empty {
            cx.warn(
                codes::EMPTY_HEADING,
                None,
                format!("heading `{}` has no content", heading.text),
            );
        }
    }

    if top == 0 {
        cx.warn(
            codes::NO_TOP_LEVEL_HEADING,
            None,
            "body has no top-level `#` heading; OKF docs conventionally open with one",
        );
    }
}

/// `L2`: frontmatter keys in the canonical order.
///
/// Only the keys §5 names are ordered. A producer's own keys are theirs and
/// are skipped, so a bundle is not nagged for carrying extra metadata.
fn lint_key_order(cx: &mut Cx<'_>, fm: &Frontmatter) {
    let rank: BTreeMap<&str, usize> = PREFERRED_KEY_ORDER
        .iter()
        .enumerate()
        .map(|(i, k)| (*k, i))
        .collect();
    let ranked: Vec<usize> = fm.keys().filter_map(|k| rank.get(k).copied()).collect();
    if ranked.windows(2).any(|w| w[0] > w[1]) {
        cx.info(
            codes::KEY_ORDER,
            Some("§5"),
            "frontmatter keys are not in canonical order (§5's reading order)",
        );
    }
}

/// `L5`: a declared source nobody cites.
fn lint_unused_sources(cx: &mut Cx<'_>, concept: &Concept, doc: &Document) {
    let cited: BTreeSet<String> = doc
        .footnote_refs()
        .into_iter()
        .map(|r| r.label)
        .chain(doc.footnote_definitions().into_iter().map(|d| d.label))
        .collect();
    for source in concept.sources() {
        let Some(id) = &source.id else { continue };
        if !cited.contains(id) {
            cx.warn(
                codes::UNUSED_SOURCE,
                None,
                format!(
                    "source `{id}` is declared in frontmatter but never cited with footnote `[^{id}]`"
                ),
            );
        }
    }
}

/// `L6`: §7's actor convention on a source's author.
fn lint_actor_convention(cx: &mut Cx<'_>, concept: &Concept) {
    for source in concept.sources() {
        let Some(author) = &source.author else {
            continue;
        };
        if author.kind() == okf_core::ActorKind::Other {
            cx.info(
                codes::ACTOR_CONVENTION,
                Some("§7"),
                format!(
                    "author `{}` in `sources.author` does not follow §7's `human:<id>`, \
                     `process:<id>` or `<producer>/<version>` convention",
                    author.as_str()
                ),
            );
        }
    }
}

/// `L7`: a `# Computation` block with no language tag.
///
/// Reported, not acted on. A separate syntax checker is what would read the
/// code, and an untagged block is exactly the one it must skip — so this rule
/// is the reason a reader ever sees "skipped" there.
fn lint_computation_block(cx: &mut Cx<'_>, doc: &Document) {
    if let Some(inline) = doc.inline_computation()
        && inline.fenced
        && inline.language.is_none()
    {
        cx.warn(
            codes::UNTAGGED_COMPUTATION_BLOCK,
            None,
            "`# Computation` code block carries no language tag \
             (e.g. ```sql), so no syntax check can read it",
        );
    }
}

/// `L8`: trailing whitespace in the body.
fn lint_whitespace(cx: &mut Cx<'_>, doc: &Document) {
    let offending: Vec<usize> = doc
        .body
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.is_empty() && l.trim_end() != *l)
        .map(|(i, _)| i + 1)
        .collect();
    if let Some(first) = offending.first() {
        cx.info(
            codes::TRAILING_WHITESPACE,
            None,
            format!(
                "trailing whitespace found on {} line(s) in markdown body (first at line {first})",
                offending.len()
            ),
        );
    }
}

/// `L9`: a concept no index lists.
fn lint_orphan(cx: &mut Cx<'_>, concept: &Concept, indexed: &BTreeSet<String>) {
    if !indexed.contains(&concept.id.to_string()) {
        cx.warn(
            codes::ORPHAN,
            None,
            "no `index.md` lists this concept, so nothing walking the bundle's \
             listings will reach it",
        );
    }
}

/// `R1`: a concept id that will not survive every checkout.
///
/// **`R` and not `L`, deliberately.** `L1`..`L12` are upstream's hygiene rules
/// and that namespace is theirs; this one is ours, and the specification
/// states no portability requirement for path segments — §6 constrains what a
/// path *means*, not what characters it may contain. Numbering it `L13` would
/// both claim their vocabulary and imply a conformance basis it does not have.
fn lint_portable_id(cx: &mut Cx<'_>, concept: &Concept) {
    for segment in concept.id.segments() {
        if !okf_core::concept_id::is_portable_segment(segment) {
            cx.warn(
                codes::UNPORTABLE_ID_SEGMENT,
                None,
                format!(
                    "concept-id segment `{segment}` may not survive a checkout on every \
                     filesystem; the specification does not forbid it, but a consumer on \
                     a case-insensitive or restricted filesystem cannot read the bundle"
                ),
            );
        }
    }
}

/// `L10`: a concept that links to itself.
fn lint_self_link(cx: &mut Cx<'_>, bundle: &Bundle, concept: &Concept) {
    if bundle
        .links_from(&concept.id)
        .iter()
        .any(|l| l.target == concept.id)
    {
        cx.warn(
            codes::SELF_LINK,
            None,
            "self-link; a concept that links to itself usually signals a stray reference",
        );
    }
}

/// `L11`: nothing has confirmed this concept.
fn lint_unverified(cx: &mut Cx<'_>, concept: &Concept) {
    if concept.trust_tier() == TrustTier::Unverified {
        cx.info(
            codes::UNVERIFIED,
            None,
            "no `verified` events; trust tier is `unverified`",
        );
    }
}

/// `L12`: a draft concept.
fn lint_draft(cx: &mut Cx<'_>, concept: &Concept) {
    if concept.status() == Status::Draft {
        cx.warn(
            codes::DRAFT,
            None,
            "`status: draft`; a draft concept is not ready for production consumption",
        );
    }
}

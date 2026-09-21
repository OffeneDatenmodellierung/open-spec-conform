//! Does this bundle conform to OKF v0.2?
//!
//! The question [`hygiene`](crate::hygiene) does *not* ask. Conformance asks
//! whether the bundle **is** OKF; hygiene asks whether it is **good** OKF.

use std::collections::{BTreeMap, BTreeSet};

use conform_core::{ConformanceReport, GatePolicy, SpecRef, Validator};
use okf_core::{Bundle, Concept, Document, Frontmatter, Status, Value};

use crate::codes;
use crate::context::Cx;
use crate::index::index_listings;
use crate::resolve::{bundle_relative, resolve_resource};
use crate::{OKF_VERSION, spec_ref};

/// Conformance with the OKF v0.2 specification.
///
/// ```
/// use conform_core::{GatePolicy, Validator};
/// use conform_okf::{Bundle, OkfConformance};
///
/// let bundle = Bundle::load(
///     concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/okf-upstream/ga4"),
/// )?;
///
/// let report = OkfConformance.validate(&bundle);
/// assert_eq!(OkfConformance.spec().to_string(), "okf@0.2");
///
/// // Reporting and gating are separate questions, and this one gates on
/// // errors — a bundle that is not OKF is not a matter of taste.
/// assert_eq!(OkfConformance.gate_policy(), GatePolicy::errors_only());
/// # let _ = report;
/// # Ok::<(), okf_core::BundleError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OkfConformance;

impl OkfConformance {
    /// The policy this check is meant to be gated under: errors fail, and
    /// nothing else does.
    ///
    /// Warnings deliberately do not fail. §11 tells a consumer not to reject a
    /// document over a soft-guidance deviation, and a check that failed on one
    /// would be unusable against real third-party bundles — of the diagnostics
    /// the reference validator reports over the four published bundles, none
    /// is an error, so a gate that failed on warnings would reject the
    /// specification's own corpus.
    ///
    /// This is [`GatePolicy::errors_only`], which is also
    /// [`GatePolicy::default`]. It is stated rather than inherited so that the
    /// contrast with [`OkfHygiene::gate_policy`](crate::OkfHygiene::gate_policy)
    /// is visible in one place.
    #[must_use]
    pub const fn gate_policy(self) -> GatePolicy {
        GatePolicy::errors_only()
    }
}

impl Validator for OkfConformance {
    type Document = Bundle;

    fn spec(&self) -> SpecRef {
        spec_ref()
    }

    fn validate(&self, bundle: &Bundle) -> ConformanceReport {
        let root = bundle.root();
        let mut cx = Cx::new(root);

        // A document that did not parse is a conformance error, and it is the
        // only class here that is: everything else is a judgement about a
        // document we could read.
        for (path, error) in bundle.parse_errors() {
            cx.at_file(path);
            cx.err(
                codes::UNREADABLE_DOCUMENT,
                None,
                format!("not a readable OKF document: {error}"),
            );
        }

        for concept in bundle.concepts() {
            cx.at(concept);
            let doc = &concept.document;
            let fm = &doc.frontmatter;

            check_type(&mut cx, concept, fm);
            check_recommended(&mut cx, doc);
            check_empty_body(&mut cx, doc);
            check_tags(&mut cx, fm);
            check_trust(&mut cx, fm);
            check_lifecycle(&mut cx, fm);
            check_usage_window(&mut cx, fm);
            check_attribution(&mut cx, doc);
            check_legacy(&mut cx, doc, fm);
            check_computation(&mut cx, concept);
            check_resources(&mut cx, bundle, concept);
            check_link_targets(&mut cx, bundle, concept);
            check_reserved_filename(&mut cx, concept);
        }

        check_declared_version(&mut cx, bundle);
        check_duplicate_titles(&mut cx, bundle);
        check_circular_derivation(&mut cx, bundle);
        check_stale_indexes(&mut cx, bundle);

        cx.finish()
    }
}

/// §4.1: every concept carries a non-empty `type`.
fn check_type(cx: &mut Cx<'_>, concept: &Concept, fm: &Frontmatter) {
    if concept.type_().is_none_or(|t| t.trim().is_empty()) {
        cx.err(
            codes::TYPE_MISSING,
            Some("§4.1"),
            "`type` is missing or empty; §4.1 requires one on every concept",
        );
    }
    // An unknown *value* is not an error. §11 tells a consumer to read
    // liberally, and a producer's vocabulary is theirs — this is the line
    // between "not OKF" and "not our OKF".
    if let Some(t) = fm.type_()
        && !t.trim().is_empty()
        && t.trim() != t
    {
        cx.info(
            codes::TYPE_PADDED,
            Some("§11"),
            format!(
                "`type` has surrounding whitespace (`{t}`); consumers that compare it literally will not match"
            ),
        );
    }
}

/// §4.1's recommended keys. Always a warning — conformance forbids rejecting a
/// concept over an optional field, however much a producer wants it filled in.
fn check_recommended(cx: &mut Cx<'_>, doc: &Document) {
    for key in doc.missing_recommended() {
        cx.warn(
            codes::RECOMMENDED_KEY_MISSING,
            Some("§4.1"),
            format!("recommended key `{key}` is missing"),
        );
    }
}

fn check_empty_body(cx: &mut Cx<'_>, doc: &Document) {
    if doc.body.trim().is_empty() {
        cx.warn(
            codes::EMPTY_BODY,
            None,
            "body is empty; a concept should carry at least one line of prose or code",
        );
    }
}

/// §4.1: `tags` is a list of short strings.
///
/// A bare scalar is the shape one upstream bundle writes in seven documents.
/// A liberal *reader* accepts it (§11); this reports it, because a producer
/// that wrote one string meant one tag and most consumers will read none.
fn check_tags(cx: &mut Cx<'_>, fm: &Frontmatter) {
    match fm.get("tags") {
        Some(Value::String(_)) => cx.warn(
            codes::TAGS_SHAPE,
            Some("§4.1"),
            "`tags` should be a list of short strings, found a string; \
             a strict consumer reads no tags from it",
        ),
        Some(Value::Sequence(items)) => {
            if let Some(bad) = items.iter().find(|v| !matches!(v, Value::String(_))) {
                cx.warn(
                    codes::TAGS_SHAPE,
                    Some("§4.1"),
                    format!(
                        "`tags` contains a non-string entry ({}); §4.1 asks for short strings",
                        kind_of(bad)
                    ),
                );
            }
        }
        Some(other) => cx.warn(
            codes::TAGS_SHAPE,
            Some("§4.1"),
            format!(
                "`tags` should be a list of short strings, found {}",
                kind_of(other)
            ),
        ),
        None => {}
    }
}

/// §5.2: the `generated` and `verified` trust events.
fn check_trust(cx: &mut Cx<'_>, fm: &Frontmatter) {
    if let Some(generated) = fm.generated() {
        if generated.by.is_none() {
            cx.warn(
                codes::GENERATED_BY_MISSING,
                Some("§5.2"),
                "`generated.by` is required within `generated`",
            );
        }
        match &generated.at {
            None => cx.warn(
                codes::GENERATED_AT_MISSING,
                Some("§5.2"),
                "`generated.at` is required within `generated`",
            ),
            Some(at) if at.datetime.is_none() => cx.warn(
                codes::GENERATED_AT_MALFORMED,
                Some("§5.2"),
                format!("`generated.at` is not an ISO-8601 datetime (`{}`)", at.raw),
            ),
            Some(_) => {}
        }
    }

    // Present-but-empty is the case worth reporting: it asserts that
    // verification happened and then names nobody.
    if fm.contains_key("verified") {
        let events = fm.verified();
        if events.is_empty() {
            cx.warn(
                codes::VERIFIED_EMPTY,
                Some("§5.2"),
                "`verified` is present but contains no `{ by, at }` events",
            );
        }
        for (i, event) in events.iter().enumerate() {
            if event.by.is_none() {
                cx.warn(
                    codes::VERIFIED_BY_MISSING,
                    Some("§5.2"),
                    format!("`verified[{i}].by` is missing"),
                );
            }
            match &event.at {
                None => cx.warn(
                    codes::VERIFIED_AT_MISSING,
                    Some("§5.2"),
                    format!("`verified[{i}].at` is missing"),
                ),
                Some(at) if at.datetime.is_none() => cx.warn(
                    codes::VERIFIED_AT_MALFORMED,
                    Some("§5.2"),
                    format!(
                        "`verified[{i}].at` is not an ISO-8601 datetime (`{}`)",
                        at.raw
                    ),
                ),
                Some(_) => {}
            }
        }
    }
}

/// §5.4 lifecycle, and §5.5's `stale_after` — its **syntax**, never its
/// relation to now.
fn check_lifecycle(cx: &mut Cx<'_>, fm: &Frontmatter) {
    if let Status::Other(value) = Status::parse(fm.get("status").and_then(as_str)) {
        cx.info(
            codes::STATUS_UNKNOWN,
            Some("§5.4"),
            format!(
                "`status: {value}` is outside §5.4's `draft | stable | deprecated`; \
                 consumers must tolerate it, but few will act on it"
            ),
        );
    }
    if let Some(raw) = fm.get("stale_after").and_then(as_str)
        && okf_core::DateTime::parse(raw).is_none()
    {
        cx.warn(
            codes::STALE_AFTER_MALFORMED,
            Some("§5.5"),
            format!(
                "`stale_after` is not an ISO-8601 datetime (`{raw}`), so no consumer can act on it"
            ),
        );
    }
}

/// §5.1: a `usage_window` frames sources, so it needs some to frame.
fn check_usage_window(cx: &mut Cx<'_>, fm: &Frontmatter) {
    if fm.usage_window().is_some() && fm.sources().is_empty() {
        cx.warn(
            codes::USAGE_WINDOW_WITHOUT_SOURCES,
            Some("§5.1"),
            "`usage_window` is present without `sources` to frame",
        );
    }
}

/// §5.1: a footnote is the join key between body prose and a `sources` entry.
/// (The footnote syntax itself is §4.2.)
fn check_attribution(cx: &mut Cx<'_>, doc: &Document) {
    for attribution in doc.attributions() {
        if attribution.source.is_none() {
            cx.warn(
                codes::FOOTNOTE_WITHOUT_SOURCE,
                Some("§5.1"),
                format!(
                    "footnote [^{}] matches no `sources[].id`; the label is the join key for attribution",
                    attribution.label
                ),
            );
        }
    }
}

/// §13.1: the keys and body conventions v0.2 superseded.
fn check_legacy(cx: &mut Cx<'_>, doc: &Document, fm: &Frontmatter) {
    if fm.timestamp().is_some() {
        cx.warn(
            codes::LEGACY_TIMESTAMP,
            Some("§13.1"),
            "`timestamp` is superseded by `generated.at` (§13.1)",
        );
    }
    if doc.has_legacy_citations() {
        cx.warn(
            codes::LEGACY_CITATIONS,
            Some("§13.1"),
            "the body `# Citations` list is superseded by `sources` (§13.1)",
        );
    }
}

/// §10: an Attested Computation carries a runnable, checkable contract.
fn check_computation(cx: &mut Cx<'_>, concept: &Concept) {
    let Some(computation) = concept.attested_computation() else {
        return;
    };

    if computation.runtime.as_deref().is_none_or(str::is_empty) {
        cx.warn(
            codes::COMPUTATION_RUNTIME_MISSING,
            Some("§10"),
            "`runtime` is missing; without it nothing knows how to run the computation",
        );
    }
    for (i, parameter) in computation.parameters.iter().enumerate() {
        if parameter.name.is_none() {
            cx.warn(
                codes::COMPUTATION_PARAMETER_NAME_MISSING,
                Some("§10"),
                format!("`parameters[{i}].name` is missing"),
            );
        }
        if parameter.type_.is_none() {
            cx.warn(
                codes::COMPUTATION_PARAMETER_TYPE_MISSING,
                Some("§10"),
                format!("`parameters[{i}].type` is missing"),
            );
        }
    }
    match &computation.executor {
        None => cx.warn(
            codes::COMPUTATION_EXECUTOR_MISSING,
            Some("§10"),
            "missing `executor`: nothing says how to run the computation",
        ),
        Some(e) if e.resource.is_none() => cx.warn(
            codes::COMPUTATION_EXECUTOR_RESOURCE_MISSING,
            Some("§10"),
            "`executor.resource` is missing; it names the run instructions or code",
        ),
        Some(_) => {}
    }
    match &computation.attester {
        None => cx.warn(
            codes::COMPUTATION_ATTESTER_MISSING,
            Some("§10"),
            "missing `attester`: nothing can check a run's receipt",
        ),
        Some(a) if a.resource.is_none() => cx.warn(
            codes::COMPUTATION_ATTESTER_RESOURCE_MISSING,
            Some("§10"),
            "`attester.resource` is missing; it names the deterministic check",
        ),
        Some(_) => {}
    }
    if computation.computation.is_missing() {
        cx.warn(
            codes::COMPUTATION_ABSENT,
            Some("§10"),
            "no computation: neither a `# Computation` block nor a `computation:` path is present, \
             so there is nothing for an executor to run or an attester to check",
        );
    }
    if computation.has_redundant_inline {
        cx.warn(
            codes::COMPUTATION_REDUNDANT_INLINE,
            Some("§10"),
            "both a `# Computation` block and a `computation:` path are present; \
             §10 asks for one or the other, and two copies can disagree",
        );
    }
}

/// Every `resource:` that names something inside the bundle must be there.
///
/// A URL is left alone, and not because it is uninteresting: whether
/// `https://…` resolves is a network question, this crate is offline by
/// construction, and [`resolve_resource`] returns
/// [`Resolution::NotInspected`] for it rather than an answer nobody earned.
/// Only [`Resolution::DoesNotExist`] becomes a finding.
fn check_resources(cx: &mut Cx<'_>, bundle: &Bundle, concept: &Concept) {
    let check = |label: &str, raw: &str, cx: &mut Cx<'_>| {
        if resolve_resource(bundle, raw).does_not_exist() {
            cx.warn(
                codes::RESOURCE_MISSING,
                None,
                format!("{label} names `{raw}`, which the bundle does not contain"),
            );
        }
    };

    if let Some(resource) = concept.document.frontmatter.resource() {
        check("`resource`", &resource, cx);
    }
    for source in concept.sources() {
        if let Some(resource) = &source.resource {
            check("a `sources` entry", resource, cx);
        }
    }
    if let Some(computation) = concept.attested_computation() {
        if let Some(e) = computation
            .executor
            .as_ref()
            .and_then(|e| e.resource.clone())
        {
            check("`executor.resource`", &e, cx);
        }
        if let Some(a) = computation
            .attester
            .as_ref()
            .and_then(|a| a.resource.clone())
        {
            check("`attester.resource`", &a, cx);
        }
        if let Some(path) = computation.computation.path() {
            check("`computation`", path, cx);
        }
    }
}

/// Links that resolve, and what they resolve *to*.
///
/// A broken cross-link is `info`, not an error: §6 permits it, and a bundle is
/// often one half of a set. A link to a **deprecated** concept is a warning,
/// because the target is telling the reader to go somewhere else.
fn check_link_targets(cx: &mut Cx<'_>, bundle: &Bundle, concept: &Concept) {
    // Deprecation is reported once per *target*, not once per link. A document
    // that mentions a retired concept twice has one problem.
    let mut deprecated: BTreeSet<String> = BTreeSet::new();
    for link in bundle.links_from(&concept.id) {
        if !link.exists {
            // `exists` means "resolves to a concept", so a link to a diagram or
            // a data file beside the concept lands here — and saying the bundle
            // "does not contain" a file it demonstrably does is simply false.
            // `check_resources` above settles this the right way for
            // frontmatter paths, by asking the filesystem; this asks the same
            // question of link targets.
            if bundle.resolve_path_field(&concept.id, &link.raw).is_none() {
                cx.info(
                    codes::LINK_TARGET_MISSING,
                    Some("§6"),
                    format!(
                        "link `{}` names `{}`, which the bundle does not contain; \
                         §6 tells a consumer to tolerate this",
                        link.raw, link.target
                    ),
                );
            }
            continue;
        }
        if let Some(target) = bundle.get(&link.target)
            && target.status().is_deprecated()
        {
            deprecated.insert(link.target.to_string());
        }
    }
    for target in deprecated {
        cx.warn(
            codes::LINK_TO_DEPRECATED,
            None,
            format!("links to deprecated concept `{target}`"),
        );
    }
}

/// §3.1: the reserved filenames a concept document may not take.
///
/// One of the few plain `MUST NOT`s in the specification, so one of the few
/// errors here.
fn check_reserved_filename(cx: &mut Cx<'_>, concept: &Concept) {
    let name = concept
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    if okf_core::RESERVED_FILENAMES.contains(&name) {
        cx.err(
            codes::RESERVED_FILENAME,
            Some("§3.1"),
            format!(
                "`{name}` is a reserved filename and §3.1 forbids using it for a concept document"
            ),
        );
    }
}

/// §12: a declared OKF version this crate does not implement.
///
/// **Absence is not reported**, and that is the specification's decision
/// rather than leniency: §8 and §12 both say a bundle-root `index.md` *MAY*
/// carry `okf_version`. An earlier draft of this rule warned when it was
/// missing, and that warning fired on **all four** bundles published with the
/// specification — the same shape as any check that disagrees with an entire
/// corpus, and the same conclusion.
///
/// A version we do not implement is `info` rather than a warning because §12
/// tells a consumer that does not understand the declared version to "attempt
/// best-effort consumption rather than refusing the bundle" — which is what
/// this crate does, so the note is for the reader and not against the bundle.
fn check_declared_version(cx: &mut Cx<'_>, bundle: &Bundle) {
    if let Some(version) = bundle.okf_version()
        && version != OKF_VERSION
    {
        cx.at_file(&bundle.root().join("index.md"));
        cx.info(
            codes::UNIMPLEMENTED_VERSION,
            Some("§12"),
            format!(
                "the bundle declares `okf_version: {version}`; this reader implements {OKF_VERSION}, \
                 so it is read best-effort (§12)"
            ),
        );
        cx.at_bundle();
    }
}

/// Two concepts with the same title are indistinguishable in any listing.
fn check_duplicate_titles(cx: &mut Cx<'_>, bundle: &Bundle) {
    let mut by_title: BTreeMap<String, Vec<&Concept>> = BTreeMap::new();
    for concept in bundle.concepts() {
        by_title
            .entry(concept.display_title())
            .or_default()
            .push(concept);
    }
    for (title, concepts) in by_title {
        if concepts.len() < 2 {
            continue;
        }
        let others: Vec<String> = concepts.iter().map(|c| c.id.to_string()).collect();
        for concept in &concepts {
            cx.at(concept);
            // Formatted once per concept rather than once per comparison.
            let self_id = concept.id.to_string();
            let siblings: Vec<&String> = others.iter().filter(|id| **id != self_id).collect();
            cx.warn(
                codes::DUPLICATE_TITLE,
                None,
                format!(
                    "title `{title}` is shared with {}; \
                     the two are indistinguishable in any listing that shows titles",
                    siblings
                        .iter()
                        .map(|s| format!("`{s}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            );
        }
    }
}

/// A concept derived, through `sources`, from itself.
///
/// An **error**, unlike every other provenance finding: a cycle means no
/// reader can establish where the claim came from, and following it is
/// unbounded.
fn check_circular_derivation(cx: &mut Cx<'_>, bundle: &Bundle) {
    // Edges: concept → the concepts its `sources` name.
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for concept in bundle.concepts() {
        let from = concept.id.to_string();
        for source in concept.sources() {
            let Some(resource) = &source.resource else {
                continue;
            };
            let Some(relative) = bundle_relative(resource) else {
                continue;
            };
            // A self-edge is kept. A concept whose `sources` names itself is a
            // cycle of length one — the shortest way to make provenance
            // unresolvable — and dropping it as "not really an edge" was the
            // one shape this rule could not see.
            if let Some(id) = okf_core::links::concept_id_for_path(&relative)
                && bundle.contains(&id)
            {
                edges
                    .entry(from.clone())
                    .or_default()
                    .insert(id.to_string());
            }
        }
    }

    // Depth-first, reporting the cycle's members rather than only its
    // existence: "there is a cycle" is not actionable, and a reader needs the
    // ring.
    let mut seen: BTreeSet<String> = BTreeSet::new();
    // Keyed by the ring's *members*, not by where the walk happened to start.
    // `a → b → a` and `b → a → b` are one cycle seen from two ends, and keying
    // on the start reported it once per member — two errors for one ring.
    let mut reported: BTreeSet<Vec<String>> = BTreeSet::new();
    for start in edges.keys() {
        let mut stack = vec![(start.clone(), vec![start.clone()])];
        while let Some((node, trail)) = stack.pop() {
            for next in edges.get(&node).into_iter().flatten() {
                if next == start {
                    let ring = trail.join(" → ");
                    let mut members = trail.clone();
                    members.sort();
                    members.dedup();
                    if reported.insert(members) {
                        if let Some(concept) = bundle
                            .concepts()
                            .iter()
                            .find(|c| c.id.to_string() == *start)
                        {
                            cx.at(concept);
                        }
                        cx.err(
                            codes::CIRCULAR_DERIVATION,
                            None,
                            format!(
                                "circular derivation: {ring} → {start}; \
                                 no reader can establish where this claim came from"
                            ),
                        );
                    }
                    continue;
                }
                if seen.insert(format!("{start}\u{0}{next}")) {
                    let mut trail = trail.clone();
                    trail.push(next.clone());
                    stack.push((next.clone(), trail));
                }
            }
        }
    }
}

/// An `index.md` that lists a concept the bundle no longer contains.
///
/// The other direction — a concept no index lists — is hygiene rather than
/// conformance, and is `OKFL09`.
fn check_stale_indexes(cx: &mut Cx<'_>, bundle: &Bundle) {
    for index in bundle.index_files() {
        cx.at_file(index);
        for (target, resolved) in index_listings(bundle, index) {
            if !resolved.exists() {
                cx.warn(
                    codes::STALE_INDEX_ENTRY,
                    None,
                    format!(
                        "index lists `{target}`, which no longer exists; \
                         a reader following the listing lands on nothing"
                    ),
                );
            }
        }
    }
    cx.at_bundle();
}

fn as_str(value: &Value) -> Option<&str> {
    match value {
        Value::String(s) => Some(s.as_str()),
        _ => None,
    }
}

/// A frontmatter value's shape, for a diagnostic that says what was found
/// rather than only what was wanted.
const fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Int(_) => "an integer",
        Value::Float(_) => "a number",
        Value::String(_) => "a string",
        Value::Sequence(_) => "a list",
        Value::Mapping(_) => "a mapping",
    }
}

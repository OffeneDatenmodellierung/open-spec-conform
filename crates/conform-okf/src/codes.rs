//! The stable codes this crate raises findings under.
//!
//! Stability is the contract `conform-core` asks for: downstream tooling,
//! suppression lists and CI annotations match on these strings, so a code is
//! retired rather than reused, and the message attached to one may be reworded
//! freely while the code may not.
//!
//! # Why these exist at all
//!
//! The implementation this crate is migrated from attached a code to hygiene
//! findings only, and none to conformance findings, on the reasoning that
//! conformance rules are the specification's rather than ours and numbering
//! them "would invent an identifier scheme OKF does not have". That reasoning
//! was right about OKF and wrong about the report: `conform_core::Diagnostic`
//! requires a code because a finding a consumer cannot name is a finding a
//! consumer cannot suppress, annotate or count. A `None` code is not a
//! statement that the rule belongs to upstream — it is a hole where the
//! machine-readable identity should be.
//!
//! So the codes below are **this crate's**, and the `OKF` prefix says so. They
//! name the rule, not a clause: the clause the rule is drawn from travels on
//! [`Diagnostic::spec_ref`](conform_core::Diagnostic::spec_ref) instead, where
//! it belongs, and is what a reader should quote when arguing with a finding.
//!
//! # The scheme
//!
//! | Range | What it covers |
//! |---|---|
//! | `OKF0xx` | the document: required keys, shape, body |
//! | `OKF1xx` | trust, provenance and lifecycle (§5) |
//! | `OKF2xx` | constructs v0.2 superseded (§13.1) |
//! | `OKF3xx` | attested computations (§10) |
//! | `OKF4xx` | references, and the bundle as a whole (§3, §6, §8, §12) |
//! | `OKF9xx` | the bundle could not be read at all |
//! | `OKFLnn` | upstream hygiene rule `Ln` |
//! | `OKFRnn` | a hygiene rule that is **ours**, with no basis in the spec |
//!
//! `OKFL01` is upstream's `L1`, `OKFL12` is upstream's `L12`, and the mapping
//! is machine-readable in [`HYGIENE_RULES`] rather than only written here. The
//! prefix is not decoration: a report may hold findings from four adapters at
//! once, and a bare `L1` in that report names nothing.
//!
//! `OKFR01` is deliberately outside the `L` namespace. `L1`..`L12` are
//! upstream's rules and that namespace is theirs; a rule of our own numbered
//! `L13` would both claim their vocabulary and imply a conformance basis it
//! does not have.

// ---------------------------------------------------------------------------
// OKF0xx — the document
// ---------------------------------------------------------------------------

/// A file in the bundle is not a readable OKF document.
///
/// The only conformance error about a document rather than about the bundle:
/// everything else here is a judgement about a document that could be read.
pub const UNREADABLE_DOCUMENT: &str = "OKF001";

/// §4.1: a concept carries no `type`, or an empty one.
pub const TYPE_MISSING: &str = "OKF002";

/// `type` has surrounding whitespace, so a consumer comparing it literally
/// will not match. Information, not a defect: §11 says read liberally.
pub const TYPE_PADDED: &str = "OKF003";

/// §4.1: a recommended key is absent.
///
/// Always a warning. Conformance forbids rejecting a concept over an optional
/// field, however much a producer wants it filled in.
pub const RECOMMENDED_KEY_MISSING: &str = "OKF004";

/// The body carries no prose or code at all.
pub const EMPTY_BODY: &str = "OKF005";

/// §4.1: `tags` is not a list of short strings.
pub const TAGS_SHAPE: &str = "OKF006";

/// A fenced code block in the body does not parse as the language it is
/// tagged with.
///
/// **Raised only by [`OkfSyntax`](crate::OkfSyntax), which exists only when the
/// `syntax` feature is on.** The code is declared unconditionally all the same:
/// a code is an identifier a consumer writes into a suppression list, and one
/// that appears and disappears with a build flag is not an identifier.
///
/// Always *information*, never a warning, and the direction is deliberate.
/// Only 2 of the 54 concepts in the published corpus are Attested
/// Computations; everything else fenced in an OKF document is illustrative,
/// and documentation is full of fragments that no parser accepts as a
/// standalone unit. Upstream's six spurious warnings over that corpus come
/// from treating every block as if it had to run. See
/// [`COMPUTATION_CODE_SYNTAX`] for the block that does.
pub const CODE_BLOCK_SYNTAX: &str = "OKF007";

// ---------------------------------------------------------------------------
// OKF1xx — trust, provenance and lifecycle (§5)
// ---------------------------------------------------------------------------

/// §5.2: `generated` names no actor.
pub const GENERATED_BY_MISSING: &str = "OKF101";

/// §5.2: `generated` carries no timestamp.
pub const GENERATED_AT_MISSING: &str = "OKF102";

/// §5.2: `generated.at` is not an ISO-8601 datetime.
pub const GENERATED_AT_MALFORMED: &str = "OKF103";

/// §5.2: `verified` is present and holds no events — an assertion that
/// verification happened, naming nobody.
pub const VERIFIED_EMPTY: &str = "OKF104";

/// §5.2: a `verified` event names no actor.
pub const VERIFIED_BY_MISSING: &str = "OKF105";

/// §5.2: a `verified` event carries no timestamp.
pub const VERIFIED_AT_MISSING: &str = "OKF106";

/// §5.2: a `verified` event's timestamp is not an ISO-8601 datetime.
pub const VERIFIED_AT_MALFORMED: &str = "OKF107";

/// §5.4: `status` is outside `draft | stable | deprecated`.
///
/// Information: consumers must tolerate it, but few will act on it.
pub const STATUS_UNKNOWN: &str = "OKF108";

/// §5.5: `stale_after` is not an ISO-8601 datetime, so no consumer can act on
/// it.
///
/// Checked for **syntax** and never against the clock — see the crate
/// documentation on determinism.
pub const STALE_AFTER_MALFORMED: &str = "OKF109";

/// §5.1: a `usage_window` is present with no `sources` to frame.
pub const USAGE_WINDOW_WITHOUT_SOURCES: &str = "OKF110";

/// §5.1: a body footnote matches no `sources[].id`, so the attribution has no
/// join key.
pub const FOOTNOTE_WITHOUT_SOURCE: &str = "OKF111";

/// A concept derived, through `sources`, from itself.
///
/// An **error**, unlike every other provenance finding: a cycle means no
/// reader can establish where the claim came from, and following it is
/// unbounded.
pub const CIRCULAR_DERIVATION: &str = "OKF112";

// ---------------------------------------------------------------------------
// OKF2xx — superseded constructs (§13.1)
// ---------------------------------------------------------------------------

/// §13.1: `timestamp` is superseded by `generated.at`.
pub const LEGACY_TIMESTAMP: &str = "OKF201";

/// §13.1: a body `# Citations` list is superseded by `sources`.
pub const LEGACY_CITATIONS: &str = "OKF202";

// ---------------------------------------------------------------------------
// OKF3xx — attested computations (§10)
// ---------------------------------------------------------------------------

/// §10: no `runtime`, so nothing knows how to run the computation.
pub const COMPUTATION_RUNTIME_MISSING: &str = "OKF301";

/// §10: a parameter with no `name`.
pub const COMPUTATION_PARAMETER_NAME_MISSING: &str = "OKF302";

/// §10: a parameter with no `type`.
pub const COMPUTATION_PARAMETER_TYPE_MISSING: &str = "OKF303";

/// §10: no `executor`.
pub const COMPUTATION_EXECUTOR_MISSING: &str = "OKF304";

/// §10: `executor` names no `resource`.
pub const COMPUTATION_EXECUTOR_RESOURCE_MISSING: &str = "OKF305";

/// §10: no `attester`, so nothing can check a run's receipt.
pub const COMPUTATION_ATTESTER_MISSING: &str = "OKF306";

/// §10: `attester` names no `resource`.
pub const COMPUTATION_ATTESTER_RESOURCE_MISSING: &str = "OKF307";

/// §10: neither a `# Computation` block nor a `computation:` path.
pub const COMPUTATION_ABSENT: &str = "OKF308";

/// §10: both a `# Computation` block and a `computation:` path, which is two
/// copies that can disagree.
pub const COMPUTATION_REDUNDANT_INLINE: &str = "OKF309";

/// §10: the `# Computation` code block does not parse as the language it is
/// tagged with.
///
/// **Raised only by [`OkfSyntax`](crate::OkfSyntax), which exists only when the
/// `syntax` feature is on**, for the same reason [`CODE_BLOCK_SYNTAX`] is
/// declared unconditionally.
///
/// A *warning* where [`CODE_BLOCK_SYNTAX`] is information, because this is the
/// one block in an OKF document that something is expected to **execute**: §10
/// says an Attested Computation declares a `runtime` and an `executor`, and
/// code that does not parse cannot be run by either. It is still not an error
/// — a bundle whose computation will not compile is a bundle with a bug in it,
/// not a document that fails to be OKF, and [`OkfSyntax`](crate::OkfSyntax)
/// gates on nothing in any case.
pub const COMPUTATION_CODE_SYNTAX: &str = "OKF310";

// ---------------------------------------------------------------------------
// OKF4xx — references, and the bundle as a whole
// ---------------------------------------------------------------------------

/// A `resource:` names a path inside the bundle that the bundle does not
/// contain.
pub const RESOURCE_MISSING: &str = "OKF401";

/// §6: a body link names something the bundle does not contain.
///
/// Information, not a defect: §6 tells a consumer to tolerate this, and a
/// bundle is often one half of a set.
pub const LINK_TARGET_MISSING: &str = "OKF402";

/// A link to a concept whose `status` is `deprecated` — the target is telling
/// the reader to go somewhere else.
///
/// Raised once per target, not once per link: a document that mentions a
/// retired concept twice has one problem.
pub const LINK_TO_DEPRECATED: &str = "OKF403";

/// §3.1: a concept document uses a reserved filename. One of the few plain
/// `MUST NOT`s in the specification, so one of the few errors here.
pub const RESERVED_FILENAME: &str = "OKF404";

/// §12: the bundle declares an `okf_version` this crate does not implement, so
/// it is read best-effort.
///
/// Information rather than a warning, and the direction is deliberate: §12
/// tells a consumer that does not understand the declared version to attempt
/// best-effort consumption rather than refuse the bundle, so the note is for
/// the reader and not against the bundle.
pub const UNIMPLEMENTED_VERSION: &str = "OKF405";

/// Two concepts share a title, and are therefore indistinguishable in any
/// listing that shows titles.
pub const DUPLICATE_TITLE: &str = "OKF406";

/// An `index.md` lists a concept document that is no longer there.
pub const STALE_INDEX_ENTRY: &str = "OKF407";

// ---------------------------------------------------------------------------
// OKF9xx — the bundle could not be read at all
// ---------------------------------------------------------------------------

/// The path is not a loadable OKF bundle.
///
/// Distinct from [`UNREADABLE_DOCUMENT`], and the distinction is the whole
/// point: that one says "this bundle contains a file I could not parse", which
/// is a finding *about a bundle we read*. This one says there was no bundle to
/// read, so nothing below it ran and an empty report would be a lie.
pub const BUNDLE_UNREADABLE: &str = "OKF900";

// ---------------------------------------------------------------------------
// Hygiene — upstream's L-rules, and one of ours
// ---------------------------------------------------------------------------

/// `L1`: the body has no top-level `#` heading.
pub const NO_TOP_LEVEL_HEADING: &str = "OKFL01";

/// `L2`: frontmatter keys are not in §5's reading order.
pub const KEY_ORDER: &str = "OKFL02";

/// `L3`: more than one top-level heading, or a skipped heading level.
pub const HEADING_STRUCTURE: &str = "OKFL03";

/// `L4`: a heading with nothing under it.
pub const EMPTY_HEADING: &str = "OKFL04";

/// `L5`: a source declared in frontmatter that no footnote cites.
pub const UNUSED_SOURCE: &str = "OKFL05";

/// `L6`: a `sources.author` outside §7's actor convention.
pub const ACTOR_CONVENTION: &str = "OKFL06";

/// `L7`: a `# Computation` code block with no language tag, which no syntax
/// check can read.
pub const UNTAGGED_COMPUTATION_BLOCK: &str = "OKFL07";

/// `L8`: trailing whitespace in the markdown body.
pub const TRAILING_WHITESPACE: &str = "OKFL08";

/// `L9`: a concept no `index.md` lists, so nothing walking the bundle's
/// listings reaches it.
pub const ORPHAN: &str = "OKFL09";

/// `L10`: a concept that links to itself.
pub const SELF_LINK: &str = "OKFL10";

/// `L11`: no `verified` events; the trust tier is `unverified`.
pub const UNVERIFIED: &str = "OKFL11";

/// `L12`: `status: draft`.
pub const DRAFT: &str = "OKFL12";

/// `R1`: a concept-id segment that may not survive a checkout on every
/// filesystem.
///
/// **Ours, and numbered outside the `L` namespace for that reason.** The
/// specification states no portability requirement for path segments — §6
/// constrains what a path *means*, not what characters it may contain.
pub const UNPORTABLE_ID_SEGMENT: &str = "OKFR01";

/// Every hygiene code, paired with the rule identifier it carries forward.
///
/// The mapping is here rather than only in prose because it is the one piece
/// of information the migration could have lost: the implementation this crate
/// replaces put upstream's `L1`..`L12` in the finding's code field, and a
/// consumer that had learned those identifiers needs a way to follow them.
///
/// ```
/// use conform_okf::codes;
///
/// // Every hygiene finding this crate can raise is listed, exactly once.
/// assert_eq!(codes::HYGIENE_RULES.len(), 13);
///
/// let l1 = codes::HYGIENE_RULES
///     .iter()
///     .find(|(code, _)| *code == codes::NO_TOP_LEVEL_HEADING)
///     .expect("L1 is a hygiene rule");
/// assert_eq!(l1.1, "L1");
///
/// // `R1` is ours, and says so by not being an `L`.
/// assert_eq!(codes::upstream_rule(codes::UNPORTABLE_ID_SEGMENT), Some("R1"));
/// assert_eq!(codes::upstream_rule(codes::RESERVED_FILENAME), None);
/// ```
pub const HYGIENE_RULES: &[(&str, &str)] = &[
    (NO_TOP_LEVEL_HEADING, "L1"),
    (KEY_ORDER, "L2"),
    (HEADING_STRUCTURE, "L3"),
    (EMPTY_HEADING, "L4"),
    (UNUSED_SOURCE, "L5"),
    (ACTOR_CONVENTION, "L6"),
    (UNTAGGED_COMPUTATION_BLOCK, "L7"),
    (TRAILING_WHITESPACE, "L8"),
    (ORPHAN, "L9"),
    (SELF_LINK, "L10"),
    (UNVERIFIED, "L11"),
    (DRAFT, "L12"),
    (UNPORTABLE_ID_SEGMENT, "R1"),
];

/// The rule identifier a hygiene code carries forward, if it is a hygiene
/// code.
///
/// `None` for every conformance code: those never had a rule identifier, which
/// is the gap [`HYGIENE_RULES`] exists to record rather than paper over.
#[must_use]
pub fn upstream_rule(code: &str) -> Option<&'static str> {
    HYGIENE_RULES
        .iter()
        .find(|(hygiene, _)| *hygiene == code)
        .map(|(_, rule)| *rule)
}

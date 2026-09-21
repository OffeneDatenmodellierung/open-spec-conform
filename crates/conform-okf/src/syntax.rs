//! Does the code in this bundle's fenced blocks parse?
//!
//! A third question, and therefore a third type — not a flag on either of the
//! other two.
//!
//! # Why this is a separate check and not an extra rule in `OkfHygiene`
//!
//! Because a feature must not change what an existing check says. Folding
//! these rules into [`OkfHygiene`](crate::OkfHygiene) would make its report a
//! function of the build configuration as well as of the bundle: two consumers
//! on the same version of this crate would get different findings over the same
//! bytes, and neither could tell why. `conform-core` states that rule for the
//! `serde` feature — serialization must never change which diagnostics are
//! produced — and the reasoning behind it is not about serialization.
//!
//! So [`OkfConformance`](crate::OkfConformance) and
//! [`OkfHygiene`](crate::OkfHygiene) are byte-identical in every feature
//! configuration, which
//! `tests/the_syntax_feature_adds_a_check_and_changes_no_other.rs` asserts by
//! running them, and the `syntax` feature adds a third check that a caller has
//! to ask for by name.
//!
//! # Why this does not become a `Check` variant
//!
//! [`Check`](crate::Check) is a public, exhaustive enum. A variant added under
//! a feature would make a downstream `match` compile or not depending on
//! whether *some other crate in the graph* turned the feature on, because cargo
//! unifies features across a build. That is a breaking change delivered by a
//! third party, which is worse than an inconsistency. [`OkfSyntax`] is a
//! [`Validator`] like the other two and returns a plain
//! [`ConformanceReport`]; a caller that wants it merged with the other two
//! merges it.
//!
//! # What "clean" means here, and what it does not
//!
//! Every parser in [`conform_okf_syntax`] is optional, so a language can be
//! *unsupported in this build* rather than *clean*, and a block whose language
//! this build cannot read is skipped silently rather than reported as passing.
//! That is deliberate — a finding saying "not checked" on every Python block in
//! a corpus that contains none would be noise — but it means a caller
//! summarising this report must not describe an empty one as "the code is
//! fine". [`conform_okf_syntax::checkable_languages`] is what such a caller
//! should say instead, and is re-exported from the crate root for that reason.

use conform_core::{ConformanceReport, GatePolicy, SpecRef, Validator};
use conform_okf_syntax::{FencedCodeBlock, Language, check_syntax, extract_fenced_code_blocks};
use okf_core::{Bundle, Document};

use crate::codes;
use crate::context::Cx;
use crate::spec_ref;

/// The code-parsing checks: the `# Computation` block, and every other fenced
/// block in the body.
///
/// ```
/// use conform_core::{GatePolicy, Severity, Validator};
/// use conform_okf::{Bundle, OkfSyntax};
///
/// let bundle = Bundle::load(
///     concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/okf-upstream/acme_retail"),
/// )?;
///
/// let report = OkfSyntax.validate(&bundle);
///
/// // Like hygiene, this reports and never gates: code that does not parse is a
/// // bug in a bundle, not a failure to be OKF.
/// assert_eq!(report.count(Severity::Error), 0);
/// assert_eq!(OkfSyntax.gate_policy(), GatePolicy::report_only());
/// assert!(!report.should_gate(OkfSyntax.gate_policy()));
/// # Ok::<(), okf_core::BundleError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OkfSyntax;

impl OkfSyntax {
    /// The policy this check is meant to be gated under: none.
    ///
    /// Whether a fenced `sql` block is valid SQL says nothing about whether a
    /// bundle is valid OKF — a document full of pseudocode is perfectly
    /// conformant — so nothing here can fail a build on this crate's say-so. A
    /// consumer who wants their own build to fail on a computation that will
    /// not parse passes
    /// [`GatePolicy::warnings_as_errors`](conform_core::GatePolicy::warnings_as_errors)
    /// to [`Validator::check`]; that is their call, and the reason the policy
    /// is a parameter rather than a constant.
    #[must_use]
    pub const fn gate_policy(self) -> GatePolicy {
        GatePolicy::report_only()
    }
}

impl Validator for OkfSyntax {
    type Document = Bundle;

    fn spec(&self) -> SpecRef {
        spec_ref()
    }

    fn validate(&self, bundle: &Bundle) -> ConformanceReport {
        let mut cx = Cx::new(bundle.root());

        for concept in bundle.concepts() {
            cx.at(concept);
            let doc = &concept.document;
            let sanctioned = check_computation(&mut cx, doc);
            check_body_blocks(&mut cx, doc, sanctioned.as_deref());
        }

        cx.finish()
    }
}

/// `OKF310`: the `# Computation` block, which is the one block something is
/// expected to run.
///
/// Returns the code it checked, so [`check_body_blocks`] can leave it alone:
/// the `# Computation` block is part of the body, so without this the one
/// block that matters would be reported twice, once at each severity.
fn check_computation(cx: &mut Cx<'_>, doc: &Document) -> Option<String> {
    let inline = doc.inline_computation()?;
    let tag = inline.language.clone()?;

    // An untagged block is `OKFL07`'s business, not this one. That rule exists
    // precisely so a reader knows why a syntax check said nothing here.
    if let Err(error) = check_syntax(&tag, &inline.code) {
        cx.warn(
            codes::COMPUTATION_CODE_SYNTAX,
            Some("§10"),
            format!(
                "the `# Computation` block does not parse as `{tag}`: {}",
                error.message
            ),
        );
    }
    Some(inline.code)
}

/// `OKF007`: every other fenced block in the body.
fn check_body_blocks(cx: &mut Cx<'_>, doc: &Document, sanctioned: Option<&str>) {
    for block in extract_fenced_code_blocks(&doc.body) {
        let Some(tag) = block.language.as_deref() else {
            continue;
        };
        if sanctioned.is_some_and(|code| code.trim() == block.code.trim()) {
            continue;
        }
        // An unrecognised tag — `mermaid`, `text` — is not a finding, and
        // neither is a language no backend in this build can read. Asking
        // `Language` first keeps the two apart from a block that genuinely
        // failed to parse.
        if Language::from_tag(tag) == Language::Unknown {
            continue;
        }
        if let Err(error) = check_syntax(tag, &block.code) {
            cx.info(codes::CODE_BLOCK_SYNTAX, None, message(&block, tag, &error));
        }
    }
}

/// The message, with the fence's line in the body and the parser's line within
/// the block kept apart.
///
/// Two different coordinate systems, and collapsing them is how a reader ends
/// up looking at the wrong line: `start_line` counts from the top of the
/// markdown body, while [`SyntaxError::line`](conform_okf_syntax::SyntaxError)
/// counts from the top of the snippet. Both are stated rather than added
/// together, because the sum is only correct while the fence is exactly one
/// line — and a `~~~~` fence with an info string is still one line today, but
/// arithmetic that happens to be right is not the same as arithmetic that is.
fn message(block: &FencedCodeBlock, tag: &str, error: &conform_okf_syntax::SyntaxError) -> String {
    let where_in_block = match error.line {
        Some(line) => format!(" (line {line} of the block)"),
        None => String::new(),
    };
    format!(
        "fenced `{tag}` block opening at line {} does not parse: {}{where_in_block}",
        block.start_line, error.message
    )
}

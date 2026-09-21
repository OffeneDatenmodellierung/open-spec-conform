//! SC-002, enforced rather than asserted: this crate defines **no** diagnostic,
//! severity, location, report, gate or resolution type of its own.
//!
//! The rule sounds like a style preference and is not. The whole value of
//! `conform-core` is that an ODCL failure renders through the identical path
//! as an ODCS, ODPS or OKF failure — one `Diagnostic`, one `Severity`, one
//! `GatePolicy`, one three-way `Resolution`. The moment an adapter defines a
//! local `Finding` or a local `Outcome`, every consumer downstream has to
//! learn a second vocabulary and convert between them, and the conversion is
//! where the severity or the third resolution outcome quietly goes missing.
//!
//! So: if this crate ever *wants* a type for one of those roles, that want is
//! a finding about `conform-core`'s design and belongs in
//! `docs/findings/`. It is not something to satisfy locally.
//!
//! # What this test is, and what it is not
//!
//! It reads this crate's own `src/` and fails on a declaration whose name
//! occupies one of the reserved roles. That is a **syntactic** check and it
//! cannot catch a type that does the job under an unrelated name — a
//! `struct Note` with a severity field would pass. Nothing textual could catch
//! that, so this is paired with a behavioural check below: every diagnostic
//! the crate actually emits is asserted to be `conform_core::Diagnostic`, and
//! the public API is asserted to hand back `conform-core`'s types.
//!
//! `conform-okf` and `conform-core` both use this shape of test on their own
//! sources; this is the same idea pointed at a different rule.

mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use conform_core::{ConformanceReport, Diagnostic, GatePolicy, Resolution, Severity, Validator};

/// Every role `conform-core` owns. A declaration here whose name contains one
/// of these is a local vocabulary taking root.
///
/// Matched case-insensitively against the declared name, so `LocalSeverity`,
/// `severity_kind` and `FindingSeverity` are all caught.
const RESERVED_ROLES: &[&str] = &[
    "Diagnostic",
    "Severity",
    "Location",
    "Report",
    "Gate",
    "Verdict",
    "Resolution",
    "Finding",
    "SpecRef",
];

/// Declarations that name a reserved role and are nonetheless correct.
///
/// Each one has to be argued, which is the point: the list is short, visible
/// in a diff, and cannot be extended without somebody writing down why.
const ALLOWED: &[(&str, &str)] = &[(
    "SchemaError",
    "Not a report type: it *carries* a `conform_core::ConformanceReport` and \
     exists only so a failed construction is an `Err` rather than a panic. Its \
     `report()` and `into_report()` hand the core type straight back, \
     unconverted. `conform-odcs` and `conform-odps` carry the identical type \
     for the identical reason.",
)];

fn sources() -> Vec<(String, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("cannot read directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no sources were read, so this test would pass vacuously"
    );
    files
        .into_iter()
        .map(|path| {
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            (
                path.file_name()
                    .expect("a file has a name")
                    .to_string_lossy()
                    .into_owned(),
                text,
            )
        })
        .collect()
}

/// The name a `struct`/`enum`/`trait`/`type` line declares, if it declares one.
///
/// Deliberately crude — a line-oriented scan over Rust source, not a parser.
/// It over-matches rather than under-matches, which is the right direction for
/// a rule that must not be quietly evaded.
fn declared_name(line: &str) -> Option<&str> {
    let line = line.trim_start();
    // A doc comment that happens to contain the word `struct` is prose.
    if line.starts_with("//") {
        return None;
    }
    let rest = ["struct ", "enum ", "trait ", "type "]
        .iter()
        .find_map(|keyword| {
            line.strip_prefix(keyword)
                .or_else(|| line.strip_prefix(&format!("pub {keyword}")))
                .or_else(|| line.strip_prefix(&format!("pub(crate) {keyword}")))
                .or_else(|| line.strip_prefix(&format!("pub(super) {keyword}")))
        })?;
    let name = rest
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()?;
    (!name.is_empty()).then_some(name)
}

#[test]
fn this_crate_declares_no_type_in_a_role_conform_core_owns() {
    let allowed: BTreeSet<&str> = ALLOWED.iter().map(|(name, _)| *name).collect();
    let mut offenders = Vec::new();

    for (file, text) in sources() {
        for (number, line) in text.lines().enumerate() {
            let Some(name) = declared_name(line) else {
                continue;
            };
            if allowed.contains(name) {
                continue;
            }
            let lowered = name.to_lowercase();
            if let Some(role) = RESERVED_ROLES
                .iter()
                .find(|role| lowered.contains(&role.to_lowercase()))
            {
                offenders.push(format!(
                    "{file}:{}: `{name}` occupies the `{role}` role",
                    number + 1
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "this crate has begun defining its own vocabulary for a role `conform-core` owns. Do NOT \
         resolve this by renaming the type: if `conform-core` cannot express what is needed here, \
         that is a finding about `conform-core`'s design and belongs in `docs/findings/`, not a \
         local type. If the declaration really is fine, argue it into ALLOWED.\n  {}",
        offenders.join("\n  ")
    );
}

/// The control. Without it, a typo in [`declared_name`] would make the test
/// above pass over a crate that declared nothing but offenders.
#[test]
fn the_scan_can_actually_see_a_declaration() {
    assert_eq!(declared_name("pub struct Document {"), Some("Document"));
    assert_eq!(declared_name("    enum Thing {"), Some("Thing"));
    assert_eq!(
        declared_name("pub(crate) struct SchemaFacts {"),
        Some("SchemaFacts")
    );
    assert_eq!(declared_name("struct Walk<'a> {"), Some("Walk"));
    assert_eq!(declared_name("/// a struct Severity in prose"), None);
    assert_eq!(declared_name("let x = 1;"), None);

    // And it really does find this crate's own declarations.
    let names: BTreeSet<String> = sources()
        .iter()
        .flat_map(|(_, text)| {
            text.lines()
                .filter_map(declared_name)
                .map(ToOwned::to_owned)
        })
        .collect();
    for expected in ["Document", "LexiconValidator", "SchemaError", "SchemaFacts"] {
        assert!(
            names.contains(expected),
            "the scan did not find `{expected}`, so it is not reading this crate's sources \
             properly: {names:?}"
        );
    }
}

/// The behavioural half: what the crate hands back is `conform-core`'s types,
/// not merely named like them.
///
/// These are type assertions written as ordinary code — each binding is
/// annotated with the core type, so a local look-alike would fail to compile
/// here rather than pass a string scan.
#[test]
fn the_public_api_speaks_conform_cores_types() {
    let validator = support::validator();

    let report: ConformanceReport = validator.validate(&support::fixture("conformant-full.yaml"));
    let worst: Option<Severity> = report.worst_severity();
    let gated: bool = report.should_gate(GatePolicy::default());
    assert!(
        !gated,
        "the clean fixture gated: {:?}",
        support::errors(&report)
    );
    assert_eq!(worst, Some(Severity::Info));

    let first: &Diagnostic = report
        .iter()
        .next()
        .expect("every report opens with a provenance note");
    assert_eq!(
        first.code.as_str(),
        conform_lexicon::codes::VALIDATED_AGAINST
    );

    // The resolvers hand back `conform-core`'s three-way type, not a bool and
    // not an Option.
    let document: serde_json::Value = serde_json::json!({
        "models": { "orders": { "fields": { "order_id": { "type": "string" } } } }
    });
    let resolved: Resolution<conform_core::Pointer> =
        conform_lexicon::resolve_field_reference(&document, "orders.order_id");
    assert!(resolved.is_resolved());
}

/// Nothing in the crate's sources names another standard's adapter or another
/// standard's codes.
///
/// An adapter that reached into a sibling would couple two upstreams' release
/// cycles together, which is the thing the per-crate literal versions exist to
/// prevent.
#[test]
fn this_crate_does_not_reach_into_a_sibling_adapter() {
    let mut offenders = Vec::new();
    for (file, text) in sources() {
        for (number, line) in text.lines().enumerate() {
            // Prose may discuss the siblings — the crate documentation compares
            // ODCL's open root with ODCS's closed one, and should. Code may not.
            if line.trim_start().starts_with("//") {
                continue;
            }
            for sibling in ["conform_odcs", "conform_odps", "conform_okf"] {
                if line.contains(sibling) {
                    offenders.push(format!("{file}:{}: names `{sibling}`", number + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "this crate's code refers to a sibling adapter, coupling two upstreams' release cycles: \
         {offenders:?}"
    );
}

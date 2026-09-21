//! The pass no single-document validator can run.
//!
//! # Why this is here and not in an adapter
//!
//! Every validator in this family holds one document. That is deliberate: a
//! verdict you can reproduce from one file and the vendored schema is a
//! verdict anybody can check. The cost is that two things the specifications
//! state outright become invisible from inside a single document.
//!
//! **An ODPS `contractId` names an ODCS contract.** `$defs/InputPort`
//! *requires* `contractId`; `$defs/OutputPort` carries `contractId` and an
//! `inputContracts` list. All three are `"type": "string"` in the published
//! schema and nothing else — the schema cannot say "and something must answer
//! to it", because the thing that answers is in another file. The link between
//! the two standards is the entire reason an estate publishes both, and until
//! now nothing in this repository verified a single one of them.
//!
//! **An ODCS `id` is "a unique identifier used to reduce the risk of dataset
//! name collisions".** Collisions between *contracts* — and a JSON Schema
//! validates one instance at a time, so two contracts sharing an `id` are two
//! documents each of which is individually perfect.
//!
//! So the work lands here, in the one component that holds a whole run.
//!
//! # Three outcomes, not two
//!
//! Resolution goes through [`Resolution`], the same three-way type
//! `conform-lexicon` follows `$ref` and `references` with, and for the same
//! reason. Asked to resolve `contractId: orders`, this module can be in one of
//! three states, and they are three *different facts*:
//!
//! - [`Resolution::Resolved`] — a contract in this run declares that `id`.
//! - [`Resolution::DoesNotExist`] — this run loaded contracts, they were
//!   searched, and none of them declares it. A finding.
//! - [`Resolution::NotInspected`] — nobody looked. **Not** a finding.
//!
//! The third case is not hypothetical and it is not rare: `conform validate
//! products/` over a directory of ODPS files loads no contract at all. A
//! two-valued answer would report every `contractId` in that directory as
//! dangling — an entire screen of false alarms on a run where nothing was
//! wrong and nothing was even checked. A gate that does that gets switched
//! off, and then it protects nothing.
//!
//! There is a second route into `NotInspected` and it matters as much: a run
//! in which some ODCS document could not be parsed, or carries no readable
//! `id`, has an **incomplete** corpus. "Not among the contracts I loaded" is
//! then still true and no longer interesting, because the contract may be
//! precisely the one that would not parse. So the answer becomes
//! [`NotInspectedReason::InspectionFailed`] and this module declines to call
//! anything absent.
//!
//! # Nothing here changes a verdict
//!
//! Every finding below is a warning or information. The adapters' errors are
//! exactly their published schemas' findings — that is the property that lets
//! this tool be adopted without changing which documents an estate believes
//! are conformant — and a corpus rule that gated would break it from the
//! outside. If a cross-document link should fail a build, that is
//! `--gate warning`, which is the caller's decision to make and not this
//! module's.

use std::collections::BTreeMap;

use conform_core::{Diagnostic, Location, NotInspectedReason, Resolution, Severity};

use crate::codes;
use crate::discover::Target;
use crate::model::FileStandard;

/// Check the whole run, returning one list of findings per target.
///
/// Index-aligned with `targets`, which is how the engine puts each finding on
/// the document it is about: returning a map keyed by path would fuse two
/// targets when one path is given twice on the command line.
#[must_use]
pub fn check(targets: &[Target]) -> Vec<Vec<Diagnostic>> {
    let contracts = Contracts::gather(targets);

    targets
        .iter()
        .enumerate()
        .map(|(index, target)| match target {
            Target::Document {
                id,
                standard: FileStandard::Odcs,
                ..
            } => contracts.collisions_at(index, id, targets),
            Target::Document {
                id,
                text,
                standard: FileStandard::Odps,
            } => contracts.references_from(id, text, targets),
            _ => Vec::new(),
        })
        .collect()
}

/// Every ODCS contract this run discovered, indexed by the `id` it declares.
struct Contracts {
    /// `id` → the indices of the targets declaring it, in run order.
    declared: BTreeMap<String, Vec<usize>>,
    /// How many ODCS documents this run discovered, whatever came of reading
    /// them.
    examined: usize,
    /// The ODCS documents no `id` could be read from — unparseable, or missing
    /// the key. Their existence is what turns "absent" into "not inspected",
    /// because one of them may be the contract being looked for.
    unreadable: Vec<String>,
}

impl Contracts {
    /// Read every ODCS document in the run for the `id` it declares.
    ///
    /// A document that will not parse is *counted*, not skipped. Its own
    /// adapter reports the parse failure; what matters here is that the corpus
    /// is now incomplete, and this is the only place that fact is recorded.
    fn gather(targets: &[Target]) -> Self {
        let mut declared: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut examined = 0;
        let mut unreadable = Vec::new();

        for (index, target) in targets.iter().enumerate() {
            let Target::Document {
                id,
                text,
                standard: FileStandard::Odcs,
            } = target
            else {
                continue;
            };
            examined += 1;
            match declared_id(text) {
                Some(contract) => declared.entry(contract).or_default().push(index),
                None => unreadable.push(id.clone()),
            }
        }

        Self {
            declared,
            examined,
            unreadable,
        }
    }

    /// Follow one `contractId` against this run.
    ///
    /// The order of the three answers is the order of the questions: *was
    /// there anything to search?*, then *is it there?*, then *could the search
    /// have missed it?* Asking them the other way round is how "absent" comes
    /// to be reported for a corpus nobody assembled.
    fn resolve(&self, contract: &str) -> Resolution<&[usize]> {
        if self.examined == 0 {
            return Resolution::not_inspected(NotInspectedReason::OutsideDocumentSet);
        }
        if let Some(declaring) = self.declared.get(contract) {
            return Resolution::Resolved(declaring.as_slice());
        }
        if !self.unreadable.is_empty() {
            return Resolution::not_inspected(NotInspectedReason::InspectionFailed);
        }
        Resolution::DoesNotExist
    }

    /// Every contract in this run that shares an `id` with the one at `index`.
    ///
    /// Raised on both sides of a collision, each naming the other, because
    /// either may be the one that should change.
    ///
    /// Two targets with the *same path* are not a collision: that is one file
    /// named twice on the command line, and reporting it would be a finding
    /// about the invocation dressed up as one about the documents.
    fn collisions_at(&self, index: usize, document: &str, targets: &[Target]) -> Vec<Diagnostic> {
        let Some((contract, declaring)) = self
            .declared
            .iter()
            .find(|(_, indices)| indices.contains(&index))
        else {
            return Vec::new();
        };

        let elsewhere: Vec<&str> = declaring
            .iter()
            .filter(|other| **other != index)
            .filter_map(|other| path_of(targets, *other))
            .filter(|other| *other != document)
            .collect();
        if elsewhere.is_empty() {
            return Vec::new();
        }

        vec![
            Diagnostic::warning(
                codes::DUPLICATE_CONTRACT_ID,
                Location::document(document).with_pointer("/id"),
                format!("`id: {contract}` is also declared by {}", join(&elsewhere)),
            )
            .with_help(
                "the schema calls this \"a unique identifier used to reduce the risk of dataset \
                 name collisions\" and cannot check it, because a collision is a fact about two \
                 documents and a schema validates one; an ODPS `contractId` naming this `id` has \
                 more than one contract to mean",
            ),
        ]
    }

    /// Every `contractId` one ODPS product names, followed.
    fn references_from(&self, document: &str, text: &str, targets: &[Target]) -> Vec<Diagnostic> {
        references_in(text)
            .into_iter()
            .map(|(pointer, key, contract)| {
                self.finding(document, &pointer, key, &contract, targets)
            })
            .collect()
    }

    /// Turn one resolution into the finding that describes it.
    ///
    /// Three outcomes, three different things to say — the shape
    /// `conform-lexicon` established for ODCL references, kept deliberately
    /// recognisable so that a reader who has seen `ODCL302` knows what
    /// `CLI201` means without being told twice.
    fn finding(
        &self,
        document: &str,
        pointer: &str,
        key: &str,
        contract: &str,
        targets: &[Target],
    ) -> Diagnostic {
        let location = Location::document(document).with_pointer(pointer);
        match self.resolve(contract) {
            Resolution::Resolved(declaring) => {
                let paths: Vec<&str> = declaring
                    .iter()
                    .filter_map(|index| path_of(targets, *index))
                    .collect();
                Diagnostic::new(
                    Severity::Info,
                    codes::CONTRACT_RESOLVED,
                    location,
                    format!("`{key}: {contract}` resolves to {}", join(&paths)),
                )
            }
            Resolution::DoesNotExist => Diagnostic::warning(
                codes::CONTRACT_DOES_NOT_EXIST,
                location,
                format!(
                    "`{key}: {contract}` names no contract among the {} this run loaded",
                    count(self.examined, "ODCS contract"),
                ),
            )
            .with_help(
                "the contracts in this run were searched and none declares that `id` — correct \
                 the reference, or include the contract it points at in the paths given",
            ),
            Resolution::NotInspected { reason } => Diagnostic::new(
                Severity::Info,
                codes::CONTRACT_NOT_INSPECTED,
                location,
                format!("`{key}: {contract}` was not resolved ({reason})"),
            )
            .with_help(explain(reason)),
        }
    }
}

/// The path of the target at an index, as the run names it.
///
/// `None` is unreachable for an index [`Contracts`] holds — every one came out
/// of a `Target::Document` in this same slice — and is returned rather than
/// indexed-and-panicked because a binary crashing on the corpus it was handed
/// would be a worse failure than a finding naming one document fewer.
fn path_of(targets: &[Target], index: usize) -> Option<&str> {
    targets.get(index).map(Target::id)
}

/// What a [`NotInspectedReason`] means for a reader of a cross-standard link.
///
/// A fallback arm because `NotInspectedReason` is `#[non_exhaustive]`: a
/// reason added to `conform-core` later must still produce a sentence rather
/// than failing to compile or, worse, being quietly dropped.
fn explain(reason: NotInspectedReason) -> &'static str {
    match reason {
        NotInspectedReason::OutsideDocumentSet => {
            "this run loaded no ODCS contract, so there was nothing to resolve against — this is \
             not a claim that the contract is missing; pass the contracts alongside the products \
             to have the link checked"
        }
        NotInspectedReason::InspectionFailed => {
            "an ODCS document in this run could not be read for its `id`, so the set of contracts \
             is incomplete and the one named here may be exactly the one that would not parse — \
             this is not a claim that it is missing"
        }
        _ => "nobody looked, so this says nothing about whether the contract exists",
    }
}

/// The `id` an ODCS document declares, if it has one that is readable.
///
/// Parsed with `serde_norway`, the same parser the adapters use, so a document
/// that reads here cannot fail to read there for a reason this module
/// invented.
fn declared_id(text: &str) -> Option<String> {
    let value: serde_norway::Value = serde_norway::from_str(text).ok()?;
    value.get("id")?.as_str().map(str::to_owned)
}

/// Every contract reference an ODPS product makes, as
/// `(JSON Pointer, the key as the document spells it, the value)`.
///
/// The three places the vendored ODPS schema puts one, and there are exactly
/// three: `$defs/InputPort.contractId` (required), `$defs/OutputPort
/// .contractId`, and `$defs/InputContract.id`, reached through
/// `OutputPort.inputContracts`. `$defs/ManagementPort` has no contract
/// reference at all, which is why nothing below looks for one.
fn references_in(text: &str) -> Vec<(String, &'static str, String)> {
    let Ok(value) = serde_norway::from_str::<serde_norway::Value>(text) else {
        // Unparseable. The ODPS adapter says so with the parser's own line and
        // column; adding a second opinion here would be noise.
        return Vec::new();
    };

    let mut found = Vec::new();

    for (index, port) in sequence(&value, "inputPorts").iter().enumerate() {
        if let Some(contract) = string_at(port, "contractId") {
            found.push((
                format!("/inputPorts/{index}/contractId"),
                "contractId",
                contract,
            ));
        }
    }

    for (index, port) in sequence(&value, "outputPorts").iter().enumerate() {
        if let Some(contract) = string_at(port, "contractId") {
            found.push((
                format!("/outputPorts/{index}/contractId"),
                "contractId",
                contract,
            ));
        }
        for (nested, input) in sequence(port, "inputContracts").iter().enumerate() {
            if let Some(contract) = string_at(input, "id") {
                found.push((
                    format!("/outputPorts/{index}/inputContracts/{nested}/id"),
                    "id",
                    contract,
                ));
            }
        }
    }

    found
}

/// One key of a mapping, as a sequence. Empty for anything that is not one —
/// the adapter has already said so, with the schema's own words.
fn sequence<'v>(value: &'v serde_norway::Value, key: &str) -> &'v [serde_norway::Value] {
    value
        .get(key)
        .and_then(serde_norway::Value::as_sequence)
        .map_or(&[], Vec::as_slice)
}

/// One key of a mapping, as a string.
fn string_at(value: &serde_norway::Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(str::to_owned)
}

/// `a`, `a and b`, `a, b and c` — names, not a count, because the reader's
/// next action is to open one of them.
fn join(paths: &[&str]) -> String {
    match paths {
        [] => "nothing".to_owned(),
        [only] => format!("`{only}`"),
        [rest @ .., last] => format!(
            "{} and `{last}`",
            rest.iter()
                .map(|path| format!("`{path}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// `1 ODCS contract` / `3 ODCS contracts`.
fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("{n} {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

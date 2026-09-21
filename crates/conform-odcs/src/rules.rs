//! Hygiene rules: things the published schema permits and a reviewer would not.
//!
//! Every rule here raises a **warning**, never an error, and that is a
//! deliberate boundary rather than timidity. The schema is the authority on
//! what conforms; a crate that quietly added conformance requirements of its
//! own would give a different pass/fail verdict from every other ODCS tool,
//! and a validator nobody else agrees with is a validator nobody uses.
//!
//! So the split is: **errors are exactly the schema's findings, warnings are
//! ours**. Under the default [`GatePolicy`](conform_core::GatePolicy) — which
//! gates on errors alone — everything in this module reports and nothing in it
//! changes a verdict. That is also what makes the differential test against the
//! pre-existing validator meaningful: the two agree on the verdict *by
//! construction*, and the test proves the construction holds.

use conform_core::{Diagnostic, SpecRef};
use serde_json::Value;

use crate::codes;
use crate::schema::at;

/// The `status` values the schema publishes under `examples`.
///
/// `examples`, note — not `enum`. The schema therefore cannot enforce this
/// list, which is exactly what makes a rule here worth having rather than
/// redundant.
pub const CONVENTIONAL_STATUSES: &[&str] =
    &["proposed", "draft", "active", "deprecated", "retired"];

/// Every hygiene finding for one parsed, schema-checked document.
///
/// Runs whether or not the document conforms. A contract with a missing
/// `status` and no team named has two things wrong with it, and reporting only
/// the first is the behaviour this crate exists to replace.
pub(crate) fn hygiene(
    instance: &Value,
    document: &conform_core::DocumentId,
    spec: &SpecRef,
) -> Vec<Diagnostic> {
    let Some(contract) = instance.as_object() else {
        // Not an object: the schema has already said so, with a better
        // message than anything this module could add.
        return Vec::new();
    };

    let mut found = Vec::new();
    let warn = |code: &'static str, pointer: &str, message: String, help: &str| {
        Diagnostic::warning(code, at(document, pointer), message)
            .with_help(help.to_owned())
            .with_spec_ref(spec.clone())
    };

    if let (Some(declared), Some(carried)) = (
        contract.get("apiVersion").and_then(Value::as_str),
        spec.version.as_deref(),
    ) {
        let carried = format!("v{carried}");
        if declared != carried {
            found.push(warn(
                codes::API_VERSION_BEHIND,
                "/apiVersion",
                format!(
                    "document declares apiVersion `{declared}`, and this validator carries the \
                     `{carried}` schema"
                ),
                "the schema accepts a range of versions, so this is legal; it does mean rules \
                 added after the declared version were never written for this document",
            ));
        }
    }

    if is_absent_or_empty(contract.get("schema")) {
        found.push(warn(
            codes::NO_SCHEMA_OBJECTS,
            "/schema",
            "contract catalogues no schema objects".to_owned(),
            "add a `schema` entry describing the dataset's shape — a contract that does not say \
             what the data looks like cannot be checked against the data",
        ));
    }

    if is_absent_or_empty(contract.get("servers")) {
        found.push(warn(
            codes::NO_SERVERS,
            "/servers",
            "contract names no server".to_owned(),
            "add a `servers` entry, so a reader can find the data this contract is about",
        ));
    }

    if is_absent_or_empty(contract.get("team")) {
        found.push(warn(
            codes::NO_TEAM,
            "/team",
            "contract names no owning team".to_owned(),
            "add a `team`, so there is somebody to ask when this contract is wrong",
        ));
    }

    if is_absent_or_empty(contract.get("description")) {
        found.push(warn(
            codes::NO_DESCRIPTION,
            "/description",
            "contract carries no description".to_owned(),
            "add a `description` with at least `purpose` or `usage`; the `id` is not documentation",
        ));
    }

    if let Some(status) = contract.get("status").and_then(Value::as_str)
        && !CONVENTIONAL_STATUSES.contains(&status)
    {
        found.push(warn(
            codes::UNCONVENTIONAL_STATUS,
            "/status",
            format!("`status` is `{status}`, which is outside the conventional set"),
            "the schema publishes the conventional values as examples rather than as an enum, \
             so this is permitted; tooling that switches on `status` will not recognise it",
        ));
    }

    found
}

/// Whether a field is missing, explicitly null, or present but carrying
/// nothing — an empty array, object or string.
///
/// `servers: []` is a different mistake from omitting `servers`, but it is the
/// same *finding*: after reading the contract you still do not know where the
/// data is. Treating the two alike is what keeps these rules from being
/// trivially silenced by writing an empty collection.
fn is_absent_or_empty(field: Option<&Value>) -> bool {
    match field {
        None | Some(Value::Null) => true,
        Some(Value::Array(items)) => items.is_empty(),
        Some(Value::Object(members)) => members.is_empty(),
        Some(Value::String(text)) => text.trim().is_empty(),
        Some(_) => false,
    }
}

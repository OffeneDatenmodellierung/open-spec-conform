//! Hygiene rules: things the published schema permits and a reviewer would not.
//!
//! Every rule here raises a **warning**, never an error, and that is a
//! deliberate boundary rather than timidity. The schema is the authority on
//! what conforms; a crate that quietly added conformance requirements of its
//! own would give a different pass/fail verdict from every other ODPS tool,
//! and a validator nobody else agrees with is a validator nobody uses.
//!
//! So the split is: **errors are exactly the schema's findings, warnings are
//! ours**. Under the default [`GatePolicy`](conform_core::GatePolicy) — which
//! gates on errors alone — everything in this module reports and nothing in it
//! changes a verdict. That is also what makes the differential test against the
//! pre-existing validator meaningful: the two agree on the verdict *by
//! construction*, and the test proves the construction holds.
//!
//! Several of these rules are the ODPS schema's own prose, promoted to
//! something a machine can read. The schema says of `outputPorts` that "you
//! need at least one, as a data product without output is useless" — and then
//! does not require one. A sentence in a `description` cannot fail a build; a
//! warning can.

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
/// Runs whether or not the document conforms. A product with a missing
/// `status` and no output port has two things wrong with it, and reporting
/// only the first is the behaviour this crate exists to replace.
pub(crate) fn hygiene(
    instance: &Value,
    document: &conform_core::DocumentId,
    spec: &SpecRef,
) -> Vec<Diagnostic> {
    let Some(product) = instance.as_object() else {
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
        product.get("apiVersion").and_then(Value::as_str),
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

    if is_absent_or_empty(product.get("outputPorts")) {
        found.push(warn(
            codes::NO_OUTPUT_PORTS,
            "/outputPorts",
            "data product declares no output port".to_owned(),
            "the schema's own wording is \"you need at least one, as a data product without \
             output is useless\" — it does not require one, so this is the rule that says it",
        ));
    }

    if is_absent_or_empty(product.get("inputPorts")) {
        found.push(warn(
            codes::NO_INPUT_PORTS,
            "/inputPorts",
            "data product declares no input port".to_owned(),
            "the schema's own wording is \"you need at least one as a data product needs to get \
             data somewhere\" — it does not require one, so this is the rule that says it",
        ));
    }

    if is_absent_or_empty(product.get("team")) {
        found.push(warn(
            codes::NO_TEAM,
            "/team",
            "data product names no owning team".to_owned(),
            "add a `team`, so there is somebody to ask when this product is wrong",
        ));
    }

    if is_absent_or_empty(product.get("description")) {
        found.push(warn(
            codes::NO_DESCRIPTION,
            "/description",
            "data product carries no description".to_owned(),
            "add a `description` with at least `purpose` or `usage`; the `id` is not documentation",
        ));
    }

    if is_absent_or_empty(product.get("version")) {
        found.push(warn(
            codes::NO_VERSION,
            "/version",
            "data product records no version".to_owned(),
            "the schema describes `version` as \"not required, but highly recommended\"; without \
             it nothing downstream can tell two revisions of this product apart",
        ));
    }

    if let Some(status) = product.get("status").and_then(Value::as_str)
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
/// `outputPorts: []` is a different mistake from omitting `outputPorts`, but
/// it is the same *finding*: after reading the product you still do not know
/// what it serves. Treating the two alike is what keeps these rules from being
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

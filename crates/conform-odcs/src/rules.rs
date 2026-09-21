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

use std::collections::BTreeMap;

use conform_core::{Diagnostic, DocumentId, SpecRef};
use serde_json::{Map, Value};

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

    found.extend(duplicate_stable_ids(contract, document, spec));

    found
}

/// The root arrays whose members carry a `$defs/StableId` under `id`.
///
/// Read off the vendored schema rather than remembered: `/properties/servers`
/// has `items: $defs/Server`, `/properties/schema` has `items:
/// $defs/SchemaObject`, and both of those definitions give `id` as
/// `$ref: #/$defs/StableId`. `SchemaObject` carries `properties`, an array of
/// `$defs/SchemaProperty`, which reaches `StableId` the same way through
/// `SchemaBaseProperty`'s `allOf` — so the nested property lists are arrays of
/// stable identifiers too, and [`descend`] follows them.
///
/// Deliberately **not** the whole list. `roles`, `slaProperties`, `support`,
/// `price`, `team.members`, `quality`, `customProperties` and
/// `authoritativeDefinitions` also hold `StableId`-bearing members, and this
/// rule does not look at them yet. Saying which arrays are checked is the
/// honest form of that: a rule whose scope nobody wrote down reads as though
/// it covered everything.
const STABLE_ID_ARRAYS: &[&str] = &["servers", "schema"];

/// Every repeated `id` in the arrays this rule checks.
///
/// The schema's `$defs/StableId` says, in its own description, *"Must be
/// unique within its containing array"* — and then constrains only a
/// `pattern`. Neither root array carries `uniqueItems`, and `uniqueItems`
/// could not express this anyway: it compares whole members, so two schema
/// objects sharing an `id` and differing in a description are already
/// "unique" by that keyword's definition. So the sentence is unenforced by
/// construction, and a document with two `schema[].id` of `orders` passes the
/// published schema clean.
///
/// What that costs: an `id` is what a reference resolves *by*. Two members
/// answering to one identifier means anything following a reference has two
/// candidates and no rule for choosing between them.
fn duplicate_stable_ids(
    contract: &Map<String, Value>,
    document: &DocumentId,
    spec: &SpecRef,
) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    for key in STABLE_ID_ARRAYS {
        let Some(Value::Array(members)) = contract.get(*key) else {
            continue;
        };
        let array = format!("/{key}");
        duplicates_in(members, &array, document, spec, &mut found);
        for (index, member) in members.iter().enumerate() {
            descend(
                member,
                &format!("{array}/{index}"),
                document,
                spec,
                &mut found,
            );
        }
    }
    found
}

/// Follow one member's nested property lists, checking each as its own array.
///
/// "Unique within its containing array" is per array, so `/schema/0/properties`
/// and `/schema/1/properties` may each hold an `id` of `order_id` without
/// either being a finding — and two `order_id`s inside *one* of them is a
/// finding. Counting across the whole document would report the first case,
/// which the specification permits, and that false alarm is worth more care
/// than the handful of lines it takes to avoid.
///
/// Both nesting routes the schema publishes are followed: `properties`, which
/// `SchemaObject` and the `logicalType: object` branch of `SchemaBaseProperty`
/// both give as an array of `SchemaProperty`, and `items`, which the
/// `logicalType: array` branch gives as a single `SchemaItemProperty` carrying
/// a `properties` array of its own.
fn descend(
    node: &Value,
    pointer: &str,
    document: &DocumentId,
    spec: &SpecRef,
    found: &mut Vec<Diagnostic>,
) {
    if let Some(Value::Array(properties)) = node.get("properties") {
        let array = format!("{pointer}/properties");
        duplicates_in(properties, &array, document, spec, found);
        for (index, child) in properties.iter().enumerate() {
            descend(child, &format!("{array}/{index}"), document, spec, found);
        }
    }
    if let Some(items) = node.get("items") {
        descend(items, &format!("{pointer}/items"), document, spec, found);
    }
}

/// One array, checked for repeated `id` values.
///
/// The finding is located at the **repeat**, not at the first occurrence, and
/// names where the first one is. Pointing at the first would send a reader to
/// a member that may be entirely correct; the one that has to change is the
/// one that came second, and a duplicate appearing three times is three
/// findings rather than one, because each of them is a separate edit.
fn duplicates_in(
    members: &[Value],
    array: &str,
    document: &DocumentId,
    spec: &SpecRef,
    found: &mut Vec<Diagnostic>,
) {
    let mut first_seen: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, member) in members.iter().enumerate() {
        let Some(id) = member.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(first) = first_seen.insert(id, index) else {
            continue;
        };
        // Put the earlier index back: every later repeat should point at the
        // *first* occurrence, not at the one before it.
        first_seen.insert(id, first);
        found.push(
            Diagnostic::warning(
                codes::DUPLICATE_STABLE_ID,
                at(document, &format!("{array}/{index}/id")),
                format!("`id` is `{id}`, which `{array}/{first}` already carries"),
            )
            .with_help(
                "the schema's `$defs/StableId` says \"Must be unique within its containing \
                 array\" and then enforces only a character-set pattern, so this document \
                 conforms; it does mean anything resolving a reference by this `id` has two \
                 candidates and no rule for choosing",
            )
            .with_spec_ref(spec.clone()),
        );
    }
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

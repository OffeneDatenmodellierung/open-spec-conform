//! Hygiene rules and cross-reference checks: things the published schema
//! permits, or cannot express, and a reviewer would not.
//!
//! Every rule here raises a **warning** or an **info**, never an error, and
//! that is a deliberate boundary rather than timidity. The schema is the
//! authority on what conforms; a crate that quietly added conformance
//! requirements of its own would give a different pass/fail verdict from every
//! other tool reading this standard, and a validator nobody else agrees with
//! is a validator nobody uses.
//!
//! So the split is: **errors are exactly the schema's findings, warnings are
//! ours**. Under the default [`GatePolicy`](conform_core::GatePolicy) — which
//! gates on errors alone — everything in this module reports and nothing in it
//! changes a verdict. That is also what makes the differential test against
//! the pre-existing validator meaningful: the two agree on the verdict *by
//! construction*, and the test proves the construction holds.
//!
//! # What these rules are derived from
//!
//! Three of them read the vendored schema rather than a list typed out here,
//! because a transcribed list is a list that goes stale the moment upstream
//! moves:
//!
//! - the permitted **root keys** come from the schema's own `properties`;
//! - the **deprecated keywords** come from the `deprecationMessage`
//!   annotations the schema carries, and the messages are quoted verbatim;
//! - the **conventional statuses** come from `info.status`'s `examples`.
//!
//! All three are things JSON Schema records and cannot enforce — an open root
//! object, an annotation keyword, and `examples` where an `enum` would gate —
//! which is precisely what makes a rule for each of them worth having rather
//! than redundant.

use std::collections::{BTreeMap, BTreeSet};

use conform_core::{Diagnostic, DocumentId, NotInspectedReason, Resolution, Severity, SpecRef};
use serde_json::Value;

use crate::codes;
use crate::references::{resolve_field_reference, resolve_ref};
use crate::schema::at;

/// What the vendored schema says, read once at construction so no rule below
/// has to transcribe it.
///
/// Not a diagnostic, report, gate or resolution type — those all come from
/// `conform-core` and this crate defines none of its own. This is the
/// adapter's own model of its own schema, which is the part `conform-core`
/// deliberately knows nothing about.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SchemaFacts {
    /// Every top-level key the schema names.
    root_properties: BTreeSet<String>,
    /// `info.status`'s published `examples`.
    conventional_statuses: BTreeSet<String>,
    /// Deprecated keys of a `Field`, and upstream's own message for each.
    field_deprecations: BTreeMap<String, String>,
    /// Deprecated keys of a `Definition`, and upstream's own message for each.
    definition_deprecations: BTreeMap<String, String>,
    /// Every `type` the `servers` dispatch has a sub-schema for — read from
    /// the `const`s in the schema's own `if` branches.
    dispatched_server_types: BTreeSet<String>,
    /// Whether that dispatch is unreachable under the draft the schema
    /// declares. See [`servers_dispatch_is_inert`].
    server_dispatch_is_inert: bool,
}

/// The `Field` sub-schema, as a JSON Pointer into the schema document.
const FIELD_SCHEMA: &str =
    "/properties/models/additionalProperties/properties/fields/additionalProperties";
/// The `Definition` sub-schema, likewise.
const DEFINITION_SCHEMA: &str = "/properties/definitions/additionalProperties";
/// The `servers` value sub-schema, likewise.
const SERVER_SCHEMA: &str = "/properties/servers/additionalProperties";

impl SchemaFacts {
    /// Read the facts out of a compiled-and-verified schema document.
    ///
    /// Every lookup here is tolerant: a schema that no longer has the shape
    /// these pointers expect yields an empty set and the corresponding rule
    /// simply stops firing. That is a quiet degradation, so it is not left to
    /// trust — `tests/hygiene_rules_read_the_schema.rs` asserts each of these
    /// sets against what the vendored schema actually carries, and fails
    /// loudly if an upstream reshuffle empties one.
    pub(crate) fn read(schema: &Value) -> Self {
        Self {
            root_properties: keys_of(schema.get("properties")),
            conventional_statuses: schema
                .pointer("/properties/info/properties/status/examples")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToOwned::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            field_deprecations: deprecations(schema, FIELD_SCHEMA),
            definition_deprecations: deprecations(schema, DEFINITION_SCHEMA),
            dispatched_server_types: dispatched_server_types(schema),
            server_dispatch_is_inert: servers_dispatch_is_inert(schema),
        }
    }
}

/// Every server `type` the schema's `servers` dispatch has a branch for.
///
/// Read out of the `const`s in its own `allOf`/`if` chain, so this cannot
/// drift from the nineteen technologies the vendored schema actually names.
fn dispatched_server_types(schema: &Value) -> BTreeSet<String> {
    schema
        .pointer(&format!("{SERVER_SCHEMA}/allOf"))
        .and_then(Value::as_array)
        .map(|branches| {
            branches
                .iter()
                .filter_map(|branch| {
                    branch
                        .pointer("/if/properties/type/const")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Whether the `servers` dispatch is unreachable as the schema is written.
///
/// Two conditions, both read from the schema rather than assumed:
///
/// 1. the `servers` value schema carries a `$ref` **and** sibling keywords;
/// 2. the document declares a draft in which `$ref` suppresses its siblings —
///    draft-07 and earlier. From 2019-09 onward `$ref` is an ordinary
///    applicator and its siblings apply normally.
///
/// Both together mean the `allOf` chain is dead code. If upstream moves the
/// schema forward a draft, or lifts the `$ref` into the `allOf`, this returns
/// false and [`codes::SERVER_TYPE_NOT_INSPECTED`] stops firing — without
/// anybody editing this function. `tests/the_server_dispatch_is_dead.rs`
/// proves the mechanism in isolation rather than trusting this comment.
fn servers_dispatch_is_inert(schema: &Value) -> bool {
    let Some(servers) = schema.pointer(SERVER_SCHEMA).and_then(Value::as_object) else {
        return false;
    };
    if !servers.contains_key("$ref") || servers.len() < 2 {
        return false;
    }
    let draft = schema.get("$schema").and_then(Value::as_str).unwrap_or("");
    // Matched on what the drafts are actually spelled, rather than on a
    // parsed version: these are the exact `$schema` values the suppressing
    // drafts publish, and a `$schema` this does not recognise is treated as a
    // modern draft — the conservative reading, because it makes this crate
    // report less rather than assert something it has not established.
    ["draft-07", "draft-06", "draft-04", "draft-03"]
        .iter()
        .any(|suppressing| draft.contains(suppressing))
}

/// The keys of an object node, or an empty set for anything else.
fn keys_of(node: Option<&Value>) -> BTreeSet<String> {
    node.and_then(Value::as_object)
        .map(|members| members.keys().cloned().collect())
        .unwrap_or_default()
}

/// Every `deprecationMessage` under one sub-schema's `properties`.
///
/// `deprecationMessage` is an annotation, not an assertion: a JSON Schema
/// validator reads straight past it and the document passes. Harvesting it
/// here is what turns upstream's own note into something a reader is told.
fn deprecations(schema: &Value, sub_schema: &str) -> BTreeMap<String, String> {
    schema
        .pointer(&format!("{sub_schema}/properties"))
        .and_then(Value::as_object)
        .map(|properties| {
            properties
                .iter()
                .filter_map(|(key, definition)| {
                    definition
                        .get("deprecationMessage")
                        .and_then(Value::as_str)
                        .map(|message| (key.clone(), message.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Everything this crate has to say about a document the schema has already
/// been run over.
///
/// Runs whether or not the document conforms. A contract with a misspelled
/// root key, an untyped field and a dangling foreign key has three things
/// wrong with it, and reporting only the first is the behaviour this crate
/// exists to replace.
pub(crate) fn beyond_the_schema(
    instance: &Value,
    facts: &SchemaFacts,
    document: &DocumentId,
    spec: &SpecRef,
) -> Vec<Diagnostic> {
    let Some(contract) = instance.as_object() else {
        // Not an object: the schema has already said so, with a better
        // message than anything this module could add.
        return Vec::new();
    };

    let mut found = Vec::new();
    root_rules(contract, facts, document, spec, &mut found);
    info_rules(contract.get("info"), facts, document, spec, &mut found);

    if let Some(servers) = contract.get("servers").and_then(Value::as_object) {
        for (name, server) in servers {
            found.extend(server_dispatch_note(server, name, facts, document, spec));
        }
    }

    if let Some(definitions) = contract.get("definitions").and_then(Value::as_object) {
        for (name, definition) in definitions {
            found.extend(deprecated_keys_in(
                definition,
                &facts.definition_deprecations,
                &format!("/definitions/{}", escape(name)),
                document,
                spec,
            ));
        }
    }

    if let Some(models) = contract.get("models").and_then(Value::as_object) {
        let walk = Walk {
            instance,
            facts,
            document,
            spec,
        };
        for (name, model) in models {
            let pointer = format!("/models/{}", escape(name));
            if let Some(fields) = model.get("fields").and_then(Value::as_object) {
                for (field_name, field) in fields {
                    walk_field(
                        &walk,
                        field,
                        &format!("{pointer}/fields/{}", escape(field_name)),
                        &mut found,
                    );
                }
            }
        }
    }

    found
}

/// The rules about the contract's top level.
fn root_rules(
    contract: &serde_json::Map<String, Value>,
    facts: &SchemaFacts,
    document: &DocumentId,
    spec: &SpecRef,
    found: &mut Vec<Diagnostic>,
) {
    // An empty permitted set means the schema no longer has the shape this
    // rule reads, and firing on every key would be worse than silence. The
    // silence is what `tests/hygiene_rules_read_the_schema.rs` guards.
    if !facts.root_properties.is_empty() {
        for key in contract.keys() {
            if !facts.root_properties.contains(key) {
                found.push(warn(
                    codes::UNKNOWN_ROOT_KEY,
                    &format!("/{}", escape(key)),
                    format!("`{key}` is not a key this specification defines at the root"),
                    "the root object does not close `additionalProperties`, so the schema accepts \
                     this in silence; if it is a misspelling, every rule written against the real \
                     key has been skipped without saying so",
                    document,
                    spec,
                ));
            }
        }
    }

    if let (Some(declared), Some(carried)) = (
        contract
            .get("dataContractSpecification")
            .and_then(Value::as_str),
        spec.version.as_deref(),
    ) && declared != carried
    {
        found.push(warn(
            codes::SPEC_VERSION_BEHIND,
            "/dataContractSpecification",
            format!(
                "document declares specification version `{declared}`, and this validator carries \
                 the `{carried}` schema"
            ),
            "the schema's enum accepts a range of versions, so this is legal; it does mean rules \
             added after the declared version were never written for this document",
            document,
            spec,
        ));
    }

    if is_absent_or_empty(contract.get("models")) {
        found.push(warn(
            codes::NO_MODELS,
            "/models",
            "contract catalogues no models".to_owned(),
            "add a `models` entry describing the dataset's shape — a contract that does not say \
             what the data looks like cannot be checked against the data",
            document,
            spec,
        ));
    }

    if is_absent_or_empty(contract.get("servers")) {
        found.push(warn(
            codes::NO_SERVERS,
            "/servers",
            "contract names no server".to_owned(),
            "add a `servers` entry, so a reader can find the data this contract is about",
            document,
            spec,
        ));
    }
}

/// The rules about `info`, which holds everything that says who owns this
/// contract and what it is for.
fn info_rules(
    info: Option<&Value>,
    facts: &SchemaFacts,
    document: &DocumentId,
    spec: &SpecRef,
    found: &mut Vec<Diagnostic>,
) {
    if is_absent_or_empty(info.and_then(|i| i.get("owner"))) {
        found.push(warn(
            codes::NO_OWNER,
            "/info/owner",
            "contract names no owner".to_owned(),
            "add `info.owner`, so there is somebody to ask when this contract is wrong",
            document,
            spec,
        ));
    }

    if is_absent_or_empty(info.and_then(|i| i.get("description"))) {
        found.push(warn(
            codes::NO_DESCRIPTION,
            "/info/description",
            "contract carries no description".to_owned(),
            "add `info.description`; the `id` and `title` are not documentation",
            document,
            spec,
        ));
    }

    if let Some(status) = info.and_then(|i| i.get("status")).and_then(Value::as_str)
        && !facts.conventional_statuses.is_empty()
        && !facts.conventional_statuses.contains(status)
    {
        found.push(warn(
            codes::UNCONVENTIONAL_STATUS,
            "/info/status",
            format!("`info.status` is `{status}`, which is outside the conventional set"),
            "the schema publishes the conventional values as examples rather than as an enum, \
             so this is permitted; tooling that switches on `status` will not recognise it",
            document,
            spec,
        ));
    }
}

/// What this run did **not** check about one server.
///
/// Returns nothing when the dispatch works, when the server declares no
/// `type`, or when the type it declares has no sub-schema to have been skipped
/// — three different ways of having nothing to report, none of which is a
/// finding.
fn server_dispatch_note(
    server: &Value,
    name: &str,
    facts: &SchemaFacts,
    document: &DocumentId,
    spec: &SpecRef,
) -> Option<Diagnostic> {
    if !facts.server_dispatch_is_inert {
        return None;
    }
    let declared = server.get("type").and_then(Value::as_str)?;
    if !facts.dispatched_server_types.contains(declared) {
        return None;
    }
    Some(
        Diagnostic::new(
            Severity::Info,
            codes::SERVER_TYPE_NOT_INSPECTED,
            at(document, &format!("/servers/{}", escape(name))),
            format!(
                "server `{name}` declares `type: {declared}`, and was checked against \
                 `BaseServer` only — the `{declared}` sub-schema was not applied"
            ),
        )
        .with_help(
            "not a fault in this document: the schema's `servers` value carries a `$ref` \
             alongside its `allOf` dispatch, and under the draft it declares a `$ref` suppresses \
             its siblings, so every per-technology branch is unreachable",
        )
        .with_spec_ref(spec.clone()),
    )
}

/// One hygiene warning, assembled the one way every rule above assembles one.
fn warn(
    code: &'static str,
    pointer: &str,
    message: String,
    help: &str,
    document: &DocumentId,
    spec: &SpecRef,
) -> Diagnostic {
    Diagnostic::warning(code, at(document, pointer), message)
        .with_help(help.to_owned())
        .with_spec_ref(spec.clone())
}

/// The keys by which a field says what it holds.
///
/// `type` and `$ref` are both optional in the schema, so a field carrying only
/// a description conforms — and describes nothing a consumer can check
/// anything against. The structural keys count as saying something, so
/// `{ fields: {…} }` is not flagged for lacking an explicit `type: object`.
const SAYS_WHAT_IT_HOLDS: [&str; 7] = ["type", "$ref", "fields", "items", "keys", "values", "enum"];

/// Everything one walk of the models needs, so the recursion below carries a
/// context rather than eight positional arguments.
///
/// `instance` is the **whole** document, not the subtree being walked: a
/// field's `references` points at another model entirely, so the resolver has
/// to be able to see the root from anywhere in the tree.
struct Walk<'a> {
    instance: &'a Value,
    facts: &'a SchemaFacts,
    document: &'a DocumentId,
    spec: &'a SpecRef,
}

/// One field, its nested fields, and everything this crate says about them.
///
/// Recurses through the four places the schema nests a `Field` inside another:
/// the `fields` map of an object, and the single `items`, `keys` and `values`
/// of an array or map. A rule that only looked at top-level fields would go
/// quiet on exactly the deeply-nested structures hardest to review by hand.
fn walk_field(walk: &Walk<'_>, field: &Value, pointer: &str, found: &mut Vec<Diagnostic>) {
    let Some(members) = field.as_object() else {
        // Not an object: the schema has already objected.
        return;
    };

    found.extend(deprecated_keys_in(
        field,
        &walk.facts.field_deprecations,
        pointer,
        walk.document,
        walk.spec,
    ));

    if !SAYS_WHAT_IT_HOLDS
        .iter()
        .any(|key| members.contains_key(*key))
    {
        found.push(
            Diagnostic::warning(
                codes::UNTYPED_FIELD,
                at(walk.document, pointer),
                "field declares neither a `type` nor a `$ref`, so nothing says what it holds",
            )
            .with_help(
                "add a `type` from the specification's logical types, or a `$ref` to a definition \
                 that carries one",
            )
            .with_spec_ref(walk.spec.clone()),
        );
    }

    if let Some(raw) = members.get("references").and_then(Value::as_str) {
        found.push(reference_finding(
            walk,
            &resolve_field_reference(walk.instance, raw),
            raw,
            "references",
            codes::REFERENCE_DOES_NOT_EXIST,
            &format!("{pointer}/references"),
        ));
    }

    if let Some(raw) = members.get("$ref").and_then(Value::as_str) {
        found.push(reference_finding(
            walk,
            &resolve_ref(walk.instance, raw),
            raw,
            "$ref",
            codes::DEFINITION_DOES_NOT_EXIST,
            &format!("{pointer}/$ref"),
        ));
    }

    if let Some(nested) = members.get("fields").and_then(Value::as_object) {
        for (name, child) in nested {
            walk_field(
                walk,
                child,
                &format!("{pointer}/fields/{}", escape(name)),
                found,
            );
        }
    }
    for key in ["items", "keys", "values"] {
        if let Some(child) = members.get(key) {
            walk_field(walk, child, &format!("{pointer}/{key}"), found);
        }
    }
}

/// Turn one [`Resolution`] into the finding that describes it.
///
/// The three outcomes become three different things to report, which is the
/// whole reason the resolvers return a `Resolution` rather than an `Option`:
///
/// - **resolved** — an info naming where it landed. Reported rather than
///   passed over in silence, because "checked and fine" and "never checked"
///   must not look identical;
/// - **does not exist** — a *warning*, because the document was asked and the
///   target is genuinely absent;
/// - **not inspected** — an info carrying the reason, because an absence of
///   evidence reported as a defect is a false alarm, and a gate that raises
///   false alarms is a gate that gets switched off.
fn reference_finding(
    walk: &Walk<'_>,
    resolution: &Resolution<conform_core::Pointer>,
    raw: &str,
    key: &str,
    absent_code: &'static str,
    pointer: &str,
) -> Diagnostic {
    let location = at(walk.document, pointer);
    match resolution {
        Resolution::Resolved(target) => Diagnostic::new(
            Severity::Info,
            codes::REFERENCE_RESOLVED,
            location,
            format!("`{key}: {raw}` resolves to `{target}` in this document"),
        )
        .with_spec_ref(walk.spec.clone()),
        Resolution::DoesNotExist => Diagnostic::warning(
            absent_code,
            location,
            format!("`{key}: {raw}` names nothing in this document"),
        )
        .with_help(
            "this document was searched and the target is genuinely absent — correct the \
             reference, or define what it points at",
        )
        .with_spec_ref(walk.spec.clone()),
        Resolution::NotInspected { reason } => Diagnostic::new(
            Severity::Info,
            codes::REFERENCE_NOT_INSPECTED,
            location,
            format!("`{key}: {raw}` was not followed ({reason})"),
        )
        .with_help(explain(*reason))
        .with_spec_ref(walk.spec.clone()),
    }
}

/// What a [`NotInspectedReason`] means for a reader of this standard.
///
/// Matched with a fallback arm because `NotInspectedReason` is
/// `#[non_exhaustive]`: a reason added to `conform-core` later must still
/// produce a sentence here rather than failing to compile or, worse, being
/// quietly dropped.
fn explain(reason: NotInspectedReason) -> &'static str {
    match reason {
        NotInspectedReason::OutsideDocumentSet => {
            "the target is in another document, and this validator reads one document and \
             performs no network access — this is not a claim that the target is missing"
        }
        NotInspectedReason::Unsupported => {
            "this validator does not know how to follow a reference of this form, so it did not \
             try — this is not a claim that the target is missing"
        }
        _ => "nobody looked, so this says nothing about whether the target exists",
    }
}

/// Every deprecated key present on one node, quoting upstream's own message.
fn deprecated_keys_in(
    node: &Value,
    deprecated: &BTreeMap<String, String>,
    pointer: &str,
    document: &DocumentId,
    spec: &SpecRef,
) -> Vec<Diagnostic> {
    let Some(members) = node.as_object() else {
        return Vec::new();
    };
    deprecated
        .iter()
        .filter(|(key, _)| members.contains_key(*key))
        .map(|(key, message)| {
            Diagnostic::warning(
                codes::DEPRECATED_KEYWORD,
                at(document, &format!("{pointer}/{}", escape(key))),
                format!("`{key}` is deprecated: {message}"),
            )
            .with_help(
                "the schema records this as a `deprecationMessage`, which is an annotation \
                 rather than an assertion, so no JSON Schema validator will ever object to it",
            )
            .with_spec_ref(spec.clone())
        })
        .collect()
}

/// Whether a field is missing, explicitly null, or present but carrying
/// nothing — an empty array, object or string.
///
/// `servers: {}` is a different mistake from omitting `servers`, but it is the
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

/// RFC 6901 token escaping for a pointer this module builds.
///
/// Not decoration: `definitions`' `propertyNames` pattern permits `/` inside a
/// key, so an unescaped name would produce a pointer that reads as two levels
/// of nesting and resolves to the wrong place — or to nothing.
fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

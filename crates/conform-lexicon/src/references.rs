//! Following a reference the document makes, and saying which of three things
//! happened.
//!
//! ODCL documents carry two kinds of internal reference, and the published
//! schema types both of them as a bare `"type": "string"` — it says what shape
//! the text has and nothing whatever about where it points:
//!
//! - `field.references`, `"orders.order_id"`, a foreign key into another
//!   model's field;
//! - `field.$ref`, "a reference URI to a definition in the specification,
//!   internally or externally", which is the schema's own wording and is the
//!   whole difficulty — the same key holds `#/definitions/order_id`, which
//!   this document can answer for, and `https://example.org/defs.yaml#/x`,
//!   which it cannot.
//!
//! So a validator following these has **three** possible answers, not two, and
//! that is why everything here returns [`Resolution`] rather than an `Option`:
//!
//! - [`Resolution::Resolved`] — the target is in this document, and here is
//!   where.
//! - [`Resolution::DoesNotExist`] — the reference names something inside this
//!   document, the document was asked, and it is not there. A finding.
//! - [`Resolution::NotInspected`] — nobody looked, and why. An external `$ref`
//!   is a network or second-file question and this crate is offline by
//!   construction and single-document by construction. **Not** a finding.
//!
//! Collapsing the last two is the failure this family of crates exists to
//! prevent: reporting "broken reference" for `https://…` is a false alarm, and
//! a gate that raises false alarms gets switched off, after which it protects
//! nothing at all.

use conform_core::{NotInspectedReason, Pointer, Resolution};
use serde_json::Value;

/// Follow a field's `references` — `"orders.order_id"` — against the document
/// it appears in.
///
/// The first segment names a model in `models`; each remaining segment names a
/// field, descending through `fields` maps so that
/// `"model.nested_field.field"` — the schema's own second example — resolves
/// through a nested object field.
///
/// # What is deliberately *not* followed
///
/// Two cases return
/// [`NotInspected`](conform_core::Resolution::NotInspected) rather than an
/// answer, and both are the same judgement: the specification does not say
/// what the reference means, so this crate does not decide on its behalf.
///
/// - A reference with **no `.`** in it names no model. Nothing was followed.
/// - A segment that does not match any key of the field's `fields`, on a
///   field that carries `items`, `keys` or `values` instead. Those hold a
///   nested `Field` each — an array's element, a map's key and value — and the
///   specification documents `references` only as `model.field` and
///   `model.nested_field.field`. It publishes no spelling for "the `sku` of
///   the objects in this array", so there is no path here to follow and
///   guessing one would invent a rule upstream has not written.
///
/// Reporting either as `DoesNotExist` would be a false alarm about a target
/// that may very well be there, which is the failure this whole type exists to
/// prevent.
///
/// ```
/// use conform_core::Resolution;
/// use conform_lexicon::resolve_field_reference;
///
/// let document: serde_json::Value = serde_json::json!({
///     "models": { "orders": { "fields": { "order_id": { "type": "string" } } } }
/// });
///
/// assert!(resolve_field_reference(&document, "orders.order_id").is_resolved());
/// assert!(resolve_field_reference(&document, "orders.nope").does_not_exist());
///
/// // Not a model-qualified reference at all. Nobody looked, and it says so.
/// assert!(resolve_field_reference(&document, "order_id").is_not_inspected());
///
/// // An array's element fields. The specification publishes no spelling for
/// // these, so this is "not followed", never "not there".
/// let nested: serde_json::Value = serde_json::json!({
///     "models": { "orders": { "fields": { "lines": {
///         "type": "array",
///         "items": { "fields": { "sku": { "type": "string" } } }
///     } } } }
/// });
/// let through_an_array = resolve_field_reference(&nested, "orders.lines.sku");
/// assert!(through_an_array.is_not_inspected());
/// assert!(!through_an_array.does_not_exist());
/// ```
#[must_use]
pub fn resolve_field_reference(document: &Value, raw: &str) -> Resolution<Pointer> {
    let mut segments = raw.split('.');
    let (Some(model), Some(first_field)) = (segments.next(), segments.next()) else {
        return Resolution::not_inspected(NotInspectedReason::Unsupported);
    };
    if model.is_empty() || first_field.is_empty() {
        return Resolution::not_inspected(NotInspectedReason::Unsupported);
    }

    let Some(mut node) = document.get("models").and_then(|m| m.get(model)) else {
        return Resolution::DoesNotExist;
    };
    let mut pointer = format!("/models/{}", escape(model));

    for field in std::iter::once(first_field).chain(segments) {
        if field.is_empty() {
            return Resolution::not_inspected(NotInspectedReason::Unsupported);
        }
        let Some(next) = node.get("fields").and_then(|f| f.get(field)) else {
            // Nothing under `fields` — but if this field's contents live in an
            // `items`, `keys` or `values` instead, the target may exist under
            // a spelling the specification has never published. Absent is a
            // claim this crate is not entitled to make there.
            if ["items", "keys", "values"]
                .iter()
                .any(|nested| node.get(*nested).is_some())
            {
                return Resolution::not_inspected(NotInspectedReason::Unsupported);
            }
            return Resolution::DoesNotExist;
        };
        pointer.push_str("/fields/");
        pointer.push_str(&escape(field));
        node = next;
    }

    Resolution::Resolved(Pointer::new(pointer))
}

/// Follow a field's `$ref` against the document it appears in.
///
/// Only a **document-local** reference — one whose fragment is the whole of it,
/// `#/definitions/order_id` — is followed. Anything naming another document,
/// by URL or by relative path, is
/// [`NotInspectedReason::OutsideDocumentSet`](conform_core::NotInspectedReason::OutsideDocumentSet):
/// this crate validates one document and performs no network access, so it has
/// no way to know whether that target exists and will not pretend otherwise.
///
/// # The one ambiguity, resolved deliberately
///
/// The schema permits `/` **inside** a definition name — `definitions`'
/// `propertyNames` pattern is `^[a-zA-Z0-9/_-]+$`, and the specification's own
/// advice is to "encode the domain into the ID using slashes". So
/// `#/definitions/sales/order_id` has two readings: a JSON Pointer descending
/// through a nested object, or the single definition named `sales/order_id`.
/// Both are tried, pointer-walk first, and the reference resolves if **either**
/// finds something. Reporting a live definition as absent because the other
/// reading was tried first would be exactly the false alarm this module is
/// built to avoid.
///
/// ```
/// use conform_core::NotInspectedReason;
/// use conform_lexicon::resolve_ref;
///
/// let document: serde_json::Value = serde_json::json!({
///     "definitions": { "order_id": { "type": "string" }, "sales/total": { "type": "number" } }
/// });
///
/// assert!(resolve_ref(&document, "#/definitions/order_id").is_resolved());
/// // A slash inside the name, which the schema permits explicitly.
/// assert!(resolve_ref(&document, "#/definitions/sales/total").is_resolved());
/// assert!(resolve_ref(&document, "#/definitions/absent").does_not_exist());
///
/// // Another document. Nobody looked, and the reason is on the record.
/// let remote = resolve_ref(&document, "https://example.org/defs.yaml#/definitions/x");
/// assert!(remote.is_not_inspected());
/// assert_eq!(remote.not_inspected_reason(), Some(NotInspectedReason::OutsideDocumentSet));
/// ```
#[must_use]
pub fn resolve_ref(document: &Value, raw: &str) -> Resolution<Pointer> {
    let trimmed = raw.trim();
    let Some(fragment) = trimmed.strip_prefix('#') else {
        // No fragment at all is a bare document reference — `orders.yaml`, or
        // a URL with no fragment. Either way it names another document.
        return Resolution::not_inspected(NotInspectedReason::OutsideDocumentSet);
    };
    if fragment.is_empty() {
        // `#` alone points at this document's root, which trivially exists and
        // says nothing. Following it is not a check.
        return Resolution::not_inspected(NotInspectedReason::Unsupported);
    }
    if !fragment.starts_with('/') {
        // A plain-name fragment (`#order_id`) is the older anchor form. This
        // crate does not index anchors, and saying so is better than guessing
        // that it means `#/definitions/order_id`.
        return Resolution::not_inspected(NotInspectedReason::Unsupported);
    }

    let tokens: Vec<String> = fragment[1..].split('/').map(unescape).collect();

    if let Some(found) = walk(document, &tokens) {
        return Resolution::Resolved(Pointer::new(found));
    }

    // The second reading: `definitions` holding one key that itself contains
    // slashes. Only attempted under `definitions`, because that is the only
    // place the schema permits `/` in a key.
    if tokens.len() > 2
        && tokens[0] == "definitions"
        && let Some(definitions) = document.get("definitions")
    {
        let joined = tokens[1..].join("/");
        if definitions.get(&joined).is_some() {
            return Resolution::Resolved(Pointer::new(format!("/definitions/{}", escape(&joined))));
        }
    }

    Resolution::DoesNotExist
}

/// Walk a JSON Pointer's already-unescaped tokens, returning the re-escaped
/// pointer when every one of them lands somewhere.
fn walk(document: &Value, tokens: &[String]) -> Option<String> {
    let mut node = document;
    let mut pointer = String::new();
    for token in tokens {
        node = match node {
            Value::Object(members) => members.get(token)?,
            Value::Array(items) => items.get(token.parse::<usize>().ok()?)?,
            _ => return None,
        };
        pointer.push('/');
        pointer.push_str(&escape(token));
    }
    Some(pointer)
}

/// RFC 6901 token escaping: `~` becomes `~0`, `/` becomes `~1`.
///
/// `~` first, or the `~1` written for a `/` would itself be re-escaped to
/// `~01` on the next pass. This is the order the RFC specifies and the order
/// is the whole correctness of it.
fn escape(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// RFC 6901 token unescaping, the exact inverse of [`escape`].
///
/// `~1` first for the same reason, read backwards: unescaping `~0` first would
/// turn a literal `~01` — which encodes `~1` — into `~1` and then into `/`.
fn unescape(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

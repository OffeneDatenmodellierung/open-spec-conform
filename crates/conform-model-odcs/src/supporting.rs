//! Supporting types for the ODCS native data structures.
//!
//! These types are used across [`ODCSContract`](crate::ODCSContract),
//! [`SchemaObject`](crate::SchemaObject) and [`Property`](crate::Property) to
//! represent shared concepts like quality rules, custom properties, and
//! relationships.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Every key of an ODCS object that this model does not name.
///
/// ODCS is an evolving standard and documents in the wild carry vendor
/// extensions, so a model that dropped what it did not recognise would turn a
/// read-modify-write into silent data loss. Every struct in this crate ends in
/// a `#[serde(flatten)] extra: Extra`, and the round-trip tests assert that a
/// document deserialized and re-serialized is byte-for-byte the same document.
pub type Extra = IndexMap<String, serde_json::Value>;

/// Quality rule for data validation (ODCS v3.1.0 `DataQuality`).
///
/// Quality rules can be defined at schema or property level.
/// Field order matches the ODCS v3.1.0 JSON schema for stable serialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct QualityRule {
    // === Identity ===
    /// Stable identifier for the quality rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Type of quality rule (e.g. `sql`, `custom`, `library`, `text`).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub rule_type: Option<String>,
    /// Name of the data quality check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Quality dimension (e.g. `accuracy`, `completeness`, `timeliness`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimension: Option<String>,
    /// Description of the quality rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Consequences of rule failure (e.g. `operational`, `regulatory`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub business_impact: Option<String>,
    /// Severity of the quality rule (e.g. `info`, `warning`, `error`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
    /// Method of validation (e.g. `reconciliation`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// Unit the rule uses (e.g. `rows`, `percent`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    // === Library-type fields ===
    /// Predefined metric name (e.g. `nullValues`, `missingValues`, `rowCount`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metric: Option<String>,
    /// Deprecated: use [`metric`](Self::metric) instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
    /// Additional arguments for the metric.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,

    // === Comparison operators (DataQualityOperators) ===
    /// Condition that must be true (equals).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must_be: Option<serde_json::Value>,
    /// Condition that must be false (not equals).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must_not_be: Option<serde_json::Value>,
    /// Greater than condition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must_be_greater_than: Option<serde_json::Value>,
    /// Greater than or equal condition (ODCS: `mustBeGreaterOrEqualTo`).
    ///
    /// The two aliases are spellings seen in documents written against earlier
    /// drafts; they read, but this model always writes the published spelling.
    #[serde(
        rename = "mustBeGreaterOrEqualTo",
        alias = "mustBeGreaterThanOrEqual",
        alias = "mustBeGreaterThanOrEqualTo",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub must_be_greater_or_equal_to: Option<serde_json::Value>,
    /// Less than condition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must_be_less_than: Option<serde_json::Value>,
    /// Less than or equal condition (ODCS: `mustBeLessOrEqualTo`).
    #[serde(
        rename = "mustBeLessOrEqualTo",
        alias = "mustBeLessThanOrEqual",
        alias = "mustBeLessThanOrEqualTo",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub must_be_less_or_equal_to: Option<serde_json::Value>,
    /// Range: value must be between two numbers `[min, max]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must_be_between: Option<Vec<serde_json::Value>>,
    /// Range: value must not be between two numbers `[min, max]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must_not_be_between: Option<Vec<serde_json::Value>>,
    /// Value must be in this set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must_be_in: Option<Vec<serde_json::Value>>,
    /// Value must not be in this set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub must_not_be_in: Option<Vec<serde_json::Value>>,

    // === SQL-type fields ===
    /// SQL query for validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,

    // === Custom-type fields ===
    /// Engine for running the quality check (e.g. `soda`, `great-expectations`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
    /// Engine-specific implementation details (string or object).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub implementation: Option<serde_json::Value>,
    /// URL to quality tool or dashboard.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    // === Scheduling ===
    /// Scheduler type for quality checks (e.g. `cron`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,
    /// Schedule expression (e.g. `0 20 * * *`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,

    // === References & Metadata ===
    /// Links to authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<AuthoritativeDefinition>,
    /// Tags for categorization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Additional properties for rule execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<CustomProperty>,

    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Custom property for format-specific metadata (ODCS v3.1.0 `CustomProperty`).
///
/// Used to store metadata that does not fit into the standard ODCS fields,
/// such as Avro-specific or Protobuf-specific attributes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomProperty {
    /// Stable identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Property name.
    pub property: String,
    /// Property value (flexible type).
    pub value: serde_json::Value,
    /// Optional description of the property.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

impl CustomProperty {
    /// Create a new custom property.
    #[must_use]
    pub fn new(property: impl Into<String>, value: serde_json::Value) -> Self {
        Self {
            id: None,
            property: property.into(),
            value,
            description: None,
            extra: Extra::new(),
        }
    }

    /// Create a string custom property.
    #[must_use]
    pub fn string(property: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(property, serde_json::Value::String(value.into()))
    }
}

/// Authoritative definition reference (ODCS v3.1.0).
///
/// Links to external authoritative sources for definitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuthoritativeDefinition {
    /// Stable identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Type of the reference (e.g. `businessDefinition`, `transformationImplementation`).
    #[serde(rename = "type")]
    pub definition_type: String,
    /// URL to the authoritative definition.
    pub url: String,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

impl AuthoritativeDefinition {
    /// Create a new authoritative definition.
    #[must_use]
    pub fn new(definition_type: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            id: None,
            definition_type: definition_type.into(),
            url: url.into(),
            extra: Extra::new(),
        }
    }
}

/// One end of a schema-level relationship.
///
/// ODCS v3.1.0 models `from` and `to` as `oneOf` a single column name or an
/// array of column names for a composite key, and requires the two ends to
/// agree in arity. This model records which spelling the document used rather
/// than normalising to a vector, so a single-column relationship written as a
/// string is written back out as a string.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum RelationshipEnd {
    /// Single-column relationship: `from: order_id`.
    Single(String),
    /// Composite-key relationship: `from: [tenant_id, order_id]`.
    Composite(Vec<String>),
}

impl RelationshipEnd {
    /// The column names at this end, whichever spelling the document used.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        match self {
            Self::Single(s) => vec![s.as_str()],
            Self::Composite(v) => v.iter().map(String::as_str).collect(),
        }
    }
}

/// Schema-level relationship (ODCS v3.1.0 `RelationshipSchemaLevel`).
///
/// Represents relationships between schema objects (tables).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SchemaRelationship {
    /// Relationship type (e.g. `foreignKey`, `parent`, `child`).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub relationship_type: Option<String>,
    /// Source column or columns in this schema object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<RelationshipEnd>,
    /// Target reference, as a shorthand or fully qualified reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<RelationshipEnd>,
    /// Additional properties carried on the relationship.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<CustomProperty>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Property-level relationship (ODCS v3.1.0 `RelationshipPropertyLevel`).
///
/// Represents relationships from a property to other definitions. The `from`
/// end is deliberately absent: ODCS derives it from the property the
/// relationship is written on, and the published schema forbids stating it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PropertyRelationship {
    /// Relationship type (e.g. `foreignKey`, `parent`, `child`).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub relationship_type: Option<String>,
    /// Target reference (e.g. `definitions/order_id`, `schema/id/properties/id`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<RelationshipEnd>,
    /// Additional properties carried on the relationship.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<CustomProperty>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

impl PropertyRelationship {
    /// Create a new property relationship.
    #[must_use]
    pub fn new(relationship_type: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            relationship_type: Some(relationship_type.into()),
            to: Some(RelationshipEnd::Single(to.into())),
            custom_properties: Vec::new(),
            extra: Extra::new(),
        }
    }
}

/// Logical type options for additional type metadata (ODCS v3.1.0).
///
/// ODCS constrains which of these keys are legal for a given `logicalType` —
/// `minLength` for strings, `precision` for numbers and so on. That is a
/// validation question, not a modelling one, so every key is optional here and
/// the constraint is left to a validator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct LogicalTypeOptions {
    /// Minimum length for strings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<i64>,
    /// Maximum length for strings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<i64>,
    /// Regex pattern for strings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Format hint (e.g. `email`, `uuid`, `uri`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Minimum value for numbers/dates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<serde_json::Value>,
    /// Maximum value for numbers/dates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<serde_json::Value>,
    /// Exclusive minimum for numbers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive_minimum: Option<serde_json::Value>,
    /// Exclusive maximum for numbers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusive_maximum: Option<serde_json::Value>,
    /// Precision for decimals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<i32>,
    /// Scale for decimals.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<i32>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

impl LogicalTypeOptions {
    /// Check if all options are empty/None.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.min_length.is_none()
            && self.max_length.is_none()
            && self.pattern.is_none()
            && self.format.is_none()
            && self.minimum.is_none()
            && self.maximum.is_none()
            && self.exclusive_minimum.is_none()
            && self.exclusive_maximum.is_none()
            && self.precision.is_none()
            && self.scale.is_none()
            && self.extra.is_empty()
    }
}

/// Contract ownership, as either an object or a bare list of members.
///
/// ODCS v3.1.0 publishes `team` as a `oneOf`: the v3 spelling is an object with
/// a `members` array, and the v2 spelling — still accepted — is the array of
/// members on its own. Normalising the two would make a v2 document come back
/// out as a v3 one, so this model records which was written.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TeamRef {
    /// The v3 spelling: an object carrying `members`.
    Object(Box<Team>),
    /// The v2 spelling: the member list on its own.
    Members(Vec<TeamMember>),
}

impl TeamRef {
    /// The members named, under either spelling.
    #[must_use]
    pub fn members(&self) -> &[TeamMember] {
        match self {
            Self::Object(t) => &t.members,
            Self::Members(m) => m,
        }
    }
}

/// Team information (ODCS v3.1.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    /// Stable identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Team name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Team description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Team members.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<TeamMember>,
    /// Tags for categorization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<CustomProperty>,
    /// Authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<AuthoritativeDefinition>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Team member information (ODCS v3.1.0 `TeamMember`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TeamMember {
    /// Stable identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Username or email; the one field ODCS requires of a member.
    pub username: String,
    /// Member name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Description of the member's involvement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Member role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Date the member joined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_in: Option<String>,
    /// Date the member left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_out: Option<String>,
    /// Username of whoever replaced this member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by_username: Option<String>,
    /// Tags for categorization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<CustomProperty>,
    /// Authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<AuthoritativeDefinition>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// One support channel (ODCS v3.1.0 `SupportItem`).
///
/// ODCS publishes `support` as an array of these; a document naming one
/// channel still writes a one-element array.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SupportItem {
    /// Stable identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Support channel; the one field ODCS requires of a channel.
    pub channel: String,
    /// Support URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Description of what this channel is for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tool backing the channel (e.g. `slack`, `email`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Scope of the channel (e.g. `interactive`, `announcements`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// URL to request access to the channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitation_url: Option<String>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<CustomProperty>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Server configuration (ODCS v3.1.0 `Server`).
///
/// ODCS requires `server` and `type`, then makes the rest of the keys
/// conditional on the `type`: a `postgresql` server takes `host`/`port`, an
/// `s3` server takes `location`, a `bigquery` server takes `project`/`dataset`.
/// This model names the union of the common ones and lets `extra` carry the
/// remainder, because which subset is legal is a validator's question.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    /// Stable identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Server name/identifier.
    pub server: String,
    /// Server type (e.g. `bigquery`, `snowflake`, `s3`, `postgresql`).
    #[serde(rename = "type")]
    pub server_type: String,
    /// Server environment (e.g. `production`, `development`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Server description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Database name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    /// Project name (for cloud platforms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Schema name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    /// Catalog name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<String>,
    /// Dataset name (for `BigQuery`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<String>,
    /// Account name (for Snowflake).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// Host name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Location/region, or a URI for object stores.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// Format for file-based servers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Delimiter for CSV files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delimiter: Option<String>,
    /// Topic name for streaming.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    /// Roles granting access to this server.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<Role>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<CustomProperty>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Role definition (ODCS v3.1.0 `Role`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    /// Stable identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Role name; the one field ODCS requires of a role.
    pub role: String,
    /// Role description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Access level granted by the role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    /// First level approvers for a request for this role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_level_approvers: Option<String>,
    /// Second level approvers for a request for this role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_level_approvers: Option<String>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<CustomProperty>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Service level agreement property (ODCS v3.1.0 `ServiceLevelAgreementProperty`).
///
/// These live under the contract's `slaProperties` key.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceLevelAgreementProperty {
    /// Stable identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Service level property name (e.g. `latency`, `retention`).
    pub property: String,
    /// Value.
    pub value: serde_json::Value,
    /// Extended value, for properties that take two (e.g. a range).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_ext: Option<serde_json::Value>,
    /// Unit of measurement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Element this applies to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    /// Driver for this SLA (e.g. `analytics`, `regulatory`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Scheduler.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduler: Option<String>,
    /// Schedule expression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Price information (ODCS v3.1.0 `Pricing`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Pricing {
    /// Stable identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Price amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_amount: Option<serde_json::Value>,
    /// Currency, as an ISO 4217 code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_currency: Option<String>,
    /// The unit the amount is charged per (e.g. `megabyte`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_unit: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Description that can be a string or a structured object (ODCS v3.1.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Description {
    /// Simple string description.
    Simple(String),
    /// Structured description object.
    Structured(Box<StructuredDescription>),
}

impl Default for Description {
    fn default() -> Self {
        Self::Simple(String::new())
    }
}

impl Description {
    /// Get the description as a simple string.
    ///
    /// For a structured description this is its `purpose`, which is the field
    /// ODCS documents as the one-line answer to "what is this data for".
    #[must_use]
    pub fn as_string(&self) -> String {
        match self {
            Self::Simple(s) => s.clone(),
            Self::Structured(d) => d.purpose.clone().unwrap_or_default(),
        }
    }
}

/// Structured description with multiple fields (ODCS v3.1.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StructuredDescription {
    /// Purpose of the data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    /// Limitations of the data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limitations: Option<String>,
    /// Usage guidelines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
    /// Authoritative definitions for the description.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<AuthoritativeDefinition>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<CustomProperty>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

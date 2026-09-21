//! `ODCSContract` type for the ODCS native data structures.
//!
//! Represents the root data contract document following the ODCS v3.1.0
//! specification.

use serde::{Deserialize, Serialize};

use crate::schema::SchemaObject;
use crate::supporting::{
    AuthoritativeDefinition, CustomProperty, Description, Extra, Pricing, Role, Server,
    ServiceLevelAgreementProperty, SupportItem, TeamRef,
};

/// `ODCSContract` — the root data contract document (ODCS v3.1.0).
///
/// This is the top-level structure that represents an entire ODCS data
/// contract. It contains all contract-level metadata plus zero or more schema
/// objects (tables).
///
/// ODCS v3.1.0 requires exactly five keys of a contract — `version`,
/// `apiVersion`, `kind`, `id` and `status` — and those five are the only
/// non-`Option` fields here. Notably `name` is **not** one of them: the
/// smallest document the published schema accepts carries no name at all.
///
/// # Example
///
/// ```rust
/// use conform_model_odcs::{ODCSContract, SchemaObject, Property};
///
/// let contract = ODCSContract::new("customer-contract", "v1.0.0")
///     .with_domain("retail")
///     .with_status("active")
///     .with_schema(
///         SchemaObject::new("customers")
///             .with_physical_type("table")
///             .with_properties(vec![
///                 Property::new("id", "integer").with_primary_key(true),
///                 Property::new("name", "string").with_required(true),
///             ])
///     );
///
/// assert_eq!(contract.schema_count(), 1);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ODCSContract {
    // === Required Identity Fields ===
    /// Contract version (semantic versioning recommended).
    pub version: String,
    /// API version (e.g. `v3.1.0`).
    pub api_version: String,
    /// Kind identifier (always `DataContract`).
    pub kind: String,
    /// Unique contract ID (UUID or other identifier).
    pub id: String,
    /// Contract status: `draft`, `active`, `deprecated`, `retired`.
    ///
    /// ODCS publishes the conventional values as `examples`, not as an `enum`,
    /// so this is a `String` and not a closed set: a document reading
    /// `status: mothballed` is a conformant ODCS document and this model reads
    /// it without complaint. Judging it is a validator's job.
    pub status: String,

    // === Identity ===
    /// Contract name. Optional: ODCS does not require it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    // === Organization ===
    /// Domain this contract belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Data product this contract belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_product: Option<String>,
    /// Tenant identifier for multi-tenant systems.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,

    // === Description ===
    /// Contract description (can be a simple string or a structured object).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<Description>,

    // === Schema (Tables) ===
    /// Schema objects (tables, views, topics) in this contract.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schema: Vec<SchemaObject>,

    // === Configuration ===
    /// Server configurations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub servers: Vec<Server>,
    /// Team information, under either the v3 object or the v2 array spelling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<TeamRef>,
    /// Support channels. ODCS publishes this as an array.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub support: Vec<SupportItem>,
    /// Role definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<Role>,

    // === SLA ===
    /// The schema element an `slaProperties` entry refers to by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sla_default_element: Option<String>,
    /// Service level agreement properties.
    ///
    /// ODCS spells this key `slaProperties`. The `serviceLevels` alias reads
    /// documents written by tools that guessed otherwise; this model always
    /// writes the published spelling.
    #[serde(
        default,
        alias = "serviceLevels",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub sla_properties: Vec<ServiceLevelAgreementProperty>,

    // === Pricing ===
    /// Price information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<Pricing>,

    // === References ===
    /// Authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<AuthoritativeDefinition>,

    // === Tags & Custom Properties ===
    /// Contract-level tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Custom properties for format-specific metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<CustomProperty>,

    // === Timestamps ===
    /// Contract creation timestamp (ISO 8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_created_ts: Option<String>,

    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for ODCSContract {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            api_version: "v3.1.0".to_string(),
            kind: "DataContract".to_string(),
            id: String::new(),
            status: "draft".to_string(),
            name: None,
            domain: None,
            data_product: None,
            tenant: None,
            description: None,
            schema: Vec::new(),
            servers: Vec::new(),
            team: None,
            support: Vec::new(),
            roles: Vec::new(),
            sla_default_element: None,
            sla_properties: Vec::new(),
            price: None,
            authoritative_definitions: Vec::new(),
            tags: Vec::new(),
            custom_properties: Vec::new(),
            contract_created_ts: None,
            extra: Extra::new(),
        }
    }
}

impl ODCSContract {
    /// Create a new contract with the given name and version.
    ///
    /// The `id` is left empty: ODCS requires one, and inventing a UUID here
    /// would need a random-number dependency this crate does not want and
    /// would hand the caller an identifier they did not choose. Set it with
    /// [`with_id`](Self::with_id), or build with [`new_with_id`](Self::new_with_id).
    #[must_use]
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            version: version.into(),
            ..Default::default()
        }
    }

    /// Create a new contract with a specific ID.
    #[must_use]
    pub fn new_with_id(
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: Some(name.into()),
            version: version.into(),
            ..Default::default()
        }
    }

    /// Set the contract ID.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Set the API version.
    #[must_use]
    pub fn with_api_version(mut self, api_version: impl Into<String>) -> Self {
        self.api_version = api_version.into();
        self
    }

    /// Set the status.
    #[must_use]
    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    /// Set the domain.
    #[must_use]
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Set the data product.
    #[must_use]
    pub fn with_data_product(mut self, data_product: impl Into<String>) -> Self {
        self.data_product = Some(data_product.into());
        self
    }

    /// Set the tenant.
    #[must_use]
    pub fn with_tenant(mut self, tenant: impl Into<String>) -> Self {
        self.tenant = Some(tenant.into());
        self
    }

    /// Set the description (simple string).
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(Description::Simple(description.into()));
        self
    }

    /// Set a structured description.
    #[must_use]
    pub fn with_structured_description(mut self, description: Description) -> Self {
        self.description = Some(description);
        self
    }

    /// Add a schema object.
    #[must_use]
    pub fn with_schema(mut self, schema: SchemaObject) -> Self {
        self.schema.push(schema);
        self
    }

    /// Set all schema objects.
    #[must_use]
    pub fn with_schemas(mut self, schemas: Vec<SchemaObject>) -> Self {
        self.schema = schemas;
        self
    }

    /// Add a server configuration.
    #[must_use]
    pub fn with_server(mut self, server: Server) -> Self {
        self.servers.push(server);
        self
    }

    /// Set the team information.
    #[must_use]
    pub fn with_team(mut self, team: TeamRef) -> Self {
        self.team = Some(team);
        self
    }

    /// Add a support channel.
    #[must_use]
    pub fn with_support(mut self, support: SupportItem) -> Self {
        self.support.push(support);
        self
    }

    /// Add a role.
    #[must_use]
    pub fn with_role(mut self, role: Role) -> Self {
        self.roles.push(role);
        self
    }

    /// Add a service level agreement property.
    #[must_use]
    pub fn with_sla_property(mut self, sla_property: ServiceLevelAgreementProperty) -> Self {
        self.sla_properties.push(sla_property);
        self
    }

    /// Set the price.
    #[must_use]
    pub fn with_price(mut self, price: Pricing) -> Self {
        self.price = Some(price);
        self
    }

    /// Add an authoritative definition.
    #[must_use]
    pub fn with_authoritative_definition(mut self, definition: AuthoritativeDefinition) -> Self {
        self.authoritative_definitions.push(definition);
        self
    }

    /// Add a tag.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set all tags.
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Add a custom property.
    #[must_use]
    pub fn with_custom_property(mut self, custom_property: CustomProperty) -> Self {
        self.custom_properties.push(custom_property);
        self
    }

    /// Set the contract creation timestamp.
    #[must_use]
    pub fn with_contract_created_ts(mut self, timestamp: impl Into<String>) -> Self {
        self.contract_created_ts = Some(timestamp.into());
        self
    }

    /// Get the number of schema objects.
    #[must_use]
    pub fn schema_count(&self) -> usize {
        self.schema.len()
    }

    /// Get a schema object by name.
    #[must_use]
    pub fn get_schema(&self, name: &str) -> Option<&SchemaObject> {
        self.schema.iter().find(|s| s.name == name)
    }

    /// Get a mutable schema object by name.
    pub fn get_schema_mut(&mut self, name: &str) -> Option<&mut SchemaObject> {
        self.schema.iter_mut().find(|s| s.name == name)
    }

    /// Get all schema names.
    #[must_use]
    pub fn schema_names(&self) -> Vec<&str> {
        self.schema.iter().map(|s| s.name.as_str()).collect()
    }

    /// Check if this is a multi-table contract.
    #[must_use]
    pub fn is_multi_table(&self) -> bool {
        self.schema.len() > 1
    }

    /// Get the first schema (for single-table contracts).
    #[must_use]
    pub fn first_schema(&self) -> Option<&SchemaObject> {
        self.schema.first()
    }

    /// Get the description as a simple string.
    #[must_use]
    pub fn description_string(&self) -> Option<String> {
        self.description.as_ref().map(Description::as_string)
    }
}

#[cfg(feature = "yaml")]
impl ODCSContract {
    /// Parse a contract from a YAML (or JSON — YAML is a superset) document.
    ///
    /// # Errors
    ///
    /// Returns the underlying `serde_norway` error if the document is not
    /// well-formed YAML, or if it is missing one of the five keys ODCS
    /// requires of every contract.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_norway::Error> {
        serde_norway::from_str(yaml)
    }

    /// Render this contract as a YAML document.
    ///
    /// # Errors
    ///
    /// Returns the underlying `serde_norway` error if the contract cannot be
    /// represented as YAML.
    pub fn to_yaml(&self) -> Result<String, serde_norway::Error> {
        serde_norway::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::property::Property;

    #[test]
    fn contract_creation() {
        let contract = ODCSContract::new("my-contract", "1.0.0")
            .with_domain("retail")
            .with_status("active");

        assert_eq!(contract.name.as_deref(), Some("my-contract"));
        assert_eq!(contract.version, "1.0.0");
        assert_eq!(contract.domain, Some("retail".to_string()));
        assert_eq!(contract.status, "active");
        assert_eq!(contract.api_version, "v3.1.0");
        assert_eq!(contract.kind, "DataContract");
    }

    #[test]
    fn contract_with_schema() {
        let contract = ODCSContract::new("order-contract", "2.0.0")
            .with_schema(
                SchemaObject::new("orders")
                    .with_physical_type("table")
                    .with_properties(vec![
                        Property::new("id", "integer").with_primary_key(true),
                        Property::new("customer_id", "integer"),
                        Property::new("total", "number"),
                    ]),
            )
            .with_schema(
                SchemaObject::new("order_items")
                    .with_physical_type("table")
                    .with_properties(vec![
                        Property::new("id", "integer").with_primary_key(true),
                        Property::new("order_id", "integer"),
                        Property::new("product_id", "integer"),
                    ]),
            );

        assert_eq!(contract.schema_count(), 2);
        assert!(contract.is_multi_table());
        assert_eq!(contract.schema_names(), vec!["orders", "order_items"]);
        assert_eq!(contract.get_schema("orders").unwrap().property_count(), 3);
    }

    #[test]
    fn contract_serializes_in_camel_case() {
        let contract = ODCSContract::new_with_id(
            "550e8400-e29b-41d4-a716-446655440000",
            "test-contract",
            "1.0.0",
        )
        .with_domain("test")
        .with_status("draft")
        .with_description("A test contract")
        .with_tag("test")
        .with_schema(SchemaObject::new("test_table").with_property(Property::new("id", "string")));

        let json = serde_json::to_string_pretty(&contract).unwrap();

        assert!(json.contains("\"apiVersion\": \"v3.1.0\""));
        assert!(json.contains("\"kind\": \"DataContract\""));
        assert!(json.contains("\"id\": \"550e8400-e29b-41d4-a716-446655440000\""));
        assert!(json.contains("\"name\": \"test-contract\""));
        assert!(json.contains("\"domain\": \"test\""));
        assert!(json.contains("\"status\": \"draft\""));
        assert!(!json.contains("api_version"));
    }

    #[test]
    fn contract_deserialization() {
        let json = r#"{
            "apiVersion": "v3.1.0",
            "kind": "DataContract",
            "id": "test-id-123",
            "version": "2.0.0",
            "name": "customer-contract",
            "status": "active",
            "domain": "customers",
            "description": "Customer data contract",
            "schema": [
                {
                    "name": "customers",
                    "physicalType": "table",
                    "properties": [
                        { "name": "id", "logicalType": "integer", "primaryKey": true },
                        { "name": "name", "logicalType": "string", "required": true }
                    ]
                }
            ],
            "tags": ["customer", "pii"]
        }"#;

        let contract: ODCSContract = serde_json::from_str(json).unwrap();
        assert_eq!(contract.api_version, "v3.1.0");
        assert_eq!(contract.kind, "DataContract");
        assert_eq!(contract.id, "test-id-123");
        assert_eq!(contract.version, "2.0.0");
        assert_eq!(contract.name.as_deref(), Some("customer-contract"));
        assert_eq!(contract.status, "active");
        assert_eq!(contract.schema_count(), 1);
        assert_eq!(contract.tags, vec!["customer", "pii"]);
        assert_eq!(
            contract.get_schema("customers").unwrap().property_count(),
            2
        );
    }

    #[test]
    fn structured_description() {
        let json = r#"{
            "apiVersion": "v3.1.0", "kind": "DataContract", "id": "test",
            "version": "1.0.0", "status": "active",
            "description": {
                "purpose": "Store customer information",
                "usage": "Read-only access for analytics"
            }
        }"#;

        let contract: ODCSContract = serde_json::from_str(json).unwrap();
        assert_eq!(
            contract.description_string(),
            Some("Store customer information".to_string())
        );
    }

    #[test]
    fn a_contract_without_a_name_is_legal() {
        // The five keys ODCS lists as `required`, and nothing else.
        let json = r#"{"version":"1.0.0","apiVersion":"v3.1.0","kind":"DataContract",
                       "id":"53581432-6c55-4ba2-a65f-72344a91553a","status":"active"}"#;
        let contract: ODCSContract = serde_json::from_str(json).unwrap();
        assert!(contract.name.is_none());
        assert!(contract.schema.is_empty());
    }

    #[test]
    fn a_contract_missing_a_required_key_is_refused() {
        // No `id`. This model reads ODCS; it does not repair it.
        let json =
            r#"{"version":"1.0.0","apiVersion":"v3.1.0","kind":"DataContract","status":"active"}"#;
        let err = serde_json::from_str::<ODCSContract>(json).unwrap_err();
        assert!(err.to_string().contains("id"), "unexpected error: {err}");
    }

    #[test]
    fn service_levels_alias_reads_but_does_not_write() {
        let json = r#"{"version":"1.0.0","apiVersion":"v3.1.0","kind":"DataContract","id":"x",
                       "status":"active","serviceLevels":[{"property":"latency","value":4}]}"#;
        let contract: ODCSContract = serde_json::from_str(json).unwrap();
        assert_eq!(contract.sla_properties.len(), 1);
        let back = serde_json::to_string(&contract).unwrap();
        assert!(back.contains("slaProperties"));
        assert!(!back.contains("serviceLevels"));
    }
}

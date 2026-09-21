//! `SchemaObject` type for the ODCS native data structures.
//!
//! Represents a table/view/topic in an ODCS contract with full support
//! for all schema-level metadata fields.

use serde::{Deserialize, Serialize};

use crate::property::Property;
use crate::supporting::{
    AuthoritativeDefinition, CustomProperty, Extra, QualityRule, SchemaRelationship,
};

/// `SchemaObject` — one table/view/topic in a contract (ODCS v3.1.0).
///
/// Schema objects represent individual data structures within a contract.
/// Each schema object contains properties (columns) and can have its own
/// metadata like quality rules, relationships, and authoritative definitions.
///
/// # Example
///
/// ```rust
/// use conform_model_odcs::{SchemaObject, Property};
///
/// let users_table = SchemaObject::new("users")
///     .with_physical_name("tbl_users")
///     .with_physical_type("table")
///     .with_business_name("User Accounts")
///     .with_description("Contains registered user information")
///     .with_properties(vec![
///         Property::new("id", "integer").with_primary_key(true),
///         Property::new("email", "string").with_required(true),
///         Property::new("name", "string"),
///     ]);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct SchemaObject {
    // === Core Identity Fields ===
    /// Stable technical identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Schema object name (table/view name). The one field ODCS requires here.
    pub name: String,
    /// Logical type of the object, which ODCS v3.1.0 spells `object`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical_type: Option<String>,
    /// Physical name in the data source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_name: Option<String>,
    /// Physical type (`table`, `view`, `topic`, `file`, `object`, `stream`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_type: Option<String>,
    /// Business name for the schema object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub business_name: Option<String>,
    /// Schema object description/documentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    // === Granularity ===
    /// Description of the data granularity (e.g. "One row per customer per day").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_granularity_description: Option<String>,

    // === Properties (Columns) ===
    /// List of properties/columns in this schema object.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<Property>,

    // === Relationships ===
    /// Schema-level relationships to other schema objects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<SchemaRelationship>,

    // === Quality & Validation ===
    /// Quality rules and checks at schema level.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality: Vec<QualityRule>,

    // === References ===
    /// Authoritative definitions for this schema object.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<AuthoritativeDefinition>,

    // === Tags & Custom Properties ===
    /// Schema-level tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Custom properties for format-specific metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<CustomProperty>,

    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

impl SchemaObject {
    /// Create a new schema object with the given name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Set the logical type.
    #[must_use]
    pub fn with_logical_type(mut self, logical_type: impl Into<String>) -> Self {
        self.logical_type = Some(logical_type.into());
        self
    }

    /// Set the physical name.
    #[must_use]
    pub fn with_physical_name(mut self, physical_name: impl Into<String>) -> Self {
        self.physical_name = Some(physical_name.into());
        self
    }

    /// Set the physical type.
    #[must_use]
    pub fn with_physical_type(mut self, physical_type: impl Into<String>) -> Self {
        self.physical_type = Some(physical_type.into());
        self
    }

    /// Set the business name.
    #[must_use]
    pub fn with_business_name(mut self, business_name: impl Into<String>) -> Self {
        self.business_name = Some(business_name.into());
        self
    }

    /// Set the description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the data granularity description.
    #[must_use]
    pub fn with_data_granularity_description(mut self, description: impl Into<String>) -> Self {
        self.data_granularity_description = Some(description.into());
        self
    }

    /// Set the properties (columns).
    #[must_use]
    pub fn with_properties(mut self, properties: Vec<Property>) -> Self {
        self.properties = properties;
        self
    }

    /// Add a property.
    #[must_use]
    pub fn with_property(mut self, property: Property) -> Self {
        self.properties.push(property);
        self
    }

    /// Set the relationships.
    #[must_use]
    pub fn with_relationships(mut self, relationships: Vec<SchemaRelationship>) -> Self {
        self.relationships = relationships;
        self
    }

    /// Add a relationship.
    #[must_use]
    pub fn with_relationship(mut self, relationship: SchemaRelationship) -> Self {
        self.relationships.push(relationship);
        self
    }

    /// Set the quality rules.
    #[must_use]
    pub fn with_quality(mut self, quality: Vec<QualityRule>) -> Self {
        self.quality = quality;
        self
    }

    /// Add a quality rule.
    #[must_use]
    pub fn with_quality_rule(mut self, rule: QualityRule) -> Self {
        self.quality.push(rule);
        self
    }

    /// Set the authoritative definitions.
    #[must_use]
    pub fn with_authoritative_definitions(
        mut self,
        definitions: Vec<AuthoritativeDefinition>,
    ) -> Self {
        self.authoritative_definitions = definitions;
        self
    }

    /// Add an authoritative definition.
    #[must_use]
    pub fn with_authoritative_definition(mut self, definition: AuthoritativeDefinition) -> Self {
        self.authoritative_definitions.push(definition);
        self
    }

    /// Set the tags.
    #[must_use]
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Add a tag.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set the custom properties.
    #[must_use]
    pub fn with_custom_properties(mut self, custom_properties: Vec<CustomProperty>) -> Self {
        self.custom_properties = custom_properties;
        self
    }

    /// Add a custom property.
    #[must_use]
    pub fn with_custom_property(mut self, custom_property: CustomProperty) -> Self {
        self.custom_properties.push(custom_property);
        self
    }

    /// Get the number of properties.
    #[must_use]
    pub fn property_count(&self) -> usize {
        self.properties.len()
    }

    /// Get a property by name.
    #[must_use]
    pub fn get_property(&self, name: &str) -> Option<&Property> {
        self.properties.iter().find(|p| p.name_or_empty() == name)
    }

    /// Get a mutable property by name.
    pub fn get_property_mut(&mut self, name: &str) -> Option<&mut Property> {
        self.properties
            .iter_mut()
            .find(|p| p.name_or_empty() == name)
    }

    /// Get all property names.
    #[must_use]
    pub fn property_names(&self) -> Vec<&str> {
        self.properties
            .iter()
            .map(Property::name_or_empty)
            .collect()
    }

    /// Get the primary key properties, ordered by their declared position.
    ///
    /// Properties that declare `primaryKey` without a `primaryKeyPosition`
    /// sort after those that declare one, in document order.
    #[must_use]
    pub fn primary_key_properties(&self) -> Vec<&Property> {
        let mut keys: Vec<&Property> = self.properties.iter().filter(|p| p.primary_key).collect();
        keys.sort_by_key(|p| p.primary_key_position.unwrap_or(i32::MAX));
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_object_creation() {
        let schema = SchemaObject::new("orders")
            .with_physical_type("table")
            .with_property(Property::new("id", "string"));

        assert_eq!(schema.name, "orders");
        assert_eq!(schema.physical_type.as_deref(), Some("table"));
        assert_eq!(schema.property_count(), 1);
        assert_eq!(schema.property_names(), vec!["id"]);
    }

    #[test]
    fn primary_key_properties_sort_by_position() {
        let schema = SchemaObject::new("orders").with_properties(vec![
            Property::new("b", "string")
                .with_primary_key(true)
                .with_primary_key_position(2),
            Property::new("not_a_key", "string"),
            Property::new("a", "string")
                .with_primary_key(true)
                .with_primary_key_position(1),
        ]);

        let names: Vec<&str> = schema
            .primary_key_properties()
            .iter()
            .map(|p| p.name_or_empty())
            .collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn unknown_keys_survive_a_round_trip() {
        let json = r#"{"name":"orders","x-owner":"retail"}"#;
        let schema: SchemaObject = serde_json::from_str(json).unwrap();
        assert_eq!(
            schema.extra.get("x-owner"),
            Some(&serde_json::Value::String("retail".into()))
        );
        let back: serde_json::Value = serde_json::to_value(&schema).unwrap();
        assert_eq!(
            back,
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }
}

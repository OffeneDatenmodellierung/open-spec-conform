//! `Property` type for the ODCS native data structures.
//!
//! Represents a column/field in an ODCS schema object with full support
//! for nested properties (OBJECT and ARRAY types).

use serde::{Deserialize, Serialize};

use crate::supporting::{
    AuthoritativeDefinition, CustomProperty, Extra, LogicalTypeOptions, PropertyRelationship,
    QualityRule,
};

/// Helper predicate for `skip_serializing_if`: omit a `false` boolean.
///
/// Taken by reference because that is the signature `skip_serializing_if`
/// requires, not because a `bool` is expensive to copy.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_false(b: &bool) -> bool {
    !*b
}

/// Property — one column in a schema object (ODCS v3.1.0 `SchemaProperty`).
///
/// Properties represent individual fields in a schema. They support nested
/// structures through the `properties` field (for OBJECT types) and the
/// `items` field (for ARRAY types).
///
/// `name` and `logical_type` are `Option` rather than `String`. ODCS requires
/// `name` of a `SchemaProperty` but *not* of a `SchemaItemProperty` — the
/// anonymous element type under an array's `items` — and this one type models
/// both, so a required `name` here would refuse to read a legal document.
/// `logicalType` ODCS does not require anywhere.
///
/// # Example
///
/// ```rust
/// use conform_model_odcs::{Property, LogicalTypeOptions};
///
/// // Simple property
/// let id_prop = Property::new("id", "integer")
///     .with_primary_key(true)
///     .with_required(true);
///
/// // Nested object property
/// let address_prop = Property::new("address", "object")
///     .with_nested_properties(vec![
///         Property::new("street", "string"),
///         Property::new("city", "string"),
///         Property::new("zip", "string"),
///     ]);
///
/// // Array property, whose element type is anonymous
/// let tags_prop = Property::new("tags", "array")
///     .with_items(Property::anonymous("string"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
// ODCS models eight independent boolean facets of a column. They are the
// specification's shape, not an accident of ours, and collapsing them into a
// flags type would make the model harder to read than the document it models.
#[allow(clippy::struct_excessive_bools)]
pub struct Property {
    // === Core Identity Fields ===
    /// Stable technical identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Property name. Absent only for the element type under `items`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Business name for the property.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub business_name: Option<String>,
    /// Property description/documentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    // === Type Information ===
    /// Logical data type (e.g. `string`, `integer`, `number`, `boolean`, `object`, `array`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical_type: Option<String>,
    /// Physical database type (e.g. `VARCHAR(100)`, `BIGINT`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_type: Option<String>,
    /// Physical name in the data source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_name: Option<String>,
    /// Additional type options (min/max length, pattern, precision, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical_type_options: Option<LogicalTypeOptions>,

    // === Key Constraints ===
    /// Whether the property is required (inverse of nullable).
    #[serde(default, skip_serializing_if = "is_false")]
    pub required: bool,
    /// Whether this property is part of the primary key.
    #[serde(default, skip_serializing_if = "is_false")]
    pub primary_key: bool,
    /// Position in composite primary key, 1-based.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_key_position: Option<i32>,
    /// Whether the property contains unique values.
    #[serde(default, skip_serializing_if = "is_false")]
    pub unique: bool,

    // === Partitioning & Clustering ===
    /// Whether the property is used for partitioning.
    #[serde(default, skip_serializing_if = "is_false")]
    pub partitioned: bool,
    /// Position in partition key, 1-based.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition_key_position: Option<i32>,

    // === Data Classification & Security ===
    /// Data classification level (e.g. `confidential`, `public`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification: Option<String>,
    /// Whether this is a critical data element.
    #[serde(default, skip_serializing_if = "is_false")]
    pub critical_data_element: bool,
    /// Name of the encrypted version of this property.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_name: Option<String>,

    // === Transformation Metadata ===
    /// Source objects used in transformation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transform_source_objects: Vec<String>,
    /// Transformation logic/expression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_logic: Option<String>,
    /// Human-readable transformation description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform_description: Option<String>,

    // === Examples & Defaults ===
    /// Example values for this property.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<serde_json::Value>,

    // === Relationships & References ===
    /// Property-level relationships.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<PropertyRelationship>,
    /// Authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<AuthoritativeDefinition>,

    // === Quality & Validation ===
    /// Quality rules and checks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality: Vec<QualityRule>,

    // === Tags & Custom Properties ===
    /// Property-level tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Custom properties for format-specific metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<CustomProperty>,

    // === Nested Properties (for OBJECT/ARRAY types) ===
    /// For ARRAY types: the item type definition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<Property>>,
    /// For OBJECT types: nested property definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<Property>,

    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Property {
    /// Create a new property with the given name and logical type.
    #[must_use]
    pub fn new(name: impl Into<String>, logical_type: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            logical_type: Some(logical_type.into()),
            ..Default::default()
        }
    }

    /// Create the anonymous element type that sits under an array's `items`.
    #[must_use]
    pub fn anonymous(logical_type: impl Into<String>) -> Self {
        Self {
            logical_type: Some(logical_type.into()),
            ..Default::default()
        }
    }

    /// Set the property as required.
    #[must_use]
    pub fn with_required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Set the property as a primary key.
    #[must_use]
    pub fn with_primary_key(mut self, primary_key: bool) -> Self {
        self.primary_key = primary_key;
        self
    }

    /// Set the primary key position.
    #[must_use]
    pub fn with_primary_key_position(mut self, position: i32) -> Self {
        self.primary_key_position = Some(position);
        self
    }

    /// Set the property description.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the business name.
    #[must_use]
    pub fn with_business_name(mut self, business_name: impl Into<String>) -> Self {
        self.business_name = Some(business_name.into());
        self
    }

    /// Set the physical type.
    #[must_use]
    pub fn with_physical_type(mut self, physical_type: impl Into<String>) -> Self {
        self.physical_type = Some(physical_type.into());
        self
    }

    /// Set the physical name.
    #[must_use]
    pub fn with_physical_name(mut self, physical_name: impl Into<String>) -> Self {
        self.physical_name = Some(physical_name.into());
        self
    }

    /// Set nested properties (for OBJECT types).
    #[must_use]
    pub fn with_nested_properties(mut self, properties: Vec<Property>) -> Self {
        self.properties = properties;
        self
    }

    /// Set items property (for ARRAY types).
    #[must_use]
    pub fn with_items(mut self, items: Property) -> Self {
        self.items = Some(Box::new(items));
        self
    }

    /// Add a custom property.
    #[must_use]
    pub fn with_custom_property(mut self, property: CustomProperty) -> Self {
        self.custom_properties.push(property);
        self
    }

    /// Add a tag.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set unique constraint.
    #[must_use]
    pub fn with_unique(mut self, unique: bool) -> Self {
        self.unique = unique;
        self
    }

    /// Set classification.
    #[must_use]
    pub fn with_classification(mut self, classification: impl Into<String>) -> Self {
        self.classification = Some(classification.into());
        self
    }

    /// The property name, or the empty string for an anonymous element type.
    #[must_use]
    pub fn name_or_empty(&self) -> &str {
        self.name.as_deref().unwrap_or("")
    }

    /// Check if this property has nested structure (OBJECT or ARRAY type).
    #[must_use]
    pub fn has_nested_structure(&self) -> bool {
        !self.properties.is_empty() || self.items.is_some()
    }

    /// Check if this is an object type.
    #[must_use]
    pub fn is_object(&self) -> bool {
        matches!(
            self.logical_type
                .as_deref()
                .map(str::to_lowercase)
                .as_deref(),
            Some("object" | "struct")
        ) || !self.properties.is_empty()
    }

    /// Check if this is an array type.
    #[must_use]
    pub fn is_array(&self) -> bool {
        matches!(
            self.logical_type
                .as_deref()
                .map(str::to_lowercase)
                .as_deref(),
            Some("array")
        ) || self.items.is_some()
    }

    /// Get all nested properties recursively, returning `(path, property)` pairs.
    ///
    /// Path uses dot notation for nested objects and `[]` for arrays.
    #[must_use]
    pub fn flatten_to_paths(&self) -> Vec<(String, &Property)> {
        let mut result = Vec::new();
        self.flatten_recursive(self.name_or_empty(), &mut result);
        result
    }

    fn flatten_recursive<'a>(
        &'a self,
        current_path: &str,
        result: &mut Vec<(String, &'a Property)>,
    ) {
        // Add current property
        result.push((current_path.to_string(), self));

        // Recurse into nested object properties
        for nested in &self.properties {
            let nested_path = if current_path.is_empty() {
                nested.name_or_empty().to_string()
            } else {
                format!("{current_path}.{}", nested.name_or_empty())
            };
            nested.flatten_recursive(&nested_path, result);
        }

        // Recurse into array items
        if let Some(items) = &self.items {
            let items_path = if current_path.is_empty() {
                "[]".to_string()
            } else {
                format!("{current_path}.[]")
            };
            items.flatten_recursive(&items_path, result);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_creation() {
        let prop = Property::new("id", "integer")
            .with_primary_key(true)
            .with_required(true)
            .with_description("Unique identifier");

        assert_eq!(prop.name.as_deref(), Some("id"));
        assert_eq!(prop.logical_type.as_deref(), Some("integer"));
        assert!(prop.primary_key);
        assert!(prop.required);
        assert_eq!(prop.description, Some("Unique identifier".to_string()));
    }

    #[test]
    fn nested_object_property() {
        let address = Property::new("address", "object").with_nested_properties(vec![
            Property::new("street", "string"),
            Property::new("city", "string"),
            Property::new("zip", "string"),
        ]);

        assert!(address.is_object());
        assert!(!address.is_array());
        assert!(address.has_nested_structure());
        assert_eq!(address.properties.len(), 3);
    }

    #[test]
    fn array_property() {
        let tags = Property::new("tags", "array").with_items(Property::anonymous("string"));

        assert!(tags.is_array());
        assert!(!tags.is_object());
        assert!(tags.has_nested_structure());
        assert!(tags.items.is_some());
    }

    #[test]
    fn flatten_to_paths_walks_objects() {
        let address = Property::new("address", "object").with_nested_properties(vec![
            Property::new("street", "string"),
            Property::new("city", "string"),
        ]);

        let paths = address.flatten_to_paths();
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0].0, "address");
        assert!(paths.iter().any(|(p, _)| p == "address.street"));
        assert!(paths.iter().any(|(p, _)| p == "address.city"));
    }

    #[test]
    fn flatten_to_paths_walks_arrays() {
        let items = Property::new("items", "array").with_items(
            Property::anonymous("object").with_nested_properties(vec![
                Property::new("name", "string"),
                Property::new("quantity", "integer"),
            ]),
        );

        let paths = items.flatten_to_paths();
        assert!(paths.iter().any(|(p, _)| p == "items"));
        assert!(paths.iter().any(|(p, _)| p == "items.[]"));
        assert!(paths.iter().any(|(p, _)| p == "items.[].name"));
        assert!(paths.iter().any(|(p, _)| p == "items.[].quantity"));
    }

    #[test]
    fn serializes_in_camel_case() {
        let prop = Property::new("name", "string")
            .with_required(true)
            .with_description("User name");

        let json = serde_json::to_string_pretty(&prop).unwrap();
        assert!(json.contains("\"name\": \"name\""));
        assert!(json.contains("\"logicalType\": \"string\""));
        assert!(json.contains("\"required\": true"));
        assert!(!json.contains("logical_type"));
    }

    #[test]
    fn deserializes_logical_type_options() {
        let json = r#"{
            "name": "email",
            "logicalType": "string",
            "required": true,
            "logicalTypeOptions": {
                "format": "email",
                "maxLength": 255
            }
        }"#;

        let prop: Property = serde_json::from_str(json).unwrap();
        assert_eq!(prop.name.as_deref(), Some("email"));
        assert_eq!(prop.logical_type.as_deref(), Some("string"));
        assert!(prop.required);
        let opts = prop.logical_type_options.unwrap();
        assert_eq!(opts.format, Some("email".to_string()));
        assert_eq!(opts.max_length, Some(255));
    }

    #[test]
    fn unknown_keys_survive_a_round_trip() {
        let json = r#"{"name":"id","logicalType":"string","x-vendor-flag":true}"#;
        let prop: Property = serde_json::from_str(json).unwrap();
        assert_eq!(
            prop.extra.get("x-vendor-flag"),
            Some(&serde_json::Value::Bool(true))
        );
        let back: serde_json::Value = serde_json::to_value(&prop).unwrap();
        assert_eq!(
            back,
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }
}

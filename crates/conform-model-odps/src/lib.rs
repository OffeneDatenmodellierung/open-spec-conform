//! Typed Rust model of the **Open Data Product Standard (ODPS) v1.0.0**.
//!
//! ODPS describes a data product: the contracts it consumes on its input
//! ports, the contracts it publishes on its output ports, how it is managed,
//! who owns it, and how to get help with it. The contracts themselves are
//! ODCS documents, modelled by `conform-model-odcs`; this crate models the
//! product that points at them.
//!
//! This crate is types and nothing else. It does not validate, does not
//! resolve a `contractId` into anything, and does not depend on
//! `conform-core` or on any adapter in this workspace.
//!
//! # Unknown fields are kept, never dropped
//!
//! Every struct ends in `#[serde(flatten)] extra: `[`Extra`], an
//! insertion-ordered map of every key the model does not name. A document that
//! goes in comes out again with every key it arrived with. Asserted by exact
//! equality of the parse trees over the whole conformant fixture corpus in
//! `tests/roundtrip.rs`.
//!
//! The single normalisation this model performs is that a collection written
//! explicitly empty — `tags: []` — is written back out absent. ODPS gives the
//! two the same meaning. It is tested, in `an_explicitly_empty_collection_is_written_back_absent`.
//!
//! The model refuses a document missing one of the four keys ODPS lists as
//! `required`: `apiVersion`, `kind`, `id`, `status`. Those four are the only
//! non-`Option` fields of [`ODPSDataProduct`].
//!
//! # Example
//!
//! ```rust
//! use conform_model_odps::ODPSDataProduct;
//!
//! let yaml = r#"
//! apiVersion: v1.0.0
//! kind: DataProduct
//! id: c9c1a5ee-7f70-4a1e-9f0f-0f0f1f2e3d4c
//! status: active
//! name: Order Analytics
//! outputPorts:
//!   - name: orders_gold
//!     version: 1.4.0
//!     contractId: 7a2e9c41-11f0-4c2a-9a6f-2f1d6e5b3c88
//! "#;
//!
//! let product: ODPSDataProduct = serde_norway::from_str(yaml).unwrap();
//! assert_eq!(product.output_ports[0].contract_id.as_deref(),
//!            Some("7a2e9c41-11f0-4c2a-9a6f-2f1d6e5b3c88"));
//! ```
//!
//! # Provenance
//!
//! Ported from `data-modelling-sdk/crates/core/src/models/odps.rs` at commit
//! `22c9c218`, under that repository's MIT licence, and corrected against the
//! ODPS v1.0.0 JSON Schema vendored in this repository at
//! `schemas/odps-json-schema-v1.0.0.json`. The corrections and the cuts are
//! listed in this crate's README.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Every key of an ODPS object that this model does not name.
///
/// See the crate documentation: nothing is dropped, and the order a document
/// carried its extensions in is the order they are written back out.
pub type Extra = IndexMap<String, serde_json::Value>;

/// Authoritative definition reference (ODPS v1.0.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ODPSAuthoritativeDefinition {
    /// Type of definition.
    #[serde(rename = "type")]
    pub definition_type: String,
    /// URL to the authority.
    pub url: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Custom property (ODPS v1.0.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ODPSCustomProperty {
    /// Property name.
    pub property: String,
    /// Property value; ODPS leaves the type open.
    pub value: serde_json::Value,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Product description (ODPS v1.0.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ODPSDescription {
    /// Intended purpose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    /// Limitations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limitations: Option<String>,
    /// Recommended usage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
    /// Authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<ODPSAuthoritativeDefinition>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<ODPSCustomProperty>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Input port — a contract this product consumes (ODPS v1.0.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ODPSInputPort {
    /// Port name.
    pub name: String,
    /// Port version.
    pub version: String,
    /// Contract ID, which resolves to an ODCS contract.
    pub contract_id: String,
    /// Tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<ODPSCustomProperty>,
    /// Authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<ODPSAuthoritativeDefinition>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Software Bill of Materials reference (ODPS v1.0.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ODPSSBOM {
    /// SBOM type.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub sbom_type: Option<String>,
    /// URL to the SBOM.
    pub url: String,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Input contract dependency of an output port (ODPS v1.0.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ODPSInputContract {
    /// Contract ID.
    pub id: String,
    /// Contract version.
    pub version: String,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Output port — a contract this product publishes (ODPS v1.0.0).
///
/// `contractId` is optional here and required on an input port: a product may
/// publish a port before the contract describing it exists, but it cannot
/// consume one that does not.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ODPSOutputPort {
    /// Port name.
    pub name: String,
    /// Port version.
    pub version: String,
    /// Port description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Port type (e.g. `tables`, `topic`, `files`).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub port_type: Option<String>,
    /// Contract ID, which resolves to an ODCS contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract_id: Option<String>,
    /// Software bills of materials for what backs this port.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sbom: Vec<ODPSSBOM>,
    /// Input contracts this port is derived from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_contracts: Vec<ODPSInputContract>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<ODPSCustomProperty>,
    /// Authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<ODPSAuthoritativeDefinition>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Management port — how the product is discovered, observed or controlled.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ODPSManagementPort {
    /// Port name.
    pub name: String,
    /// What the port carries (e.g. `discoverability`, `observability`, `control`).
    pub content: String,
    /// Port type (`rest` or `topic`).
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub port_type: Option<String>,
    /// URL of the access endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Channel name, for a topic-backed port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<ODPSCustomProperty>,
    /// Authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<ODPSAuthoritativeDefinition>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Support channel (ODPS v1.0.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ODPSSupport {
    /// Channel name.
    pub channel: String,
    /// Access URL.
    pub url: String,
    /// Description.
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
    /// Tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<ODPSCustomProperty>,
    /// Authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<ODPSAuthoritativeDefinition>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Team member (ODPS v1.0.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ODPSTeamMember {
    /// Username or email; the one field ODPS requires of a member.
    pub username: String,
    /// Member name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Description of the member's involvement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Date joined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_in: Option<String>,
    /// Date left.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_out: Option<String>,
    /// Username of whoever replaced this member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced_by_username: Option<String>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<ODPSCustomProperty>,
    /// Authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<ODPSAuthoritativeDefinition>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Team owning the product (ODPS v1.0.0).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ODPSTeam {
    /// Team name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Team description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Team members.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<ODPSTeamMember>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<ODPSCustomProperty>,
    /// Authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<ODPSAuthoritativeDefinition>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Data product — the root ODPS document (ODPS v1.0.0).
///
/// ODPS requires four keys of a product — `apiVersion`, `kind`, `id` and
/// `status` — and those four are the only non-`Option` fields here. Neither
/// `name` nor `version` is required: the smallest document the published
/// schema accepts has neither.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ODPSDataProduct {
    /// API version (e.g. `v1.0.0`).
    pub api_version: String,
    /// Kind identifier (always `DataProduct`).
    pub kind: String,
    /// Unique identifier.
    pub id: String,
    /// Status: `proposed`, `draft`, `active`, `deprecated`, `retired`.
    ///
    /// A `String` and not a closed enum. ODPS publishes the conventional
    /// values as `examples`, not as an `enum`, so `status: mothballed` is a
    /// conformant ODPS document and this model reads it. Judging it is a
    /// validator's job.
    pub status: String,

    /// Product name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Product version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Business domain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Tenant/organization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,

    /// Authoritative definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authoritative_definitions: Vec<ODPSAuthoritativeDefinition>,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<ODPSDescription>,
    /// Custom properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_properties: Vec<ODPSCustomProperty>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Input ports: the contracts this product consumes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_ports: Vec<ODPSInputPort>,
    /// Output ports: the contracts this product publishes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_ports: Vec<ODPSOutputPort>,
    /// Management ports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub management_ports: Vec<ODPSManagementPort>,
    /// Support channels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub support: Vec<ODPSSupport>,
    /// Team.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<ODPSTeam>,
    /// Product creation timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub product_created_ts: Option<String>,

    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for ODPSDataProduct {
    fn default() -> Self {
        Self {
            api_version: "v1.0.0".to_string(),
            kind: "DataProduct".to_string(),
            id: String::new(),
            status: "draft".to_string(),
            name: None,
            version: None,
            domain: None,
            tenant: None,
            authoritative_definitions: Vec::new(),
            description: None,
            custom_properties: Vec::new(),
            tags: Vec::new(),
            input_ports: Vec::new(),
            output_ports: Vec::new(),
            management_ports: Vec::new(),
            support: Vec::new(),
            team: None,
            product_created_ts: None,
            extra: Extra::new(),
        }
    }
}

impl ODPSDataProduct {
    /// Create a new data product with the given id and name.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: Some(name.into()),
            ..Default::default()
        }
    }

    /// Set the status.
    #[must_use]
    pub fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    /// Set the product version.
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set the domain.
    #[must_use]
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Add an input port.
    #[must_use]
    pub fn with_input_port(mut self, port: ODPSInputPort) -> Self {
        self.input_ports.push(port);
        self
    }

    /// Add an output port.
    #[must_use]
    pub fn with_output_port(mut self, port: ODPSOutputPort) -> Self {
        self.output_ports.push(port);
        self
    }

    /// Every contract id this product references, input and output ports alike.
    ///
    /// This is the question an ODPS document is usually read to answer: which
    /// ODCS contracts does this product depend on or publish? Output ports may
    /// name a contract directly and may also derive from input contracts, so
    /// both are included, in document order and without deduplication.
    #[must_use]
    pub fn contract_ids(&self) -> Vec<&str> {
        let inputs = self.input_ports.iter().map(|p| p.contract_id.as_str());
        let outputs = self.output_ports.iter().flat_map(|p| {
            p.contract_id
                .as_deref()
                .into_iter()
                .chain(p.input_contracts.iter().map(|c| c.id.as_str()))
        });
        inputs.chain(outputs).collect()
    }

    /// Get an output port by name.
    #[must_use]
    pub fn output_port(&self, name: &str) -> Option<&ODPSOutputPort> {
        self.output_ports.iter().find(|p| p.name == name)
    }

    /// Get an input port by name.
    #[must_use]
    pub fn input_port(&self, name: &str) -> Option<&ODPSInputPort> {
        self.input_ports.iter().find(|p| p.name == name)
    }
}

#[cfg(feature = "yaml")]
impl ODPSDataProduct {
    /// Parse a data product from a YAML (or JSON — YAML is a superset) document.
    ///
    /// # Errors
    ///
    /// Returns the underlying `serde_norway` error if the document is not
    /// well-formed YAML, or is missing one of the four keys ODPS requires.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_norway::Error> {
        serde_norway::from_str(yaml)
    }

    /// Render this data product as a YAML document.
    ///
    /// # Errors
    ///
    /// Returns the underlying `serde_norway` error if the product cannot be
    /// represented as YAML.
    pub fn to_yaml(&self) -> Result<String, serde_norway::Error> {
        serde_norway::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_required_keys_are_enough() {
        let json = r#"{"apiVersion":"v1.0.0","kind":"DataProduct",
                       "id":"3f9b8b41-0f2c-4f75-a15e-f6b1f2c6a4d9","status":"active"}"#;
        let product: ODPSDataProduct = serde_json::from_str(json).unwrap();
        assert!(product.name.is_none());
        assert!(product.version.is_none());
        assert!(product.output_ports.is_empty());
    }

    #[test]
    fn a_product_missing_a_required_key_is_refused() {
        let json = r#"{"apiVersion":"v1.0.0","kind":"DataProduct","status":"active"}"#;
        let err = serde_json::from_str::<ODPSDataProduct>(json).unwrap_err();
        assert!(err.to_string().contains("id"), "unexpected error: {err}");
    }

    #[test]
    fn an_unconventional_status_is_read_rather_than_rejected() {
        let json = r#"{"apiVersion":"v1.0.0","kind":"DataProduct","id":"x","status":"mothballed"}"#;
        let product: ODPSDataProduct = serde_json::from_str(json).unwrap();
        assert_eq!(product.status, "mothballed");
    }

    #[test]
    fn tags_are_strings_and_keep_their_exact_spelling() {
        // The upstream model parsed tags into a Simple/Pair/List enum whose
        // Display re-rendered them, so `A:[x,  y]` came back as `A:[x, y]`.
        // ODPS publishes tags as an array of strings; this model keeps them.
        let json = r#"{"apiVersion":"v1.0.0","kind":"DataProduct","id":"x","status":"active",
                       "tags":["sales","SecondaryDomains:[XXXXX,  PPPP]"]}"#;
        let product: ODPSDataProduct = serde_json::from_str(json).unwrap();
        assert_eq!(product.tags[1], "SecondaryDomains:[XXXXX,  PPPP]");
        let back: serde_json::Value = serde_json::to_value(&product).unwrap();
        assert_eq!(
            back,
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }

    #[test]
    fn contract_ids_collects_both_port_kinds() {
        let json = r#"{"apiVersion":"v1.0.0","kind":"DataProduct","id":"x","status":"active",
            "inputPorts":[{"name":"raw","version":"1","contractId":"in-1"}],
            "outputPorts":[{"name":"gold","version":"1","contractId":"out-1",
                            "inputContracts":[{"id":"in-1","version":"1"}]}]}"#;
        let product: ODPSDataProduct = serde_json::from_str(json).unwrap();
        assert_eq!(product.contract_ids(), vec!["in-1", "out-1", "in-1"]);
    }

    #[test]
    fn unknown_keys_survive_a_round_trip() {
        let json = r#"{"apiVersion":"v1.0.0","kind":"DataProduct","id":"x","status":"active",
                       "x-lineage":{"upstream":["warehouse"]}}"#;
        let product: ODPSDataProduct = serde_json::from_str(json).unwrap();
        assert!(product.extra.contains_key("x-lineage"));
        let back: serde_json::Value = serde_json::to_value(&product).unwrap();
        assert_eq!(
            back,
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }
}

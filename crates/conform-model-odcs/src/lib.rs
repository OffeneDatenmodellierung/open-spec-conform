//! Typed Rust model of the **Open Data Contract Standard (ODCS) v3.1.0**.
//!
//! This crate is types and nothing else. It does not validate, does not fetch,
//! does not depend on `conform-core` or on any adapter in this workspace, and
//! has no opinion about whether the document it just read is any good. A
//! consumer who wants `contract.servers[0].host` takes this crate and pays for
//! `serde`, `serde_json` and `indexmap`.
//!
//! The model preserves the three-level hierarchy ODCS describes:
//!
//! 1. **Contract level** ([`ODCSContract`]) — root document with metadata
//! 2. **Schema level** ([`SchemaObject`]) — tables/views/topics within a contract
//! 3. **Property level** ([`Property`]) — columns/fields within a schema
//!
//! # Unknown fields are kept, never dropped
//!
//! Every struct in this crate ends in `#[serde(flatten)] extra: `[`Extra`],
//! an insertion-ordered map of every key the model does not name. A document
//! that goes in comes out again unchanged — vendor extensions, keys added by a
//! later ODCS revision, and typos alike. This is the property the crate exists
//! to provide: a model that silently dropped what it did not recognise would
//! turn a read-modify-write into data loss, which is worse than having no
//! model at all. It is tested against the whole ODCS fixture corpus in
//! `tests/roundtrip.rs`, by exact equality rather than by spot check.
//!
//! The one thing this model *does* refuse is a document missing one of the
//! five keys ODCS lists as `required` — `version`, `apiVersion`, `kind`, `id`,
//! `status`. Those are the five non-`Option` fields of [`ODCSContract`].
//!
//! # Example
//!
//! ```rust
//! use conform_model_odcs::{ODCSContract, SchemaObject, Property};
//!
//! // Create a contract with two tables
//! let contract = ODCSContract::new("ecommerce", "1.0.0")
//!     .with_id("4b1f0f0e-2bb8-4a9d-9c17-4a4cbe3f2f2a")
//!     .with_domain("retail")
//!     .with_status("active")
//!     .with_schema(
//!         SchemaObject::new("orders")
//!             .with_physical_type("table")
//!             .with_properties(vec![
//!                 Property::new("id", "integer").with_primary_key(true),
//!                 Property::new("customer_id", "integer").with_required(true),
//!                 Property::new("total", "number"),
//!             ])
//!     )
//!     .with_schema(
//!         SchemaObject::new("order_items")
//!             .with_physical_type("table")
//!             .with_properties(vec![
//!                 Property::new("id", "integer").with_primary_key(true),
//!                 Property::new("order_id", "integer").with_required(true),
//!                 Property::new("product_name", "string"),
//!             ])
//!     );
//!
//! assert_eq!(contract.schema_count(), 2);
//! ```
//!
//! # Provenance
//!
//! Ported from `data-modelling-sdk/crates/core/src/models/odcs/` at commit
//! `22c9c218`, under that repository's MIT licence, and corrected against the
//! ODCS v3.1.0 JSON Schema vendored in this repository at
//! `schemas/odcs-json-schema-v3.1.0.json`. The corrections, and the parts of
//! the original deliberately left behind, are listed in this crate's README.

pub mod contract;
pub mod property;
pub mod schema;
pub mod supporting;

pub use contract::ODCSContract;
pub use property::Property;
pub use schema::SchemaObject;

pub use supporting::{
    AuthoritativeDefinition, CustomProperty, Description, Extra, LogicalTypeOptions, Pricing,
    PropertyRelationship, QualityRule, RelationshipEnd, Role, SchemaRelationship, Server,
    ServiceLevelAgreementProperty, StructuredDescription, SupportItem, Team, TeamMember, TeamRef,
};

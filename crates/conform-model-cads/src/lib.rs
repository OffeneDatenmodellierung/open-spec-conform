//! Typed Rust model of the **Compute Asset Description Specification (CADS) v1.0**.
//!
//! CADS describes a compute asset — an AI model, an ML or data pipeline, an
//! application, a source or destination system — alongside the runtime it
//! needs, the risk it carries, the frameworks it is assessed against, and the
//! process and API models that describe its behaviour.
//!
//! This crate is types and nothing else. It does not validate and does not
//! depend on `conform-core` or on any adapter in this workspace.
//!
//! # Closed sets, held open
//!
//! Unlike ODCS and ODPS, CADS publishes real `enum`s: `kind`, `status`,
//! `pricing.model`, `risk.classification`, `risk.impactAreas`, the two
//! statuses under `risk.mitigations` and `compliance.frameworks`, and the
//! three `format` fields. Those are modelled as Rust enums, so the conventional
//! values are named and matchable.
//!
//! Every one of them also carries an `Other(String)` variant. A value outside
//! the published set is a *validation* failure, and this crate is not a
//! validator: refusing to deserialize would mean a caller could not read,
//! inspect or re-emit the document at all, and the round-trip guarantee below
//! would have a hole in it exactly where a document is unusual. The value is
//! kept verbatim and written back verbatim.
//!
//! # Unknown fields are kept, never dropped
//!
//! Every struct ends in `#[serde(flatten)] extra: `[`Extra`], an
//! insertion-ordered map of every key the model does not name. Asserted by
//! exact equality of the parse trees in `tests/roundtrip.rs`.
//!
//! The single normalisation this model performs is that a collection written
//! explicitly empty — `tags: []` — is written back out absent. CADS gives the
//! two the same meaning. It is tested, in
//! `an_explicitly_empty_collection_is_written_back_absent`.
//!
//! The model refuses a document missing one of the six keys CADS lists as
//! `required`: `apiVersion`, `kind`, `id`, `name`, `version`, `status`.
//!
//! # Example
//!
//! ```rust
//! use conform_model_cads::{CADSAsset, CADSKind, CADSStatus};
//!
//! let yaml = r#"
//! apiVersion: v1.0
//! kind: AIModel
//! id: 550e8400-e29b-41d4-a716-446655440000
//! name: sentiment-analysis-model
//! version: 1.0.0
//! status: production
//! risk:
//!   classification: medium
//!   impactAreas: [fairness, privacy]
//! "#;
//!
//! let asset: CADSAsset = serde_norway::from_str(yaml).unwrap();
//! assert_eq!(asset.kind, CADSKind::AIModel);
//! assert_eq!(asset.status, CADSStatus::Production);
//! ```
//!
//! # Provenance
//!
//! Ported from `data-modelling-sdk/crates/core/src/models/cads.rs` at commit
//! `22c9c218`, under that repository's MIT licence, and corrected against
//! `schemas/cads.schema.json` as vendored and pinned here by `specs.toml`.
//! The corrections and the cuts are listed in this crate's README.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Every key of a CADS object that this model does not name.
pub type Extra = IndexMap<String, serde_json::Value>;

/// A free-form map, as CADS publishes `customProperties`: an object with
/// `additionalProperties: true` and no further structure.
pub type PropertyMap = IndexMap<String, serde_json::Value>;

/// CADS asset kinds.
///
/// `Other` holds a `kind` outside the published set, so an unrecognised asset
/// is readable rather than unreadable. See the crate documentation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CADSKind {
    /// A trained model served for inference.
    AIModel,
    /// A machine-learning training or serving pipeline.
    MLPipeline,
    /// A deployed application.
    Application,
    /// A data pipeline.
    DataPipeline,
    /// An ETL process.
    ETLProcess,
    /// An ETL pipeline.
    ETLPipeline,
    /// A system data originates from.
    SourceSystem,
    /// A system data is delivered to.
    DestinationSystem,
    /// A kind outside the published set, kept verbatim.
    #[serde(untagged)]
    Other(String),
}

/// CADS asset status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CADSStatus {
    /// Not yet assessed.
    Draft,
    /// Assessed and signed off, not yet serving.
    Validated,
    /// Serving.
    Production,
    /// Serving, but on its way out.
    Deprecated,
    /// A status outside the published set, kept verbatim.
    #[serde(untagged)]
    Other(String),
}

/// External link in a CADS description.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CADSExternalLink {
    /// URL of the external link.
    pub url: String,
    /// Description of the link.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// CADS description object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CADSDescription {
    /// Purpose of the asset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    /// Usage instructions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
    /// Limitations and constraints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limitations: Option<String>,
    /// External links and references.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_links: Vec<CADSExternalLink>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Container configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CADSRuntimeContainer {
    /// Container image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Resource requirements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CADSRuntimeResources {
    /// CPU requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,
    /// Memory requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    /// GPU requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// CADS runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CADSRuntime {
    /// Runtime environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Service endpoints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub endpoints: Vec<String>,
    /// Container configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<CADSRuntimeContainer>,
    /// Resource requirements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<CADSRuntimeResources>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// One service level property.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CADSSLAProperty {
    /// SLA element name.
    pub element: String,
    /// Value (number or string).
    pub value: serde_json::Value,
    /// Unit of measurement.
    pub unit: String,
    /// Driver (e.g. `operational`, `compliance`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// CADS service level agreement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CADSSLA {
    /// SLA properties.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub properties: Vec<CADSSLAProperty>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Pricing model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CADSPricingModel {
    /// Charged per request.
    PerRequest,
    /// Charged per hour of runtime.
    PerHour,
    /// Charged per batch run.
    PerBatch,
    /// Charged as a subscription.
    Subscription,
    /// Not charged for; an internal asset.
    Internal,
    /// A pricing model outside the published set, kept verbatim.
    #[serde(untagged)]
    Other(String),
}

/// CADS pricing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CADSPricing {
    /// Pricing model type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<CADSPricingModel>,
    /// Currency code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Unit cost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_cost: Option<serde_json::Value>,
    /// Billing unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub billing_unit: Option<String>,
    /// Additional notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Team member.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CADSTeamMember {
    /// Role of the team member.
    pub role: String,
    /// Name of the team member.
    pub name: String,
    /// Contact information.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Risk classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CADSRiskClassification {
    /// Minimal risk.
    Minimal,
    /// Low risk.
    Low,
    /// Medium risk.
    Medium,
    /// High risk.
    High,
    /// A classification outside the published set, kept verbatim.
    #[serde(untagged)]
    Other(String),
}

/// Impact area.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CADSImpactArea {
    /// Fairness and bias.
    Fairness,
    /// Privacy.
    Privacy,
    /// Physical or psychological safety.
    Safety,
    /// Security.
    Security,
    /// Financial impact.
    Financial,
    /// Operational impact.
    Operational,
    /// Reputational impact.
    Reputational,
    /// An impact area outside the published set, kept verbatim.
    #[serde(untagged)]
    Other(String),
}

/// Risk assessment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CADSRiskAssessment {
    /// Assessment methodology.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub methodology: Option<String>,
    /// Assessment date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Assessor name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessor: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Mitigation status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CADSMitigationStatus {
    /// Planned but not started.
    Planned,
    /// Implemented but not verified.
    Implemented,
    /// Implemented and verified.
    Verified,
    /// A status outside the published set, kept verbatim.
    #[serde(untagged)]
    Other(String),
}

/// Risk mitigation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CADSRiskMitigation {
    /// Mitigation description.
    pub description: String,
    /// Mitigation status.
    pub status: CADSMitigationStatus,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// CADS risk management.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CADSRisk {
    /// Risk classification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification: Option<CADSRiskClassification>,
    /// Impact areas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub impact_areas: Vec<CADSImpactArea>,
    /// Intended use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intended_use: Option<String>,
    /// Out-of-scope use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_of_scope_use: Option<String>,
    /// Risk assessment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment: Option<CADSRiskAssessment>,
    /// Risk mitigations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mitigations: Vec<CADSRiskMitigation>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Compliance status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CADSComplianceStatus {
    /// The framework does not apply to this asset.
    NotApplicable,
    /// Assessed, verdict not recorded here.
    Assessed,
    /// Compliant.
    Compliant,
    /// Not compliant.
    NonCompliant,
    /// A status outside the published set, kept verbatim.
    #[serde(untagged)]
    Other(String),
}

/// Compliance framework.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CADSComplianceFramework {
    /// Framework name.
    pub name: String,
    /// Framework category.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Compliance status.
    pub status: CADSComplianceStatus,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Compliance control.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CADSComplianceControl {
    /// Control ID.
    pub id: String,
    /// Control description.
    pub description: String,
    /// Evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// CADS compliance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CADSCompliance {
    /// Compliance frameworks.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frameworks: Vec<CADSComplianceFramework>,
    /// Compliance controls.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub controls: Vec<CADSComplianceControl>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// What a validation profile applies to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CADSValidationProfileAppliesTo {
    /// Asset kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Risk classification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_classification: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Validation profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CADSValidationProfile {
    /// Profile name.
    pub name: String,
    /// Applies-to criteria.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applies_to: Option<CADSValidationProfileAppliesTo>,
    /// Required checks.
    pub required_checks: Vec<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// BPMN model format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CADSBPMNFormat {
    /// BPMN 2.0 XML.
    #[serde(rename = "bpmn20-xml")]
    Bpmn20Xml,
    /// JSON.
    #[serde(rename = "json")]
    Json,
    /// A format outside the published set, kept verbatim.
    #[serde(untagged)]
    Other(String),
}

/// BPMN model reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CADSBPMNModel {
    /// Model name.
    pub name: String,
    /// Reference to the BPMN model.
    pub reference: String,
    /// Format.
    pub format: CADSBPMNFormat,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// DMN model format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CADSDMNFormat {
    /// DMN 1.3 XML.
    #[serde(rename = "dmn13-xml")]
    Dmn13Xml,
    /// JSON.
    #[serde(rename = "json")]
    Json,
    /// A format outside the published set, kept verbatim.
    #[serde(untagged)]
    Other(String),
}

/// DMN model reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CADSDMNModel {
    /// Model name.
    pub name: String,
    /// Reference to the DMN model.
    pub reference: String,
    /// Format.
    pub format: CADSDMNFormat,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// `OpenAPI` specification format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CADSOpenAPIFormat {
    /// `OpenAPI` 3.0.
    #[serde(rename = "openapi-3.0")]
    Openapi30,
    /// `OpenAPI` 3.1.
    #[serde(rename = "openapi-3.1")]
    Openapi31,
    /// Swagger 2.0.
    #[serde(rename = "swagger-2.0")]
    Swagger20,
    /// A format outside the published set, kept verbatim.
    #[serde(untagged)]
    Other(String),
}

/// `OpenAPI` specification reference.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CADSOpenAPISpec {
    /// Spec name.
    pub name: String,
    /// Reference to the `OpenAPI` specification.
    pub reference: String,
    /// Format.
    pub format: CADSOpenAPIFormat,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// CADS asset — the root document (CADS v1.0).
///
/// CADS requires six keys of an asset — `apiVersion`, `kind`, `id`, `name`,
/// `version` and `status` — and those six are the only non-`Option` fields
/// here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CADSAsset {
    /// API version.
    pub api_version: String,
    /// Asset kind.
    pub kind: CADSKind,
    /// Unique identifier (UUID or URN).
    pub id: String,
    /// Asset name.
    pub name: String,
    /// Version.
    pub version: String,
    /// Status.
    pub status: CADSStatus,

    /// Domain name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Domain reference. CADS publishes this as a plain string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_id: Option<String>,
    /// Tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<CADSDescription>,
    /// Runtime configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<CADSRuntime>,
    /// Service level agreement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sla: Option<CADSSLA>,
    /// Pricing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pricing: Option<CADSPricing>,
    /// Team. CADS publishes this as a bare array of members.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub team: Vec<CADSTeamMember>,
    /// Risk management.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<CADSRisk>,
    /// Compliance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compliance: Option<CADSCompliance>,
    /// Validation profiles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validation_profiles: Vec<CADSValidationProfile>,
    /// BPMN models describing this asset's process.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bpmn_models: Vec<CADSBPMNModel>,
    /// DMN models describing this asset's decisions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dmn_models: Vec<CADSDMNModel>,
    /// `OpenAPI` specifications describing this asset's interface.
    ///
    /// CADS spells this key `openApiSpecs`, with a capital `A`.
    #[serde(
        rename = "openApiSpecs",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub open_api_specs: Vec<CADSOpenAPISpec>,
    /// Custom properties. CADS leaves the shape entirely open.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub custom_properties: PropertyMap,
    /// Creation timestamp, kept as written rather than reformatted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Last update timestamp, kept as written rather than reformatted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,

    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

impl CADSAsset {
    /// Create a new asset with the six keys CADS requires.
    #[must_use]
    pub fn new(
        kind: CADSKind,
        id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
        status: CADSStatus,
    ) -> Self {
        Self {
            api_version: "v1.0".to_string(),
            kind,
            id: id.into(),
            name: name.into(),
            version: version.into(),
            status,
            domain: None,
            domain_id: None,
            tags: Vec::new(),
            description: None,
            runtime: None,
            sla: None,
            pricing: None,
            team: Vec::new(),
            risk: None,
            compliance: None,
            validation_profiles: Vec::new(),
            bpmn_models: Vec::new(),
            dmn_models: Vec::new(),
            open_api_specs: Vec::new(),
            custom_properties: PropertyMap::new(),
            created_at: None,
            updated_at: None,
            extra: Extra::new(),
        }
    }

    /// Set the domain.
    #[must_use]
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Add a tag.
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set the risk block.
    #[must_use]
    pub fn with_risk(mut self, risk: CADSRisk) -> Self {
        self.risk = Some(risk);
        self
    }

    /// Whether this asset declares a risk classification of `high`.
    ///
    /// The question a governance reader opens a CADS document to ask.
    #[must_use]
    pub fn is_high_risk(&self) -> bool {
        self.risk.as_ref().and_then(|r| r.classification.as_ref())
            == Some(&CADSRiskClassification::High)
    }

    /// Every compliance framework this asset is not compliant with.
    #[must_use]
    pub fn non_compliant_frameworks(&self) -> Vec<&CADSComplianceFramework> {
        self.compliance
            .iter()
            .flat_map(|c| &c.frameworks)
            .filter(|f| f.status == CADSComplianceStatus::NonCompliant)
            .collect()
    }
}

#[cfg(feature = "yaml")]
impl CADSAsset {
    /// Parse an asset from a YAML (or JSON — YAML is a superset) document.
    ///
    /// # Errors
    ///
    /// Returns the underlying `serde_norway` error if the document is not
    /// well-formed YAML, or is missing one of the six keys CADS requires.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_norway::Error> {
        serde_norway::from_str(yaml)
    }

    /// Render this asset as a YAML document.
    ///
    /// # Errors
    ///
    /// Returns the underlying `serde_norway` error if the asset cannot be
    /// represented as YAML.
    pub fn to_yaml(&self) -> Result<String, serde_norway::Error> {
        serde_norway::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"{"apiVersion":"v1.0","kind":"AIModel","id":"x",
                              "name":"m","version":"1.0.0","status":"production"}"#;

    #[test]
    fn the_six_required_keys_are_enough() {
        let asset: CADSAsset = serde_json::from_str(MINIMAL).unwrap();
        assert_eq!(asset.kind, CADSKind::AIModel);
        assert_eq!(asset.status, CADSStatus::Production);
        assert!(asset.tags.is_empty());
    }

    #[test]
    fn an_asset_missing_a_required_key_is_refused() {
        let json =
            r#"{"apiVersion":"v1.0","kind":"AIModel","id":"x","version":"1.0.0","status":"draft"}"#;
        let err = serde_json::from_str::<CADSAsset>(json).unwrap_err();
        assert!(err.to_string().contains("name"), "unexpected error: {err}");
    }

    #[test]
    fn a_value_outside_a_published_enum_is_kept_rather_than_refused() {
        let json = r#"{"apiVersion":"v1.0","kind":"QuantumThing","id":"x","name":"m",
                       "version":"1.0.0","status":"mothballed"}"#;
        let asset: CADSAsset = serde_json::from_str(json).unwrap();
        assert_eq!(asset.kind, CADSKind::Other("QuantumThing".into()));
        assert_eq!(asset.status, CADSStatus::Other("mothballed".into()));

        // And it is written back exactly as it arrived, not as `Other`.
        let back: serde_json::Value = serde_json::to_value(&asset).unwrap();
        assert_eq!(
            back,
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }

    #[test]
    fn open_api_specs_uses_the_published_spelling() {
        let json = r#"{"apiVersion":"v1.0","kind":"AIModel","id":"x","name":"m","version":"1.0.0",
                       "status":"draft","openApiSpecs":[{"name":"api","reference":"./api.yaml",
                       "format":"openapi-3.1"}]}"#;
        let asset: CADSAsset = serde_json::from_str(json).unwrap();
        assert_eq!(asset.open_api_specs.len(), 1);
        assert_eq!(asset.open_api_specs[0].format, CADSOpenAPIFormat::Openapi31);

        let back = serde_json::to_string(&asset).unwrap();
        assert!(back.contains("openApiSpecs"));
        assert!(!back.contains("openapiSpecs"));
    }

    #[test]
    fn risk_and_compliance_are_queryable() {
        let json = r#"{"apiVersion":"v1.0","kind":"AIModel","id":"x","name":"m","version":"1.0.0",
            "status":"production",
            "risk":{"classification":"high","impactAreas":["fairness","privacy"]},
            "compliance":{"frameworks":[
                {"name":"EU AI Act","status":"non_compliant"},
                {"name":"ISO 42001","status":"compliant"}]}}"#;
        let asset: CADSAsset = serde_json::from_str(json).unwrap();
        assert!(asset.is_high_risk());
        assert_eq!(
            asset.risk.as_ref().unwrap().impact_areas,
            vec![CADSImpactArea::Fairness, CADSImpactArea::Privacy]
        );
        let bad = asset.non_compliant_frameworks();
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].name, "EU AI Act");
    }

    #[test]
    fn unknown_keys_survive_a_round_trip() {
        let json = r#"{"apiVersion":"v1.0","kind":"AIModel","id":"x","name":"m","version":"1.0.0",
                       "status":"draft","x-model-card":"https://example.com/card"}"#;
        let asset: CADSAsset = serde_json::from_str(json).unwrap();
        assert!(asset.extra.contains_key("x-model-card"));
        let back: serde_json::Value = serde_json::to_value(&asset).unwrap();
        assert_eq!(
            back,
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }

    #[test]
    fn an_explicitly_empty_collection_is_written_back_absent() {
        let json = r#"{"apiVersion":"v1.0","kind":"AIModel","id":"x","name":"m","version":"1.0.0",
                       "status":"draft","tags":[]}"#;
        let asset: CADSAsset = serde_json::from_str(json).unwrap();
        let back = serde_json::to_string(&asset).unwrap();
        assert!(!back.contains("tags"), "got {back}");
    }
}

//! Typed Rust model of the **Databricks Metric Views (DBMV)** document format.
//!
//! A metric view is a semantic layer over a raw table: it names the
//! dimensions you may group by, the measures you may aggregate, the joins that
//! widen the source, and how the whole thing is materialised. DBMV documents
//! use the `.dbmv.yaml` extension and carry one or more metric views.
//!
//! # Two casings in one document, on purpose
//!
//! The envelope keys are **camelCase** — `apiVersion`, `kind`, `metricViews` —
//! because that is the convention every other document format in this family
//! uses. Everything inside a metric view is **`snake_case`** — `display_name`,
//! `materialized_views` — because that is Databricks' own spelling, and
//! rewriting it would mean the inner content no longer matched what Databricks
//! accepts. The split is deliberate and load-bearing; `two_casings_in_one_document`
//! pins it.
//!
//! # Unknown fields are kept, never dropped
//!
//! Every struct ends in `#[serde(flatten)] extra: `[`Extra`], an
//! insertion-ordered map of every key the model does not name. This matters
//! more here than anywhere else in this family: the inner content is
//! Databricks', it gains keys on Databricks' schedule, and a model that
//! dropped what it had not heard of would quietly delete a `measures[].format`
//! variant the day Databricks added one.
//!
//! The single normalisation this model performs is that a collection written
//! explicitly empty — `dimensions: []` — is written back out absent.
//!
//! # No vendored schema, and no pretending otherwise
//!
//! `conform-model-odcs`, `-odps` and `-cads` were each corrected against a
//! JSON Schema vendored and pinned in this repository. There is **no vendored
//! DBMV schema** — `specs.toml` has no `dbmv` entry, because Databricks
//! publishes the format as prose rather than as a schema. This model is
//! therefore the upstream author's reading of that prose, carried across
//! faithfully but *not* independently verified against Databricks'
//! documentation. That is a weaker footing than the other three crates have
//! and it is said here rather than left to be assumed.
//!
//! # Example
//!
//! ```rust
//! use conform_model_dbmv::DBMVDocument;
//!
//! let yaml = r#"
//! apiVersion: v1.0.0
//! kind: MetricViews
//! system: my-databricks-system
//! metricViews:
//!   - name: orders_metrics
//!     source: catalog.schema.orders
//!     dimensions:
//!       - name: order_date
//!         expr: order_date
//!     measures:
//!       - name: total_revenue
//!         expr: SUM(revenue)
//! "#;
//!
//! let doc: DBMVDocument = serde_norway::from_str(yaml).unwrap();
//! let view = doc.get_metric_view("orders_metrics").unwrap();
//! assert_eq!(view.source, "catalog.schema.orders");
//! assert_eq!(view.measures[0].expr, "SUM(revenue)");
//! ```
//!
//! # Provenance
//!
//! Ported from `data-modelling-sdk/crates/core/src/models/dbmv.rs` at commit
//! `22c9c218`, under that repository's MIT licence. See this crate's README.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Every key of a DBMV object that this model does not name.
pub type Extra = IndexMap<String, serde_json::Value>;

/// Default version for metric views.
fn default_version() -> String {
    "1.1".to_string()
}

/// Default API version.
fn default_api_version() -> String {
    "v1.0.0".to_string()
}

/// Default kind.
fn default_kind() -> String {
    "MetricViews".to_string()
}

/// DBMV document — the envelope around one system's metric views.
///
/// Envelope fields are camelCase. One document per system, containing several
/// metric view definitions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DBMVDocument {
    /// API version of the DBMV format (e.g. `v1.0.0`).
    #[serde(default = "default_api_version")]
    pub api_version: String,
    /// Document kind — always `MetricViews`.
    #[serde(default = "default_kind")]
    pub kind: String,
    /// System name this document belongs to.
    pub system: String,
    /// Optional description of the metric views collection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Metric view definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metric_views: Vec<DBMVMetricView>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for DBMVDocument {
    fn default() -> Self {
        Self {
            api_version: default_api_version(),
            kind: default_kind(),
            system: String::new(),
            description: None,
            metric_views: Vec::new(),
            extra: Extra::new(),
        }
    }
}

impl DBMVDocument {
    /// Create a new DBMV document for a system.
    #[must_use]
    pub fn new(system: impl Into<String>) -> Self {
        Self {
            system: system.into(),
            ..Default::default()
        }
    }

    /// Add a metric view to the document.
    pub fn add_metric_view(&mut self, view: DBMVMetricView) {
        self.metric_views.push(view);
    }

    /// Get a metric view by name.
    #[must_use]
    pub fn get_metric_view(&self, name: &str) -> Option<&DBMVMetricView> {
        self.metric_views.iter().find(|v| v.name == name)
    }

    /// Every source table this document reads from, metric views and joins
    /// alike, in document order and without deduplication.
    ///
    /// The lineage question a metric view document is usually read to answer.
    #[must_use]
    pub fn source_tables(&self) -> Vec<&str> {
        let mut out = Vec::new();
        for view in &self.metric_views {
            out.push(view.source.as_str());
            collect_join_sources(&view.joins, &mut out);
        }
        out
    }
}

fn collect_join_sources<'a>(joins: &'a [DBMVJoin], out: &mut Vec<&'a str>) {
    for join in joins {
        out.push(join.source.as_str());
        collect_join_sources(&join.joins, out);
    }
}

/// Databricks metric view definition.
///
/// Uses `snake_case` — Databricks' own spelling for the inner content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DBMVMetricView {
    /// Metric view name.
    pub name: String,
    /// Version of the metric view definition.
    #[serde(default = "default_version")]
    pub version: String,
    /// Fully qualified source table (e.g. `catalog.schema.table`).
    pub source: String,
    /// Optional SQL filter expression applied to the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// Optional comment/description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Dimension definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<DBMVDimension>,
    /// Measure definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measures: Vec<DBMVMeasure>,
    /// Join definitions (supports nested joins for snowflake schemas).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joins: Vec<DBMVJoin>,
    /// Materialization configuration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materialization: Option<DBMVMaterialization>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

impl Default for DBMVMetricView {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: default_version(),
            source: String::new(),
            filter: None,
            comment: None,
            dimensions: Vec::new(),
            measures: Vec::new(),
            joins: Vec::new(),
            materialization: None,
            extra: Extra::new(),
        }
    }
}

/// Dimension definition in a metric view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DBMVDimension {
    /// Dimension name.
    pub name: String,
    /// SQL expression for the dimension.
    pub expr: String,
    /// Human-readable display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Optional comment/description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Measure definition in a metric view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DBMVMeasure {
    /// Measure name.
    pub name: String,
    /// SQL aggregation expression (e.g. `SUM(revenue)`).
    pub expr: String,
    /// Human-readable display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Optional comment/description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Format specification for the measure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<DBMVMeasureFormat>,
    /// Window function specifications.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub window: Vec<DBMVWindow>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Format specification for a measure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DBMVMeasureFormat {
    /// Format type (e.g. `currency`, `percentage`, `number`).
    #[serde(rename = "type")]
    pub format_type: String,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Window function specification for a measure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DBMVWindow {
    /// Column to order by.
    pub order: String,
    /// Window range (e.g. `cumulative`, `unbounded`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<String>,
    /// Semi-additive behaviour (e.g. `last`, `first`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semiadditive: Option<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Join definition (supports recursive nesting for snowflake schemas).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DBMVJoin {
    /// Join alias name.
    pub name: String,
    /// Fully qualified source table for the join.
    pub source: String,
    /// Join condition expression (e.g. `source.customer_id = customers.id`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
    /// Column names for an equi-join (alternative to `on`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub using: Vec<String>,
    /// Nested joins (for snowflake schema patterns).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub joins: Vec<DBMVJoin>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Materialization configuration for a metric view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DBMVMaterialization {
    /// Refresh schedule (e.g. `every 6 hours`, `daily`).
    pub schedule: String,
    /// Materialization mode (e.g. `relaxed`, `strict`).
    pub mode: String,
    /// Pre-computed materialized views.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub materialized_views: Vec<DBMVMaterializedView>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

/// Pre-computed materialized view definition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DBMVMaterializedView {
    /// Materialized view name.
    pub name: String,
    /// View type: `aggregated` or `unaggregated`.
    #[serde(rename = "type")]
    pub view_type: String,
    /// Dimensions to include (for aggregated views).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dimensions: Vec<String>,
    /// Measures to include (for aggregated views).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub measures: Vec<String>,
    /// Keys not named above, preserved verbatim.
    #[serde(flatten)]
    pub extra: Extra,
}

#[cfg(feature = "yaml")]
impl DBMVDocument {
    /// Parse a DBMV document from YAML.
    ///
    /// # Errors
    ///
    /// Returns the underlying `serde_norway` error if the document is not
    /// well-formed YAML or does not name a `system`.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_norway::Error> {
        serde_norway::from_str(yaml)
    }

    /// Render this document as YAML.
    ///
    /// # Errors
    ///
    /// Returns the underlying `serde_norway` error if the document cannot be
    /// represented as YAML.
    pub fn to_yaml(&self) -> Result<String, serde_norway::Error> {
        serde_norway::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_yaml(value: &impl Serialize) -> String {
        serde_norway::to_string(value).unwrap()
    }

    #[test]
    fn document_new() {
        let doc = DBMVDocument::new("my-system");
        assert_eq!(doc.system, "my-system");
        assert_eq!(doc.api_version, "v1.0.0");
        assert_eq!(doc.kind, "MetricViews");
        assert!(doc.metric_views.is_empty());
    }

    #[test]
    fn document_add_metric_view() {
        let mut doc = DBMVDocument::new("test-system");
        doc.add_metric_view(DBMVMetricView {
            name: "orders".to_string(),
            source: "catalog.schema.orders".to_string(),
            ..Default::default()
        });
        assert_eq!(doc.metric_views.len(), 1);
        assert_eq!(doc.get_metric_view("orders").unwrap().name, "orders");
        assert!(doc.get_metric_view("nonexistent").is_none());
    }

    #[test]
    fn metric_view_version_defaults() {
        assert_eq!(DBMVMetricView::default().version, "1.1");
    }

    #[test]
    fn type_is_spelled_type_and_not_format_type() {
        let format = DBMVMeasureFormat {
            format_type: "currency".to_string(),
            extra: Extra::new(),
        };
        assert!(to_yaml(&format).contains("type: currency"));

        let mv = DBMVMaterializedView {
            name: "test".to_string(),
            view_type: "aggregated".to_string(),
            ..Default::default()
        };
        assert!(to_yaml(&mv).contains("type: aggregated"));
    }

    #[test]
    fn two_casings_in_one_document() {
        let mut doc = DBMVDocument::new("test");
        doc.add_metric_view(DBMVMetricView {
            name: "test_view".to_string(),
            source: "catalog.schema.table".to_string(),
            dimensions: vec![DBMVDimension {
                name: "dim1".to_string(),
                expr: "col1".to_string(),
                display_name: Some("Dimension 1".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        });
        let yaml = to_yaml(&doc);

        // Envelope: camelCase.
        assert!(yaml.contains("apiVersion:"));
        assert!(yaml.contains("metricViews:"));
        assert!(!yaml.contains("api_version:"));
        assert!(!yaml.contains("metric_views:"));

        // Inner content: Databricks' own snake_case.
        assert!(yaml.contains("display_name:"));
    }

    #[test]
    fn nested_joins_round_trip() {
        let join = DBMVJoin {
            name: "customers".to_string(),
            source: "catalog.schema.customers".to_string(),
            on: Some("source.customer_id = customers.id".to_string()),
            joins: vec![DBMVJoin {
                name: "nation".to_string(),
                source: "catalog.schema.nations".to_string(),
                on: Some("customers.nation_id = nation.id".to_string()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let parsed: DBMVJoin = serde_norway::from_str(&to_yaml(&join)).unwrap();
        assert_eq!(join, parsed);
    }

    #[test]
    fn source_tables_walks_nested_joins() {
        let mut doc = DBMVDocument::new("s");
        doc.add_metric_view(DBMVMetricView {
            name: "v".to_string(),
            source: "cat.sch.orders".to_string(),
            joins: vec![DBMVJoin {
                name: "customers".to_string(),
                source: "cat.sch.customers".to_string(),
                joins: vec![DBMVJoin {
                    name: "nation".to_string(),
                    source: "cat.sch.nations".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        });
        assert_eq!(
            doc.source_tables(),
            vec!["cat.sch.orders", "cat.sch.customers", "cat.sch.nations"]
        );
    }

    #[test]
    fn optional_fields_are_omitted() {
        let view = DBMVMetricView {
            name: "simple".to_string(),
            source: "catalog.schema.table".to_string(),
            ..Default::default()
        };
        let yaml = to_yaml(&view);
        for absent in [
            "filter:",
            "comment:",
            "dimensions:",
            "measures:",
            "joins:",
            "materialization:",
        ] {
            assert!(!yaml.contains(absent), "{absent} should be omitted: {yaml}");
        }
    }

    #[test]
    fn unknown_keys_survive_a_round_trip() {
        // A `measures[].format` variant Databricks adds tomorrow.
        let json = r#"{"system":"s","metricViews":[{"name":"v","version":"1.1",
                       "source":"cat.sch.t","measures":[{"name":"m","expr":"SUM(x)",
                       "format":{"type":"currency","decimal_places":2}}]}]}"#;
        let doc: DBMVDocument = serde_json::from_str(json).unwrap();
        assert_eq!(
            doc.metric_views[0].measures[0]
                .format
                .as_ref()
                .unwrap()
                .extra
                .get("decimal_places"),
            Some(&serde_json::json!(2))
        );

        // apiVersion and kind have defaults, so they appear on the way out and
        // exact equality would not hold. Compare against the document with
        // those two defaults made explicit.
        let expected = r#"{"apiVersion":"v1.0.0","kind":"MetricViews","system":"s",
                          "metricViews":[{"name":"v","version":"1.1","source":"cat.sch.t",
                          "measures":[{"name":"m","expr":"SUM(x)",
                          "format":{"type":"currency","decimal_places":2}}]}]}"#;
        let back: serde_json::Value = serde_json::to_value(&doc).unwrap();
        assert_eq!(
            back,
            serde_json::from_str::<serde_json::Value>(expected).unwrap()
        );
    }
}

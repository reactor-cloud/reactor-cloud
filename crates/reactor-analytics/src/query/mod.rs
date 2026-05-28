//! Query plane for analytics.

pub mod compiler;
pub mod ops;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Query kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryKind {
    /// Raw event rows.
    Events,
    /// Aggregate metrics.
    Aggregate,
    /// Funnel analysis.
    Funnel,
    /// Retention analysis.
    Retention,
    /// Breakdown by property.
    Breakdown,
    /// Path analysis.
    Path,
}

/// Time range for queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TimeRange {
    /// Absolute time range.
    Absolute {
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    },
    /// Relative time range (e.g., "30d", "7d", "24h").
    Relative { last: String },
}

/// Time bucket granularity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeBucket {
    /// 1 minute.
    #[serde(rename = "1m")]
    Minute1,
    /// 5 minutes.
    #[serde(rename = "5m")]
    Minute5,
    /// 1 hour.
    #[serde(rename = "1h")]
    Hour1,
    /// 1 day.
    #[serde(rename = "1d")]
    Day1,
    /// 1 week.
    #[serde(rename = "1w")]
    Week1,
    /// 1 month.
    #[serde(rename = "1mo")]
    Month1,
}

/// Measure type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Measure {
    /// Count of events.
    Count,
    /// Count of unique users.
    UniqueUsers,
    /// Count of unique sessions.
    UniqueSessions,
    /// Sum of a property.
    Sum(String),
    /// Average of a property.
    Avg(String),
    /// 50th percentile.
    P50(String),
    /// 95th percentile.
    P95(String),
    /// 99th percentile.
    P99(String),
}

/// Filter expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterExpr {
    /// All conditions must match (AND).
    All(Vec<FilterExpr>),
    /// Any condition must match (OR).
    Any(Vec<FilterExpr>),
    /// Condition must not match (NOT).
    Not(Box<FilterExpr>),
    /// Filter by event name.
    Event(StringOp),
    /// Filter by property value.
    Prop { name: String, op: ValueOp },
    /// Filter by user ID.
    User(StringOp),
    /// Filter by anonymous ID.
    Anon(StringOp),
}

/// String comparison operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StringOp {
    Eq(String),
    Neq(String),
    Contains(String),
    StartsWith(String),
    EndsWith(String),
    In(Vec<String>),
    NotIn(Vec<String>),
}

/// Value comparison operator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueOp {
    Eq(serde_json::Value),
    Neq(serde_json::Value),
    Gt(serde_json::Value),
    Gte(serde_json::Value),
    Lt(serde_json::Value),
    Lte(serde_json::Value),
    In(Vec<serde_json::Value>),
    NotIn(Vec<serde_json::Value>),
    IsNull,
    IsNotNull,
}

/// Group by key.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupKey {
    /// Group by a property.
    Prop(String),
    /// Group by event name.
    Event,
    /// Group by user ID.
    User,
    /// Group by anonymous ID.
    Anon,
}

/// Funnel step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelStep {
    /// Event name for this step.
    pub event: String,
    /// Optional filter for this step.
    #[serde(default)]
    pub filter: Option<FilterExpr>,
}

/// Cohort definition for retention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CohortDef {
    /// Cohorting event.
    pub event: String,
    /// Optional filter for cohorting.
    #[serde(default)]
    pub filter: Option<FilterExpr>,
}

/// Order specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderSpec {
    /// Field to order by.
    pub field: String,
    /// Ascending order.
    #[serde(default = "default_asc")]
    pub asc: bool,
}

fn default_asc() -> bool {
    true
}

/// Query request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    /// Project ID.
    pub project_id: Uuid,
    /// Query kind.
    pub kind: QueryKind,
    /// Time range.
    pub time_range: TimeRange,
    /// Filter expression.
    #[serde(default)]
    pub filter: Option<FilterExpr>,
    /// Group by keys.
    #[serde(default)]
    pub group_by: Vec<GroupKey>,
    /// Time bucket.
    #[serde(default)]
    pub time_bucket: Option<TimeBucket>,
    /// Measure (for aggregate queries).
    #[serde(default)]
    pub measure: Option<Measure>,
    /// Funnel steps (for funnel queries).
    #[serde(default)]
    pub steps: Option<Vec<FunnelStep>>,
    /// Conversion window (for funnel queries).
    #[serde(default)]
    pub conversion_window: Option<String>,
    /// Cohort definition (for retention queries).
    #[serde(default)]
    pub cohort: Option<CohortDef>,
    /// Return event (for retention queries).
    #[serde(default)]
    pub return_event: Option<String>,
    /// Limit.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Order by.
    #[serde(default)]
    pub order_by: Option<OrderSpec>,
}

/// Query result row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRow {
    /// Group values.
    #[serde(default)]
    pub group: serde_json::Map<String, serde_json::Value>,
    /// Time bucket (if applicable).
    #[serde(default)]
    pub time: Option<DateTime<Utc>>,
    /// Value.
    pub value: serde_json::Value,
}

/// Funnel result row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunnelRow {
    /// Group values.
    #[serde(default)]
    pub group: serde_json::Map<String, serde_json::Value>,
    /// Step counts.
    pub steps: Vec<u64>,
    /// Overall conversion rate.
    pub conversion: f64,
}

/// Query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueryResult {
    /// Event rows.
    Events {
        rows: Vec<crate::store::StoredEvent>,
        execution_ms: u64,
        rows_scanned: u64,
    },
    /// Aggregate rows.
    Aggregate {
        rows: Vec<QueryRow>,
        execution_ms: u64,
        rows_scanned: u64,
    },
    /// Funnel rows.
    Funnel {
        rows: Vec<FunnelRow>,
        execution_ms: u64,
        rows_scanned: u64,
    },
    /// Retention rows.
    Retention {
        rows: Vec<QueryRow>,
        execution_ms: u64,
        rows_scanned: u64,
    },
    /// Breakdown rows.
    Breakdown {
        rows: Vec<QueryRow>,
        execution_ms: u64,
        rows_scanned: u64,
    },
    /// Path rows.
    Path {
        rows: Vec<QueryRow>,
        execution_ms: u64,
        rows_scanned: u64,
    },
}

//! SQL query compiler.
//!
//! Compiles QueryRequest AST into SQL with hot-column awareness.

use super::{FilterExpr, GroupKey, Measure, QueryRequest, StringOp, TimeBucket, TimeRange, ValueOp};
use crate::config::AnalyticsConfig;
use crate::error::AnalyticsError;
use chrono::{DateTime, Duration, Utc};
use std::fmt::Write;

/// Hot columns that exist directly on the events table.
pub const HOT_COLUMNS: &[&str] = &[
    "event",
    "anonymous_id",
    "user_id",
    "session_id",
    "url",
    "path",
    "referrer_host",
    "utm_source",
    "country",
    "device_type",
    "library_name",
    "library_version",
];

/// Compiled SQL query.
#[derive(Debug, Clone)]
pub struct CompiledQuery {
    /// SQL query string.
    pub sql: String,
    /// Bind parameters.
    pub params: Vec<QueryParam>,
    /// Estimated cost (for query planning).
    pub estimated_cost: u64,
}

/// Query parameter.
#[derive(Debug, Clone)]
pub enum QueryParam {
    Uuid(uuid::Uuid),
    Timestamp(DateTime<Utc>),
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Json(serde_json::Value),
    StringArray(Vec<String>),
}

/// SQL compiler.
pub struct SqlCompiler<'a> {
    config: &'a AnalyticsConfig,
    param_idx: usize,
    params: Vec<QueryParam>,
}

impl<'a> SqlCompiler<'a> {
    /// Create a new SQL compiler.
    pub fn new(config: &'a AnalyticsConfig) -> Self {
        Self {
            config,
            param_idx: 1,
            params: Vec::new(),
        }
    }

    /// Compile a query request into SQL.
    pub fn compile(&mut self, req: &QueryRequest) -> Result<CompiledQuery, AnalyticsError> {
        // Resolve time range
        let (from, to) = self.resolve_time_range(&req.time_range)?;

        // Check time range limits
        let days = (to - from).num_days() as u32;
        let max_days = self.config.query_raw_range_days;
        if days > max_days {
            return Err(AnalyticsError::QueryRangeTooWide {
                days,
                limit: max_days,
            });
        }

        let sql = match req.kind {
            super::QueryKind::Events => self.compile_events(req, from, to)?,
            super::QueryKind::Aggregate => self.compile_aggregate(req, from, to)?,
            super::QueryKind::Breakdown => self.compile_breakdown(req, from, to)?,
            super::QueryKind::Funnel => self.compile_funnel(req, from, to)?,
            super::QueryKind::Retention => self.compile_retention(req, from, to)?,
            super::QueryKind::Path => self.compile_path(req, from, to)?,
        };

        // Add statement timeout
        let timeout_ms = self.config.query_timeout_ms;
        let sql = format!("SET statement_timeout = '{timeout_ms}ms';\n{sql}");

        Ok(CompiledQuery {
            sql,
            params: std::mem::take(&mut self.params),
            estimated_cost: days as u64 * 1000,
        })
    }

    /// Resolve time range to absolute timestamps.
    fn resolve_time_range(
        &self,
        range: &TimeRange,
    ) -> Result<(DateTime<Utc>, DateTime<Utc>), AnalyticsError> {
        match range {
            TimeRange::Absolute { from, to } => Ok((*from, *to)),
            TimeRange::Relative { last } => {
                let to = Utc::now();
                let from = parse_relative_duration(last, to)?;
                Ok((from, to))
            }
        }
    }

    /// Compile events query.
    fn compile_events(
        &mut self,
        req: &QueryRequest,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<String, AnalyticsError> {
        let mut sql = String::from("SELECT * FROM _reactor_analytics.events WHERE ");

        // Project filter
        sql.push_str(&format!("project_id = ${} ", self.next_param()));
        self.params.push(QueryParam::Uuid(req.project_id));

        // Time range filter
        sql.push_str(&format!("AND received_at >= ${} ", self.next_param()));
        self.params.push(QueryParam::Timestamp(from));

        sql.push_str(&format!("AND received_at < ${} ", self.next_param()));
        self.params.push(QueryParam::Timestamp(to));

        // Custom filter
        if let Some(ref filter) = req.filter {
            sql.push_str("AND ");
            self.compile_filter(filter, &mut sql)?;
        }

        // Order by
        sql.push_str("ORDER BY received_at DESC ");

        // Limit
        let limit = req.limit.unwrap_or(1000).min(self.config.query_max_rows as u32);
        sql.push_str(&format!("LIMIT {limit}"));

        Ok(sql)
    }

    /// Compile aggregate query.
    fn compile_aggregate(
        &mut self,
        req: &QueryRequest,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<String, AnalyticsError> {
        let mut sql = String::from("SELECT ");

        // Time bucket
        if let Some(bucket) = req.time_bucket {
            let interval = bucket_to_interval(bucket);
            sql.push_str(&format!(
                "date_trunc('{}', received_at) as time_bucket, ",
                interval
            ));
        }

        // Group by columns
        for key in &req.group_by {
            let col = self.group_key_to_column(key);
            sql.push_str(&format!("{col}, "));
        }

        // Measure
        let measure = req.measure.as_ref().unwrap_or(&Measure::Count);
        sql.push_str(&self.measure_to_sql(measure));

        sql.push_str(" FROM _reactor_analytics.events WHERE ");

        // Project filter
        sql.push_str(&format!("project_id = ${} ", self.next_param()));
        self.params.push(QueryParam::Uuid(req.project_id));

        // Time range filter
        sql.push_str(&format!("AND received_at >= ${} ", self.next_param()));
        self.params.push(QueryParam::Timestamp(from));

        sql.push_str(&format!("AND received_at < ${} ", self.next_param()));
        self.params.push(QueryParam::Timestamp(to));

        // Custom filter
        if let Some(ref filter) = req.filter {
            sql.push_str("AND ");
            self.compile_filter(filter, &mut sql)?;
        }

        // Group by clause
        let mut group_cols = Vec::new();
        if req.time_bucket.is_some() {
            group_cols.push("time_bucket".to_string());
        }
        for key in &req.group_by {
            group_cols.push(self.group_key_to_column(key));
        }
        if !group_cols.is_empty() {
            sql.push_str("GROUP BY ");
            sql.push_str(&group_cols.join(", "));
            sql.push(' ');
        }

        // Order by
        if req.time_bucket.is_some() {
            sql.push_str("ORDER BY time_bucket ASC ");
        } else if !req.group_by.is_empty() {
            sql.push_str("ORDER BY value DESC ");
        }

        // Limit
        if let Some(limit) = req.limit {
            sql.push_str(&format!("LIMIT {limit}"));
        }

        Ok(sql)
    }

    /// Compile breakdown query.
    fn compile_breakdown(
        &mut self,
        req: &QueryRequest,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<String, AnalyticsError> {
        // Breakdown is essentially aggregate with required group_by
        self.compile_aggregate(req, from, to)
    }

    /// Compile funnel query.
    fn compile_funnel(
        &mut self,
        req: &QueryRequest,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<String, AnalyticsError> {
        let steps = req
            .steps
            .as_ref()
            .ok_or_else(|| AnalyticsError::Validation("funnel query requires steps".to_string()))?;

        if steps.is_empty() {
            return Err(AnalyticsError::Validation(
                "funnel requires at least one step".to_string(),
            ));
        }

        let conversion_window = req
            .conversion_window
            .as_ref()
            .map(|s| parse_duration_secs(s))
            .transpose()?
            .unwrap_or(86400 * 7); // 7 days default

        // Build funnel SQL using window functions
        let mut sql = String::new();

        // CTE for ordered events per user
        sql.push_str("WITH user_events AS (\n");
        sql.push_str("  SELECT user_id, anonymous_id, event, timestamp,\n");
        sql.push_str("         ROW_NUMBER() OVER (PARTITION BY COALESCE(user_id, anonymous_id) ORDER BY timestamp) as rn\n");
        sql.push_str("  FROM _reactor_analytics.events\n");
        sql.push_str(&format!("  WHERE project_id = ${}\n", self.next_param()));
        self.params.push(QueryParam::Uuid(req.project_id));

        sql.push_str(&format!("    AND received_at >= ${}\n", self.next_param()));
        self.params.push(QueryParam::Timestamp(from));

        sql.push_str(&format!("    AND received_at < ${}\n", self.next_param()));
        self.params.push(QueryParam::Timestamp(to));

        // Event filter for funnel steps
        sql.push_str("    AND event IN (");
        for (i, step) in steps.iter().enumerate() {
            if i > 0 {
                sql.push_str(", ");
            }
            sql.push_str(&format!("${}", self.next_param()));
            self.params.push(QueryParam::String(step.event.clone()));
        }
        sql.push_str(")\n");
        sql.push_str("),\n");

        // Build step counts
        sql.push_str("funnel_counts AS (\n");
        for (i, step) in steps.iter().enumerate() {
            if i > 0 {
                sql.push_str("  UNION ALL\n");
            }
            sql.push_str(&format!(
                "  SELECT {} as step, COUNT(DISTINCT COALESCE(user_id, anonymous_id)) as users\n",
                i + 1
            ));
            sql.push_str("  FROM user_events\n");
            sql.push_str(&format!("  WHERE event = ${}\n", self.next_param()));
            self.params.push(QueryParam::String(step.event.clone()));
        }
        sql.push_str(")\n");

        // Final aggregation
        sql.push_str("SELECT step, users FROM funnel_counts ORDER BY step");

        Ok(sql)
    }

    /// Compile retention query.
    fn compile_retention(
        &mut self,
        req: &QueryRequest,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<String, AnalyticsError> {
        let cohort = req.cohort.as_ref().ok_or_else(|| {
            AnalyticsError::Validation("retention query requires cohort".to_string())
        })?;

        let return_event = req.return_event.as_ref().ok_or_else(|| {
            AnalyticsError::Validation("retention query requires return_event".to_string())
        })?;

        let mut sql = String::new();

        // Cohort CTE - users who did the cohort event
        sql.push_str("WITH cohorts AS (\n");
        sql.push_str("  SELECT DISTINCT COALESCE(user_id, anonymous_id) as user_key,\n");
        sql.push_str("         DATE_TRUNC('day', timestamp) as cohort_date\n");
        sql.push_str("  FROM _reactor_analytics.events\n");
        sql.push_str(&format!("  WHERE project_id = ${}\n", self.next_param()));
        self.params.push(QueryParam::Uuid(req.project_id));

        sql.push_str(&format!("    AND received_at >= ${}\n", self.next_param()));
        self.params.push(QueryParam::Timestamp(from));

        sql.push_str(&format!("    AND received_at < ${}\n", self.next_param()));
        self.params.push(QueryParam::Timestamp(to));

        sql.push_str(&format!("    AND event = ${}\n", self.next_param()));
        self.params.push(QueryParam::String(cohort.event.clone()));
        sql.push_str("),\n");

        // Return events CTE
        sql.push_str("returns AS (\n");
        sql.push_str("  SELECT COALESCE(user_id, anonymous_id) as user_key,\n");
        sql.push_str("         DATE_TRUNC('day', timestamp) as return_date\n");
        sql.push_str("  FROM _reactor_analytics.events\n");
        sql.push_str(&format!("  WHERE project_id = ${}\n", self.next_param()));
        self.params.push(QueryParam::Uuid(req.project_id));

        sql.push_str(&format!("    AND received_at >= ${}\n", self.next_param()));
        self.params.push(QueryParam::Timestamp(from));

        sql.push_str(&format!("    AND received_at < ${}\n", self.next_param()));
        self.params.push(QueryParam::Timestamp(to));

        sql.push_str(&format!("    AND event = ${}\n", self.next_param()));
        self.params.push(QueryParam::String(return_event.clone()));
        sql.push_str(")\n");

        // Join and calculate retention
        sql.push_str(
            "SELECT c.cohort_date, (r.return_date - c.cohort_date)::int as day_n,\n",
        );
        sql.push_str("       COUNT(DISTINCT c.user_key) as cohort_size,\n");
        sql.push_str("       COUNT(DISTINCT r.user_key) as retained\n");
        sql.push_str("FROM cohorts c\n");
        sql.push_str("LEFT JOIN returns r ON c.user_key = r.user_key AND r.return_date >= c.cohort_date\n");
        sql.push_str("GROUP BY c.cohort_date, day_n\n");
        sql.push_str("ORDER BY c.cohort_date, day_n");

        Ok(sql)
    }

    /// Compile path query.
    fn compile_path(
        &mut self,
        req: &QueryRequest,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<String, AnalyticsError> {
        let mut sql = String::new();

        // Get sequential events per user
        sql.push_str("WITH user_paths AS (\n");
        sql.push_str("  SELECT COALESCE(user_id, anonymous_id) as user_key,\n");
        sql.push_str("         event,\n");
        sql.push_str("         LEAD(event) OVER (PARTITION BY COALESCE(user_id, anonymous_id) ORDER BY timestamp) as next_event\n");
        sql.push_str("  FROM _reactor_analytics.events\n");
        sql.push_str(&format!("  WHERE project_id = ${}\n", self.next_param()));
        self.params.push(QueryParam::Uuid(req.project_id));

        sql.push_str(&format!("    AND received_at >= ${}\n", self.next_param()));
        self.params.push(QueryParam::Timestamp(from));

        sql.push_str(&format!("    AND received_at < ${}\n", self.next_param()));
        self.params.push(QueryParam::Timestamp(to));

        // Custom filter
        if let Some(ref filter) = req.filter {
            sql.push_str("    AND ");
            self.compile_filter(filter, &mut sql)?;
            sql.push('\n');
        }

        sql.push_str(")\n");

        // Aggregate path transitions
        sql.push_str("SELECT event as from_event, next_event as to_event, COUNT(*) as transitions\n");
        sql.push_str("FROM user_paths\n");
        sql.push_str("WHERE next_event IS NOT NULL\n");
        sql.push_str("GROUP BY event, next_event\n");
        sql.push_str("ORDER BY transitions DESC\n");

        if let Some(limit) = req.limit {
            sql.push_str(&format!("LIMIT {limit}"));
        }

        Ok(sql)
    }

    /// Compile filter expression to SQL.
    fn compile_filter(&mut self, filter: &FilterExpr, sql: &mut String) -> Result<(), AnalyticsError> {
        match filter {
            FilterExpr::All(exprs) => {
                sql.push('(');
                for (i, expr) in exprs.iter().enumerate() {
                    if i > 0 {
                        sql.push_str(" AND ");
                    }
                    self.compile_filter(expr, sql)?;
                }
                sql.push(')');
            }
            FilterExpr::Any(exprs) => {
                sql.push('(');
                for (i, expr) in exprs.iter().enumerate() {
                    if i > 0 {
                        sql.push_str(" OR ");
                    }
                    self.compile_filter(expr, sql)?;
                }
                sql.push(')');
            }
            FilterExpr::Not(expr) => {
                sql.push_str("NOT ");
                self.compile_filter(expr, sql)?;
            }
            FilterExpr::Event(op) => {
                self.compile_string_op("event", op, sql)?;
            }
            FilterExpr::User(op) => {
                self.compile_string_op("user_id", op, sql)?;
            }
            FilterExpr::Anon(op) => {
                self.compile_string_op("anonymous_id", op, sql)?;
            }
            FilterExpr::Prop { name, op } => {
                // Check if it's a hot column
                if HOT_COLUMNS.contains(&name.as_str()) {
                    self.compile_value_op(name, op, sql)?;
                } else {
                    // Use jsonb extraction
                    let json_path = format!("properties->>{}", self.next_param());
                    self.params.push(QueryParam::String(name.clone()));
                    self.compile_value_op_jsonb(&json_path, op, sql)?;
                }
            }
        }
        Ok(())
    }

    /// Compile string operator.
    fn compile_string_op(
        &mut self,
        col: &str,
        op: &StringOp,
        sql: &mut String,
    ) -> Result<(), AnalyticsError> {
        match op {
            StringOp::Eq(v) => {
                write!(sql, "{col} = ${}", self.next_param()).unwrap();
                self.params.push(QueryParam::String(v.clone()));
            }
            StringOp::Neq(v) => {
                write!(sql, "{col} != ${}", self.next_param()).unwrap();
                self.params.push(QueryParam::String(v.clone()));
            }
            StringOp::Contains(v) => {
                write!(sql, "{col} LIKE '%' || ${} || '%'", self.next_param()).unwrap();
                self.params.push(QueryParam::String(v.clone()));
            }
            StringOp::StartsWith(v) => {
                write!(sql, "{col} LIKE ${} || '%'", self.next_param()).unwrap();
                self.params.push(QueryParam::String(v.clone()));
            }
            StringOp::EndsWith(v) => {
                write!(sql, "{col} LIKE '%' || ${}", self.next_param()).unwrap();
                self.params.push(QueryParam::String(v.clone()));
            }
            StringOp::In(vals) => {
                write!(sql, "{col} = ANY(${})", self.next_param()).unwrap();
                self.params.push(QueryParam::StringArray(vals.clone()));
            }
            StringOp::NotIn(vals) => {
                write!(sql, "{col} != ALL(${})", self.next_param()).unwrap();
                self.params.push(QueryParam::StringArray(vals.clone()));
            }
        }
        Ok(())
    }

    /// Compile value operator for direct column.
    fn compile_value_op(
        &mut self,
        col: &str,
        op: &ValueOp,
        sql: &mut String,
    ) -> Result<(), AnalyticsError> {
        match op {
            ValueOp::Eq(v) => {
                write!(sql, "{col} = ${}", self.next_param()).unwrap();
                self.params.push(value_to_param(v));
            }
            ValueOp::Neq(v) => {
                write!(sql, "{col} != ${}", self.next_param()).unwrap();
                self.params.push(value_to_param(v));
            }
            ValueOp::Gt(v) => {
                write!(sql, "{col} > ${}", self.next_param()).unwrap();
                self.params.push(value_to_param(v));
            }
            ValueOp::Gte(v) => {
                write!(sql, "{col} >= ${}", self.next_param()).unwrap();
                self.params.push(value_to_param(v));
            }
            ValueOp::Lt(v) => {
                write!(sql, "{col} < ${}", self.next_param()).unwrap();
                self.params.push(value_to_param(v));
            }
            ValueOp::Lte(v) => {
                write!(sql, "{col} <= ${}", self.next_param()).unwrap();
                self.params.push(value_to_param(v));
            }
            ValueOp::In(vals) => {
                write!(sql, "{col} = ANY(${})", self.next_param()).unwrap();
                self.params.push(QueryParam::Json(serde_json::Value::Array(vals.clone())));
            }
            ValueOp::NotIn(vals) => {
                write!(sql, "{col} != ALL(${})", self.next_param()).unwrap();
                self.params.push(QueryParam::Json(serde_json::Value::Array(vals.clone())));
            }
            ValueOp::IsNull => {
                write!(sql, "{col} IS NULL").unwrap();
            }
            ValueOp::IsNotNull => {
                write!(sql, "{col} IS NOT NULL").unwrap();
            }
        }
        Ok(())
    }

    /// Compile value operator for jsonb extraction.
    fn compile_value_op_jsonb(
        &mut self,
        json_path: &str,
        op: &ValueOp,
        sql: &mut String,
    ) -> Result<(), AnalyticsError> {
        match op {
            ValueOp::Eq(v) => {
                write!(sql, "({json_path})::text = ${}::text", self.next_param()).unwrap();
                self.params.push(value_to_param(v));
            }
            ValueOp::Neq(v) => {
                write!(sql, "({json_path})::text != ${}::text", self.next_param()).unwrap();
                self.params.push(value_to_param(v));
            }
            ValueOp::Gt(v) => {
                write!(sql, "({json_path})::numeric > ${}::numeric", self.next_param()).unwrap();
                self.params.push(value_to_param(v));
            }
            ValueOp::Gte(v) => {
                write!(sql, "({json_path})::numeric >= ${}::numeric", self.next_param()).unwrap();
                self.params.push(value_to_param(v));
            }
            ValueOp::Lt(v) => {
                write!(sql, "({json_path})::numeric < ${}::numeric", self.next_param()).unwrap();
                self.params.push(value_to_param(v));
            }
            ValueOp::Lte(v) => {
                write!(sql, "({json_path})::numeric <= ${}::numeric", self.next_param()).unwrap();
                self.params.push(value_to_param(v));
            }
            ValueOp::In(_) => {
                return Err(AnalyticsError::Validation(
                    "IN operator not supported for jsonb properties".to_string(),
                ));
            }
            ValueOp::NotIn(_) => {
                return Err(AnalyticsError::Validation(
                    "NOT IN operator not supported for jsonb properties".to_string(),
                ));
            }
            ValueOp::IsNull => {
                write!(sql, "({json_path}) IS NULL").unwrap();
            }
            ValueOp::IsNotNull => {
                write!(sql, "({json_path}) IS NOT NULL").unwrap();
            }
        }
        Ok(())
    }

    /// Convert group key to column name.
    fn group_key_to_column(&self, key: &GroupKey) -> String {
        match key {
            GroupKey::Prop(name) => {
                if HOT_COLUMNS.contains(&name.as_str()) {
                    name.clone()
                } else {
                    format!("properties->>'{name}'")
                }
            }
            GroupKey::Event => "event".to_string(),
            GroupKey::User => "user_id".to_string(),
            GroupKey::Anon => "anonymous_id".to_string(),
        }
    }

    /// Convert measure to SQL expression.
    fn measure_to_sql(&self, measure: &Measure) -> String {
        match measure {
            Measure::Count => "COUNT(*) as value".to_string(),
            Measure::UniqueUsers => "COUNT(DISTINCT user_id) as value".to_string(),
            Measure::UniqueSessions => "COUNT(DISTINCT session_id) as value".to_string(),
            Measure::Sum(prop) => {
                if HOT_COLUMNS.contains(&prop.as_str()) {
                    format!("SUM({prop}::numeric) as value")
                } else {
                    format!("SUM((properties->>'{prop}')::numeric) as value")
                }
            }
            Measure::Avg(prop) => {
                if HOT_COLUMNS.contains(&prop.as_str()) {
                    format!("AVG({prop}::numeric) as value")
                } else {
                    format!("AVG((properties->>'{prop}')::numeric) as value")
                }
            }
            Measure::P50(prop) => {
                if HOT_COLUMNS.contains(&prop.as_str()) {
                    format!("PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY {prop}::numeric) as value")
                } else {
                    format!("PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY (properties->>'{prop}')::numeric) as value")
                }
            }
            Measure::P95(prop) => {
                if HOT_COLUMNS.contains(&prop.as_str()) {
                    format!("PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY {prop}::numeric) as value")
                } else {
                    format!("PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY (properties->>'{prop}')::numeric) as value")
                }
            }
            Measure::P99(prop) => {
                if HOT_COLUMNS.contains(&prop.as_str()) {
                    format!("PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY {prop}::numeric) as value")
                } else {
                    format!("PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY (properties->>'{prop}')::numeric) as value")
                }
            }
        }
    }

    /// Get next parameter index.
    fn next_param(&mut self) -> usize {
        let idx = self.param_idx;
        self.param_idx += 1;
        idx
    }
}

/// Parse relative duration string (e.g., "30d", "7d", "24h").
fn parse_relative_duration(s: &str, from: DateTime<Utc>) -> Result<DateTime<Utc>, AnalyticsError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(AnalyticsError::Validation("empty duration".to_string()));
    }

    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str
        .parse()
        .map_err(|_| AnalyticsError::Validation(format!("invalid duration: {s}")))?;

    let duration = match unit {
        "m" => Duration::minutes(num),
        "h" => Duration::hours(num),
        "d" => Duration::days(num),
        "w" => Duration::weeks(num),
        _ => return Err(AnalyticsError::Validation(format!("invalid duration unit: {unit}"))),
    };

    Ok(from - duration)
}

/// Parse duration to seconds.
fn parse_duration_secs(s: &str) -> Result<i64, AnalyticsError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(AnalyticsError::Validation("empty duration".to_string()));
    }

    // Handle compound durations like "7d" or "24h"
    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str
        .parse()
        .map_err(|_| AnalyticsError::Validation(format!("invalid duration: {s}")))?;

    let secs = match unit {
        "s" => num,
        "m" => num * 60,
        "h" => num * 3600,
        "d" => num * 86400,
        "w" => num * 86400 * 7,
        _ => return Err(AnalyticsError::Validation(format!("invalid duration unit: {unit}"))),
    };

    Ok(secs)
}

/// Convert time bucket to PostgreSQL interval string.
fn bucket_to_interval(bucket: TimeBucket) -> &'static str {
    match bucket {
        TimeBucket::Minute1 => "minute",
        TimeBucket::Minute5 => "minute",
        TimeBucket::Hour1 => "hour",
        TimeBucket::Day1 => "day",
        TimeBucket::Week1 => "week",
        TimeBucket::Month1 => "month",
    }
}

/// Convert JSON value to query parameter.
fn value_to_param(v: &serde_json::Value) -> QueryParam {
    match v {
        serde_json::Value::String(s) => QueryParam::String(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                QueryParam::Int(i)
            } else if let Some(f) = n.as_f64() {
                QueryParam::Float(f)
            } else {
                QueryParam::String(n.to_string())
            }
        }
        serde_json::Value::Bool(b) => QueryParam::Bool(*b),
        _ => QueryParam::Json(v.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_relative_duration() {
        let now = Utc::now();
        let result = parse_relative_duration("7d", now).unwrap();
        assert!((now - result).num_days() == 7);
    }

    #[test]
    fn test_parse_duration_secs() {
        assert_eq!(parse_duration_secs("1h").unwrap(), 3600);
        assert_eq!(parse_duration_secs("7d").unwrap(), 86400 * 7);
    }
}

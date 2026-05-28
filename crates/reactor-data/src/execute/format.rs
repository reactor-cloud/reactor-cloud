//! SQL query builder.
//!
//! Builds parameterized SQL from QueryPlan.

use crate::error::DataError;
use crate::execute::embed::ResolvedEmbed;
use crate::middleware::DataCtx;
use crate::policy::{compile_for_scope, PolicyDecision, PolicyScope};
use crate::query::{
    FilterExpr, FilterOp, FilterValue, Prefer, QueryPlan, Resolution, ReturnMode, SelectColumn,
};
use crate::store::{SqlValue, TableSnapshot};
use serde_json::{Map, Value};
use sqlx::PgPool;

/// SQL query builder.
pub struct SqlBuilder {
    schema: String,
    table: String,
}

impl SqlBuilder {
    /// Create a new SQL builder.
    pub fn new(schema: &str, table: &str) -> Self {
        Self {
            schema: schema.to_string(),
            table: table.to_string(),
        }
    }

    /// Get the fully qualified table name.
    fn qualified_table(&self) -> String {
        format!("\"{}\".\"{}\"", self.schema, self.table)
    }

    /// Build a SELECT query.
    pub fn build_select(
        &self,
        plan: &QueryPlan,
        _table_info: &TableSnapshot,
    ) -> Result<(String, Vec<SqlValue>), DataError> {
        let mut sql = String::new();
        let mut params = Vec::new();

        // SELECT clause
        sql.push_str("SELECT ");
        sql.push_str(&self.build_select_columns(&plan.select));

        // FROM clause
        sql.push_str(" FROM ");
        sql.push_str(&self.qualified_table());

        // WHERE clause (combine filters and policy predicate)
        let has_filters = !plan.filters.is_empty();
        let has_policy = plan.policy_predicate.is_some();

        if has_filters || has_policy {
            sql.push_str(" WHERE ");

            let mut where_parts = Vec::new();

            // User filters
            if has_filters {
                let (where_clause, where_params) = self.build_where(&plan.filters, params.len())?;
                where_parts.push(where_clause);
                params.extend(where_params);
            }

            // Policy predicate (injected directly as SQL)
            if let Some(ref policy) = plan.policy_predicate {
                where_parts.push(format!("({})", policy));
            }

            sql.push_str(&where_parts.join(" AND "));
        }

        // ORDER BY clause
        if !plan.order.is_empty() {
            sql.push_str(" ORDER BY ");
            sql.push_str(&self.build_order(&plan.order));
        }

        // LIMIT/OFFSET
        sql.push_str(&format!(" LIMIT {}", plan.pagination.limit));
        if plan.pagination.offset > 0 {
            sql.push_str(&format!(" OFFSET {}", plan.pagination.offset));
        }

        Ok((sql, params))
    }

    /// Build a SELECT query with embedded resources.
    ///
    /// Uses LEFT JOIN LATERAL to fetch related rows with per-embed policy enforcement.
    pub async fn build_select_with_embeds(
        &self,
        plan: &QueryPlan,
        _table_info: &TableSnapshot,
        embeds: &[ResolvedEmbed],
        ctx: &DataCtx,
        pool: &PgPool,
    ) -> Result<(String, Vec<SqlValue>), DataError> {
        let mut sql = String::new();
        let mut params = Vec::new();
        let base_alias = "base";

        // SELECT clause - base columns + embed data columns
        sql.push_str("SELECT ");

        // Build base columns with alias
        let base_columns = self.build_select_columns_aliased(&plan.select, base_alias);
        sql.push_str(&base_columns);

        // Add embed data columns
        for embed in embeds {
            let output_name = embed.spec.alias.as_deref().unwrap_or(&embed.spec.name);
            sql.push_str(&format!(", \"{}\".data AS \"{}\"", embed.sql_alias, output_name));
        }

        // FROM clause with alias
        sql.push_str(" FROM ");
        sql.push_str(&self.qualified_table());
        sql.push_str(&format!(" AS \"{}\"", base_alias));

        // Add LEFT JOIN LATERAL for each embed
        for embed in embeds {
            let embed_sql = self
                .build_embed_lateral(embed, base_alias, ctx, pool)
                .await?;
            sql.push(' ');
            sql.push_str(&embed_sql);
        }

        // WHERE clause (combine filters and policy predicate)
        let has_filters = !plan.filters.is_empty();
        let has_policy = plan.policy_predicate.is_some();

        if has_filters || has_policy {
            sql.push_str(" WHERE ");

            let mut where_parts = Vec::new();

            if has_filters {
                let (where_clause, where_params) =
                    self.build_where_aliased(&plan.filters, params.len(), base_alias)?;
                where_parts.push(where_clause);
                params.extend(where_params);
            }

            if let Some(ref policy) = plan.policy_predicate {
                where_parts.push(format!("({})", policy));
            }

            sql.push_str(&where_parts.join(" AND "));
        }

        // ORDER BY clause
        if !plan.order.is_empty() {
            sql.push_str(" ORDER BY ");
            sql.push_str(&self.build_order_aliased(&plan.order, base_alias));
        }

        // LIMIT/OFFSET
        sql.push_str(&format!(" LIMIT {}", plan.pagination.limit));
        if plan.pagination.offset > 0 {
            sql.push_str(&format!(" OFFSET {}", plan.pagination.offset));
        }

        Ok((sql, params))
    }

    /// Build a LEFT JOIN LATERAL clause for an embedded resource.
    async fn build_embed_lateral(
        &self,
        embed: &ResolvedEmbed,
        parent_alias: &str,
        ctx: &DataCtx,
        pool: &PgPool,
    ) -> Result<String, DataError> {
        use crate::execute::embed::EmbedDirection;

        let ref_schema = &embed.ref_table.schema;
        let ref_table = &embed.ref_table.name;
        let alias = &embed.sql_alias;

        // Compile policy for the embedded table
        let policy_decision =
            compile_for_scope(pool, ref_table, PolicyScope::Select, ctx).await?;

        let policy_predicate = match policy_decision {
            PolicyDecision::AlwaysDeny { reason } => {
                // If policy denies, return empty result
                return Ok(format!(
                    "LEFT JOIN LATERAL (SELECT NULL::jsonb AS data WHERE false) AS \"{}\" ON true /* policy denied: {} */",
                    alias, reason
                ));
            }
            PolicyDecision::AlwaysAllow => None,
            PolicyDecision::Conditional { sql_fragment } => Some(sql_fragment),
        };

        // Build column list for jsonb_build_object
        let columns_json = self.build_embed_columns_json(&embed.spec.columns);

        // Build join condition
        let join_condition = match embed.direction {
            EmbedDirection::ToOne => {
                let fk_col = embed.fk.columns.first().unwrap();
                let ref_col = embed.fk.ref_columns.first().unwrap();
                format!(
                    "\"{}\".\"{}\".\"{}\" = \"{}\".\"{}\"",
                    ref_schema, ref_table, ref_col, parent_alias, fk_col
                )
            }
            EmbedDirection::ToMany => {
                let fk_col = embed.fk.columns.first().unwrap();
                let ref_col = embed.fk.ref_columns.first().unwrap();
                format!(
                    "\"{}\".\"{}\".\"{}\" = \"{}\".\"{}\"",
                    ref_schema, ref_table, fk_col, parent_alias, ref_col
                )
            }
        };

        // Build full WHERE clause
        let mut where_parts = vec![join_condition];
        if let Some(policy) = &policy_predicate {
            where_parts.push(format!("({})", policy));
        }
        let where_clause = where_parts.join(" AND ");

        // Generate the subquery based on direction
        let lateral_sql = match embed.direction {
            EmbedDirection::ToOne => {
                format!(
                    "LEFT JOIN LATERAL (\
                        SELECT jsonb_build_object({}) AS data \
                        FROM \"{}\".\"{}\" \
                        WHERE {} \
                        LIMIT 1\
                    ) AS \"{}\" ON true",
                    columns_json, ref_schema, ref_table, where_clause, alias
                )
            }
            EmbedDirection::ToMany => {
                format!(
                    "LEFT JOIN LATERAL (\
                        SELECT COALESCE(jsonb_agg(jsonb_build_object({})), '[]'::jsonb) AS data \
                        FROM \"{}\".\"{}\" \
                        WHERE {}\
                    ) AS \"{}\" ON true",
                    columns_json, ref_schema, ref_table, where_clause, alias
                )
            }
        };

        Ok(lateral_sql)
    }

    /// Build select columns with table alias prefix.
    fn build_select_columns_aliased(&self, columns: &[SelectColumn], alias: &str) -> String {
        if columns.is_empty() || columns.iter().any(|c| matches!(c, SelectColumn::All)) {
            return format!("\"{}\".*", alias);
        }

        let non_embed_columns: Vec<_> = columns
            .iter()
            .filter(|c| !matches!(c, SelectColumn::Embed(_)))
            .collect();

        if non_embed_columns.is_empty() {
            return format!("\"{}\".*", alias);
        }

        non_embed_columns
            .iter()
            .map(|c| match c {
                SelectColumn::All => format!("\"{}\".*", alias),
                SelectColumn::Column(name) => format!("\"{}\".\"{}\"", alias, name),
                SelectColumn::Aliased {
                    alias: col_alias,
                    column,
                } => {
                    format!("\"{}\".\"{}\" AS \"{}\"", alias, column, col_alias)
                }
                SelectColumn::Embed(_) => unreachable!("filtered above"),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Build WHERE clause with table alias prefix.
    fn build_where_aliased(
        &self,
        filters: &[FilterExpr],
        param_offset: usize,
        alias: &str,
    ) -> Result<(String, Vec<SqlValue>), DataError> {
        let mut parts = Vec::new();
        let mut params = Vec::new();

        for filter in filters {
            let (expr, filter_params) =
                self.build_filter_expr_aliased(filter, param_offset + params.len(), alias)?;
            parts.push(expr);
            params.extend(filter_params);
        }

        Ok((parts.join(" AND "), params))
    }

    /// Build a single filter expression with table alias prefix.
    fn build_filter_expr_aliased(
        &self,
        filter: &FilterExpr,
        param_offset: usize,
        alias: &str,
    ) -> Result<(String, Vec<SqlValue>), DataError> {
        let col = format!("\"{}\".\"{}\"", alias, filter.column);
        let mut params = Vec::new();

        let expr = match filter.op {
            FilterOp::Is => {
                let value = match &filter.value {
                    FilterValue::Null => "NULL",
                    FilterValue::Bool(true) => "TRUE",
                    FilterValue::Bool(false) => "FALSE",
                    _ => {
                        return Err(DataError::InvalidFilter(
                            "IS requires null/true/false".to_string(),
                        ))
                    }
                };
                if filter.negated {
                    format!("{} IS NOT {}", col, value)
                } else {
                    format!("{} IS {}", col, value)
                }
            }
            FilterOp::In => {
                let list = match &filter.value {
                    FilterValue::List(items) => items,
                    _ => return Err(DataError::InvalidFilter("IN requires a list".to_string())),
                };

                let placeholders: Vec<String> = list
                    .iter()
                    .map(|v| {
                        params.push(filter_value_to_sql(v));
                        format!("${}", param_offset + params.len())
                    })
                    .collect();

                if filter.negated {
                    format!("{} NOT IN ({})", col, placeholders.join(", "))
                } else {
                    format!("{} IN ({})", col, placeholders.join(", "))
                }
            }
            _ => {
                params.push(filter_value_to_sql(&filter.value));
                let placeholder = format!("${}", param_offset + params.len());
                let op = filter.op.to_sql();

                if filter.negated {
                    format!("NOT ({} {} {})", col, op, placeholder)
                } else {
                    format!("{} {} {}", col, op, placeholder)
                }
            }
        };

        Ok((expr, params))
    }

    /// Build ORDER BY clause with table alias prefix.
    fn build_order_aliased(&self, order: &[crate::query::OrderColumn], alias: &str) -> String {
        order
            .iter()
            .map(|o| {
                let mut part = format!("\"{}\".\"{}\" {}", alias, o.column, o.direction.as_sql());
                if let Some(nulls) = &o.nulls {
                    part.push(' ');
                    part.push_str(nulls.as_sql());
                }
                part
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Build the columns argument for jsonb_build_object in an embed.
    fn build_embed_columns_json(&self, columns: &[SelectColumn]) -> String {
        let mut parts = Vec::new();

        for col in columns {
            match col {
                SelectColumn::All => {
                    // For now, we'd need to enumerate - simplification: just use row_to_json
                    // This is a limitation - full implementation would need table info
                }
                SelectColumn::Column(name) => {
                    parts.push(format!("'{}', \"{}\"", name, name));
                }
                SelectColumn::Aliased { alias, column } => {
                    parts.push(format!("'{}', \"{}\"", alias, column));
                }
                SelectColumn::Embed(_) => {
                    // Nested embeds would require recursive lateral joins
                    // For now, skip them (they're handled at a higher level)
                }
            }
        }

        if parts.is_empty() {
            // If only * or embeds, use a simple identifier
            "'*', to_jsonb(*)".to_string()
        } else {
            parts.join(", ")
        }
    }

    /// Build a COUNT query.
    pub fn build_count(&self, plan: &QueryPlan) -> Result<(String, Vec<SqlValue>), DataError> {
        let mut sql = String::new();
        let mut params = Vec::new();

        sql.push_str("SELECT COUNT(*) as count FROM ");
        sql.push_str(&self.qualified_table());

        // WHERE clause (combine filters and policy predicate)
        let has_filters = !plan.filters.is_empty();
        let has_policy = plan.policy_predicate.is_some();

        if has_filters || has_policy {
            sql.push_str(" WHERE ");

            let mut where_parts = Vec::new();

            if has_filters {
                let (where_clause, where_params) = self.build_where(&plan.filters, params.len())?;
                where_parts.push(where_clause);
                params.extend(where_params);
            }

            if let Some(ref policy) = plan.policy_predicate {
                where_parts.push(format!("({})", policy));
            }

            sql.push_str(&where_parts.join(" AND "));
        }

        Ok((sql, params))
    }

    /// Build an INSERT query.
    pub fn build_insert(
        &self,
        rows: &[Value],
        table_info: &TableSnapshot,
        prefer: &Prefer,
        plan: &QueryPlan,
    ) -> Result<(String, Vec<SqlValue>), DataError> {
        if rows.is_empty() {
            return Err(DataError::InvalidQuery("no rows to insert".to_string()));
        }

        // Collect all columns from all rows
        let mut all_columns: Vec<String> = Vec::new();
        for row in rows {
            if let Value::Object(obj) = row {
                for key in obj.keys() {
                    if !all_columns.contains(key) {
                        // Validate column exists
                        if !table_info.columns.iter().any(|c| &c.name == key) {
                            return Err(DataError::ColumnNotFound(key.clone()));
                        }
                        all_columns.push(key.clone());
                    }
                }
            }
        }

        let mut sql = String::new();
        let mut params = Vec::new();

        // INSERT INTO table (cols)
        sql.push_str("INSERT INTO ");
        sql.push_str(&self.qualified_table());
        sql.push_str(" (");
        sql.push_str(
            &all_columns
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", "),
        );
        sql.push_str(") VALUES ");

        // VALUES
        let mut value_groups = Vec::new();
        for row in rows {
            if let Value::Object(obj) = row {
                let mut placeholders = Vec::new();
                for col in &all_columns {
                    params.push(json_to_sql_value(obj.get(col)));
                    placeholders.push(format!("${}", params.len()));
                }
                value_groups.push(format!("({})", placeholders.join(", ")));
            }
        }
        sql.push_str(&value_groups.join(", "));

        // ON CONFLICT
        if prefer.resolution == Resolution::IgnoreDuplicates {
            sql.push_str(" ON CONFLICT DO NOTHING");
        } else if prefer.resolution == Resolution::MergeDuplicates
            && !table_info.primary_key.is_empty()
        {
            let pk_cols = table_info
                .primary_key
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", ");

            let update_cols: Vec<String> = all_columns
                .iter()
                .filter(|c| !table_info.primary_key.contains(c))
                .map(|c| format!("\"{}\" = EXCLUDED.\"{}\"", c, c))
                .collect();

            if !update_cols.is_empty() {
                sql.push_str(&format!(
                    " ON CONFLICT ({}) DO UPDATE SET {}",
                    pk_cols,
                    update_cols.join(", ")
                ));
            }
        }

        // RETURNING
        if prefer.return_mode == ReturnMode::Representation {
            sql.push_str(" RETURNING ");
            sql.push_str(&self.build_select_columns(&plan.select));
        }

        Ok((sql, params))
    }

    /// Build an UPDATE query.
    pub fn build_update(
        &self,
        updates: &Map<String, Value>,
        table_info: &TableSnapshot,
        prefer: &Prefer,
        plan: &QueryPlan,
    ) -> Result<(String, Vec<SqlValue>), DataError> {
        let mut sql = String::new();
        let mut params = Vec::new();

        // UPDATE table SET
        sql.push_str("UPDATE ");
        sql.push_str(&self.qualified_table());
        sql.push_str(" SET ");

        // Build SET clause
        let mut set_parts = Vec::new();
        for (col, val) in updates {
            // Validate column exists
            if !table_info.columns.iter().any(|c| &c.name == col) {
                return Err(DataError::ColumnNotFound(col.clone()));
            }
            params.push(json_to_sql_value(Some(val)));
            set_parts.push(format!("\"{}\" = ${}", col, params.len()));
        }
        sql.push_str(&set_parts.join(", "));

        // WHERE clause (combine filters and policy predicate)
        let has_filters = !plan.filters.is_empty();
        let has_policy = plan.policy_predicate.is_some();

        if has_filters || has_policy {
            sql.push_str(" WHERE ");

            let mut where_parts = Vec::new();

            if has_filters {
                let (where_clause, where_params) = self.build_where(&plan.filters, params.len())?;
                where_parts.push(where_clause);
                params.extend(where_params);
            }

            if let Some(ref policy) = plan.policy_predicate {
                where_parts.push(format!("({})", policy));
            }

            sql.push_str(&where_parts.join(" AND "));
        }

        // RETURNING
        if prefer.return_mode == ReturnMode::Representation {
            sql.push_str(" RETURNING ");
            sql.push_str(&self.build_select_columns(&plan.select));
        }

        Ok((sql, params))
    }

    /// Build a DELETE query.
    pub fn build_delete(
        &self,
        prefer: &Prefer,
        plan: &QueryPlan,
    ) -> Result<(String, Vec<SqlValue>), DataError> {
        let mut sql = String::new();
        let mut params = Vec::new();

        // DELETE FROM table
        sql.push_str("DELETE FROM ");
        sql.push_str(&self.qualified_table());

        // WHERE clause (combine filters and policy predicate)
        let has_filters = !plan.filters.is_empty();
        let has_policy = plan.policy_predicate.is_some();

        if has_filters || has_policy {
            sql.push_str(" WHERE ");

            let mut where_parts = Vec::new();

            if has_filters {
                let (where_clause, where_params) = self.build_where(&plan.filters, params.len())?;
                where_parts.push(where_clause);
                params.extend(where_params);
            }

            if let Some(ref policy) = plan.policy_predicate {
                where_parts.push(format!("({})", policy));
            }

            sql.push_str(&where_parts.join(" AND "));
        }

        // RETURNING
        if prefer.return_mode == ReturnMode::Representation {
            sql.push_str(" RETURNING ");
            sql.push_str(&self.build_select_columns(&plan.select));
        }

        Ok((sql, params))
    }

    /// Build select columns.
    fn build_select_columns(&self, columns: &[SelectColumn]) -> String {
        if columns.is_empty() || columns.iter().any(|c| matches!(c, SelectColumn::All)) {
            return "*".to_string();
        }

        // Filter out embeds for now - they're handled separately via LEFT JOIN LATERAL
        let non_embed_columns: Vec<_> = columns
            .iter()
            .filter(|c| !matches!(c, SelectColumn::Embed(_)))
            .collect();

        if non_embed_columns.is_empty() {
            // Only embeds selected, still need a base column
            return "*".to_string();
        }

        non_embed_columns
            .iter()
            .map(|c| match c {
                SelectColumn::All => "*".to_string(),
                SelectColumn::Column(name) => format!("\"{}\"", name),
                SelectColumn::Aliased { alias, column } => {
                    format!("\"{}\" AS \"{}\"", column, alias)
                }
                SelectColumn::Embed(_) => unreachable!("filtered above"),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Build WHERE clause from filters.
    fn build_where(
        &self,
        filters: &[FilterExpr],
        param_offset: usize,
    ) -> Result<(String, Vec<SqlValue>), DataError> {
        let mut parts = Vec::new();
        let mut params = Vec::new();

        for filter in filters {
            let (expr, filter_params) =
                self.build_filter_expr(filter, param_offset + params.len())?;
            parts.push(expr);
            params.extend(filter_params);
        }

        Ok((parts.join(" AND "), params))
    }

    /// Build a single filter expression.
    fn build_filter_expr(
        &self,
        filter: &FilterExpr,
        param_offset: usize,
    ) -> Result<(String, Vec<SqlValue>), DataError> {
        let col = format!("\"{}\"", filter.column);
        let mut params = Vec::new();

        let expr = match filter.op {
            FilterOp::Is => {
                let value = match &filter.value {
                    FilterValue::Null => "NULL",
                    FilterValue::Bool(true) => "TRUE",
                    FilterValue::Bool(false) => "FALSE",
                    _ => {
                        return Err(DataError::InvalidFilter(
                            "IS requires null/true/false".to_string(),
                        ))
                    }
                };
                if filter.negated {
                    format!("{} IS NOT {}", col, value)
                } else {
                    format!("{} IS {}", col, value)
                }
            }
            FilterOp::In => {
                let list = match &filter.value {
                    FilterValue::List(items) => items,
                    _ => return Err(DataError::InvalidFilter("IN requires a list".to_string())),
                };

                let placeholders: Vec<String> = list
                    .iter()
                    .map(|v| {
                        params.push(filter_value_to_sql(v));
                        format!("${}", param_offset + params.len())
                    })
                    .collect();

                if filter.negated {
                    format!("{} NOT IN ({})", col, placeholders.join(", "))
                } else {
                    format!("{} IN ({})", col, placeholders.join(", "))
                }
            }
            _ => {
                params.push(filter_value_to_sql(&filter.value));
                let placeholder = format!("${}", param_offset + params.len());
                let op = filter.op.to_sql();

                if filter.negated {
                    format!("NOT ({} {} {})", col, op, placeholder)
                } else {
                    format!("{} {} {}", col, op, placeholder)
                }
            }
        };

        Ok((expr, params))
    }

    /// Build ORDER BY clause.
    fn build_order(&self, order: &[crate::query::OrderColumn]) -> String {
        order
            .iter()
            .map(|o| {
                let mut part = format!("\"{}\" {}", o.column, o.direction.as_sql());
                if let Some(nulls) = &o.nulls {
                    part.push(' ');
                    part.push_str(nulls.as_sql());
                }
                part
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Convert JSON value to SQL value.
fn json_to_sql_value(value: Option<&Value>) -> SqlValue {
    match value {
        None | Some(Value::Null) => SqlValue::Null,
        Some(Value::Bool(b)) => SqlValue::Bool(*b),
        Some(Value::Number(n)) => {
            if let Some(i) = n.as_i64() {
                SqlValue::Int(i)
            } else if let Some(f) = n.as_f64() {
                SqlValue::Float(f)
            } else {
                SqlValue::Text(n.to_string())
            }
        }
        Some(Value::String(s)) => {
            // Try to parse as UUID
            if let Ok(uuid) = s.parse::<uuid::Uuid>() {
                SqlValue::Uuid(uuid)
            } else {
                SqlValue::Text(s.clone())
            }
        }
        Some(Value::Array(_)) | Some(Value::Object(_)) => SqlValue::Json(value.cloned().unwrap()),
    }
}

/// Convert FilterValue to SqlValue.
fn filter_value_to_sql(value: &FilterValue) -> SqlValue {
    match value {
        FilterValue::Null => SqlValue::Null,
        FilterValue::Bool(b) => SqlValue::Bool(*b),
        FilterValue::Int(i) => SqlValue::Int(*i),
        FilterValue::Float(f) => SqlValue::Float(*f),
        FilterValue::String(s) => {
            // Try to parse as UUID
            if let Ok(uuid) = s.parse::<uuid::Uuid>() {
                SqlValue::Uuid(uuid)
            } else {
                SqlValue::Text(s.clone())
            }
        }
        FilterValue::List(_) => {
            // Lists are handled specially in build_filter_expr
            SqlValue::Null
        }
    }
}

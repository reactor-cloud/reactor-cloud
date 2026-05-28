//! Reactor Data capability.
//!
//! Provides a PostgREST-shaped HTTP surface over a relational store with:
//! - Portable SQL dialect (compiles to Postgres in v0, SQLite in v0.2)
//! - Rust-owned policy engine (no native Postgres RLS)
//! - Full PostgREST filter/embed/RPC semantics
//! - Integration with reactor-auth via AuthClient trait
//!
//! See `docs/reactor-data.design.md` for the full specification.

pub mod audit;
pub mod config;
pub mod dialect;
pub mod error;
pub mod execute;
pub mod middleware;
pub mod migrate;
pub mod policy;
pub mod query;
pub mod realtime;
pub mod router;
pub mod routes;
pub mod rpc;
pub mod state;
pub mod store;

pub use audit::{write_audit_event, AuditEvent, AuditEventType};
pub use config::{DataConfig, Deployment};
pub use dialect::{
    emit_postgres, lint_statement, parse_migration, LintError, Migration, ParseError,
};
pub use error::DataError;
pub use execute::embed::{
    build_embed_lateral, resolve_embeds, EmbedDirection, ResolvedEmbed,
};
pub use execute::{execute_delete, execute_insert, execute_select, execute_update, QueryResult};
pub use middleware::{auth_middleware, DataCtx};
pub use migrate::{FilesystemSource, MigrationError, MigrationRunner, MigrationSource};
pub use policy::{
    check_row, check_rows_batch, compile_for_scope, evaluate, parse_policy_expr, BatchCheckResult,
    EvalResult, PolicyDecision, PolicyExpr, PolicyParseError, PolicyStore, StoredPolicy,
};
pub use query::{
    parse_query, EmbedSpec, FilterExpr, FilterOp, Pagination, Prefer, QueryPlan, SelectColumn,
};
pub use realtime::{
    build_topic, in_process_realtime, DataChangeEvent, DataChangeOp, InProcessRealtime,
    RealtimeBackend, RealtimeError, RealtimeSubscription,
};
pub use router::router;
pub use rpc::{execute_rpc, RpcFunction, RpcParam, RpcStore, SecurityMode};
pub use state::DataState;
pub use store::{DataStore, DataTx, PgDataStore, SchemaSnapshot};

use utoipa::OpenApi;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// OpenAPI documentation for the data service.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Reactor Data API",
        version = "1.0.0",
        description = "PostgREST-style API for database access"
    ),
    paths(
        routes::crud::get_table,
        routes::crud::post_table,
        routes::crud::patch_table,
        routes::crud::delete_table,
        routes::rpc::post_rpc,
        routes::admin::generate_typescript,
        routes::health::health,
    ),
    components(schemas(
        routes::health::HealthResponse,
    )),
    tags(
        (name = "data", description = "CRUD operations on tables"),
        (name = "data.rpc", description = "RPC function invocation"),
        (name = "data.admin", description = "Admin operations"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::Http::new(
                        utoipa::openapi::security::HttpAuthScheme::Bearer,
                    ),
                ),
            );
        }
    }
}

/// Returns the OpenAPI specification for the data service.
pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

//! Operations control surface composition.
//!
//! Mounts the `/_ops/v1/*` routes and configures the ops middleware stack.
//! This is the secure, authenticated, audited control plane for platform operators.
//!
//! Two router layers are composed:
//!
//! 1. **`reactor_ops::router(state)`** — the ops crate's own routes
//!    (operators bootstrap/status/promote, doctor, version, audit, vault).
//! 2. **Server-side delegating routes** — the routes that need access to
//!    `CloudApiState` (`/_ops/v1/projects/*`) and the existing admin deploy
//!    handler (`/_ops/v1/deployments`). These live in `reactor-server` because
//!    `CloudApiState` cannot be referenced from `reactor-ops` without a
//!    circular crate dependency.
//!
//! Both layers share the same middleware stack (network → identity → scope →
//! step-up → audit) so callers get one consistent control surface at
//! `/_ops/v1/*` regardless of which crate the handler ultimately lives in.

use crate::admin::AdminAuthState;
use crate::compose::cloud::CloudApiState;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::StatusCode,
    middleware::from_fn_with_state,
    response::IntoResponse,
    routing::{delete, get, post},
    Extension, Json, Router,
};
use reactor_core::auth::AuthClient;
use reactor_ops::{
    audit::AuditLogger,
    middleware::{audit_log, identity_check, network_check, scope_check, step_up_check, OpsMiddlewareState, RouteMeta},
    OpsConfig, OpsState,
};
use sqlx::PgPool;
use std::sync::Arc;

/// Build the full ops router.
///
/// Composes `reactor_ops::router` with server-side delegating routes for
/// `/_ops/v1/projects/*` and `/_ops/v1/deployments`, all under the same
/// network/identity/scope/step-up/audit middleware stack.
///
/// # Arguments
///
/// * `pool` - Database connection pool (shared across ops and cloud)
/// * `auth` - Auth client for identity verification
/// * `config` - Ops configuration (trusted networks, step-up window, etc.)
/// * `cloud` - Optional [`CloudApiState`] to back `/_ops/v1/projects/*`
/// * `admin_state` - Optional [`AdminAuthState`] to back `/_ops/v1/deployments`
pub fn build_router(
    pool: PgPool,
    auth: Arc<dyn AuthClient>,
    config: OpsConfig,
    cloud: Option<CloudApiState>,
    admin_state: Option<AdminAuthState>,
) -> Router {
    let state = OpsState::new(auth.clone(), pool.clone(), config.clone());
    let base = reactor_ops::router(state);

    // The server-side delegating routes (projects + deployments) need access
    // to capability state that lives in `reactor-server` itself. We build them
    // here and merge into the same `/_ops/v1` namespace.
    let extras = build_delegating_router(pool, auth, Arc::new(config), cloud, admin_state);

    base.merge(extras)
}

/// Build the server-side delegating ops routes (projects + deployments).
///
/// Mounted at `/_ops/v1/*` and wrapped with the same middleware stack as the
/// [`reactor_ops`] router so authentication, scope enforcement, and audit
/// logging behave identically across the two layers.
fn build_delegating_router(
    pool: PgPool,
    auth: Arc<dyn AuthClient>,
    config: Arc<OpsConfig>,
    cloud: Option<CloudApiState>,
    admin_state: Option<AdminAuthState>,
) -> Router {
    let mut api = Router::new();

    // ----- Projects routes (delegate to CloudApiState) -----
    if let Some(cloud_state) = cloud {
        let projects = Router::new()
            .route(
                "/projects",
                post(ops_create_project)
                    .layer(Extension(RouteMeta::new("cloud:projects:write", "projects.create"))),
            )
            .route(
                "/projects",
                get(ops_list_projects)
                    .layer(Extension(RouteMeta::new("cloud:projects:read", "projects.list"))),
            )
            .route(
                "/projects/:project_ref",
                get(ops_get_project)
                    .layer(Extension(RouteMeta::new("cloud:projects:read", "projects.get"))),
            )
            .route(
                "/projects/:project_ref",
                delete(ops_delete_project)
                    .layer(Extension(RouteMeta::new("cloud:projects:delete", "projects.delete"))),
            )
            .with_state(cloud_state);

        api = api.merge(projects);
    }

    // ----- Deployment route (delegates to existing admin deploy handler) -----
    if let Some(admin) = admin_state {
        // Deploy bundles are tar.zst files containing the full site (Next.js
        // standalone bundles can easily run 20-50 MB) plus any function
        // .fnpkg.zip blobs. Mirror the 256 MB cap that the legacy
        // `/_admin/deploy` route allows.
        let deploy = Router::new()
            .route(
                "/deployments",
                post(ops_deploy)
                    .layer(Extension(RouteMeta::new("ops:deploy", "deployments.create"))),
            )
            .layer(DefaultBodyLimit::max(256 * 1024 * 1024))
            .with_state(admin);

        api = api.merge(deploy);
    }

    // Apply the same middleware stack as `reactor_ops::router` so the two
    // layers behave identically. Order matters: middleware run in reverse of
    // declaration, so `network_check` runs first.
    let audit = AuditLogger::new(pool);
    let middleware_state = OpsMiddlewareState {
        auth: auth.clone(),
        config: config.clone(),
        audit,
    };

    let api = api
        .layer(from_fn_with_state(middleware_state, audit_log))
        .layer(from_fn_with_state(config.clone(), step_up_check))
        .layer(from_fn_with_state((), scope_check))
        .layer(from_fn_with_state(auth, identity_check))
        .layer(from_fn_with_state(config, network_check));

    Router::new().nest("/_ops/v1", api)
}

// ============================================================================
// Project handlers (delegate to CloudApiState)
// ============================================================================

#[derive(Debug, serde::Deserialize)]
struct CreateProjectBody {
    name: String,
    region: Option<String>,
    /// If omitted, the operator is used as the owner (recorded in audit too).
    owner_user_id: Option<uuid::Uuid>,
}

async fn ops_create_project(
    State(state): State<CloudApiState>,
    ctx: reactor_ops::middleware::OpsAuthCtx,
    Json(req): Json<CreateProjectBody>,
) -> impl IntoResponse {
    // Treat both `null` and the all-zero UUID as "use the operator". The CLI
    // currently sends `Uuid::nil()` because it has no notion of a user yet;
    // the ops surface always knows who the operator is via the JWT, so use
    // that as the canonical owner.
    let owner = req
        .owner_user_id
        .filter(|u| !u.is_nil())
        .unwrap_or_else(|| ctx.user_id.into());
    let inner = reactor_cloud_api::CreateProjectRequest {
        name: req.name,
        region: req.region,
        owner_user_id: owner,
    };

    match state.projects().create(inner).await {
        Ok(result) => {
            let base_domain = state.base_domain().to_string();
            (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "ok": true,
                    "data": {
                        "project": project_json(&result.project, &base_domain),
                        "anon_key": result.anon_key,
                        "service_key": result.service_key,
                    }
                })),
            )
                .into_response()
        }
        Err(e) => cloud_error_response(e),
    }
}

#[derive(Debug, serde::Deserialize)]
struct ListProjectsQuery {
    owner: Option<uuid::Uuid>,
    #[serde(default = "default_list_limit")]
    limit: i32,
    #[serde(default)]
    offset: i32,
}

fn default_list_limit() -> i32 {
    50
}

async fn ops_list_projects(
    State(state): State<CloudApiState>,
    _ctx: reactor_ops::middleware::OpsAuthCtx,
    Query(q): Query<ListProjectsQuery>,
) -> impl IntoResponse {
    let projects = match q.owner {
        Some(owner) => state.projects().list_for_user(owner, q.limit, q.offset).await,
        None => state.projects().list_all(q.limit, q.offset).await,
    };

    match projects {
        Ok(items) => {
            let base_domain = state.base_domain().to_string();
            let data: Vec<_> = items.iter().map(|p| project_json(p, &base_domain)).collect();
            (StatusCode::OK, Json(serde_json::json!({ "ok": true, "data": data }))).into_response()
        }
        Err(e) => cloud_error_response(e),
    }
}

async fn ops_get_project(
    State(state): State<CloudApiState>,
    _ctx: reactor_ops::middleware::OpsAuthCtx,
    Path(project_ref): Path<String>,
) -> impl IntoResponse {
    match state.projects().get_by_ref(&project_ref).await {
        Ok(Some(p)) => {
            let base_domain = state.base_domain().to_string();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "ok": true, "data": project_json(&p, &base_domain) })),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "ok": false,
                "error": { "code": "not_found", "message": format!("project not found: {}", project_ref) }
            })),
        )
            .into_response(),
        Err(e) => cloud_error_response(e),
    }
}

async fn ops_delete_project(
    State(state): State<CloudApiState>,
    ctx: reactor_ops::middleware::OpsAuthCtx,
    Path(project_ref): Path<String>,
) -> impl IntoResponse {
    let actor = ctx.user_id.to_string();
    match state.projects().schedule_delete(&project_ref, &actor).await {
        Ok(p) => {
            let base_domain = state.base_domain().to_string();
            (
                StatusCode::OK,
                Json(serde_json::json!({ "ok": true, "data": project_json(&p, &base_domain) })),
            )
                .into_response()
        }
        Err(e) => cloud_error_response(e),
    }
}

fn project_json(p: &reactor_cloud_api::Project, base_domain: &str) -> serde_json::Value {
    serde_json::json!({
        "id": p.id,
        "ref": p.project_ref,
        "name": p.name,
        "owner_user_id": p.owner_user_id,
        "backend_kind": p.backend_kind,
        "status": p.status,
        "region": p.region,
        "hostname": p.hostname_for(base_domain),
        "created_at": p.created_at,
        "updated_at": p.updated_at,
    })
}

fn cloud_error_response(e: reactor_cloud_api::CloudError) -> axum::response::Response {
    use reactor_cloud_api::CloudError;
    let (status, code) = match &e {
        CloudError::ProjectNotFound(_) | CloudError::KeyNotFound(_) | CloudError::MemberNotFound { .. } => {
            (StatusCode::NOT_FOUND, "not_found")
        }
        CloudError::ProjectAlreadyExists(_) => (StatusCode::CONFLICT, "conflict"),
        CloudError::InvalidArgument(_) | CloudError::InvalidStatusTransition { .. } => {
            (StatusCode::BAD_REQUEST, "validation_error")
        }
        CloudError::PermissionDenied(_) => (StatusCode::FORBIDDEN, "permission_denied"),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
    };
    let message = e.to_string();
    (
        status,
        Json(serde_json::json!({
            "ok": false,
            "error": { "code": code, "message": message }
        })),
    )
        .into_response()
}

// ============================================================================
// Deployment handler (delegates to existing admin deploy_handler)
// ============================================================================

/// `POST /_ops/v1/deployments` — multipart bundle upload, gated by ops auth.
///
/// Reuses [`crate::admin::deploy::deploy_handler`] verbatim because the deploy
/// pipeline (data → storage → functions → jobs → sites) does not depend on the
/// admin token; it only needs access to the capability state already carried
/// by [`AdminAuthState`]. The static admin token is left in place purely as a
/// loopback break-glass on `/_admin/deploy`; production traffic uses this
/// route with a session JWT and the `ops:deploy` scope.
async fn ops_deploy(
    State(admin_state): State<AdminAuthState>,
    _ctx: reactor_ops::middleware::OpsAuthCtx,
    multipart: Multipart,
) -> axum::response::Response {
    crate::admin::deploy::deploy_handler(State(admin_state), multipart)
        .await
        .into_response()
}

/// Default ops configuration for shared clusters.
///
/// This is suitable for Fly.io deployments where the platform-level proxy
/// terminates TLS and forwards over the Fly internal network. Trusted
/// networks include loopback, Fly 6PN (`fdaa::/16`), and the Fly internal
/// IPv4 proxy range (`172.16.0.0/12`) used by the public HTTP proxy.
///
/// The network allowlist is defense-in-depth — the real security boundary
/// for `/_ops/v1/*` is the identity (JWT) + scope + step-up middleware.
pub fn default_shared_cluster_config() -> OpsConfig {
    OpsConfig {
        enabled: true,
        trusted_networks: vec![
            "127.0.0.0/8".to_string(),    // IPv4 loopback
            "::1/128".to_string(),        // IPv6 loopback
            "fdaa::/16".to_string(),      // Fly.io 6PN
            "172.16.0.0/12".to_string(),  // Fly.io internal IPv4 proxy NAT
        ],
        session_ttl_secs: 1800,           // 30 minutes
        step_up_window_secs: 300,         // 5 minutes
        require_step_up_for: vec![
            "ops:cluster_admin".to_string(),
            "vault:write".to_string(),
            "cloud:projects:delete".to_string(),
        ],
        audit_retention_days: 365,
    }
}

/// Ops configuration for single-node/local development.
///
/// This is more permissive for local development scenarios.
pub fn default_dev_config() -> OpsConfig {
    OpsConfig {
        enabled: true,
        trusted_networks: vec![
            "127.0.0.0/8".to_string(),
            "::1/128".to_string(),
        ],
        session_ttl_secs: 3600,           // 1 hour
        step_up_window_secs: 600,         // 10 minutes
        require_step_up_for: vec![],      // No step-up required in dev
        audit_retention_days: 30,
    }
}

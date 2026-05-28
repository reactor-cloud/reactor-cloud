//! Router configuration for the auth service.

use crate::middleware::OrgContextLayer;
use crate::routes;
use crate::routes::keys::KeysState;
use crate::state::AuthState;
use crate::store::PgIdentityStore;
use crate::webauthn::{routes as webauthn_routes, WebAuthnProvider, WebAuthnStore};
use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};

/// Create the auth service router.
///
/// All routes are prefixed with `/auth/v1`.
/// Internal routes are mounted at `/_internal`.
pub fn router(state: AuthState) -> Router {
    let service = state.service.clone();
    let keyring = state.keyring.clone();
    let public_url = state.config.public_url.to_string();

    // KeysState for the JWKS endpoint
    let keys_state = KeysState {
        keyring: keyring.clone(),
    };

    // ===== Public routes (no auth required) =====
    let public_routes = Router::new()
        .route("/health", get(routes::health))
        .route("/openapi.json", get(openapi_handler))
        .route(
            "/signup",
            post(routes::signup::<PgIdentityStore>),
        )
        .route(
            "/token",
            post(routes::token::<PgIdentityStore>),
        )
        .route(
            "/authorize",
            get(routes::authorize::<PgIdentityStore>)
                .post(routes::authorize_submit::<PgIdentityStore>),
        )
        .route(
            "/invitations/accept",
            post(routes::accept_invitation::<PgIdentityStore>),
        )
        .route(
            "/verify",
            get(routes::verify_email::<PgIdentityStore>),
        )
        .route(
            "/verify/resend",
            post(routes::resend_verification::<PgIdentityStore>),
        )
        .route(
            "/login",
            post(routes::login::<PgIdentityStore>),
        )
        .route(
            "/password-reset/request",
            post(routes::request_password_reset::<PgIdentityStore>),
        )
        .route(
            "/password-reset/confirm",
            post(routes::confirm_password_reset::<PgIdentityStore>),
        )
        .with_state(service.clone());

    // Keys route with its own state
    let keys_routes = Router::new()
        .route("/keys", get(routes::jwks::<PgIdentityStore>))
        .with_state(keys_state);

    // OpenID configuration (stateless, just needs the public URL)
    let oidc_route = Router::new().route(
        "/.well-known/openid-configuration",
        get(move || routes::openid_configuration(public_url.clone())),
    );

    // ===== Authenticated routes (Bearer token required) =====
    
    // User routes
    let user_routes = Router::new()
        .route("/me", get(routes::get_me::<PgIdentityStore>))
        .route(
            "/user",
            get(routes::get_user::<PgIdentityStore>)
                .patch(routes::update_user::<PgIdentityStore>)
                .delete(routes::delete_user::<PgIdentityStore>),
        )
        .route("/logout", post(routes::logout::<PgIdentityStore>))
        .route(
            "/api-keys",
            get(routes::list_api_keys::<PgIdentityStore>)
                .post(routes::create_api_key::<PgIdentityStore>),
        )
        .route(
            "/api-keys/:id",
            delete(routes::revoke_api_key::<PgIdentityStore>),
        );

    // Permission routes (need OrgContext)
    let permission_routes = Router::new()
        .route(
            "/permissions",
            get(routes::get_permissions::<PgIdentityStore>)
                .post(routes::check_permissions::<PgIdentityStore>),
        )
        .layer(OrgContextLayer);

    // Org routes
    let org_routes = Router::new()
        .route(
            "/orgs",
            post(routes::create_org::<PgIdentityStore>)
                .get(routes::list_orgs::<PgIdentityStore>),
        )
        .route(
            "/orgs/:ref",
            get(routes::get_org::<PgIdentityStore>)
                .patch(routes::update_org::<PgIdentityStore>)
                .delete(routes::delete_org::<PgIdentityStore>),
        )
        .route(
            "/orgs/:ref/roles",
            get(routes::list_roles::<PgIdentityStore>),
        );

    // Member routes
    let member_routes = Router::new()
        .route(
            "/orgs/:ref/members",
            get(routes::list_members::<PgIdentityStore>),
        )
        .route(
            "/orgs/:ref/members/:user_id",
            get(routes::get_member::<PgIdentityStore>)
                .patch(routes::update_member::<PgIdentityStore>)
                .delete(routes::delete_member::<PgIdentityStore>),
        );

    // Invitation routes (authenticated)
    let invitation_routes = Router::new()
        .route(
            "/orgs/:ref/invitations",
            post(routes::create_invitation::<PgIdentityStore>)
                .get(routes::list_invitations::<PgIdentityStore>),
        )
        .route(
            "/orgs/:ref/invitations/:id",
            delete(routes::delete_invitation::<PgIdentityStore>),
        );

    // Operators routes (for platform ops bootstrap)
    let operators_routes = Router::new()
        .route(
            "/operators/status",
            get(routes::operators_status::<PgIdentityStore>),
        )
        .route(
            "/operators/bootstrap",
            post(routes::bootstrap_operator::<PgIdentityStore>),
        )
        .route(
            "/operators/promote",
            post(routes::promote_operator::<PgIdentityStore>),
        )
        .with_state(service.clone());

    // Combine all authenticated routes
    let authenticated_routes = Router::new()
        .merge(user_routes)
        .merge(permission_routes)
        .merge(org_routes)
        .merge(member_routes)
        .merge(invitation_routes)
        .merge(operators_routes)
        .with_state(service.clone());

    // WebAuthn routes (separate state)
    let rp_id = state.config.public_url.host_str().unwrap_or("localhost");
    let rp_origin = state.config.public_url.to_string();
    let rp_name = "Reactor";
    
    let webauthn_state = webauthn_routes::WebAuthnState {
        provider: WebAuthnProvider::new(rp_id, &rp_origin, rp_name)
            .expect("Failed to create WebAuthn provider"),
        store: WebAuthnStore::new(state.pool.clone()),
        auth_service: service.clone(),
    };

    let webauthn_router = Router::new()
        .route(
            "/webauthn/register/start",
            post(webauthn_routes::register_start::<PgIdentityStore>),
        )
        .route(
            "/webauthn/register/finish",
            post(webauthn_routes::register_finish::<PgIdentityStore>),
        )
        .route(
            "/webauthn/authenticate/start",
            post(webauthn_routes::authenticate_start::<PgIdentityStore>),
        )
        .route(
            "/webauthn/authenticate/finish",
            post(webauthn_routes::authenticate_finish::<PgIdentityStore>),
        )
        .route(
            "/webauthn/credentials",
            get(webauthn_routes::list_credentials::<PgIdentityStore>),
        )
        .route(
            "/webauthn/credentials/:id",
            delete(webauthn_routes::delete_credential::<PgIdentityStore>),
        )
        .with_state(webauthn_state);

    // ===== Internal routes (gated by X-Reactor-Internal-Secret) =====
    let internal_routes = build_internal_routes(&state);

    // Combine all routes under /auth/v1
    let api = Router::new()
        .merge(public_routes)
        .merge(keys_routes)
        .merge(oidc_route)
        .merge(authenticated_routes)
        .merge(webauthn_router);

    Router::new()
        .nest("/auth/v1", api)
        .nest("/_internal", internal_routes)
}

/// Build internal routes (resolve_ctx) gated by X-Reactor-Internal-Secret.
fn build_internal_routes(state: &AuthState) -> Router {
    let service = state.service.clone();
    let internal_secret = state.config.internal_secret.clone();

    // Only mount if internal_secret is configured
    if let Some(secret) = internal_secret {
        Router::new()
            .route(
                "/resolve_ctx",
                post(routes::resolve_ctx::<PgIdentityStore>),
            )
            .layer(OrgContextLayer)
            .layer(axum::middleware::from_fn(move |req, next| {
                internal_secret_middleware(req, next, secret.clone())
            }))
            .with_state(service)
    } else {
        Router::new()
    }
}

/// Handler for the OpenAPI specification endpoint.
async fn openapi_handler() -> impl IntoResponse {
    let spec = crate::openapi();
    (StatusCode::OK, Json(spec))
}

/// Middleware to validate X-Reactor-Internal-Secret header.
async fn internal_secret_middleware(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
    expected_secret: String,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    let provided_secret = req
        .headers()
        .get("x-reactor-internal-secret")
        .and_then(|v| v.to_str().ok());

    match provided_secret {
        Some(secret) if secret == expected_secret => next.run(req).await,
        _ => (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({
                "error": "invalid or missing internal secret",
                "code": "unauthorized"
            })),
        )
            .into_response(),
    }
}

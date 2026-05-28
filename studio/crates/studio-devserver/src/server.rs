use axum::middleware;
use std::path::Path;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

use crate::{auth, discovery::DiscoveryInfo, routes, AppState};

pub struct DevServer {
    state: AppState,
    discovery: DiscoveryInfo,
}

impl DevServer {
    pub async fn start(workspace_path: Option<&Path>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let token = auth::generate_token();
        let state = AppState::new(token.clone());

        // Bind to random available port
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let port = addr.port();

        state.set_port(port).await;

        // Create discovery info
        let discovery = DiscoveryInfo::new(port, token);

        // Write discovery file if workspace is provided
        if let Some(ws_path) = workspace_path {
            discovery.write_to_workspace(ws_path).await?;
        }

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        // Create protected router with auth middleware
        let protected_router = routes::create_protected_router(state.clone())
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth::auth_middleware,
            ));

        // Create public router (health check doesn't need auth)
        let public_router = routes::create_public_router(state.clone());

        // Merge routers
        let app = public_router
            .merge(protected_router)
            .layer(cors);

        // Print to stdout so it's visible even without tracing subscriber
        eprintln!("[studio-devserver] Starting on http://127.0.0.1:{}", port);
        eprintln!("[studio-devserver] Token: {}", &discovery.token);
        tracing::info!("DevServer starting on http://127.0.0.1:{}", port);

        // Spawn the server
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("DevServer error: {}", e);
            }
        });

        Ok(Self { state, discovery })
    }

    #[cfg(feature = "tauri-integration")]
    pub async fn start_with_app_handle(
        workspace_path: Option<&Path>,
        app_handle: tauri::AppHandle,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let token = auth::generate_token();
        let state = AppState::new(token.clone()).with_app_handle(app_handle);

        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let port = addr.port();

        state.set_port(port).await;

        let discovery = DiscoveryInfo::new(port, token);

        if let Some(ws_path) = workspace_path {
            discovery.write_to_workspace(ws_path).await?;
        }

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        // Create protected router with auth middleware
        let protected_router = routes::create_protected_router(state.clone())
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth::auth_middleware,
            ));

        // Create public router (health check doesn't need auth)
        let public_router = routes::create_public_router(state.clone());

        // Merge routers
        let app = public_router
            .merge(protected_router)
            .layer(cors);

        // Print to stdout so it's visible even without tracing subscriber
        eprintln!("[studio-devserver] Starting on http://127.0.0.1:{}", port);
        eprintln!("[studio-devserver] Token: {}", &discovery.token);
        tracing::info!("DevServer starting on http://127.0.0.1:{}", port);

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("DevServer error: {}", e);
            }
        });

        Ok(Self { state, discovery })
    }

    pub fn discovery(&self) -> &DiscoveryInfo {
        &self.discovery
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn port(&self) -> u16 {
        self.discovery.port
    }

    pub fn base_url(&self) -> String {
        self.discovery.base_url()
    }
}

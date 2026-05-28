pub mod agent;
pub mod lifecycle;
pub mod logs;
pub mod views;
pub mod workspace;

use axum::Router;
use crate::AppState;

pub fn create_public_router(state: AppState) -> Router {
    Router::new()
        .merge(lifecycle::public_routes())
        .with_state(state)
}

pub fn create_protected_router(state: AppState) -> Router {
    Router::new()
        .merge(lifecycle::protected_routes())
        .merge(workspace::routes())
        .merge(agent::routes())
        .merge(views::routes())
        .merge(logs::routes())
        .with_state(state)
}

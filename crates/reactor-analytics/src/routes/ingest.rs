//! Event ingestion endpoints.

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::error::AnalyticsError;
use crate::ingest::{
    batch::to_stored_event,
    enrich::Enricher,
    system_events::{canonicalize_alias, canonicalize_identify},
    validate, BatchItem, EnrichmentResult, IngestEvent,
};
use crate::state::{AnalyticsCtx, AnalyticsState, ConsentState};
use crate::store::AnalyticsStore;

// ---------------------- Request/Response types ----------------------

/// Track single event request.
#[derive(Debug, Deserialize)]
pub struct TrackRequest {
    #[serde(flatten)]
    pub event: IngestEvent,
}

/// Track response.
#[derive(Debug, Serialize)]
pub struct TrackResponse {
    pub accepted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Batch track request.
#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    pub events: Vec<IngestEvent>,
}

/// Batch track response.
#[derive(Debug, Serialize)]
pub struct BatchResponse {
    pub accepted: usize,
    pub rejected: Vec<RejectedItem>,
}

/// Rejected item in batch response.
#[derive(Debug, Serialize)]
pub struct RejectedItem {
    pub index: usize,
    pub code: String,
    pub message: String,
}

// ---------------------- Handlers ----------------------

/// Track a single event.
///
/// POST /analytics/v1/track
pub async fn track<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Json(req): Json<TrackRequest>,
) -> Result<impl IntoResponse, AnalyticsError> {
    // Check DNT / consent
    if ctx.dnt && state.config.honor_dnt {
        return Ok((StatusCode::NO_CONTENT, ()).into_response());
    }

    if ctx.consent == ConsentState::Denied {
        return Ok((StatusCode::NO_CONTENT, ()).into_response());
    }

    // Check tombstone (opted-out user)
    if let Some(ref anon_id) = req.event.anonymous_id {
        if state
            .store
            .is_tombstoned(ctx.project_id, anon_id)
            .await
            .unwrap_or(false)
        {
            return Ok((StatusCode::NO_CONTENT, ()).into_response());
        }
    }

    // Validate event
    let validation = validate::validate_event(&req.event, &state.config, ctx.is_anonymous());
    if !validation.valid {
        let response = TrackResponse {
            accepted: false,
            error: validation.error_message,
            code: validation.error_code,
        };
        return Ok((StatusCode::BAD_REQUEST, Json(response)).into_response());
    }

    // Enrich event
    let enricher = Enricher::new(state.config.clone());
    let mut event = req.event;
    let enrichment = enricher.enrich(&mut event, ctx.client_ip.as_deref(), ctx.user_agent.as_deref());

    // Strip denied properties
    enricher.strip_denied_properties(&mut event);

    // Convert to stored event
    let stored = to_stored_event(
        event,
        ctx.org_id.into(),
        ctx.project_id,
        &enrichment,
    );

    // Send to batcher
    match state.batcher_tx.try_send(BatchItem { event: stored }) {
        Ok(_) => {
            let response = TrackResponse {
                accepted: true,
                error: None,
                code: None,
            };
            Ok((StatusCode::NO_CONTENT, ()).into_response())
        }
        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
            // Queue full, return 202 (backpressure)
            tracing::warn!("batcher queue full, applying backpressure");
            metrics::counter!("analytics_backpressure_total").increment(1);
            Ok((StatusCode::ACCEPTED, ()).into_response())
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
            tracing::error!("batcher channel closed");
            Err(AnalyticsError::Internal("batcher unavailable".to_string()))
        }
    }
}

/// Track a batch of events.
///
/// POST /analytics/v1/batch
pub async fn batch<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Json(req): Json<BatchRequest>,
) -> Result<impl IntoResponse, AnalyticsError> {
    // Check DNT / consent
    if ctx.dnt && state.config.honor_dnt {
        return Ok((StatusCode::NO_CONTENT, ()).into_response());
    }

    if ctx.consent == ConsentState::Denied {
        return Ok((StatusCode::NO_CONTENT, ()).into_response());
    }

    // Collect unique anonymous IDs to check for tombstones
    let mut tombstoned: std::collections::HashSet<String> = std::collections::HashSet::new();
    for event in &req.events {
        if let Some(ref anon_id) = event.anonymous_id {
            if state
                .store
                .is_tombstoned(ctx.project_id, anon_id)
                .await
                .unwrap_or(false)
            {
                tombstoned.insert(anon_id.clone());
            }
        }
    }

    // Validate batch
    let validation = validate::validate_batch(&req.events, &state.config, ctx.is_anonymous());

    if !validation.batch_valid {
        return Err(validation.batch_error.unwrap_or_else(|| {
            AnalyticsError::BatchTooLarge {
                count: req.events.len(),
                size: 0,
            }
        }));
    }

    // Process valid events
    let enricher = Enricher::new(state.config.clone());
    let mut accepted = 0;
    let mut rejected: Vec<RejectedItem> = validation
        .rejected
        .into_iter()
        .map(|r| RejectedItem {
            index: r.index,
            code: r.code,
            message: r.message,
        })
        .collect();

    let mut queue_full = false;

    for idx in validation.valid {
        let mut event = req.events[idx].clone();

        // Skip tombstoned (opted-out) users
        if let Some(ref anon_id) = event.anonymous_id {
            if tombstoned.contains(anon_id) {
                continue;
            }
        }

        let enrichment = enricher.enrich(&mut event, ctx.client_ip.as_deref(), ctx.user_agent.as_deref());
        enricher.strip_denied_properties(&mut event);

        let stored = to_stored_event(
            event,
            ctx.org_id.into(),
            ctx.project_id,
            &enrichment,
        );

        match state.batcher_tx.try_send(BatchItem { event: stored }) {
            Ok(_) => {
                accepted += 1;
            }
            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                queue_full = true;
                rejected.push(RejectedItem {
                    index: idx,
                    code: "analytics.backpressure".to_string(),
                    message: "queue full, try again".to_string(),
                });
            }
            Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                tracing::error!("batcher channel closed");
                return Err(AnalyticsError::Internal("batcher unavailable".to_string()));
            }
        }
    }

    if queue_full {
        metrics::counter!("analytics_backpressure_total").increment(1);
    }

    let response = BatchResponse { accepted, rejected };

    // Use 202 if we hit backpressure, 200 otherwise
    let status = if queue_full {
        StatusCode::ACCEPTED
    } else {
        StatusCode::OK
    };

    Ok((status, Json(response)).into_response())
}

// ---------------------- Identify / Alias types ----------------------

/// Identify request.
#[derive(Debug, Deserialize)]
pub struct IdentifyRequest {
    /// Anonymous ID.
    pub anonymous_id: String,
    /// User ID to link.
    pub user_id: String,
    /// User traits (name, email, etc.).
    #[serde(default)]
    pub traits: serde_json::Value,
}

/// Identify response.
#[derive(Debug, Serialize)]
pub struct IdentifyResponse {
    pub success: bool,
}

/// Alias request.
#[derive(Debug, Deserialize)]
pub struct AliasRequest {
    /// Anonymous ID (from).
    pub anonymous_id: String,
    /// User ID (to).
    pub user_id: String,
}

/// Alias response.
#[derive(Debug, Serialize)]
pub struct AliasResponse {
    pub success: bool,
}

// ---------------------- Identify / Alias handlers ----------------------

/// Identify an anonymous user.
///
/// Links an anonymous_id to a user_id and stores traits.
///
/// POST /analytics/v1/identify
pub async fn identify<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Json(req): Json<IdentifyRequest>,
) -> Result<impl IntoResponse, AnalyticsError> {
    // Check DNT / consent
    if ctx.dnt && state.config.honor_dnt {
        return Ok((StatusCode::NO_CONTENT, ()).into_response());
    }

    if ctx.consent == ConsentState::Denied {
        return Ok((StatusCode::NO_CONTENT, ()).into_response());
    }

    // Upsert identity in store
    state
        .store
        .upsert_identity(
            ctx.org_id.into(),
            ctx.project_id,
            &req.anonymous_id,
            &req.user_id,
            &req.traits,
        )
        .await?;

    // Also emit a $identify event
    let enricher = Enricher::new(state.config.clone());
    let dummy_event = IngestEvent {
        event: "$identify".to_string(),
        anonymous_id: Some(req.anonymous_id.clone()),
        user_id: Some(req.user_id.clone()),
        session_id: None,
        timestamp: None,
        properties: req.traits.clone(),
        context: Default::default(),
    };

    let enrichment = enricher.enrich(
        &mut dummy_event.clone(),
        ctx.client_ip.as_deref(),
        ctx.user_agent.as_deref(),
    );

    let stored = canonicalize_identify(
        &dummy_event,
        ctx.org_id.into(),
        ctx.project_id,
        &req.user_id,
        &enrichment,
    );

    // Send to batcher (fire and forget)
    let _ = state.batcher_tx.try_send(BatchItem { event: stored });

    let response = IdentifyResponse { success: true };
    Ok((StatusCode::OK, Json(response)).into_response())
}

/// Alias an anonymous user to a user ID.
///
/// POST /analytics/v1/alias
pub async fn alias<S: AnalyticsStore + Clone>(
    State(state): State<AnalyticsState<S>>,
    Extension(ctx): Extension<AnalyticsCtx>,
    Json(req): Json<AliasRequest>,
) -> Result<impl IntoResponse, AnalyticsError> {
    // Check DNT / consent
    if ctx.dnt && state.config.honor_dnt {
        return Ok((StatusCode::NO_CONTENT, ()).into_response());
    }

    if ctx.consent == ConsentState::Denied {
        return Ok((StatusCode::NO_CONTENT, ()).into_response());
    }

    // Create alias in store
    state
        .store
        .alias(
            ctx.org_id.into(),
            ctx.project_id,
            &req.anonymous_id,
            &req.user_id,
        )
        .await?;

    // Also emit a $alias event
    let enricher = Enricher::new(state.config.clone());
    let dummy_event = IngestEvent {
        event: "$alias".to_string(),
        anonymous_id: Some(req.anonymous_id.clone()),
        user_id: Some(req.user_id.clone()),
        session_id: None,
        timestamp: None,
        properties: serde_json::json!({}),
        context: Default::default(),
    };

    let enrichment = enricher.enrich(
        &mut dummy_event.clone(),
        ctx.client_ip.as_deref(),
        ctx.user_agent.as_deref(),
    );

    let stored = canonicalize_alias(
        &dummy_event,
        ctx.org_id.into(),
        ctx.project_id,
        &req.anonymous_id,
        &req.user_id,
        &enrichment,
    );

    // Send to batcher (fire and forget)
    let _ = state.batcher_tx.try_send(BatchItem { event: stored });

    let response = AliasResponse { success: true };
    Ok((StatusCode::OK, Json(response)).into_response())
}

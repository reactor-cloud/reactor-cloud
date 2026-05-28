//! Analytics service state.

use crate::config::AnalyticsConfig;
use crate::ingest::BatchItem;
use crate::store::AnalyticsStore;
use reactor_core::auth::{AuthClient, AuthCtx};
use reactor_core::id::{OrgId, UserId};
use reactor_policy::PolicyEvalContext;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Analytics service state.
#[derive(Clone)]
pub struct AnalyticsState<S: AnalyticsStore = crate::store::PgAnalyticsStore> {
    /// Analytics store.
    pub store: Arc<S>,

    /// Analytics configuration.
    pub config: Arc<AnalyticsConfig>,

    /// Authentication client.
    pub auth: Arc<dyn AuthClient>,

    /// Channel sender for background event batching.
    pub batcher_tx: mpsc::Sender<BatchItem>,
}

impl<S: AnalyticsStore> AnalyticsState<S> {
    /// Create a new analytics state.
    pub fn new(
        store: Arc<S>,
        config: Arc<AnalyticsConfig>,
        auth: Arc<dyn AuthClient>,
        batcher_tx: mpsc::Sender<BatchItem>,
    ) -> Self {
        Self {
            store,
            config,
            auth,
            batcher_tx,
        }
    }
}

/// Ingestion mode.
#[derive(Debug, Clone)]
pub enum IngestMode {
    /// Anonymous ingestion via project key.
    Anonymous {
        /// Project key ID.
        project_key_id: Uuid,
        /// Sampling rate for this key.
        sampling_rate: f64,
        /// Allowed origins for CORS.
        allowed_origins: Option<Vec<String>>,
    },
    /// Authenticated ingestion via bearer token.
    Authenticated {
        /// Full auth context.
        auth: AuthCtx,
    },
}

/// Consent state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConsentState {
    /// Consent granted.
    Granted,
    /// Consent denied (opted out).
    Denied,
    /// Consent state unknown.
    #[default]
    Unknown,
}

/// Request context for analytics operations.
#[derive(Debug, Clone)]
pub struct AnalyticsCtx {
    /// Ingestion mode (anonymous or authenticated).
    pub mode: IngestMode,
    /// Project ID.
    pub project_id: Uuid,
    /// Organization ID (owning org of the project).
    pub org_id: OrgId,
    /// Request ID for tracing.
    pub request_id: Uuid,
    /// Client IP address (already truncated to /24).
    pub client_ip: Option<String>,
    /// Raw user agent string.
    pub user_agent: Option<String>,
    /// DNT or Sec-GPC header present.
    pub dnt: bool,
    /// Consent state.
    pub consent: ConsentState,
}

impl AnalyticsCtx {
    /// Create an anonymous analytics context.
    pub fn anonymous(
        project_id: Uuid,
        org_id: OrgId,
        project_key_id: Uuid,
        sampling_rate: f64,
        allowed_origins: Option<Vec<String>>,
        request_id: Uuid,
    ) -> Self {
        Self {
            mode: IngestMode::Anonymous {
                project_key_id,
                sampling_rate,
                allowed_origins,
            },
            project_id,
            org_id,
            request_id,
            client_ip: None,
            user_agent: None,
            dnt: false,
            consent: ConsentState::Unknown,
        }
    }

    /// Create an authenticated analytics context.
    pub fn authenticated(auth: AuthCtx, project_id: Uuid, org_id: OrgId, request_id: Uuid) -> Self {
        Self {
            mode: IngestMode::Authenticated { auth },
            project_id,
            org_id,
            request_id,
            client_ip: None,
            user_agent: None,
            dnt: false,
            consent: ConsentState::Unknown,
        }
    }

    /// Check if this is an anonymous context.
    pub fn is_anonymous(&self) -> bool {
        matches!(self.mode, IngestMode::Anonymous { .. })
    }

    /// Check if this is an authenticated context.
    pub fn is_authenticated(&self) -> bool {
        matches!(self.mode, IngestMode::Authenticated { .. })
    }

    /// Get the user ID from an authenticated context.
    pub fn user_id(&self) -> Option<UserId> {
        match &self.mode {
            IngestMode::Authenticated { auth } => auth.user_id(),
            IngestMode::Anonymous { .. } => None,
        }
    }

    /// Check if user has a specific permission.
    pub fn has_permission(&self, permission: &str) -> bool {
        match &self.mode {
            IngestMode::Authenticated { auth } => auth.has_permission(permission),
            IngestMode::Anonymous { .. } => false,
        }
    }

    /// Get the sampling rate for anonymous mode.
    pub fn sampling_rate(&self) -> f64 {
        match &self.mode {
            IngestMode::Anonymous { sampling_rate, .. } => *sampling_rate,
            IngestMode::Authenticated { .. } => 1.0,
        }
    }

    /// Set the client IP (truncated).
    pub fn with_client_ip(mut self, ip: Option<String>) -> Self {
        self.client_ip = ip;
        self
    }

    /// Set the user agent.
    pub fn with_user_agent(mut self, ua: Option<String>) -> Self {
        self.user_agent = ua;
        self
    }

    /// Set the DNT flag.
    pub fn with_dnt(mut self, dnt: bool) -> Self {
        self.dnt = dnt;
        self
    }

    /// Set the consent state.
    pub fn with_consent(mut self, consent: ConsentState) -> Self {
        self.consent = consent;
        self
    }
}

impl PolicyEvalContext for AnalyticsCtx {
    fn user_id(&self) -> Option<UserId> {
        self.user_id()
    }

    fn org_id(&self) -> Option<OrgId> {
        Some(self.org_id)
    }

    fn has_permission(&self, permission: &str) -> bool {
        self.has_permission(permission)
    }

    fn email(&self) -> Option<&str> {
        match &self.mode {
            IngestMode::Authenticated { auth } => auth.claims.email.as_deref(),
            IngestMode::Anonymous { .. } => None,
        }
    }

    fn session_id(&self) -> Option<&str> {
        None
    }
}

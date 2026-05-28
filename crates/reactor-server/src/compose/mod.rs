//! Capability composition modules.
//!
//! Each capability has a composer that:
//! - Builds its state from shared resources + config
//! - Returns its axum router
//! - Registers background tasks
//!
//! The `ServerCapabilities` struct holds all composed capabilities and
//! provides methods for routing, background task management, and shutdown.

#[cfg(feature = "cap-ai")]
pub mod ai;
#[cfg(feature = "cap-analytics")]
pub mod analytics;
#[cfg(feature = "cap-auth")]
pub mod auth;
#[cfg(feature = "cap-cloud")]
pub mod cloud;
#[cfg(feature = "cap-connect")]
pub mod connect;
#[cfg(feature = "cap-data")]
pub mod data;
#[cfg(feature = "cap-functions")]
pub mod functions;
#[cfg(feature = "cap-jobs")]
pub mod jobs;
#[cfg(feature = "cap-ops")]
pub mod ops;
#[cfg(feature = "cap-sites")]
pub mod sites;
#[cfg(feature = "cap-storage")]
pub mod storage;

use crate::boot::SharedResources;
use crate::config::ReactorConfig;
use crate::error::ServerError;
use axum::Router;
use tokio::sync::watch;
use tokio::task::JoinHandle;

#[cfg(feature = "cap-auth")]
use crate::boot::AuthBundle;

/// Slot for a composed capability.
pub struct CapabilitySlot<S> {
    /// The capability state.
    pub state: S,

    /// The capability's router (to be merged into the main router).
    pub router: Router,

    /// Background task handles (if any).
    pub tasks: Vec<JoinHandle<()>>,
}

/// All composed capabilities for the server.
pub struct ServerCapabilities {
    /// AI capability slot.
    #[cfg(feature = "cap-ai")]
    pub ai: Option<CapabilitySlot<reactor_ai::AiState>>,

    /// Analytics capability slot.
    #[cfg(feature = "cap-analytics")]
    pub analytics: Option<CapabilitySlot<reactor_analytics::AnalyticsState<reactor_analytics::PgAnalyticsStore>>>,

    /// Auth capability slot.
    #[cfg(feature = "cap-auth")]
    pub auth: Option<CapabilitySlot<reactor_auth::AuthState>>,

    /// Data capability slot.
    #[cfg(feature = "cap-data")]
    pub data: Option<CapabilitySlot<reactor_data::DataState>>,

    /// Storage capability slot.
    #[cfg(feature = "cap-storage")]
    pub storage: Option<CapabilitySlot<reactor_storage::StorageState>>,

    /// Functions capability slot.
    #[cfg(feature = "cap-functions")]
    pub functions: Option<CapabilitySlot<reactor_functions::FunctionsState>>,

    /// Jobs capability slot.
    #[cfg(feature = "cap-jobs")]
    pub jobs: Option<CapabilitySlot<reactor_jobs::JobsState>>,

    /// Connect capability slot.
    #[cfg(feature = "cap-connect")]
    pub connect: Option<CapabilitySlot<reactor_connect::ConnectState<reactor_connect::PgConnectStore>>>,

    /// Sites capability slot.
    #[cfg(feature = "cap-sites")]
    pub sites: Option<CapabilitySlot<reactor_sites::SitesState>>,

    /// Sites serve router (used as fallback for Host-based routing).
    #[cfg(feature = "cap-sites")]
    pub sites_serve: Option<Router>,
}

impl ServerCapabilities {
    /// Build all capabilities from shared resources and config.
    #[allow(unused_variables)]
    pub async fn build(
        shared: &SharedResources,
        config: &ReactorConfig,
        #[cfg(feature = "cap-auth")] auth_bundle: AuthBundle,
    ) -> Result<Self, ServerError> {
        // Auth slot
        #[cfg(feature = "cap-auth")]
        let auth = if config.auth.is_some() {
            Some(auth::build(auth_bundle)?)
        } else {
            None
        };

        // Data slot
        #[cfg(feature = "cap-data")]
        let data = if let Some(ref data_config) = config.data {
            #[cfg(feature = "cap-auth")]
            let auth_client = auth
                .as_ref()
                .map(|a| a.state.service.clone())
                .map(|s| {
                    std::sync::Arc::new(reactor_auth::InProcessAuthClient::new(s))
                        as std::sync::Arc<dyn reactor_core::auth::AuthClient>
                })
                .ok_or_else(|| ServerError::Config("data requires auth".to_string()))?;

            #[cfg(feature = "cap-auth")]
            Some(data::build(shared, data_config, auth_client).await?)
        } else {
            None
        };

        // Storage slot
        #[cfg(feature = "cap-storage")]
        let storage = if let Some(ref storage_config) = config.storage {
            #[cfg(feature = "cap-auth")]
            let auth_client = auth
                .as_ref()
                .map(|a| a.state.service.clone())
                .map(|s| {
                    std::sync::Arc::new(reactor_auth::InProcessAuthClient::new(s))
                        as std::sync::Arc<dyn reactor_core::auth::AuthClient>
                })
                .ok_or_else(|| ServerError::Config("storage requires auth".to_string()))?;

            #[cfg(feature = "cap-auth")]
            Some(storage::build(shared, storage_config, config, auth_client).await?)
        } else {
            None
        };

        // Functions slot
        #[cfg(feature = "cap-functions")]
        let functions = if let Some(ref functions_config) = config.functions {
            #[cfg(feature = "cap-auth")]
            let auth_client = auth
                .as_ref()
                .map(|a| a.state.service.clone())
                .map(|s| {
                    std::sync::Arc::new(reactor_auth::InProcessAuthClient::new(s))
                        as std::sync::Arc<dyn reactor_core::auth::AuthClient>
                })
                .ok_or_else(|| ServerError::Config("functions requires auth".to_string()))?;

            #[cfg(feature = "cap-auth")]
            Some(functions::build(shared, functions_config, config, auth_client).await?)
        } else {
            None
        };

        // Jobs slot
        #[cfg(feature = "cap-jobs")]
        let jobs = if let Some(ref jobs_config) = config.jobs {
            #[cfg(feature = "cap-auth")]
            let auth_client = auth
                .as_ref()
                .map(|a| a.state.service.clone())
                .map(|s| {
                    std::sync::Arc::new(reactor_auth::InProcessAuthClient::new(s))
                        as std::sync::Arc<dyn reactor_core::auth::AuthClient>
                })
                .ok_or_else(|| ServerError::Config("jobs requires auth".to_string()))?;

            #[cfg(feature = "cap-auth")]
            Some(jobs::build(shared, jobs_config, config, auth_client).await?)
        } else {
            None
        };

        // Connect slot
        #[cfg(feature = "cap-connect")]
        let connect = if let Some(ref connect_config) = config.connect {
            #[cfg(feature = "cap-auth")]
            let auth_client = auth
                .as_ref()
                .map(|a| a.state.service.clone())
                .map(|s| {
                    std::sync::Arc::new(reactor_auth::InProcessAuthClient::new(s))
                        as std::sync::Arc<dyn reactor_core::auth::AuthClient>
                })
                .ok_or_else(|| ServerError::Config("connect requires auth".to_string()))?;

            #[cfg(feature = "cap-auth")]
            Some(connect::build(shared, connect_config, config, auth_client).await?)
        } else {
            None
        };

        // Sites slot
        #[cfg(feature = "cap-sites")]
        let (sites, sites_serve) = if let Some(ref sites_config) = config.sites {
            #[cfg(feature = "cap-auth")]
            let auth_client = auth
                .as_ref()
                .map(|a| a.state.service.clone())
                .map(|s| {
                    std::sync::Arc::new(reactor_auth::InProcessAuthClient::new(s))
                        as std::sync::Arc<dyn reactor_core::auth::AuthClient>
                })
                .ok_or_else(|| ServerError::Config("sites requires auth".to_string()))?;

            #[cfg(feature = "cap-auth")]
            let slot = sites::build(shared, sites_config, config, auth_client).await?;

            // Build the serve plane router for fallback
            let serve_router = reactor_sites::serve_router(slot.state.clone());

            (Some(slot), Some(serve_router))
        } else {
            (None, None)
        };

        // Analytics slot
        #[cfg(feature = "cap-analytics")]
        let analytics = if let Some(ref analytics_config) = config.analytics {
            #[cfg(feature = "cap-auth")]
            let auth_client = auth
                .as_ref()
                .map(|a| a.state.service.clone())
                .map(|s| {
                    std::sync::Arc::new(reactor_auth::InProcessAuthClient::new(s))
                        as std::sync::Arc<dyn reactor_core::auth::AuthClient>
                })
                .ok_or_else(|| ServerError::Config("analytics requires auth".to_string()))?;

            #[cfg(feature = "cap-auth")]
            Some(analytics::build(shared, analytics_config, config, auth_client).await?)
        } else {
            None
        };

        // AI slot
        #[cfg(feature = "cap-ai")]
        let ai = if let Some(ref ai_config) = config.ai {
            #[cfg(feature = "cap-auth")]
            let auth_client = auth
                .as_ref()
                .map(|a| a.state.service.clone())
                .map(|s| {
                    std::sync::Arc::new(reactor_auth::InProcessAuthClient::new(s))
                        as std::sync::Arc<dyn reactor_core::auth::AuthClient>
                })
                .ok_or_else(|| ServerError::Config("ai requires auth".to_string()))?;

            #[cfg(feature = "cap-auth")]
            Some(ai::build(shared, ai_config, config, auth_client).await?)
        } else {
            None
        };

        Ok(Self {
            #[cfg(feature = "cap-ai")]
            ai,
            #[cfg(feature = "cap-analytics")]
            analytics,
            #[cfg(feature = "cap-auth")]
            auth,
            #[cfg(feature = "cap-data")]
            data,
            #[cfg(feature = "cap-storage")]
            storage,
            #[cfg(feature = "cap-functions")]
            functions,
            #[cfg(feature = "cap-jobs")]
            jobs,
            #[cfg(feature = "cap-connect")]
            connect,
            #[cfg(feature = "cap-sites")]
            sites,
            #[cfg(feature = "cap-sites")]
            sites_serve,
        })
    }

    /// Build the merged router from all capability routers.
    ///
    /// If sites is enabled, the sites serve router is attached as a fallback
    /// service so that requests not matching any API prefix are routed to
    /// the sites serve handler based on the Host header.
    pub fn router(&self) -> Router {
        let mut router = Router::new();

        #[cfg(feature = "cap-ai")]
        if let Some(ref slot) = self.ai {
            router = router.merge(slot.router.clone());
        }

        #[cfg(feature = "cap-analytics")]
        if let Some(ref slot) = self.analytics {
            router = router.merge(slot.router.clone());
        }

        #[cfg(feature = "cap-auth")]
        if let Some(ref slot) = self.auth {
            router = router.merge(slot.router.clone());
        }

        #[cfg(feature = "cap-data")]
        if let Some(ref slot) = self.data {
            router = router.merge(slot.router.clone());
        }

        #[cfg(feature = "cap-storage")]
        if let Some(ref slot) = self.storage {
            router = router.merge(slot.router.clone());
        }

        #[cfg(feature = "cap-functions")]
        if let Some(ref slot) = self.functions {
            router = router.merge(slot.router.clone());
        }

        #[cfg(feature = "cap-jobs")]
        if let Some(ref slot) = self.jobs {
            router = router.merge(slot.router.clone());
        }

        #[cfg(feature = "cap-connect")]
        if let Some(ref slot) = self.connect {
            router = router.merge(slot.router.clone());
        }

        #[cfg(feature = "cap-sites")]
        if let Some(ref slot) = self.sites {
            // Merge the sites admin router (prefixed with /sites/v1)
            router = router.merge(slot.router.clone());
        }

        // Add sites serve router as fallback (Host-based routing for site serving)
        // This catches any request not matched by API prefixes
        #[cfg(feature = "cap-sites")]
        if let Some(ref serve_router) = self.sites_serve {
            router = router.fallback_service(serve_router.clone());
        }

        router
    }

    /// Spawn all background tasks.
    ///
    /// Returns the handles so they can be awaited on shutdown.
    pub fn spawn_background(&mut self, _shutdown: watch::Receiver<bool>) -> Vec<JoinHandle<()>> {
        let mut handles = Vec::new();

        // Collect all task handles from slots
        #[cfg(feature = "cap-ai")]
        if let Some(ref mut slot) = self.ai {
            handles.append(&mut slot.tasks);
        }

        #[cfg(feature = "cap-analytics")]
        if let Some(ref mut slot) = self.analytics {
            handles.append(&mut slot.tasks);
        }

        #[cfg(feature = "cap-auth")]
        if let Some(ref mut slot) = self.auth {
            handles.append(&mut slot.tasks);
        }

        #[cfg(feature = "cap-data")]
        if let Some(ref mut slot) = self.data {
            handles.append(&mut slot.tasks);
        }

        #[cfg(feature = "cap-storage")]
        if let Some(ref mut slot) = self.storage {
            handles.append(&mut slot.tasks);
        }

        #[cfg(feature = "cap-functions")]
        if let Some(ref mut slot) = self.functions {
            handles.append(&mut slot.tasks);
        }

        #[cfg(feature = "cap-jobs")]
        if let Some(ref mut slot) = self.jobs {
            handles.append(&mut slot.tasks);
        }

        #[cfg(feature = "cap-connect")]
        if let Some(ref mut slot) = self.connect {
            handles.append(&mut slot.tasks);
        }

        #[cfg(feature = "cap-sites")]
        if let Some(ref mut slot) = self.sites {
            handles.append(&mut slot.tasks);
        }

        handles
    }

    /// Wait for all background tasks to complete (with timeout).
    pub async fn join_background(
        mut self,
        timeout: std::time::Duration,
    ) -> Result<(), ServerError> {
        let handles = self.spawn_background(watch::channel(true).1);

        if handles.is_empty() {
            return Ok(());
        }

        let join_all = futures::future::join_all(handles);
        match tokio::time::timeout(timeout, join_all).await {
            Ok(results) => {
                for result in results {
                    if let Err(e) = result {
                        tracing::warn!(error = %e, "background task failed");
                    }
                }
                Ok(())
            }
            Err(_) => {
                tracing::warn!(
                    timeout_secs = timeout.as_secs(),
                    "background tasks did not complete within timeout"
                );
                Err(ServerError::Shutdown(
                    "background tasks did not complete".to_string(),
                ))
            }
        }
    }
}

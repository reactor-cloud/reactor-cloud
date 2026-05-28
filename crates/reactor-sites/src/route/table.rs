//! Route table management.

use super::decision::RouteResolver;
use crate::dispatch::RouteDecision;
use crate::store::{DeploymentRoute, SiteDeploymentId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Route table cache for active deployments.
pub struct RouteTable {
    tables: RwLock<HashMap<SiteDeploymentId, Arc<RouteResolver>>>,
}

impl RouteTable {
    /// Create a new route table.
    pub fn new() -> Self {
        Self {
            tables: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create a route resolver for a deployment.
    pub async fn get_or_create(
        &self,
        deployment_id: &SiteDeploymentId,
        routes: &[DeploymentRoute],
        function_map: HashMap<String, Uuid>,
    ) -> Result<Arc<RouteResolver>, matchit::InsertError> {
        {
            let tables = self.tables.read().await;
            if let Some(resolver) = tables.get(deployment_id) {
                return Ok(resolver.clone());
            }
        }

        let resolver = Arc::new(RouteResolver::from_routes(routes, function_map)?);

        {
            let mut tables = self.tables.write().await;
            tables.insert(*deployment_id, resolver.clone());
        }

        Ok(resolver)
    }

    /// Invalidate a cached route table.
    pub async fn invalidate(&self, deployment_id: &SiteDeploymentId) {
        let mut tables = self.tables.write().await;
        tables.remove(deployment_id);
    }

    /// Resolve a route using a cached resolver.
    pub async fn resolve(
        &self,
        deployment_id: &SiteDeploymentId,
        path: &str,
        method: &str,
    ) -> Option<RouteDecision> {
        let tables = self.tables.read().await;
        tables
            .get(deployment_id)
            .map(|resolver| resolver.resolve(path, method))
    }
}

impl Default for RouteTable {
    fn default() -> Self {
        Self::new()
    }
}

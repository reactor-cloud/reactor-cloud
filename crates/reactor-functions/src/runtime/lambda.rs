//! AWS Lambda runtime adapter.
//!
//! Deploys functions to AWS Lambda with Function URLs for streaming responses.
//! Uses the Lambda Web Adapter layer to run Bun bundles.

use super::*;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// LambdaRuntime configuration.
#[derive(Debug, Clone)]
pub struct LambdaRuntimeConfig {
    /// AWS region.
    pub region: String,
    /// Lambda execution role ARN.
    pub role_arn: String,
    /// S3 bucket for Lambda bundles.
    pub bundle_s3_bucket: String,
    /// Lambda Web Adapter layer ARN.
    pub lwa_layer_arn: String,
    /// CloudWatch log group prefix.
    pub log_group_prefix: String,
}

impl Default for LambdaRuntimeConfig {
    fn default() -> Self {
        Self {
            region: "us-east-1".to_string(),
            role_arn: String::new(),
            bundle_s3_bucket: String::new(),
            lwa_layer_arn: String::new(),
            log_group_prefix: "/reactor/functions/".to_string(),
        }
    }
}

/// A deployed Lambda function.
struct LambdaDeployment {
    /// Deployment handle.
    handle: DeploymentHandle,
    /// Lambda function ARN.
    function_arn: String,
    /// Function URL.
    function_url: String,
}

/// Lambda function runtime.
pub struct LambdaRuntime {
    config: LambdaRuntimeConfig,
    /// Deployed functions keyed by deployment ID.
    deployments: RwLock<HashMap<DeploymentId, LambdaDeployment>>,
}

impl LambdaRuntime {
    /// Create a new LambdaRuntime.
    pub fn new(config: LambdaRuntimeConfig) -> Self {
        Self {
            config,
            deployments: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl FunctionRuntime for LambdaRuntime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Lambda
    }

    async fn deploy(
        &self,
        deployment_id: DeploymentId,
        function_name: &str,
        manifest: &Manifest,
        _bundle_path: &std::path::Path,
    ) -> Result<DeploymentHandle, FunctionsError> {
        // TODO: Implement Lambda deployment
        // 1. Upload bundle to S3
        // 2. Create/update Lambda function with:
        //    - Lambda Web Adapter layer
        //    - Handler set to bootstrap
        //    - Runtime set to provided.al2023
        //    - Memory from manifest
        //    - Timeout from manifest
        // 3. Create Function URL with RESPONSE_STREAM mode
        // 4. If min_instances > 0, configure provisioned concurrency
        // 5. Wait for function to become Active

        let lambda_function_name = format!("reactor-{}-{}", function_name, deployment_id);
        let function_arn = format!(
            "arn:aws:lambda:{}:123456789012:function:{}",
            self.config.region, lambda_function_name
        );
        let function_url = format!(
            "https://{}.lambda-url.{}.on.aws/",
            lambda_function_name, self.config.region
        );

        let handle = DeploymentHandle {
            deployment_id,
            function_name: function_name.to_string(),
            runtime: RuntimeKind::Lambda,
            version: manifest.version,
            limits: Limits::from(manifest),
            max_concurrency: manifest.concurrency.max_concurrency,
            runtime_ref: Some(function_url.clone()),
        };

        tracing::info!(
            deployment_id = %deployment_id,
            function = %function_name,
            lambda_function = %lambda_function_name,
            "TODO: deploy Lambda function"
        );

        // Store deployment info
        let lambda_deployment = LambdaDeployment {
            handle: handle.clone(),
            function_arn,
            function_url,
        };

        let mut deployments = self.deployments.write().await;
        deployments.insert(deployment_id, lambda_deployment);

        Ok(handle)
    }

    async fn invoke(
        &self,
        handle: &DeploymentHandle,
        request: IncomingRequest,
    ) -> Result<InvokeResult, FunctionsError> {
        let start = std::time::Instant::now();

        // TODO: Implement Lambda invocation via Function URL
        // 1. Get the function URL from deployment info
        // 2. Forward the HTTP request to the Function URL
        // 3. Stream the response back

        let deployments = self.deployments.read().await;
        let _deployment = deployments
            .get(&handle.deployment_id)
            .ok_or_else(|| FunctionsError::DeploymentNotFound(handle.deployment_id.to_string()))?;

        tracing::debug!(
            deployment_id = %handle.deployment_id,
            method = %request.method,
            path = %request.path,
            "TODO: invoke Lambda function via Function URL"
        );

        // For now, return a placeholder response
        let body = futures::stream::once(async {
            Ok::<_, std::io::Error>(Bytes::from(
                r#"{"error": "Lambda runtime not yet fully implemented"}"#,
            ))
        });

        let response = OutgoingResponse::new(501, body)
            .with_header("content-type", "application/json");

        Ok(InvokeResult {
            response,
            cold_start: true,
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn warm(&self, handle: &DeploymentHandle, count: u32) -> Result<(), FunctionsError> {
        // Lambda warming is done via provisioned concurrency
        // This would update the provisioned concurrency setting
        if count > 0 {
            tracing::info!(
                deployment_id = %handle.deployment_id,
                count = count,
                "TODO: set provisioned concurrency for Lambda function"
            );
        }
        Ok(())
    }

    async fn destroy(&self, handle: &DeploymentHandle) -> Result<(), FunctionsError> {
        // TODO: Delete the Lambda function and Function URL
        let mut deployments = self.deployments.write().await;
        if let Some(deployment) = deployments.remove(&handle.deployment_id) {
            tracing::info!(
                deployment_id = %handle.deployment_id,
                function_arn = %deployment.function_arn,
                "TODO: delete Lambda function"
            );
        }
        Ok(())
    }

    async fn list_active(&self) -> Result<Vec<DeploymentHandle>, FunctionsError> {
        let deployments = self.deployments.read().await;
        Ok(deployments.values().map(|d| d.handle.clone()).collect())
    }
}

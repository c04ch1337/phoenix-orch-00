use anyhow::Result;
use futures::StreamExt;
use k8s_openapi::api::apps::v1::Deployment;
use kube::{
    api::{Api, ListParams, Meta, PostParams, WatchEvent},
    Client,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing::{error, info, warn};

#[derive(Debug, Serialize, Deserialize)]
struct RollbackConfig {
    monitored_deployments: Vec<MonitoredDeployment>,
    check_interval_seconds: u64,
    metrics_window_minutes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct MonitoredDeployment {
    name: String,
    namespace: String,
    thresholds: Thresholds,
}

#[derive(Debug, Serialize, Deserialize)]
struct Thresholds {
    error_rate: f64,
    latency_p95_seconds: f64,
    cpu_usage_percent: f64,
    memory_usage_percent: f64,
}

struct RollbackController {
    client: Client,
    prometheus: PrometheusClient,
    config: Arc<RollbackConfig>,
}

impl RollbackController {
    async fn new(config_path: &str) -> Result<Self> {
        let client = Client::try_default().await?;
        let prometheus = PrometheusClient::new(
            std::env::var("PROMETHEUS_URL")
                .unwrap_or_else(|_| "http://prometheus-operated:9090".to_string()),
        );
        let config = Arc::new(
            serde_yaml::from_str(&std::fs::read_to_string(config_path)?)?,
        );

        Ok(Self {
            client,
            prometheus,
            config,
        })
    }

    async fn watch_deployments(&self) -> Result<()> {
        let namespace = "phoenix-orch";
        let deployments: Api<Deployment> = Api::namespaced(
            self.client.clone(),
            namespace,
        );

        let lp = ListParams::default()
            .labels("app=master-orchestrator");

        let mut stream = deployments.watch(&lp, "0").await?.boxed();

        while let Some(event) = stream.try_next().await? {
            match event {
                WatchEvent::Modified(deployment) => {
                    self.handle_deployment_change(deployment).await?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn handle_deployment_change(&self, deployment: Deployment) -> Result<()> {
        let name = Meta::name(&deployment);
        let namespace = Meta::namespace(&deployment)
            .unwrap_or_else(|| "phoenix-orch".to_string());

        if let Some(monitored) = self.config.monitored_deployments
            .iter()
            .find(|m| m.name == name && m.namespace == namespace)
        {
            self.check_deployment_health(&deployment, monitored).await?;
        }

        Ok(())
    }

    async fn check_deployment_health(
        &self,
        deployment: &Deployment,
        monitored: &MonitoredDeployment,
    ) -> Result<()> {
        let metrics = self.collect_metrics(deployment).await?;
        
        if self.should_rollback(&metrics, &monitored.thresholds) {
            self.perform_rollback(deployment).await?;
        }

        Ok(())
    }

    async fn collect_metrics(&self, deployment: &Deployment) -> Result<DeploymentMetrics> {
        let labels = format!(
            "app={},version={}",
            Meta::name(deployment),
            deployment.spec.as_ref()
                .and_then(|s| s.template.metadata.as_ref())
                .and_then(|m| m.labels.as_ref())
                .and_then(|l| l.get("version"))
                .unwrap_or("unknown")
        );

        let window = format!("{}m", self.config.metrics_window_minutes);

        let error_rate = self.prometheus
            .query(&format!(
                "sum(rate(http_requests_total{{status=~'5..',{}}}[{}])) / \
                 sum(rate(http_requests_total{{{}}}[{}]))",
                labels, window, labels, window
            ))
            .await?;

        let latency = self.prometheus
            .query(&format!(
                "histogram_quantile(0.95, \
                 sum(rate(request_duration_seconds_bucket{{{}}}[{}])) by (le))",
                labels, window
            ))
            .await?;

        let cpu_usage = self.prometheus
            .query(&format!(
                "sum(rate(container_cpu_usage_seconds_total{{{}}}[{}])) / \
                 sum(container_spec_cpu_quota{{{}}}) * 100",
                labels, window, labels
            ))
            .await?;

        let memory_usage = self.prometheus
            .query(&format!(
                "sum(container_memory_usage_bytes{{{}}}) / \
                 sum(container_spec_memory_limit_bytes{{{}}}) * 100",
                labels, labels
            ))
            .await?;

        Ok(DeploymentMetrics {
            error_rate,
            latency_p95_seconds: latency,
            cpu_usage_percent: cpu_usage,
            memory_usage_percent: memory_usage,
        })
    }

    fn should_rollback(&self, metrics: &DeploymentMetrics, thresholds: &Thresholds) -> bool {
        metrics.error_rate > thresholds.error_rate
            || metrics.latency_p95_seconds > thresholds.latency_p95_seconds
            || metrics.cpu_usage_percent > thresholds.cpu_usage_percent
            || metrics.memory_usage_percent > thresholds.memory_usage_percent
    }

    async fn perform_rollback(&self, deployment: &Deployment) -> Result<()> {
        let name = Meta::name(deployment);
        let namespace = Meta::namespace(deployment)
            .unwrap_or_else(|| "phoenix-orch".to_string());

        let deployments: Api<Deployment> = Api::namespaced(
            self.client.clone(),
            &namespace,
        );

        // Get current revision
        let current_revision = deployment
            .metadata
            .annotations
            .as_ref()
            .and_then(|a| a.get("deployment.kubernetes.io/revision"))
            .and_then(|r| r.parse::<i32>().ok())
            .unwrap_or(1);

        // Target previous revision
        let target_revision = current_revision - 1;
        if target_revision < 1 {
            error!("No previous revision available for rollback");
            return Ok(());
        }

        info!(
            "Rolling back deployment {}/{} to revision {}",
            namespace, name, target_revision
        );

        // Create rollback patch
        let mut deployment = deployment.clone();
        let annotations = deployment
            .spec
            .as_mut()
            .and_then(|s| s.template.metadata.as_mut())
            .and_then(|m| m.annotations.as_mut())
            .unwrap_or(&mut HashMap::new());

        annotations.insert(
            "kubernetes.io/change-cause".to_string(),
            format!("Automated rollback to revision {}", target_revision),
        );

        // Apply rollback
        deployments
            .replace(&name, &PostParams::default(), &deployment)
            .await?;

        info!("Rollback initiated successfully");

        // Send alert
        self.send_rollback_alert(&name, &namespace, target_revision).await?;

        Ok(())
    }

    async fn send_rollback_alert(
        &self,
        name: &str,
        namespace: &str,
        target_revision: i32,
    ) -> Result<()> {
        let webhook_url = std::env::var("ALERT_WEBHOOK_URL")?;
        let client = reqwest::Client::new();

        client
            .post(&webhook_url)
            .json(&serde_json::json!({
                "text": format!(
                    "🔄 Automated rollback triggered for {}/{} to revision {}",
                    namespace, name, target_revision
                )
            }))
            .send()
            .await?;

        Ok(())
    }

    async fn run(&self) -> Result<()> {
        info!("Starting rollback controller");

        loop {
            if let Err(e) = self.watch_deployments().await {
                error!("Error watching deployments: {}", e);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

#[derive(Debug)]
struct DeploymentMetrics {
    error_rate: f64,
    latency_p95_seconds: f64,
    cpu_usage_percent: f64,
    memory_usage_percent: f64,
}

struct PrometheusClient {
    base_url: String,
    client: reqwest::Client,
}

impl PrometheusClient {
    fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    async fn query(&self, query: &str) -> Result<f64> {
        let response = self.client
            .get(&format!("{}/api/v1/query", self.base_url))
            .query(&[("query", query)])
            .send()
            .await?
            .json::<PrometheusResponse>()
            .await?;

        Ok(response.data.result[0].value[1].parse()?)
    }
}

#[derive(Debug, Deserialize)]
struct PrometheusResponse {
    data: PrometheusData,
}

#[derive(Debug, Deserialize)]
struct PrometheusData {
    result: Vec<PrometheusResult>,
}

#[derive(Debug, Deserialize)]
struct PrometheusResult {
    value: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let config_path = std::env::var("CONFIG_PATH")
        .unwrap_or_else(|_| "/etc/rollback-controller/config.yaml".to_string());

    let controller = RollbackController::new(&config_path).await?;
    controller.run().await
}
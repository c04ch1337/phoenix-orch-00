use axum::{
    routing::{get, post},
    Router,
};
use prometheus::{register_gauge, Gauge};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing::{error, info, warn};

#[derive(Debug, Deserialize)]
struct Config {
    validation: ValidationConfig,
    actions: ActionsConfig,
    intervals: IntervalsConfig,
}

#[derive(Debug, Deserialize)]
struct ValidationConfig {
    error_rate: MetricValidation,
    latency: MetricValidation,
    memory_usage: MetricValidation,
    cpu_usage: MetricValidation,
}

#[derive(Debug, Deserialize)]
struct MetricValidation {
    query: String,
    thresholds: Thresholds,
}

#[derive(Debug, Deserialize)]
struct Thresholds {
    warning: f64,
    critical: f64,
}

#[derive(Debug, Deserialize)]
struct ActionsConfig {
    on_warning: Vec<Action>,
    on_critical: Vec<Action>,
    on_success: Vec<Action>,
}

#[derive(Debug, Deserialize)]
struct Action {
    #[serde(rename = "type")]
    action_type: String,
    channel: Option<String>,
    message: Option<String>,
    target: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct IntervalsConfig {
    check_interval: String,
    validation_window: String,
    promotion_delay: String,
}

#[derive(Debug, Clone)]
struct AppState {
    config: Arc<Config>,
    prometheus_client: Arc<PrometheusClient>,
    slack_client: Arc<SlackClient>,
    kubernetes_client: Arc<KubernetesClient>,
}

#[derive(Debug, Serialize)]
struct ValidationResult {
    status: ValidationStatus,
    metrics: ValidationMetrics,
    timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
enum ValidationStatus {
    Success,
    Warning,
    Critical,
}

#[derive(Debug, Serialize)]
struct ValidationMetrics {
    error_rate: f64,
    latency_p95: f64,
    memory_usage: f64,
    cpu_usage: f64,
}

struct PrometheusClient {
    base_url: String,
    client: reqwest::Client,
}

impl PrometheusClient {
    async fn query(&self, query: &str) -> Result<f64, anyhow::Error> {
        let response = self.client
            .get(&format!("{}/api/v1/query", self.base_url))
            .query(&[("query", query)])
            .send()
            .await?
            .json::<PrometheusResponse>()
            .await?;

        // Extract value from response
        Ok(response.data.result[0].value[1].parse()?)
    }
}

struct SlackClient {
    webhook_url: String,
    client: reqwest::Client,
}

impl SlackClient {
    async fn send_alert(&self, message: &str) -> Result<(), anyhow::Error> {
        self.client
            .post(&self.webhook_url)
            .json(&serde_json::json!({
                "text": message
            }))
            .send()
            .await?;
        Ok(())
    }
}

struct KubernetesClient {
    client: kube::Client,
}

impl KubernetesClient {
    async fn rollback_deployment(&self, name: &str) -> Result<(), anyhow::Error> {
        let deployments: Api<Deployment> = Api::namespaced(
            self.client.clone(),
            "phoenix-orch"
        );

        let mut deployment = deployments.get(name).await?;
        let prev_revision = deployment
            .annotations()
            .get("deployment.kubernetes.io/revision")
            .and_then(|r| r.parse::<i32>().ok())
            .unwrap_or(1) - 1;

        deployment.spec.template.metadata.annotations.insert(
            "kubernetes.io/change-cause".to_string(),
            format!("Rolling back to revision {}", prev_revision)
        );

        deployments.replace(name, &PostParams::default(), &deployment).await?;
        Ok(())
    }

    async fn promote_deployment(&self, source: &str, target: &str) -> Result<(), anyhow::Error> {
        let deployments: Api<Deployment> = Api::namespaced(
            self.client.clone(),
            "phoenix-orch"
        );

        let source_deployment = deployments.get(source).await?;
        let mut target_deployment = deployments.get(target).await?;

        // Copy relevant specs from source to target
        target_deployment.spec.template = source_deployment.spec.template;
        
        deployments.replace(target, &PostParams::default(), &target_deployment).await?;
        Ok(())
    }
}

async fn validate_canary(state: Arc<AppState>) -> ValidationResult {
    let metrics = ValidationMetrics {
        error_rate: state.prometheus_client.query(&state.config.validation.error_rate.query).await?,
        latency_p95: state.prometheus_client.query(&state.config.validation.latency.query).await?,
        memory_usage: state.prometheus_client.query(&state.config.validation.memory_usage.query).await?,
        cpu_usage: state.prometheus_client.query(&state.config.validation.cpu_usage.query).await?,
    };

    let status = if metrics.error_rate > state.config.validation.error_rate.thresholds.critical
        || metrics.latency_p95 > state.config.validation.latency.thresholds.critical
        || metrics.memory_usage > state.config.validation.memory_usage.thresholds.critical
        || metrics.cpu_usage > state.config.validation.cpu_usage.thresholds.critical
    {
        ValidationStatus::Critical
    } else if metrics.error_rate > state.config.validation.error_rate.thresholds.warning
        || metrics.latency_p95 > state.config.validation.latency.thresholds.warning
        || metrics.memory_usage > state.config.validation.memory_usage.thresholds.warning
        || metrics.cpu_usage > state.config.validation.cpu_usage.thresholds.warning
    {
        ValidationStatus::Warning
    } else {
        ValidationStatus::Success
    };

    ValidationResult {
        status,
        metrics,
        timestamp: chrono::Utc::now(),
    }
}

async fn handle_validation_result(
    result: ValidationResult,
    state: Arc<AppState>,
) -> Result<(), anyhow::Error> {
    match result.status {
        ValidationStatus::Critical => {
            for action in &state.config.actions.on_critical {
                match action.action_type.as_str() {
                    "alert" => {
                        if let (Some(channel), Some(message)) = (&action.channel, &action.message) {
                            state.slack_client.send_alert(message).await?;
                        }
                    }
                    "rollback" => {
                        if let Some(target) = &action.target {
                            state.kubernetes_client.rollback_deployment(target).await?;
                        }
                    }
                    _ => warn!("Unknown action type: {}", action.action_type),
                }
            }
        }
        ValidationStatus::Warning => {
            for action in &state.config.actions.on_warning {
                if let (Some(channel), Some(message)) = (&action.channel, &action.message) {
                    state.slack_client.send_alert(message).await?;
                }
            }
        }
        ValidationStatus::Success => {
            for action in &state.config.actions.on_success {
                match action.action_type.as_str() {
                    "alert" => {
                        if let (Some(channel), Some(message)) = (&action.channel, &action.message) {
                            state.slack_client.send_alert(message).await?;
                        }
                    }
                    "promote" => {
                        if let (Some(source), Some(target)) = (&action.source, &action.target) {
                            state.kubernetes_client.promote_deployment(source, target).await?;
                        }
                    }
                    _ => warn!("Unknown action type: {}", action.action_type),
                }
            }
        }
    }
    Ok(())
}

async fn validation_loop(state: Arc<AppState>) {
    let check_interval = Duration::from_secs(15); // Parse from config

    loop {
        match validate_canary(state.clone()).await {
            Ok(result) => {
                if let Err(e) = handle_validation_result(result, state.clone()).await {
                    error!("Failed to handle validation result: {}", e);
                }
            }
            Err(e) => {
                error!("Validation failed: {}", e);
            }
        }

        sleep(check_interval).await;
    }
}

async fn health_handler() -> &'static str {
    "ok"
}

async fn metrics_handler() -> String {
    let encoder = prometheus::TextEncoder::new();
    let metric_families = prometheus::gather();
    encoder.encode_to_string(&metric_families).unwrap()
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = Arc::new(config::load_from_file("/etc/health-validator/config.yaml")?);

    // Initialize clients
    let prometheus_client = Arc::new(PrometheusClient {
        base_url: std::env::var("PROMETHEUS_URL")?,
        client: reqwest::Client::new(),
    });

    let slack_client = Arc::new(SlackClient {
        webhook_url: std::env::var("ALERT_WEBHOOK_URL")?,
        client: reqwest::Client::new(),
    });

    let kubernetes_client = Arc::new(KubernetesClient {
        client: kube::Client::try_default().await?,
    });

    let state = Arc::new(AppState {
        config,
        prometheus_client,
        slack_client,
        kubernetes_client,
    });

    // Start validation loop
    let validation_state = state.clone();
    tokio::spawn(async move {
        validation_loop(validation_state).await;
    });

    // Start HTTP server
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(state);

    let addr = "[::]:8080".parse()?;
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
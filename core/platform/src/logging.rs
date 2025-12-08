use std::env;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Layer};
use uuid::Uuid;

/// Default correlation ID field name
pub const CORRELATION_ID_FIELD: &str = "correlation_id";

/// Environment field name
pub const ENVIRONMENT_FIELD: &str = "environment";

/// Service name field
pub const SERVICE_FIELD: &str = "service";

/// Initialize structured JSON logging
pub fn init_logging(service_name: &str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    
    // Detect environment (dev, prod, staging)
    let environment = env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());
    
    // Only use JSON formatting in prod and staging
    if environment == "prod" || environment == "staging" {
        // JSON formatter with correlation ID and other structured fields
        let json_layer = fmt::layer()
            .json()
            .with_timer(fmt::time::UtcTime::rfc_3339())
            .with_target(true)
            .with_current_span(true)
            .with_span_list(true)
            .with_file(true)
            .with_line_number(true)
            .with_filter(env_filter.clone());
        
        tracing_subscriber::registry()
            .with(json_layer)
            .init();
    } else {
        // Use a more readable text formatter for development
        fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .finish()
            .init();
    }

    tracing::info!(
        service = %service_name,
        environment = %environment,
        "logging initialized"
    );
}

/// Generate a correlation ID for a request if one doesn't exist
pub fn ensure_correlation_id(existing_id: Option<Uuid>) -> Uuid {
    existing_id.unwrap_or_else(Uuid::new_v4)
}

/// Add correlation ID to the current span
pub fn add_correlation_id_to_span(correlation_id: Uuid) {
    let span = tracing::Span::current();
    span.record(CORRELATION_ID_FIELD, &tracing::field::display(correlation_id));
}

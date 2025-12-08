use crate::errors::PlatformError;
use crate::logging::{self, CORRELATION_ID_FIELD, ENVIRONMENT_FIELD, SERVICE_FIELD};
use std::env;
use tracing::{info, info_span, span, Instrument, Level, Span};
use uuid::Uuid;

/// Initialize structured tracing for the application
pub fn init_tracing(service_name: &str) -> Result<(), PlatformError> {
    // Initialize logging with the service name
    crate::logging::init_logging(service_name);
    
    // Log system information
    let environment = env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());
    
    info!(
        service = %service_name,
        environment = %environment,
        event = "startup",
        "Application tracing initialized"
    );
    
    Ok(())
}

/// Create a span with correlation ID for request tracing
pub fn correlation_span(correlation_id: Uuid, operation: &str) -> Span {
    info_span!(
        "operation",
        %operation,
        correlation_id = %correlation_id,
        event_type = "request",
    )
}

/// Instrument an async operation with correlation context
pub async fn with_correlation_context<F, R>(correlation_id: Uuid, operation: &str, f: F) -> R
where
    F: std::future::Future<Output = R>,
{
    let span = correlation_span(correlation_id, operation);
    f.instrument(span).await
}

/// Extract correlation ID from a request or generate a new one
pub fn extract_correlation_id(existing_id: Option<Uuid>) -> Uuid {
    logging::ensure_correlation_id(existing_id)
}

use crate::errors::PlatformError;
use tracing::{span, Level, Span};
use uuid::Uuid;

pub fn init_tracing(service_name: &str) -> Result<(), PlatformError> {
    crate::logging::init_logging(service_name);
    Ok(())
}

pub fn correlation_span(correlation_id: Uuid, operation: &str) -> Span {
    span!(
        Level::INFO,
        "operation",
        %operation,
        correlation_id = %correlation_id,
    )
}

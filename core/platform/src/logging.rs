use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

pub fn init_logging(service_name: &str) {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Use a simple text formatter without extra features to keep the
    // dependency surface minimal and avoid additional feature flags.
    fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .finish()
        .init();

    tracing::info!(service = %service_name, "logging initialized");
}

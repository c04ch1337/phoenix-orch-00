use std::io;
use std::net::SocketAddr;
use std::sync::Once;

use metrics::{counter, histogram};
use metrics_exporter_prometheus::PrometheusBuilder;

/// Ensure we only install a single global recorder even if `init_metrics`
/// is called multiple times.
static INIT: Once = Once::new();

/// Initialize metrics exporting using a Prometheus HTTP exporter.
///
/// This configures a global metrics recorder backed by
/// `metrics-exporter-prometheus` and exposes a `/metrics` endpoint on the
/// provided `bind_addr`. Subsequent calls are ignored after the first
/// successful initialization.
pub fn init_metrics(bind_addr: SocketAddr) -> io::Result<()> {
    let mut init_result: io::Result<()> = Ok(());

    INIT.call_once(|| {
        let builder = PrometheusBuilder::new().with_http_listener(bind_addr);

        if let Err(err) = builder.install() {
            init_result = Err(io::Error::new(
                io::ErrorKind::Other,
                format!("failed to init metrics: {err}"),
            ));
        }
    });

    init_result
}

/// Record a counter metric by name.
///
/// The `name` parameter must be a string literal or other `'static` string.
/// All current call sites use string literals, so this restriction is
/// acceptable and satisfies the metrics crate's lifetime requirements.
pub fn record_counter(name: &'static str, value: u64) {
    counter!(name).increment(value);
}

/// Record a histogram metric (in seconds or other appropriate units).
///
/// The `name` parameter must be a string literal or other `'static` string.
pub fn record_histogram(name: &'static str, value: f64) {
    histogram!(name).record(value);
}

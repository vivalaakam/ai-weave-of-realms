//! Test utilities shared across rpg-engine unit tests.

/// Initialises a `tracing-subscriber` for the current test binary.
///
/// Safe to call multiple times — subsequent calls after the first are no-ops.
/// Respects the `RUST_LOG` environment variable for log filtering.
#[cfg(test)]
pub fn init_tracing() {
    use std::sync::OnceLock;
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug")),
            )
            .with_test_writer()
            .try_init()
            .ok();
    });
}

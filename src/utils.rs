use tracing_subscriber::{EnvFilter, fmt};

pub fn setup_tracing() {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("deployer=info")),
        )
        .init();
}

use reqwest::Client;
use std::time::Duration;

/// App user-agent string derived from Cargo.toml version at compile time.
pub const APP_USER_AGENT: &str = concat!("macPlus/", env!("CARGO_PKG_VERSION"));

pub fn create_http_client() -> Client {
    Client::builder()
        .user_agent(APP_USER_AGENT)
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .gzip(true)
        .http2_adaptive_window(true)
        .pool_max_idle_per_host(3)
        .tcp_nodelay(true)
        .build()
        .expect("Failed to create HTTP client")
}

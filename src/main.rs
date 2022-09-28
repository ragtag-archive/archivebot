#![forbid(unsafe_code)]
use archivebot;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init_from_env(
        env_logger::Env::default().default_filter_or(format!("{}=info", env!("CARGO_PKG_NAME"))),
    );
    archivebot::run().await
}

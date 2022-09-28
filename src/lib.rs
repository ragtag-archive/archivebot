#![forbid(unsafe_code)]
#[macro_use]
extern crate log;

mod util;

pub static APP_NAME: &str = env!("CARGO_PKG_NAME");

pub async fn run() -> anyhow::Result<()> {
    info!("{} starting", APP_NAME);
    Ok(())
}

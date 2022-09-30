#![forbid(unsafe_code)]
#[macro_use]
extern crate log;

mod util;

pub static APP_NAME: &str = env!("CARGO_PKG_NAME");

pub async fn run() -> anyhow::Result<()> {
    info!("{} starting", APP_NAME);

    // Instantiate modules
    let (tasq, ytdlp, rclone) = tokio::join!(
        util::tasq::Tasq::new("http://localhost:8080".into(), None),
        util::ytdl::YTDL::new(),
        util::rclone::Rclone::new("".into(), "".into()),
    );

    tasq?;
    ytdlp?;
    rclone?;

    info!("Bye!");
    Ok(())
}

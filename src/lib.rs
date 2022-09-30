#![forbid(unsafe_code)]
#[macro_use]
extern crate log;

pub mod archiver;
pub mod util;

pub static APP_NAME: &str = env!("CARGO_PKG_NAME");

pub async fn run() -> anyhow::Result<()> {
    info!("{} starting", APP_NAME);

    // Instantiate modules
    let (tasq, ytdlp, rclone) = tokio::join!(
        util::tasq::Tasq::new("http://localhost:8080".into(), None),
        util::ytdl::YTDL::new(),
        util::rclone::Rclone::new("".into(), "".into()),
    );

    let tasq = Box::new(tasq.map_err(|e| anyhow::anyhow!("Could not create Tasq client: {}", e))?);
    let ytdlp =
        Box::new(ytdlp.map_err(|e| anyhow::anyhow!("Could not create YTDL client: {}", e))?);
    let rclone =
        Box::new(rclone.map_err(|e| anyhow::anyhow!("Could not create Rclone client: {}", e))?);

    let bot = archiver::ArchiveBot::new(tasq, ytdlp, rclone);

    info!("{} running", APP_NAME);
    bot.run_one()
        .await
        .map_err(|e| anyhow::anyhow!("Failure during archival: {}", e))?;

    info!("Bye!");
    Ok(())
}

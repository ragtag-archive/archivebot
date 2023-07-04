#![forbid(unsafe_code)]

use anyhow::Context;
#[macro_use]
extern crate log;

pub mod archiver;
mod config;
pub mod util;

pub static APP_NAME: &str = env!("CARGO_PKG_NAME");

pub async fn run() -> anyhow::Result<()> {
    info!("{} starting", APP_NAME);

    // Get the config
    debug!("Loading config");
    let cfg = config::Config::from_env().context("Could not load config")?;

    let ragtag: Box<dyn util::ArchiveSite> = if cfg.archive_base_url.is_empty() {
        warn!("No archive base URL specified, using mock archive site");
        Box::new(util::archive::MockRagtag::new().await?)
    } else {
        Box::new(
            util::archive::Ragtag::new(
                url::Url::parse(&cfg.archive_base_url)
                    .context("Could not parse archive base URL")?,
                None,
            )
            .await?,
        )
    };

    // Instantiate modules
    let (tasq, ytdlp, meta, rclone) = tokio::join!(
        util::tasq::Tasq::new(cfg.tasq_url, None),
        util::ytdl::YTDL::new(),
        util::metadata::YTMetadataExtractor::new(cfg.youtube_api_key, None),
        util::rclone::Rclone::new(
            cfg.rclone_config_data,
            cfg.rclone_remote_name,
            cfg.rclone_base_directory
        ),
    );

    let tasq = Box::new(tasq.context("Could not create Tasq client")?);
    let ytdlp = Box::new(ytdlp.context("Could not create YTDL client")?);
    let meta = Box::new(meta.context("Could not create metadata extractor")?);
    let rclone = Box::new(rclone.context("Could not create Rclone client")?);

    let bot = archiver::ArchiveBot::new(tasq, ytdlp, meta, rclone, ragtag);

    info!("{} running", APP_NAME);
    tokio::select! {
        _ = bot.run_forever() => unreachable!(),
        _ = tokio::signal::ctrl_c() =>
            info!("Signal received, shutting down"),
    };

    info!("Bye!");
    Ok(())
}

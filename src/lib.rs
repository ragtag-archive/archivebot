#![forbid(unsafe_code)]

use anyhow::Context;
#[macro_use]
extern crate log;

pub mod archiver;
mod config;
pub mod util;

mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

pub async fn run() -> anyhow::Result<()> {
    if let (Some(dirty), Some(short_hash)) =
        (built_info::GIT_DIRTY, built_info::GIT_COMMIT_HASH_SHORT)
    {
        info!(
            "{} {}, v{} ({}{})",
            built_info::PKG_NAME,
            built_info::PROFILE,
            built_info::PKG_VERSION,
            short_hash,
            if dirty { ", dirty" } else { "" },
        );
        info!("Built on {}", built_info::BUILT_TIME_UTC,);
    }

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
        util::metadata::YTMetadataExtractor::new(cfg.youtube_api_key, None, cfg.drive_base),
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

    // Channel for events
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let bot = archiver::ArchiveBot::new(tasq, ytdlp, meta, rclone, ragtag, Some(tx));
    let metrics_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3383));

    info!("{} running", built_info::PKG_NAME);
    tokio::select! {
        _ = bot.run_forever()
            => unreachable!(),
        _ = util::metrics::serve_metrics_endpoint(metrics_addr, rx)
            => unreachable!(),
        _ = tokio::signal::ctrl_c()
            => info!("Signal received, shutting down"),
    };

    info!("Bye!");
    Ok(())
}

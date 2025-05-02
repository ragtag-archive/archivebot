use crate::util;
use anyhow::Context;
use tokio::time::{sleep, Duration};

#[derive(Debug, PartialEq)]
pub enum ArchiverState {
    Idle,
    Starting,
    FailureBackoff,
    Downloading,
    Uploading,
}

impl std::fmt::Display for ArchiverState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub static ARCHIVER_STATES: &[ArchiverState] = &[
    ArchiverState::Idle,
    ArchiverState::Starting,
    ArchiverState::FailureBackoff,
    ArchiverState::Downloading,
    ArchiverState::Uploading,
];

pub struct ArchiveBot {
    task_queue: Box<dyn util::TaskQueue>,
    video_downloader: Box<dyn util::VideoDownloader>,
    metadata_extractor: Box<dyn util::MetadataExtractor>,
    uploader: Box<dyn util::Uploader>,
    archive_site: Box<dyn util::ArchiveSite>,
    events: Option<tokio::sync::mpsc::UnboundedSender<ArchiverState>>,
    skip_requeue: String,
}

impl ArchiveBot {
    pub fn new(
        task_queue: Box<dyn util::TaskQueue>,
        video_downloader: Box<dyn util::VideoDownloader>,
        metadata_extractor: Box<dyn util::MetadataExtractor>,
        uploader: Box<dyn util::Uploader>,
        archive_site: Box<dyn util::ArchiveSite>,
        events: Option<tokio::sync::mpsc::UnboundedSender<ArchiverState>>,
        skip_requeue: String,
    ) -> Self {
        Self {
            task_queue,
            video_downloader,
            metadata_extractor,
            uploader,
            archive_site,
            events,
            skip_requeue,
        }
    }

    fn send_event(&self, state: ArchiverState) {
        if let Some(events) = &self.events {
            let _ = events.send(state);
        }
    }

    pub async fn run_forever(&self, exit_after: chrono::Duration) -> () {
        let mut backoff_delay = Duration::from_secs(30);
        let next_exit = chrono::Utc::now() + exit_after;

        loop {
            info!("Getting next task now");
            match self.run_one().await {
                Ok(_) => {
                    info!("Successfully processed task");
                    backoff_delay = Duration::from_secs(30);
                }
                Err(e) => {
                    error!("Failure during archival: {:#}", e);
                    info!("Backing off for {} seconds", backoff_delay.as_secs());
                    self.send_event(ArchiverState::FailureBackoff);
                    sleep(backoff_delay).await;
                    backoff_delay *= 2;

                    if backoff_delay > Duration::from_secs(5 * 60) {
                        backoff_delay = Duration::from_secs(5 * 60);
                    }
                }
            }

            // Check if we need to restart
            if chrono::Utc::now() > next_exit {
                info!("Exit time has passed, exiting");
                return;
            }
        }
    }

    pub async fn run_one(&self) -> anyhow::Result<()> {
        self.send_event(ArchiverState::Starting);

        // Get a task from the queue
        info!("Getting next task from queue");
        let task = self
            .task_queue
            .consume()
            .await
            .context("Could not get next task from queue")?;

        info!("Got task: {:?}", task);
        let video_id = task.data;
        match self.run_video(&video_id).await {
            Err(e) => {
                if !self.skip_requeue.is_empty() {
                    info!("Requeuing {}", video_id);
                    let _ = self.task_queue.insert(video_id).await;
                }
                return Err(e);
            }
            x => {
                return x;
            }
        }
    }

    pub async fn run_video(&self, video_id: &str) -> anyhow::Result<()> {
        let video_url = format!("https://www.youtube.com/watch?v={}", video_id);

        // Ensure the video doesn't already exist in the archive
        if self.archive_site.is_archived(video_id).await? {
            info!("Video already archived, skipping");
            return Ok(());
        }

        // Create a temporary directory
        let destination = util::tempdir()
            .await
            .context("Could not create temporary directory")?;
        debug!(
            "Created temporary directory {}",
            destination.path().to_str().unwrap_or("???")
        );

        // Download the video
        info!("Downloading video {}", video_url);
        self.send_event(ArchiverState::Downloading);
        let dl_res = self
            .video_downloader
            .download(&video_url, destination.path())
            .await
            .context("Could not download video")?;

        if !dl_res.output.status.success() {
            return Err(anyhow::anyhow!(
                "Could not download video: downloader exited with code {}, stderr: {}",
                dl_res.output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&dl_res.output.stderr)
            ));
        }

        // Extract metadata
        info!("Extracting metadata");
        let metadata = self
            .metadata_extractor
            .extract(destination.path())
            .await
            .context("Could not extract metadata")?;

        // Upload the video
        info!("Uploading video");
        self.send_event(ArchiverState::Uploading);
        self.uploader
            .upload(destination.path(), video_id)
            .await
            .context("Could not upload video")?;

        // Add the video to the archive
        info!("Adding video to archive");
        self.archive_site
            .archive(video_id, &metadata)
            .await
            .context("Could not add video to archive")?;

        self.send_event(ArchiverState::Idle);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use async_trait::async_trait;
    use std::path::Path;

    // Mock the Tasq client
    struct MockTasq;
    #[async_trait]
    impl util::TaskQueue for MockTasq {
        async fn insert(&self, _data: String) -> anyhow::Result<util::TaskInsertResponse> {
            unimplemented!()
        }

        async fn list(&self) -> anyhow::Result<util::TaskListResponse> {
            unimplemented!()
        }

        async fn consume(&self) -> anyhow::Result<util::TaskConsumeResponse> {
            Ok(util::TaskConsumeResponse {
                key: "test".into(),
                data: "dQw4w9WgXcQ".into(),
            })
        }
    }

    // Mock the Youtube-dl client
    struct MockYTDL;
    #[async_trait]
    impl util::VideoDownloader for MockYTDL {
        async fn download(
            &self,
            url: &str,
            destination: &Path,
        ) -> anyhow::Result<util::VideoDownloadResult> {
            assert_eq!(
                url, "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
                "Unexpected URL"
            );
            assert!(destination.exists(), "Destination directory does not exist");

            use tokio::process::Command;
            let output = Command::new("echo")
                .arg("Hello, world!")
                .output()
                .await
                .unwrap();
            Ok(util::VideoDownloadResult { output })
        }
    }

    // Mock the metadata extractor
    struct MockMetadataExtractor;
    #[async_trait]
    impl util::MetadataExtractor for MockMetadataExtractor {
        async fn extract(&self, video_path: &Path) -> anyhow::Result<util::Metadata> {
            assert!(video_path.exists(), "Video path does not exist");
            Ok(util::Metadata {
                video_id: "dQw4w9WgXcQ".into(),
                channel_name: "Rick Astley".into(),
                channel_id: "UCuAXFkgsw1L7xaCfnd5JJOw".into(),
                upload_date: "2008-11-25".into(),
                title: "Rick Astley - Never Gonna Give You Up (Video)".into(),
                description: "Rick Astley's official music video for “Never Gonna Give You Up” Listen to Rick Astley: https://RickAstley.lnk.to/_listenYD Subscribe to the official Rick Astley YouTube channel: https://RickAstley.lnk.to/_subscribeYD #RickAstley #NeverGonnaGiveYouUp #Vevo #Pop #OfficialMusicVideo".into(),
                duration: 212,
                width: 1280,
                height: 720,
                fps: 30.0,
                format_id: "22".into(),
                view_count: 2250000000,
                like_count: 999999,
                dislike_count: -1,
                files: vec![],
                drive_base: "blah".into(),
                archived_timestamp: chrono::Utc::now().to_rfc3339(),
                timestamps: None,
            })
        }
    }

    // Mock the Rclone client
    struct MockRclone;
    #[async_trait]
    impl util::Uploader for MockRclone {
        async fn upload(&self, _source_dir: &Path, _target_dir: &str) -> anyhow::Result<()> {
            Ok(())
        }
    }

    // Mock the archive site client
    struct MockArchiveSite;
    #[async_trait]
    impl util::ArchiveSite for MockArchiveSite {
        async fn is_archived(&self, video_id: &str) -> anyhow::Result<bool> {
            assert_eq!(video_id, "dQw4w9WgXcQ", "Unexpected video ID");
            Ok(false)
        }

        async fn archive(&self, video_id: &str, _metadata: &util::Metadata) -> anyhow::Result<()> {
            assert_eq!(video_id, "dQw4w9WgXcQ", "Unexpected video ID");
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_run_one() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let bot = ArchiveBot::new(
            Box::new(MockTasq),
            Box::new(MockYTDL),
            Box::new(MockMetadataExtractor),
            Box::new(MockRclone),
            Box::new(MockArchiveSite),
            Some(tx),
            "".into(),
        );
        bot.run_one().await.unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event, ArchiverState::Starting);
        let event = rx.recv().await.unwrap();
        assert_eq!(event, ArchiverState::Downloading);
        let event = rx.recv().await.unwrap();
        assert_eq!(event, ArchiverState::Uploading);
        let event = rx.recv().await.unwrap();
        assert_eq!(event, ArchiverState::Idle);
        let event = rx.try_recv();
        assert!(event.is_err());
    }
}

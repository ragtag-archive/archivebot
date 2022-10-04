use crate::util;

pub struct ArchiveBot {
    task_queue: Box<dyn util::TaskQueue>,
    video_downloader: Box<dyn util::VideoDownloader>,
    metadata_extractor: Box<dyn util::MetadataExtractor<Metadata = util::Metadata>>,
    uploader: Box<dyn util::Uploader>,
    archive_site: Box<dyn util::ArchiveSite<Metadata = util::Metadata>>,
}

impl ArchiveBot {
    pub fn new(
        task_queue: Box<dyn util::TaskQueue>,
        video_downloader: Box<dyn util::VideoDownloader>,
        metadata_extractor: Box<dyn util::MetadataExtractor<Metadata = util::Metadata>>,
        uploader: Box<dyn util::Uploader>,
        archive_site: Box<dyn util::ArchiveSite<Metadata = util::Metadata>>,
    ) -> Self {
        Self {
            task_queue,
            video_downloader,
            metadata_extractor,
            uploader,
            archive_site,
        }
    }

    pub async fn run_one(&self) -> anyhow::Result<()> {
        // Get a task from the queue
        let task = self
            .task_queue
            .consume()
            .await
            .map_err(|e| anyhow::anyhow!("Could not consume task: {}", e))?;

        let video_id = task.data;
        let video_url = format!("https://www.youtube.com/watch?v={}", video_id);

        // Ensure the video doesn't already exist in the archive
        if self.archive_site.is_archived(&video_id).await? {
            info!("Video already archived");
            return Ok(());
        }

        // Download the video
        let destination = tempfile::tempdir()
            .map_err(|e| anyhow::anyhow!("Could not create temp dir for video download: {}", e))?;
        let dl_res = self
            .video_downloader
            .download(&video_url, destination.path())
            .await
            .map_err(|e| anyhow::anyhow!("Could not download video: {}", e))?;

        if !dl_res.output.status.success() {
            return Err(anyhow::anyhow!(
                "Could not download video: downloader exited with code {}, stderr: {}",
                dl_res.output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&dl_res.output.stderr)
            ));
        }

        // Extract metadata
        let metadata = self
            .metadata_extractor
            .extract(destination.path())
            .await
            .map_err(|e| anyhow::anyhow!("Could not extract metadata: {}", e))?;

        // Upload the video
        self.uploader
            .upload(destination.path(), &video_id)
            .await
            .map_err(|e| anyhow::anyhow!("Could not upload video: {}", e))?;

        // Add the video to the archive
        self.archive_site
            .archive(&video_id, &metadata)
            .await
            .map_err(|e| anyhow::anyhow!("Could not archive video: {}", e))?;

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
        type Metadata = util::Metadata;

        async fn extract(&self, video_path: &Path) -> anyhow::Result<Self::Metadata> {
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
                fps: 30,
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
        type Metadata = util::Metadata;

        async fn is_archived(&self, video_id: &str) -> anyhow::Result<bool> {
            assert_eq!(video_id, "dQw4w9WgXcQ", "Unexpected video ID");
            Ok(false)
        }

        async fn archive(&self, video_id: &str, _metadata: &Self::Metadata) -> anyhow::Result<()> {
            assert_eq!(video_id, "dQw4w9WgXcQ", "Unexpected video ID");
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_run_one() {
        let bot = ArchiveBot::new(
            Box::new(MockTasq),
            Box::new(MockYTDL),
            Box::new(MockMetadataExtractor),
            Box::new(MockRclone),
            Box::new(MockArchiveSite),
        );
        bot.run_one().await.unwrap();
    }
}

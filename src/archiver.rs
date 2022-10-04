use crate::util;

pub struct ArchiveBot {
    task_queue: Box<dyn util::TaskQueue>,
    video_downloader: Box<dyn util::VideoDownloader>,
    uploader: Box<dyn util::Uploader>,
    archive_site: Box<dyn util::ArchiveSite<Metadata = util::Metadata>>,
}

impl ArchiveBot {
    pub fn new(
        task_queue: Box<dyn util::TaskQueue>,
        video_downloader: Box<dyn util::VideoDownloader>,
        uploader: Box<dyn util::Uploader>,
        archive_site: Box<dyn util::ArchiveSite<Metadata = util::Metadata>>,
    ) -> Self {
        Self {
            task_queue,
            video_downloader,
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

        // Upload the video
        self.uploader
            .upload(destination.path(), &video_id)
            .await
            .map_err(|e| anyhow::anyhow!("Could not upload video: {}", e))?;

        // TODO: extract metadata, construct database entry, and upload to archive site

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

        async fn archive(&self, _video_id: &str, _metadata: &Self::Metadata) -> anyhow::Result<()> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_run_one() {
        let bot = ArchiveBot::new(
            Box::new(MockTasq),
            Box::new(MockYTDL),
            Box::new(MockRclone),
            Box::new(MockArchiveSite),
        );
        bot.run_one().await.unwrap();
    }
}

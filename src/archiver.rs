use crate::util;

pub struct ArchiveBot {
    task_queue: Box<dyn util::TaskQueue>,
    video_downloader: Box<dyn util::VideoDownloader>,
    uploader: Box<dyn util::Uploader>,
}

impl ArchiveBot {
    pub fn new(
        task_queue: Box<dyn util::TaskQueue>,
        video_downloader: Box<dyn util::VideoDownloader>,
        uploader: Box<dyn util::Uploader>,
    ) -> Self {
        Self {
            task_queue,
            video_downloader,
            uploader,
        }
    }

    pub async fn run_one(&self) -> anyhow::Result<()> {
        // Get a task from the queue
        let task = self
            .task_queue
            .consume()
            .await
            .map_err(|e| anyhow::anyhow!("Could not consume task: {}", e))?;

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
                data: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".into(),
            })
        }
    }

    // Mock the Youtube-dl client
    struct MockYTDL;
    #[async_trait]
    impl util::VideoDownloader for MockYTDL {
        async fn download(
            &self,
            _url: &str,
            _destination: &Path,
        ) -> anyhow::Result<util::VideoDownloadResult> {
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
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn test_run_one() {
        let bot = ArchiveBot::new(Box::new(MockTasq), Box::new(MockYTDL), Box::new(MockRclone));
        bot.run_one().await.unwrap();
    }
}

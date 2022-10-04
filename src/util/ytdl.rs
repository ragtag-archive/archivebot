use super::{SelfInstallable, VideoDownloadResult, VideoDownloader};
use anyhow::Context;
use async_trait::async_trait;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

static YTDLP_RELEASE_URL: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp";
static FFMPEG_RELEASE_URL: &str =
    "https://github.com/eugeneware/ffmpeg-static/releases/download/b5.0.1/linux-x64";

pub struct YTDL {
    ytdlp_path: PathBuf,
    ffmpeg_path: PathBuf,
}

impl YTDL {
    /// Create a new instance of yt-dlp. If the executable is not found, it will
    /// be downloaded.
    pub async fn new() -> anyhow::Result<Self> {
        let cache_dir = super::get_cache_dir().await?;
        let ytdlp_path = cache_dir.join("yt-dlp");
        let ffmpeg_path = cache_dir.join("ffmpeg");

        // Ensure the cache directory exists
        tokio::fs::create_dir_all(&cache_dir)
            .await
            .context("Could not create cache directory")?;

        let ytdl = Self {
            ytdlp_path,
            ffmpeg_path,
        };

        // Install if not already installed
        if !ytdl.is_installed().await {
            ytdl.install()
                .await
                .context("Could not install yt-dlp and ffmpeg")?;
        }

        Ok(ytdl)
    }

    async fn install_binary(url: &str, path: &PathBuf) -> anyhow::Result<()> {
        // Fetch the file
        let mut resp = reqwest::get(url).await?;
        let mut file = tokio::fs::File::create(path).await?;

        // Write the file
        while let Some(chunk) = resp.chunk().await? {
            file.write_all(&chunk).await?;
        }
        file.flush().await?;

        // Make the file executable
        let mut perms = file.metadata().await?.permissions();
        perms.set_mode(0o755);
        file.set_permissions(perms).await?;

        Ok(())
    }

    async fn download_video(
        &self,
        url: &str,
        workdir: &Path,
    ) -> std::io::Result<std::process::Output> {
        let mut cmd = Command::new(&self.ytdlp_path);
        let cmd = cmd
            .current_dir(workdir)
            .args(&[
                "-f",
                "bestvideo[protocol*=https]+bestaudio",
                "--ffmpeg-location",
                &self.ffmpeg_path.to_string_lossy(),
                // Subtitles
                "--write-subs",
                "--sub-format",
                "srv3/best",
                "--sub-langs",
                "all,-live_chat",
                // Metadata
                "--write-thumbnail",
                "--write-comments",
                "--write-info-json",
                // Embed
                "--embed-subs",
                "--embed-metadata",
                "--embed-info-json",
                "--embed-chapters",
                // Output
                "--merge-output-format",
                "webm/mp4/mkv",
                "--output",
                "%(id)s.%(ext)s",
            ])
            .arg(url);

        debug!("Downloading video with command: {:?}", cmd);
        cmd.output().await
    }

    async fn download_live_chat(
        &self,
        url: &str,
        workdir: &Path,
    ) -> std::io::Result<std::process::Output> {
        let mut cmd = Command::new(&self.ytdlp_path);
        let cmd = cmd
            .current_dir(workdir)
            .args(&[
                "--skip-download",
                "--write-subs",
                "--sub-langs",
                "live_chat",
                "--sub-format",
                "json",
                "--output",
                "%(id)s.%(ext)s",
            ])
            .arg(url);

        debug!("Downloading live chat with command: {:?}", cmd);
        cmd.output().await
    }
}

#[async_trait]
impl VideoDownloader for YTDL {
    /// Download a video from YouTube.
    async fn download(&self, url: &str, workdir: &Path) -> anyhow::Result<VideoDownloadResult> {
        info!("Downloading {}", url);

        // Download video and live chat concurrently
        let (video, live_chat) = tokio::try_join!(
            self.download_video(url, workdir),
            self.download_live_chat(url, workdir),
        )
        .context("Failed to spawn command")?;

        if !video.status.success() {
            debug!(
                "Video download failed with output: {}",
                String::from_utf8_lossy(&video.stderr)
            );
            return Err(anyhow::anyhow!(
                "yt-dlp exited with non-zero status: {}",
                video.status
            ));
        }

        if !live_chat.status.success() {
            debug!(
                "Live chat download failed with output: {}",
                String::from_utf8_lossy(&live_chat.stderr)
            );
            warn!("Could not download live chat: {}", live_chat.status);
        }

        // Download the video
        info!("yt-dlp finished {}", url);
        Ok(VideoDownloadResult { output: video })
    }
}

#[async_trait]
impl SelfInstallable for YTDL {
    /// Check whether the executables exist and can be executed.
    async fn is_installed(&self) -> bool {
        Command::new(&self.ytdlp_path)
            .arg("--version")
            .output()
            .await
            .is_ok()
            && Command::new(&self.ffmpeg_path)
                .arg("-version")
                .output()
                .await
                .is_ok()
    }

    /// Install the latest version of yt-dlp from GitHub.
    async fn install(&self) -> anyhow::Result<()> {
        info!("Installing yt-dlp and ffmpeg");

        let (ytdlp, ffmpeg) = tokio::join!(
            Self::install_binary(YTDLP_RELEASE_URL, &self.ytdlp_path),
            Self::install_binary(FFMPEG_RELEASE_URL, &self.ffmpeg_path),
        );

        ytdlp.context("Could not install yt-dlp")?;
        ffmpeg.context("Could not install ffmpeg")?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    #[ignore] // Takes >150s to run
    async fn test_download() {
        let ytdl = YTDL::new().await.expect("Could not create yt-dlp instance");
        assert!(ytdl.is_installed().await);

        let workdir = super::super::tempdir()
            .await
            .expect("Could not create temp dir");
        println!("Workdir: {:?}", workdir);

        let result = ytdl
            .download(
                "https://www.youtube.com/watch?v=stmZAThUl64",
                workdir.path(),
            )
            .await
            .expect("Could not download video");

        assert!(
            result.output.status.success(),
            "yt-dlp did not exit successfully: {}",
            String::from_utf8_lossy(&result.output.stderr)
        );
        assert!(workdir.path().exists(), "Workdir does not exist");

        // Check the list of files in the workdir
        let mut files = tokio::fs::read_dir(workdir.path())
            .await
            .expect("Could not read workdir");

        let mut file_names = Vec::new();
        while let Some(file) = files.next_entry().await.expect("Could not read workdir") {
            let filename = file.file_name().into_string().unwrap();
            file_names.push(filename.clone());
        }
        println!("Files: {:?}", file_names);

        // Check that the requested files exist
        let expected_files = vec![
            "stmZAThUl64.webm",
            "stmZAThUl64.webp",
            "stmZAThUl64.en.srv3",
            "stmZAThUl64.id.srv3",
            "stmZAThUl64.ja.srv3",
            "stmZAThUl64.info.json",
            "stmZAThUl64.live_chat.json",
        ];
        assert!(
            expected_files
                .iter()
                .all(|f| file_names.iter().any(|n| n == f)),
            "Not all expected files were downloaded"
        );
        assert!(
            file_names
                .iter()
                .all(|f| expected_files.iter().any(|n| n == f)),
            "Unexpected files were present in the workdir"
        );
    }
}

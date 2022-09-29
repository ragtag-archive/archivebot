use super::{VideoDownloadResult, VideoDownloader};
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

#[async_trait]
impl VideoDownloader for YTDL {
    /// Download a video from YouTube.
    async fn download(&self, url: &str, destination: &Path) -> anyhow::Result<VideoDownloadResult> {
        info!("Downloading {}", url);

        // Download the video
        let mut cmd = Command::new(&self.ytdlp_path);
        let cmd = cmd
            .current_dir(destination)
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
                "--write-thumbnail",
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

        debug!("Running command: {:?}", cmd);
        let output = cmd.output().await?;

        info!("yt-dlp finished {}", url);
        Ok(VideoDownloadResult { output })
    }
}

impl YTDL {
    /// Create a new instance of yt-dlp. If the executable is not found, it will
    /// be downloaded.
    pub async fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .expect("Could not find cache dir")
            .join("archivebot");
        let ytdlp_path = cache_dir.join("yt-dlp");
        let ffmpeg_path = cache_dir.join("ffmpeg");

        // Ensure the cache directory exists
        tokio::fs::create_dir_all(&cache_dir)
            .await
            .expect("Could not create cache dir");

        let ytdl = Self {
            ytdlp_path,
            ffmpeg_path,
        };

        // Install if not already installed
        if !ytdl.is_installed().await {
            ytdl.install().await.expect("Could not install yt-dlp");
        }

        ytdl
    }

    /// Check whether the executables exist and can be executed.
    pub async fn is_installed(&self) -> bool {
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

    /// Install the latest version of yt-dlp from GitHub.
    pub async fn install(&self) -> anyhow::Result<()> {
        info!("Installing yt-dlp");
        Self::install_binary(YTDLP_RELEASE_URL, &self.ytdlp_path).await?;

        info!("Installing ffmpeg");
        Self::install_binary(FFMPEG_RELEASE_URL, &self.ffmpeg_path).await?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    #[ignore] // Takes >150s to run
    async fn test_download() {
        let ytdl = YTDL::new().await;
        assert!(ytdl.is_installed().await);

        let workdir = tempfile::tempdir().expect("Could not create temp dir");
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

        // Check that the requested files exist
        while let Some(file) = files.next_entry().await.expect("Could not read workdir") {
            let filename = file.file_name().into_string().unwrap();
            assert!(
                filename.starts_with("stmZAThUl64"),
                "Unexpected file: {}",
                filename
            );
        }
    }
}

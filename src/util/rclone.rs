use super::Uploader;
use async_trait::async_trait;
use std::path::Path;
use tokio::process::Command;

static RCLONE_RELEASE_URL: &str = "https://downloads.rclone.org/rclone-current-linux-amd64.zip";

pub struct Rclone {
    remote_name: String,
    base_directory: String,
}

impl Rclone {
    pub async fn new(remote_name: String, base_directory: String) -> Self {
        debug!(
            "Creating Rclone client with remote {} and base directory {}",
            remote_name, base_directory
        );
        let rclone = Rclone {
            remote_name,
            base_directory,
        };

        // Check if rclone is installed
        if !rclone.is_installed().await {
            rclone.install().await.expect("Failed to install rclone");
        }

        rclone
    }

    /// Check if rclone is installed
    pub async fn is_installed(&self) -> bool {
        Command::new("rclone")
            .arg("--version")
            .output()
            .await
            .is_ok()
    }

    /// Download and install rclone
    pub async fn install(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[async_trait]
impl Uploader for Rclone {
    async fn upload(&self, source_dir: &Path, target_dir: &str) -> anyhow::Result<()> {
        let output = Command::new("rclone")
            .arg("copy")
            .arg(source_dir)
            .arg(format!(
                "{}:{}/{}",
                self.remote_name,
                self.base_directory.trim_matches('/'),
                target_dir.trim_matches('/')
            ))
            .output()
            .await?;
        debug!("Rclone output: {:?}", output);
        Ok(())
    }
}

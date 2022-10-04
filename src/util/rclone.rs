use crate::util::github;

use super::{SelfInstallable, Uploader};
use async_trait::async_trait;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub struct Rclone {
    rclone_path: PathBuf,
    remote_name: String,
    base_directory: String,
}

impl Rclone {
    pub async fn new(remote_name: String, base_directory: String) -> anyhow::Result<Self> {
        debug!(
            "Creating Rclone client with remote {} and base directory {}",
            remote_name, base_directory
        );

        let rclone = Rclone {
            rclone_path: super::get_cache_dir().await?.join("rclone"),
            remote_name,
            base_directory,
        };

        // Check if rclone is installed
        if !rclone.is_installed().await {
            rclone
                .install()
                .await
                .map_err(|e| anyhow::anyhow!("Could not install rclone: {}", e))?;
        }

        Ok(rclone)
    }
}

#[async_trait]
impl SelfInstallable for Rclone {
    /// Check if rclone is installed
    async fn is_installed(&self) -> bool {
        Command::new(&self.rclone_path)
            .arg("--version")
            .output()
            .await
            .is_ok()
    }

    /// Download and install rclone
    async fn install(&self) -> anyhow::Result<()> {
        info!("Installing rclone");

        // Create the destination file
        let mut destfile = std::fs::File::create(&self.rclone_path)
            .map_err(|e| anyhow::anyhow!("Failed to create destination file: {}", e))?;

        // Get the latest release info from GitHub
        let client = reqwest::Client::new();
        let release = github::get_latest_release("rclone/rclone", Some(client.clone()))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get latest release info: {}", e))?;

        // Get the download URL
        let download_url = release
            .assets
            .into_iter()
            .find(|asset| asset.name.ends_with("linux-amd64.zip"))
            .take()
            .ok_or_else(|| anyhow::anyhow!("Could not find download URL"))?
            .browser_download_url;

        // Fetch the zip file
        let mut resp = client
            .get(&download_url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch rclone release: {}", e))?;
        let zipfile = tempfile::tempfile()
            .map_err(|e| anyhow::anyhow!("Failed to create temp file: {}", e))?;
        let mut zipfile_async = tokio::fs::File::from_std(
            zipfile
                .try_clone()
                .map_err(|e| anyhow::anyhow!("Failed to clone temp file: {}", e))?,
        );

        // Write the zip file to a temporary file
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to read zip chunk: {}", e))?
        {
            zipfile_async
                .write_all(&chunk)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to write zip chunk: {}", e))?;
        }

        // Extract the zip file
        tokio::task::spawn_blocking(move || {
            let mut archive = zip::ZipArchive::new(zipfile)
                .map_err(|e| anyhow::anyhow!("Failed to open zip file: {}", e))?;

            // Find the file
            let file_name = archive
                .file_names()
                .find(|name| name.ends_with("rclone"))
                .ok_or_else(|| anyhow::anyhow!("Failed to find rclone binary in zip file"))?
                .to_string();

            let mut file = archive
                .by_name(&file_name)
                .map_err(|e| anyhow::anyhow!("Failed to find rclone binary in zip file: {}", e))?;

            // Copy the file to the destination
            std::io::copy(&mut file, &mut destfile)
                .map_err(|e| anyhow::anyhow!("Failed to copy rclone binary: {}", e))?;

            // Make the file executable
            let mut perms = destfile
                .metadata()
                .map_err(|e| anyhow::anyhow!("Failed to get file metadata: {}", e))?
                .permissions();
            perms.set_mode(0o755);
            destfile
                .set_permissions(perms)
                .map_err(|e| anyhow::anyhow!("Failed to set file permissions: {}", e))?;

            Ok::<_, anyhow::Error>(())
        })
        .await?
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

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_rclone() {
        let rclone = Rclone::new("test".to_string(), "test".to_string())
            .await
            .expect("Failed to create Rclone client");
        assert!(rclone.is_installed().await);
    }
}

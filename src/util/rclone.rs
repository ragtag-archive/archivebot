use super::{SelfInstallable, Uploader};
use crate::util::{format_path, github};
use anyhow::Context;
use async_trait::async_trait;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub struct Rclone {
    rclone_path: PathBuf,
    remote_name: String,
    base_directory: String,
    config_filepath: PathBuf,
}

impl Rclone {
    pub async fn new(
        config_data: String,
        remote_name: String,
        base_directory: String,
    ) -> anyhow::Result<Self> {
        debug!(
            "Creating Rclone client with remote {} and base directory {}",
            remote_name, base_directory
        );

        // Write the config file
        let config_filepath = super::get_cache_dir().await?.join("rclone.conf");
        let mut config_file = tokio::fs::File::create(&config_filepath)
            .await
            .context("Could not create rclone config file")?;
        config_file
            .write_all(config_data.as_bytes())
            .await
            .context("Could not write rclone config file")?;

        let rclone = Rclone {
            rclone_path: super::get_cache_dir().await?.join("rclone"),
            remote_name,
            base_directory,
            config_filepath,
        };

        // Check if rclone is installed
        if !rclone.is_installed().await {
            rclone.install().await.context("Could not install rclone")?;
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
            .context("Could not create destination file")?;

        // Get the latest release info from GitHub
        let client = reqwest::Client::new();
        let release = github::get_latest_release("rclone/rclone", Some(client.clone()))
            .await
            .context("Could not get latest release info from GitHub")?;

        let asset_name = match crate::built_info::CFG_TARGET_ARCH {
            "x86_64" => "linux-amd64.zip",
            "aarch64" => "linux-arm64.zip",
            _ => anyhow::bail!("Unsupported architecture"),
        };

        // Get the download URL
        let download_url = release
            .assets
            .into_iter()
            .find(|asset| asset.name.ends_with(asset_name))
            .take()
            .ok_or_else(|| anyhow::anyhow!("Could not find download URL"))?
            .browser_download_url;

        // Fetch the zip file
        let mut resp = client
            .get(&download_url)
            .send()
            .await
            .context("Could not fetch zip file")?;
        let zipfile = super::tempfile()
            .await
            .context("Could not create temporary file")?;
        let mut zipfile_async = tokio::fs::File::from_std(
            zipfile
                .try_clone()
                .context("Could not clone temporary file")?,
        );

        // Write the zip file to a temporary file
        while let Some(chunk) = resp
            .chunk()
            .await
            .context("Could not read zip chunk from response")?
        {
            zipfile_async
                .write_all(&chunk)
                .await
                .context("Could not write zip chunk to temporary file")?;
        }

        // Extract the zip file
        tokio::task::spawn_blocking(move || {
            let mut archive =
                zip::ZipArchive::new(zipfile).context("Could not open zip file for reading")?;

            // Find the file
            let file_name = archive
                .file_names()
                .find(|name| name.ends_with("rclone"))
                .ok_or_else(|| anyhow::anyhow!("Could not find rclone binary in zip file"))?
                .to_string();

            let mut file = archive
                .by_name(&file_name)
                .context("Could not find rclone binary in zip file")?;

            // Copy the file to the destination
            std::io::copy(&mut file, &mut destfile)
                .context("Could not copy rclone binary to destination")?;

            // Make the file executable
            let mut perms = destfile
                .metadata()
                .context("Could not get file metadata")?
                .permissions();
            perms.set_mode(0o755);
            destfile
                .set_permissions(perms)
                .context("Could not set file permissions")?;

            Ok::<_, anyhow::Error>(())
        })
        .await?
    }
}

#[async_trait]
impl Uploader for Rclone {
    async fn upload(&self, source_dir: &Path, target_dir: &str) -> anyhow::Result<()> {
        let output = Command::new(&self.rclone_path)
            .arg("--config")
            .arg(&self.config_filepath)
            .arg("copy")
            .arg(source_dir)
            .arg(format!(
                "{}:{}/{}",
                self.remote_name,
                format_path(self.base_directory.trim_matches('/')),
                target_dir.trim_matches('/')
            ))
            .output()
            .await?;
        debug!("Rclone output: {:?}", output);

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Rclone exited with status {}",
                output.status
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_rclone() {
        let rclone = Rclone::new("".to_string(), "test".to_string(), "test".to_string())
            .await
            .expect("Failed to create Rclone client");
        assert!(rclone.is_installed().await);
    }
}

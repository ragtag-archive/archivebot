use anyhow::Context;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub mod archive;
pub mod github;
pub mod metadata;
pub mod metrics;
pub mod rclone;
pub mod tasq;
pub mod ytdl;

pub async fn get_cache_dir() -> anyhow::Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find cache directory"))?
        .join("archivebot");
    tokio::fs::create_dir_all(&cache_dir).await?;
    Ok(cache_dir)
}

pub fn dir_size(path: &Path) -> anyhow::Result<u64> {
    let mut size = 0;

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let path = entry.path();

        if metadata.is_dir() {
            size += dir_size(&path)?;
        } else {
            size += metadata.len();
        }
    }

    Ok(size)
}

pub async fn tempfile() -> anyhow::Result<std::fs::File> {
    tempfile::tempfile_in(
        get_cache_dir()
            .await
            .context("Could not get cache directory")?,
    )
    .map_err(|e| e.into())
}

pub async fn tempdir() -> anyhow::Result<tempfile::TempDir> {
    tempfile::tempdir_in(
        get_cache_dir()
            .await
            .context("Could not get cache directory")?,
    )
    .map_err(|e| e.into())
}

#[derive(Debug, Deserialize)]
pub struct TaskInsertResponse {
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub struct TaskListResponse {
    pub tasks: Vec<String>,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
pub struct TaskConsumeResponse {
    pub key: String,
    pub data: String,
}

#[async_trait]
pub trait TaskQueue {
    async fn insert(&self, data: String) -> anyhow::Result<TaskInsertResponse>;
    async fn list(&self) -> anyhow::Result<TaskListResponse>;
    async fn consume(&self) -> anyhow::Result<TaskConsumeResponse>;
}

pub struct VideoDownloadResult {
    pub output: std::process::Output,
}

#[async_trait]
pub trait VideoDownloader {
    async fn download(&self, url: &str, workdir: &Path) -> anyhow::Result<VideoDownloadResult>;
}

#[async_trait]
pub trait Uploader {
    async fn upload(&self, source_dir: &Path, target_dir: &str) -> anyhow::Result<()>;
}

#[async_trait]
pub trait SelfInstallable {
    async fn is_installed(&self) -> bool;
    async fn install(&self) -> anyhow::Result<()>;
}

#[derive(Serialize, Debug)]
pub struct Metadata {
    pub video_id: String,
    pub channel_name: String,
    pub channel_id: String,
    pub upload_date: String,
    pub title: String,
    pub description: String,
    pub duration: u64,
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub format_id: String,
    pub view_count: u64,
    pub like_count: u64,
    pub dislike_count: i64,
    pub files: Vec<MetadataFileEntry>,
    pub drive_base: String,
    pub archived_timestamp: String,
    pub timestamps: Option<MetadataTimestamps>,
}

#[derive(Serialize, Debug)]
pub struct MetadataFileEntry {
    pub name: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MetadataTimestamps {
    #[serde(rename = "actualStartTime")]
    pub actual_start_time: Option<String>,
    #[serde(rename = "publishedAt")]
    pub published_at: Option<String>,
    #[serde(rename = "scheduledStartTime")]
    pub scheduled_start_time: Option<String>,
    #[serde(rename = "actualEndTime")]
    pub actual_end_time: Option<String>,
}

#[async_trait]
pub trait ArchiveSite {
    async fn is_archived(&self, id: &str) -> anyhow::Result<bool>;
    async fn archive(&self, id: &str, metadata: &Metadata) -> anyhow::Result<()>;
}

#[async_trait]
pub trait MetadataExtractor {
    async fn extract(&self, workdir: &Path) -> anyhow::Result<Metadata>;
}

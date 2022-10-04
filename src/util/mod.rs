use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub mod archive;
pub mod github;
pub mod rclone;
pub mod tasq;
pub mod ytdl;

async fn get_cache_dir() -> anyhow::Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find cache directory"))?
        .join("archivebot");
    tokio::fs::create_dir_all(&cache_dir).await?;
    Ok(cache_dir)
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

#[derive(Serialize)]
pub struct Metadata {
    pub video_id: String,
    pub channel_name: String,
    pub channel_id: String,
    pub upload_date: String,
    pub title: String,
    pub description: String,
    pub duration: i32,
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub format_id: String,
    pub view_count: i32,
    pub like_count: i32,
    pub dislike_count: i32,
    pub files: Vec<MetadataFileEntry>,
    pub drive_base: String,
    pub archived_timestamp: String,
    pub timestamps: MetadataTimestamps,
}

#[derive(Serialize)]
pub struct MetadataFileEntry {
    pub name: String,
    pub size: i32,
}

#[derive(Serialize, Deserialize)]
pub struct MetadataTimestamps {
    #[serde(rename = "actualStartTime")]
    pub actual_start_time: String,
    #[serde(rename = "publishedAt")]
    pub published_at: String,
    #[serde(rename = "scheduledStartTime")]
    pub scheduled_start_time: String,
    #[serde(rename = "actualEndTime")]
    pub actual_end_time: String,
}

#[async_trait]
pub trait ArchiveSite {
    type Metadata;

    async fn is_archived(&self, id: &str) -> anyhow::Result<bool>;
    async fn archive(&self, id: &str, metadata: &Self::Metadata) -> anyhow::Result<()>;
}

#[async_trait]
pub trait MetadataExtractor {
    type Metadata;

    async fn extract(&self, workdir: &Path) -> anyhow::Result<Self::Metadata>;
}

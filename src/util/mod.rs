use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;

pub mod rclone;
pub mod tasq;
pub mod ytdl;

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
    async fn download(&self, url: &str, destination: &Path) -> anyhow::Result<VideoDownloadResult>;
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

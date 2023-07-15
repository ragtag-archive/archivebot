use super::{Metadata, MetadataExtractor};
use anyhow::Context;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct InfoJson {
    id: String,
    uploader: String,
    channel_id: String,
    upload_date: String,
    title: String,
    description: String,
    duration: u64,
    width: i32,
    height: i32,
    fps: i32,
    format_id: String,
    view_count: u64,
    like_count: u64,
    dislike_count: Option<i64>,
}

pub struct YTMetadataExtractor {
    youtube_api_key: String,
    youtube_api_url: String,
    client: Client,
    drive_base: String,
}

#[derive(Deserialize)]
struct YTTSResponse {
    items: Vec<YTTSItem>,
}
#[derive(Deserialize)]
struct YTTSItem {
    #[serde(rename = "liveStreamingDetails")]
    live_streaming_details: Option<YTTSItemLiveStreamingDetails>,
    snippet: Option<YTTSItemSnippet>,
}
#[derive(Deserialize)]
struct YTTSItemLiveStreamingDetails {
    #[serde(rename = "scheduledStartTime")]
    scheduled_start_time: Option<String>,
    #[serde(rename = "actualStartTime")]
    actual_start_time: Option<String>,
    #[serde(rename = "actualEndTime")]
    actual_end_time: Option<String>,
}
#[derive(Deserialize)]
struct YTTSItemSnippet {
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
}

impl YTMetadataExtractor {
    pub async fn new(
        youtube_api_key: String,
        client: Option<Client>,
        drive_base: String,
    ) -> anyhow::Result<Self> {
        let client = client.unwrap_or_else(|| Client::new());
        let youtube_api_url = "https://youtube.googleapis.com".into();
        Ok(Self {
            youtube_api_key,
            youtube_api_url,
            client,
            drive_base,
        })
    }

    async fn get_timestamps(&self, id: &str) -> anyhow::Result<super::MetadataTimestamps> {
        let url = format!(
            "{}/youtube/v3/videos?part=snippet%2CliveStreamingDetails&id={}&key={}",
            self.youtube_api_url, id, self.youtube_api_key,
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("Could not send request")?
            .error_for_status()
            .context("Unexpected status code")?
            .json::<YTTSResponse>()
            .await
            .context("Could not parse response")?;

        let item = resp
            .items
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No items in response"))?;

        Ok(super::MetadataTimestamps {
            published_at: item.snippet.as_ref().and_then(|s| s.published_at.clone()),
            scheduled_start_time: item
                .live_streaming_details
                .as_ref()
                .and_then(|d| d.scheduled_start_time.clone()),
            actual_start_time: item
                .live_streaming_details
                .as_ref()
                .and_then(|d| d.actual_start_time.clone()),
            actual_end_time: item
                .live_streaming_details
                .as_ref()
                .and_then(|d| d.actual_end_time.clone()),
        })
    }
}

#[async_trait]
impl MetadataExtractor for YTMetadataExtractor {
    async fn extract(&self, workdir: &std::path::Path) -> anyhow::Result<Metadata> {
        // Scan all files in the workdir
        let mut files = vec![];
        let mut dirents = tokio::fs::read_dir(workdir).await?;
        while let Some(entry) = dirents.next_entry().await? {
            let metadata = entry.metadata().await?;
            files.push(super::MetadataFileEntry {
                name: entry
                    .file_name()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("Invalid UTF-8 in filename"))?,
                size: metadata.len(),
            });
        }

        // Look for *.info.json
        let info_json = workdir
            .read_dir()
            .context("Could not read workdir")?
            .find_map(|entry| {
                let path = entry.ok()?.path();
                let fname = path.file_name()?.to_str()?;
                if fname.ends_with(".info.json") {
                    Some(path)
                } else {
                    None
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Could not find info.json"))?;

        // Deserialize
        let info_json = tokio::fs::read_to_string(info_json)
            .await
            .context("Could not read info.json")?;
        let info_json: InfoJson =
            serde_json::from_str(&info_json).context("Could not deserialize info.json")?;

        // Get the timestamps
        let timestamps = self.get_timestamps(&info_json.id).await?;

        // Map the infojson to our metadata
        Ok(Metadata {
            video_id: info_json.id,
            channel_name: info_json.uploader,
            channel_id: info_json.channel_id,
            upload_date: fix_upload_date(&info_json.upload_date),
            title: info_json.title,
            description: info_json.description,
            duration: info_json.duration,
            width: info_json.width,
            height: info_json.height,
            fps: info_json.fps,
            format_id: info_json.format_id,
            view_count: info_json.view_count,
            like_count: info_json.like_count,
            dislike_count: info_json.dislike_count.unwrap_or(-1),
            files,
            drive_base: self.drive_base.clone(),
            archived_timestamp: chrono::Utc::now().to_rfc3339(),
            timestamps: Some(timestamps),
        })
    }
}

/// Convert YYYYMMDD to YYYY-MM-DD
fn fix_upload_date(date: &str) -> String {
    let mut chars = date.chars();
    let year = chars.by_ref().take(4).collect::<String>();
    let month = chars.by_ref().take(2).collect::<String>();
    let day = chars.take(2).collect::<String>();
    format!("{}-{}-{}", year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::mock;

    #[test]
    fn test_fix_upload_date() {
        assert_eq!(fix_upload_date("19840102"), "1984-01-02");
    }

    fn get_mock_yt(api_key: &str, video_id: &str) -> mockito::Mock {
        mock("GET", "/youtube/v3/videos")
            .match_query(mockito::Matcher::UrlEncoded("key".into(), api_key.into()))
            .match_query(mockito::Matcher::UrlEncoded("id".into(), video_id.into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{
                    "kind": "youtube#videoListResponse",
                    "etag": "etag",
                    "items": [
                        {
                            "kind": "youtube#video",
                            "etag": "etag",
                            "id": "id",
                            "snippet": {
                                "publishedAt": "2020-01-01T00:00:00Z",
                                "title": "title",
                                "description": "description",
                                "thumbnails": {
                                    "default": {
                                        "url": "https://example.com/default.jpg",
                                        "width": 120,
                                        "height": 90
                                    },
                                    "medium": {
                                        "url": "https://example.com/medium.jpg",
                                        "width": 320,
                                        "height": 180
                                    },
                                    "high": {
                                        "url": "https://example.com/high.jpg",
                                        "width": 480,
                                        "height": 360
                                    },
                                    "standard": {
                                        "url": "https://example.com/standard.jpg",
                                        "width": 640,
                                        "height": 480
                                    },
                                    "maxres": {
                                        "url": "https://example.com/maxres.jpg",
                                        "width": 1280,
                                        "height": 720
                                    }
                                },
                                "channelTitle": "channelTitle",
                                "tags": [
                                    "tag1",
                                    "tag2"
                                ],
                                "categoryId": "categoryId",
                                "liveBroadcastContent": "liveBroadcastContent",
                                "localized": {
                                    "title": "localizedTitle",
                                    "description": "localizedDescription"
                                }
                            },
                            "liveStreamingDetails": {
                                "actualStartTime": "1111-01-01T00:00:00Z",
                                "actualEndTime": "2222-01-01T00:00:00Z",
                                "scheduledStartTime": "3333-01-01T00:00:00Z",
                                "concurrentViewers": "concurrentViewers",
                                "activeLiveChatId": "activeLiveChatId"
                            }
                        }
                    ]
                }"#,
            )
            .create()
    }

    #[tokio::test]
    async fn test_get_timestamps() {
        let api_key = "test-api-key";
        let video_id = "test-video-id";

        let _m = get_mock_yt(api_key, video_id);
        let mut extractor = YTMetadataExtractor::new("asdf".to_string(), None, "drive".to_string())
            .await
            .unwrap();

        // Override the URL
        extractor.youtube_api_url = mockito::server_url();
        let timestamps = extractor.get_timestamps(video_id).await.unwrap();
        assert_eq!(
            timestamps.actual_start_time.expect("actual_start_time"),
            "1111-01-01T00:00:00Z"
        );
        assert_eq!(
            timestamps.actual_end_time.expect("actual_end_time"),
            "2222-01-01T00:00:00Z"
        );
        assert_eq!(
            timestamps
                .scheduled_start_time
                .expect("scheduled_start_time"),
            "3333-01-01T00:00:00Z"
        );
    }
}

use super::{ArchiveSite, Metadata};
use anyhow::Context;
use async_trait::async_trait;
use serde::Deserialize;

pub struct Ragtag {
    pub url: url::Url,
    pub client: reqwest::Client,
}

impl Ragtag {
    pub async fn new(url: url::Url, client: Option<reqwest::Client>) -> anyhow::Result<Self> {
        let client = client.unwrap_or_else(|| reqwest::Client::new());
        Ok(Self { url, client })
    }
}

#[derive(Deserialize)]
struct SearchResult {
    hits: Hits,
}

#[derive(Deserialize)]
struct Hits {
    total: Total,
}

#[derive(Deserialize)]
struct Total {
    value: u64,
}

#[async_trait]
impl ArchiveSite for Ragtag {
    async fn is_archived(&self, id: &str) -> anyhow::Result<bool> {
        self.client
            .get(
                self.url
                    .join(&format!("api/v1/search?v={}", id))
                    .context("Could not construct search URL")?,
            )
            .send()
            .await
            .context("Could not send search request")?
            .error_for_status()
            .context("Got unexpected status code")?
            .json::<SearchResult>()
            .await
            .map(|r| r.hits.total.value > 0)
            .context("Could not parse search result")
    }

    async fn archive(&self, id: &str, metadata: &Metadata) -> anyhow::Result<()> {
        self.client
            .put(
                self.url
                    .join(&format!("api/v2/archive/{}", id))
                    .context("Could not construct archive URL")?,
            )
            .body(serde_json::to_string(metadata).context("Could not serialize metadata")?)
            .send()
            .await
            .context("Could not send archive request")?
            .error_for_status()
            .context("Got unexpected status code")
            .map(|_| ())
    }
}

pub struct MockRagtag {}

impl MockRagtag {
    pub async fn new() -> anyhow::Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl ArchiveSite for MockRagtag {
    async fn is_archived(&self, _id: &str) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn archive(&self, id: &str, metadata: &Metadata) -> anyhow::Result<()> {
        info!("[Mock] Archived video {} with metadata {:?}", id, metadata);
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use mockito::mock;

    #[tokio::test]
    async fn test_is_archived() {
        let m1 = mock("GET", "/api/v1/search")
            .match_query(mockito::Matcher::UrlEncoded("v".into(), "123".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"hits":{"total":{"value":1}}}"#)
            .expect(1)
            .create();
        let m2 = mock("GET", "/api/v1/search")
            .match_query(mockito::Matcher::UrlEncoded("v".into(), "456".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"hits":{"total":{"value":0}}}"#)
            .expect(1)
            .create();

        let ragtag = Ragtag::new(
            url::Url::parse(&mockito::server_url()).expect("Failed to parse mock URL"),
            None,
        )
        .await
        .unwrap();
        assert!(
            ragtag
                .is_archived("123")
                .await
                .expect("Failed to check if video is archived"),
            "Video should be archived"
        );
        assert!(
            !ragtag
                .is_archived("456")
                .await
                .expect("Failed to check if video is archived"),
            "Video should not be archived"
        );

        m1.assert();
        m2.assert();
    }
}

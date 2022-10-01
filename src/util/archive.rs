use super::ArchiveSite;
use async_trait::async_trait;
use serde::Deserialize;

pub struct Ragtag {
    pub url: url::Url,
    pub client: reqwest::Client,
}

impl Ragtag {
    fn new(url: url::Url, client: reqwest::Client) -> Self {
        Self { url, client }
    }
}

impl Default for Ragtag {
    fn default() -> Self {
        Self::new(
            url::Url::parse("https://archive.ragtag.moe").unwrap(),
            reqwest::Client::new(),
        )
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
                    .map_err(|e| anyhow::anyhow!("Failed to construct search URL: {}", e))?,
            )
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send search request: {}", e))?
            .error_for_status()
            .map_err(|e| anyhow::anyhow!("Search request failed: {}", e))?
            .json::<SearchResult>()
            .await
            .map(|r| r.hits.total.value > 0)
            .map_err(|e| anyhow::anyhow!("Could not parse response: {}", e))
    }

    async fn archive(&self, _id: &str, _metadata: serde_json::Value) -> anyhow::Result<()> {
        unimplemented!();
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
            reqwest::Client::new(),
        );
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

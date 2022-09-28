use reqwest::Client;
use serde::Deserialize;

/// A basic, easy to use task queue service.
pub struct Tasq {
    url: String,
    client: Client,
}

#[derive(Debug, Deserialize)]
struct TasqResponse<T> {
    ok: bool,
    payload: T,
    message: String,
}

#[derive(Debug, Deserialize)]
struct PutResponse {
    key: String,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    tasks: Vec<String>,
    count: usize,
}

#[derive(Debug, Deserialize)]
struct ConsumeResponse {
    key: String,
    data: String,
}

impl Tasq {
    /// Create a new Tasq client. The URL should already include the list ID.
    fn new(url: String, client: Option<Client>) -> Self {
        let client = client.unwrap_or_else(|| Client::new());
        Tasq { url, client }
    }

    /// Insert a new item into the queue. If the item is already in the queue,
    /// its priority will be bumped up by one.
    async fn insert(&self, data: String) -> anyhow::Result<PutResponse> {
        let res = self.client.put(&self.url).body(data).send().await?;
        let res = res.json::<TasqResponse<PutResponse>>().await?;
        Ok(res.payload)
    }

    /// List the first 100 task keys and total count in the specified list,
    /// ordered by priority from highest to lowest.
    async fn list(&self) -> anyhow::Result<ListResponse> {
        let res = self.client.get(&self.url).send().await?;
        let res = res.json::<TasqResponse<ListResponse>>().await?;
        Ok(res.payload)
    }

    /// Consume an item from the queue. Once consumed, the item will be removed
    /// from the list. The item with the highest priority will be consumed first.
    /// If the queue is empty, this will return an error.
    async fn consume(&self) -> anyhow::Result<ConsumeResponse> {
        let res = self.client.post(&self.url).send().await?;
        let res = res.json::<TasqResponse<ConsumeResponse>>().await?;
        Ok(res.payload)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use mockito::mock;

    #[tokio::test]
    async fn test_put() {
        let mock = mock("PUT", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok":true,"payload":{"key":"test:wowzers"},"message":""}"#)
            .expect(1)
            .create();

        let tasq = Tasq::new(mockito::server_url(), None);
        let res = tasq
            .insert("wowzers".to_string())
            .await
            .expect("failed to insert");
        assert_eq!(res.key, "test:wowzers");

        mock.assert();
    }

    #[tokio::test]
    async fn test_list() {
        let mock = mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"ok":true,"payload":{"tasks":["test:wowzers"],"count":1},"message":""}"#)
            .expect(1)
            .create();

        let tasq = Tasq::new(mockito::server_url(), None);
        let res = tasq.list().await.expect("failed to list");
        assert_eq!(res.tasks, vec!["test:wowzers"]);
        assert_eq!(res.count, 1);

        mock.assert();
    }

    #[tokio::test]
    async fn test_consume() {
        let mock = mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"ok":true,"payload":{"key":"test:wowzers","data":"wowzers"},"message":""}"#,
            )
            .expect(1)
            .create();

        let tasq = Tasq::new(mockito::server_url(), None);
        let res = tasq.consume().await.expect("failed to consume");
        assert_eq!(res.key, "test:wowzers");
        assert_eq!(res.data, "wowzers");

        mock.assert();
    }
}

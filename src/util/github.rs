use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

static USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("CARGO_PKG_REPOSITORY"),
    ")"
);

pub async fn get_latest_release(repo: &str, client: Option<Client>) -> anyhow::Result<Release> {
    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);

    let client = client.unwrap_or_else(Client::new);
    let req = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", USER_AGENT)
        .build()?;

    Ok(client.execute(req).await?.json().await?)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_get_latest_release() {
        let release = get_latest_release("yt-dlp/yt-dlp", None).await.unwrap();
        // yt-dlp creates releases with the format yyyy.mm.dd -> 10 chars
        assert_eq!(release.tag_name.len(), 10, "Unexpected tag name length");
        assert!(!release.assets.is_empty(), "No assets found");

        // Check that the assets are named correctly
        let asset_names = release
            .assets
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>();
        assert!(
            asset_names.iter().any(|n| n == "yt-dlp.exe"),
            "Missing yt-dlp.exe"
        );
        assert!(asset_names.iter().any(|n| n == "yt-dlp"), "Missing yt-dlp");
    }
}

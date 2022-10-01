pub struct Config {
    pub tasq_url: String,
    pub rclone_remote_name: String,
    pub rclone_base_directory: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let config = Config {
            tasq_url: std::env::var("TASQ_URL")?,
            rclone_remote_name: std::env::var("RCLONE_REMOTE_NAME")?,
            rclone_base_directory: std::env::var("RCLONE_BASE_DIRECTORY")?,
        };
        Ok(config)
    }
}

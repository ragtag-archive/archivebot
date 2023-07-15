use anyhow::Context;

// Macro to generate a config struct from a list of fields.
macro_rules! envcfg {
    ($($name:ident),*) => {
        pub struct Config {
            $(
                pub $name: String,
            )*
        }

        impl Config {
            pub fn from_env() -> anyhow::Result<Self> {
                Ok(Config {
                    $(
                        $name: std::env::var(stringify!($name).to_string().to_uppercase())
                            .with_context(|| format!("Missing environment variable {}", stringify!($name).to_string().to_uppercase()))?,
                    )*
                })
            }
        }
    };
}

envcfg!(
    archive_base_url,
    tasq_url,
    rclone_config_data,
    rclone_remote_name,
    rclone_base_directory,
    drive_base,
    youtube_api_key
);

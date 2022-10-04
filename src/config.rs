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
                            .map_err(|_| anyhow::anyhow!("Missing environment variable {}", stringify!($name).to_string().to_uppercase()))?,
                    )*
                })
            }
        }
    };
}

envcfg!(
    tasq_url,
    rclone_remote_name,
    rclone_base_directory,
    youtube_api_key
);

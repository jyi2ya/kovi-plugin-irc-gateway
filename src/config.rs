use tokio_listener::{ListenerAddress, UserOptions};

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum ImageMode {
    #[serde(rename = "ascii_art")]
    AsciiArt,
    #[serde(rename = "placeholder")]
    Placeholder,
}

impl Default for ImageMode {
    fn default() -> Self {
        Self::AsciiArt
    }
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Config {
    #[serde(default = "rvs_default_bind_addr")]
    pub bind_addr: ListenerAddress,
    #[serde(default)]
    pub user_options: UserOptions,
    #[serde(default)]
    pub image_mode: ImageMode,
}

fn rvs_default_bind_addr() -> ListenerAddress {
    "0.0.0.0:8621".parse().unwrap()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: rvs_default_bind_addr(),
            user_options: UserOptions::default(),
            image_mode: ImageMode::default(),
        }
    }
}

use tokio_listener::{ListenerAddress, UserOptions};

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub enum ImageMode {
    #[default]
    #[serde(rename = "ascii_art")]
    AsciiArt,
    #[serde(rename = "placeholder")]
    Placeholder,
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

/// Returns the default bind address `0.0.0.0:8621`.
///
/// # Panics
///
/// Never panics: hardcoded valid address.
fn rvs_default_bind_addr() -> ListenerAddress {
    "0.0.0.0:8621".parse().expect("never: valid socket address")
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

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(name: &str, content: &str) {
        let path = format!("test_out/test_{}.out", name);
        std::fs::write(&path, content).unwrap();
    }

    #[test]
    fn test_20260528_default_bind_addr() {
        let result = rvs_default_bind_addr();
        snapshot("20260528_default_bind_addr", &format!("{result:?}"));
    }
}

use tokio_listener::{ListenerAddress, UserOptions};

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Config {
    #[serde(default = "rvs_default_bind_addr")]
    pub bind_addr: ListenerAddress,
    #[serde(default)]
    pub user_options: UserOptions,
}

fn rvs_default_bind_addr() -> ListenerAddress {
    "0.0.0.0:8621".parse().unwrap()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_addr: rvs_default_bind_addr(),
            user_options: UserOptions::default(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IrcGatewayError {
    #[error("irc connection broken")]
    ConnectionBroken(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("failed to bind irc server to {addr}")]
    BindFailed {
        addr: std::net::SocketAddr,
        #[source]
        source: std::io::Error,
    },
}

impl IrcGatewayError {
    #[allow(non_snake_case)]
    pub fn rvs_connection_broken<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::ConnectionBroken(Box::new(e))
    }
}

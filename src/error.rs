#[derive(Debug, thiserror::Error)]
pub enum IrcGatewayError {
    #[error("irc connection broken")]
    ConnectionBroken(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("failed to bind irc server to {bind_addr}")]
    BindFailed {
        bind_addr: String,
        #[source]
        source: std::io::Error,
    },
}

impl IrcGatewayError {
    /// Constructs a `ConnectionBroken` error wrapping the underlying cause.
    pub fn connection_broken<E>(e: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::ConnectionBroken(Box::new(e))
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "test assertions may panic")]
mod tests {
    use super::*;

    fn snapshot(name: &str, content: &str) {
        let path = format!("test_out/test_{}.out", name);
        std::fs::write(&path, content).unwrap();
    }

    #[test]
    fn test_20260528_connection_broken() {
        let result = IrcGatewayError::connection_broken(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "reset",
        ));
        assert!(matches!(result, IrcGatewayError::ConnectionBroken(_)));
        snapshot("20260528_connection_broken", &format!("{result:?}"));
    }
}

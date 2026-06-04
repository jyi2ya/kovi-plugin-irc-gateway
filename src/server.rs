use std::sync::Arc;

use kovi::log;
use kovi::tokio::sync::broadcast;
use tokio_listener::{Listener, ListenerAddress, SystemOptions, UserOptions};

use crate::irc_handler::rvs_handle_irc_connection_ABIMPS;
use crate::onebot::RenderedOnebotMessage;

/// Runs the IRC server, accepting connections and spawning handlers for each.
///
/// # Panics
///
/// Panics if spawning a connection handler fails.
pub async fn rvs_irc_server_main_ABIMPS(
    bind_addr: ListenerAddress,
    user_options: UserOptions,
    system_options: SystemOptions,
    broadcast_tx: broadcast::Sender<RenderedOnebotMessage>,
    bot: Arc<kovi::RuntimeBot>,
) -> Result<(), crate::error::IrcGatewayError> {
    let mut listener = Listener::bind(&bind_addr, &system_options, &user_options)
        .await
        .map_err(|e| crate::error::IrcGatewayError::BindFailed {
            bind_addr: format!("{bind_addr}"),
            source: e,
        })?;
    log::info!("IRC gateway listening on {bind_addr}");
    loop {
        match listener.accept().await {
            Ok((conn, addr)) => {
                log::info!("incoming IRC connection from {addr}");
                let rx = broadcast_tx.subscribe();
                let bot = Arc::clone(&bot);
                kovi::spawn(rvs_handle_irc_connection_ABIMPS(conn, rx, bot));
            }
            Err(e) => {
                log::warn!("accept error: {e}");
            }
        }
    }
}

use std::sync::Arc;

use kovi::log;
use kovi::tokio;
use kovi::tokio::sync::broadcast;

use crate::irc_handler::rvs_handle_irc_connection_ABEIMP;
use crate::onebot::RenderedOnebotMessage;

#[allow(non_snake_case)]
pub async fn rvs_irc_server_main_ABEIMP(
    bind_addr: std::net::SocketAddr,
    broadcast_tx: broadcast::Sender<RenderedOnebotMessage>,
    bot: Arc<kovi::RuntimeBot>,
) -> Result<(), crate::error::IrcGatewayError> {
    let acceptor = tokio::net::TcpListener::bind(bind_addr)
        .await
        .map_err(|e| crate::error::IrcGatewayError::BindFailed {
            addr: bind_addr,
            source: e,
        })?;
    log::info!("IRC gateway listening on {bind_addr}");
    loop {
        match acceptor.accept().await {
            Ok((conn, peer)) => {
                log::info!("incoming IRC connection from {peer}");
                let rx = broadcast_tx.subscribe();
                let bot = Arc::clone(&bot);
                tokio::spawn(rvs_handle_irc_connection_ABEIMP(conn, rx, bot));
            }
            Err(e) => {
                log::warn!("accept error: {e}");
            }
        }
    }
}

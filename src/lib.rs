mod config;
mod error;
mod irc_handler;
mod irc_proto;
mod onebot;
mod server;

use std::sync::Arc;

use config::Config;

#[allow(non_snake_case)]
#[kovi::plugin]
async fn rvs_plugin_main_ABEIMP() {
    let bot = kovi::PluginBuilder::get_runtime_bot();
    let data_path = bot.get_data_path();
    let config_path = data_path.join("config.toml");
    let config: Config = kovi::utils::load_toml_data(Config::default(), config_path).unwrap();

    let broadcast_tx = kovi::tokio::sync::broadcast::Sender::new(16);
    let _irc = kovi::spawn(server::rvs_irc_server_main_ABEIMP(
        config.bind_addr,
        broadcast_tx.clone(),
        Arc::clone(&bot),
    ));

    let broadcast_tx = Arc::new(broadcast_tx);

    kovi::PluginBuilder::on_msg(move |event: Arc<kovi::MsgEvent>| {
        let broadcast_tx: Arc<kovi::tokio::sync::broadcast::Sender<onebot::RenderedOnebotMessage>> = Arc::clone(&broadcast_tx);
        let bot = Arc::clone(&bot);
        async move {
            let rendered = onebot::RenderedOnebotMessage::rvs_from_msg_event_AEIP(
                event.as_ref(),
                bot,
            )
            .await;
            let _ = broadcast_tx.send(rendered);
        }
    })
}

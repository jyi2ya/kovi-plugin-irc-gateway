mod config;
mod error;
mod irc_handler;
mod irc_proto;
mod onebot;
mod onebot_types;
mod server;

use std::sync::Arc;

use config::Config;
use kovi_onebot::EventRegistrar;

#[kovi::plugin]
/// Entry point for the kovi plugin: loads config and starts the IRC gateway.
///
/// # Panics
///
/// Panics if the config file cannot be loaded or parsed.
async fn plugin_main() {
    let bot = kovi::PluginBuilder::get_runtime_bot();
    let data_path = bot.get_data_path();
    let config_path = data_path.join("config.toml");
    let config: Config = kovi::utils::load_toml_data(Config::default(), config_path)
        .expect("never: config is valid TOML with defaults");

    let broadcast_tx = kovi::tokio::sync::broadcast::Sender::new(16);
    let _irc = kovi::spawn(server::irc_server_main(
        config.bind_addr,
        config.user_options,
        Default::default(),
        broadcast_tx.clone(),
        Arc::clone(&bot),
    ));

    let broadcast_tx = Arc::new(broadcast_tx);
    let config = Arc::new(config.image_mode.clone());

    kovi::PluginBuilder::on_msg(move |event: Arc<kovi_onebot::MsgEvent>| {
        let broadcast_tx: Arc<kovi::tokio::sync::broadcast::Sender<onebot::RenderedOnebotMessage>> =
            Arc::clone(&broadcast_tx);
        let bot = Arc::clone(&bot);
        let image_mode = config.clone();
        async move {
            let rendered =
                onebot::RenderedOnebotMessage::from_msg_event(event.as_ref(), bot, &image_mode)
                    .await;
            drop(broadcast_tx.send(rendered));
        }
    })
}

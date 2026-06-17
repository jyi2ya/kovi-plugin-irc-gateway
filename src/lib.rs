#![expect(
    non_snake_case,
    reason = "rvs_ functions use uppercase capability suffixes (A/B/I/M/P/S/T/U)"
)]

mod config;
mod error;
mod irc_handler;
mod irc_proto;
mod onebot;
mod onebot_types;
mod server;

use std::sync::Arc;

use config::Config;

#[kovi::plugin]
/// Entry point for the kovi plugin: loads config and starts the IRC gateway.
///
/// # Panics
///
/// Panics if the config file cannot be loaded or parsed.
async fn rvs_plugin_main_ABIS() {
    let bot = kovi::PluginBuilder::get_runtime_bot();
    let data_path = bot.get_data_path();
    let config_path = data_path.join("config.toml");
    let config: Config = kovi::utils::load_toml_data(Config::default(), config_path)
        .expect("never: config is valid TOML with defaults");

    let broadcast_tx = kovi::tokio::sync::broadcast::Sender::new(16);
    let _irc = kovi::spawn(server::rvs_irc_server_main_ABIS(
        config.bind_addr,
        config.user_options,
        Default::default(),
        broadcast_tx.clone(),
        Arc::clone(&bot),
    ));

    let broadcast_tx = Arc::new(broadcast_tx);
    let config = Arc::new(config.image_mode.clone());

    kovi::PluginBuilder::on_msg(move |event: Arc<kovi::MsgEvent>| {
        let broadcast_tx: Arc<kovi::tokio::sync::broadcast::Sender<onebot::RenderedOnebotMessage>> =
            Arc::clone(&broadcast_tx);
        let bot = Arc::clone(&bot);
        let image_mode = config.clone();
        async move {
            let rendered = onebot::RenderedOnebotMessage::rvs_from_msg_event_ABIS(
                event.as_ref(),
                bot,
                &image_mode,
            )
            .await;
            drop(broadcast_tx.send(rendered));
        }
    })
}

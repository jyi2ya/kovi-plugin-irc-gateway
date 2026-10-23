use kovi::build_bot;
use kovi::tokio;

#[tokio::main]
async fn main() {
    let driver = match kovi_onebot::load_local_conf() {
        Ok(config) => kovi_onebot::OneBotDriver::new(config),
        Err(err) => {
            kovi::log::error!("failed to load kovi config: {err}");
            std::process::exit(1);
        }
    };
    let bot = build_bot!(driver; kovi_plugin_irc_gateway);
    match bot.run().await {
        kovi::ExitEvent::FromDrive => kovi::log::info!("bot exited by driver"),
        kovi::ExitEvent::FromSignal => kovi::log::info!("bot exited by signal"),
    }
}

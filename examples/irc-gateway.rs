use kovi::build_bot;

fn main() {
    let bot = build_bot!(kovi_plugin_irc_gateway);
    bot.run();
}

use std::sync::Arc;

use futures::{Sink, SinkExt, Stream, StreamExt};
use irc_proto::{Command, Message};
use kovi::log;
use kovi::tokio;
use kovi::tokio::sync::{Mutex, watch};
use tokio_util::codec::Decoder;
use tokio_util::sync::CancellationToken;

use crate::error::IrcGatewayError;
use crate::irc_proto::*;
use crate::onebot::RenderedOnebotMessage;

#[derive(Debug)]
pub struct ParsedChannel {
    pub raw: String,
    pub group_id: Option<i64>,
}

#[allow(non_snake_case)]
pub fn rvs_parse_join_channels(chanlist: &str) -> Vec<ParsedChannel> {
    chanlist
        .split(',')
        .filter_map(|ch| {
            let ch = ch.trim();
            if ch.is_empty() {
                return None;
            }
            let stripped = ch.strip_prefix('#').unwrap_or(ch);
            let digits = rvs_extract_digits(stripped);
            let group_id = if digits.is_empty() {
                None
            } else {
                digits.parse::<i64>().ok()
            };
            Some(ParsedChannel {
                raw: ch.to_owned(),
                group_id,
            })
        })
        .collect()
}

#[allow(non_snake_case)]
pub async fn rvs_send_messages_ABEIM<OUT>(
    irc_tx: &Arc<Mutex<OUT>>,
    messages: Vec<Message>,
) -> Result<(), IrcGatewayError>
where
    OUT: Sink<Message> + Unpin,
    <OUT as futures::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    if messages.is_empty() {
        return Ok(());
    }
    let mut tx = irc_tx.lock().await;
    for msg in messages {
        log::debug!("-> {msg}");
        tx.feed(msg)
            .await
            .map_err(IrcGatewayError::rvs_connection_broken)?;
    }
    tx.flush()
        .await
        .map_err(IrcGatewayError::rvs_connection_broken)?;
    Ok(())
}

#[allow(non_snake_case)]
pub async fn rvs_handle_irc_messages_ABEIMP<IN, OUT, E>(
    mut irc_rx: IN,
    irc_tx: Arc<Mutex<OUT>>,
    nick_tx: watch::Sender<String>,
    bot: Arc<kovi::RuntimeBot>,
    cancel_signal: CancellationToken,
) -> Result<(), IrcGatewayError>
where
    IN: Stream<Item = Result<Message, E>> + Unpin,
    OUT: Sink<Message> + Unpin,
    E: std::fmt::Debug,
    <OUT as futures::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    let mut nick = String::new();
    let mut user = String::new();
    let mut reg_state = RegistrationState::Pending {
        got_nick: false,
        got_user: false,
    };
    let mut cap_negotiating = false;

    loop {
        let next_item = tokio::select! {
            _ = cancel_signal.cancelled() => {
                return Ok(());
            },
            next_item = irc_rx.next() => next_item,
        };

        let next_frame = match next_item {
            Some(frame) => frame,
            None => {
                log::info!("irc client disconnected");
                return Ok(());
            }
        };

        let message = match next_frame {
            Ok(message) => message,
            Err(err) => {
                log::warn!("invalid irc message received: {:?}", err);
                continue;
            }
        };

        log::debug!("<- {message}");

        let reply = rvs_dispatch_irc_command_ABEIMP(
            &message.command,
            &mut nick,
            &mut user,
            &mut reg_state,
            &mut cap_negotiating,
            &nick_tx,
            &bot,
        )
        .await;

        if let Err(e) = rvs_send_messages_ABEIM(&irc_tx, reply).await {
            log::warn!("failed to send IRC reply: {e}");
            return Ok(());
        }
    }
}

#[allow(non_snake_case)]
fn rvs_make_now_str_P() -> String {
    rvs_format_utc_timestamp(rvs_current_utc_secs_P())
}

#[allow(non_snake_case)]
async fn rvs_execute_irc_effects_ABEIMP(
    effects: Vec<IrcCommandEffect>,
    nick: &mut String,
    user: &mut String,
    reg_state: &mut RegistrationState,
    cap_negotiating: &mut bool,
    nick_tx: &watch::Sender<String>,
    bot: &Arc<kovi::RuntimeBot>,
) -> Vec<Message> {
    let mut replies = Vec::new();
    for effect in effects {
        match effect {
            IrcCommandEffect::Reply(msgs) => replies.extend(msgs),
            IrcCommandEffect::SetNick(n) => *nick = n,
            IrcCommandEffect::SetUser(u) => *user = u,
            IrcCommandEffect::SetRegState(s) => *reg_state = s,
            IrcCommandEffect::SetCapNegotiating(v) => *cap_negotiating = v,
            IrcCommandEffect::BroadcastNick(n) => {
                let _ = nick_tx.send(n);
            }
            IrcCommandEffect::AsyncJoin(chanlist) => {
                replies.extend(rvs_handle_join_ABEIP(nick, user, &chanlist, bot).await);
            }
            IrcCommandEffect::SendPrivmsg { target, text } => {
                rvs_handle_privmsg_P(&target, &text, bot);
            }
            IrcCommandEffect::LogDebug(msg) => log::debug!("{}", msg),
        }
    }
    replies
}

#[allow(non_snake_case)]
async fn rvs_dispatch_irc_command_ABEIMP(
    command: &Command,
    nick: &mut String,
    user: &mut String,
    reg_state: &mut RegistrationState,
    cap_negotiating: &mut bool,
    nick_tx: &watch::Sender<String>,
    bot: &Arc<kovi::RuntimeBot>,
) -> Vec<Message> {
    let now_str = rvs_make_now_str_P();
    let effects = rvs_plan_irc_command(command, nick, user, reg_state, *cap_negotiating, &now_str);
    rvs_execute_irc_effects_ABEIMP(effects, nick, user, reg_state, cap_negotiating, nick_tx, bot).await
}

#[allow(non_snake_case)]
fn rvs_plan_join_effects(
    channels: &[ParsedChannel],
    nick: &str,
) -> (Vec<Message>, Vec<(String, i64)>) {
    let mut immediate = Vec::new();
    let mut lookups = Vec::new();
    for ch in channels {
        match ch.group_id {
            Some(id) => lookups.push((ch.raw.clone(), id)),
            None => immediate.push(rvs_build_no_such_channel(nick, &ch.raw)),
        }
    }
    (immediate, lookups)
}

#[allow(non_snake_case)]
async fn rvs_handle_join_ABEIP(
    nick: &str,
    user: &str,
    chanlist: &str,
    bot: &Arc<kovi::RuntimeBot>,
) -> Vec<Message> {
    let channels = rvs_parse_join_channels(chanlist);
    let (mut reply, lookups) = rvs_plan_join_effects(&channels, nick);
    for (_raw, group_id) in lookups {
        let group_name = match bot.get_group_info(group_id, false).await {
            Ok(resp) => resp.data["group_name"]
                .as_str()
                .unwrap_or("Unknown Group")
                .to_owned(),
            Err(_) => "无法获取群名".to_owned(),
        };
        reply.extend(rvs_build_join_reply(nick, user, group_id, &group_name));
    }
    reply
}

#[allow(non_snake_case)]
fn rvs_handle_privmsg_P(target: &str, text: &str, bot: &Arc<kovi::RuntimeBot>) {
    match rvs_resolve_privmsg_target(target) {
        PrivmsgTarget::Group(group_id) => {
            bot.send_group_msg(group_id, text.to_owned());
        }
        PrivmsgTarget::Private(peer_id) => {
            bot.send_private_msg(peer_id, text.to_owned());
        }
        PrivmsgTarget::Invalid(t) => {
            log::warn!("PRIVMSG to invalid target: {t}");
        }
    }
}

#[allow(non_snake_case)]
pub async fn rvs_handle_onebot_messages_ABEIM<OUT>(
    mut onebot_rx: kovi::tokio::sync::broadcast::Receiver<RenderedOnebotMessage>,
    irc_tx: Arc<Mutex<OUT>>,
    nick_rx: watch::Receiver<String>,
    cancel_signal: CancellationToken,
) -> Result<(), IrcGatewayError>
where
    OUT: Sink<Message> + Unpin,
    <OUT as futures::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    loop {
        let recv_item = tokio::select! {
            _ = cancel_signal.cancelled() => {
                return Ok(());
            }
            recv_item = onebot_rx.recv() => recv_item,
        };

        let message = match recv_item {
            Ok(msg) => msg,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                log::warn!("onebot broadcast lagged, dropped {n} messages");
                continue;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                return Ok(());
            }
        };

        let my_nick = nick_rx.borrow().clone();
        let messages = rvs_render_onebot_to_irc(message, &my_nick);

        if let Err(e) = rvs_send_messages_ABEIM(&irc_tx, messages).await {
            log::warn!("irc connection broken while forwarding onebot message: {e}");
            return Ok(());
        }
    }
}

#[allow(non_snake_case)]
fn rvs_render_onebot_to_irc(message: RenderedOnebotMessage, my_nick: &str) -> Vec<Message> {
    use irc_proto::Prefix;

    match message {
        RenderedOnebotMessage::Group {
            content,
            sender_name,
            group_id,
            sender_id,
        } => {
            let nick_prefix = Prefix::Nickname(
                sender_name,
                sender_id.to_string(),
                rvs_server_name().to_owned(),
            );
            content
                .into_iter()
                .filter(|line| !line.is_empty())
                .map(|line| {
                    rvs_prefixed(
                        nick_prefix.clone(),
                        Message::from(Command::PRIVMSG(format!("#{group_id}"), line)),
                    )
                })
                .collect()
        }
        RenderedOnebotMessage::Private {
            content,
            sender_id,
            sender_name,
        } => {
            let nick_prefix = Prefix::Nickname(
                sender_name,
                sender_id.to_string(),
                rvs_server_name().to_owned(),
            );
            let target = if my_nick.is_empty() {
                "you".to_owned()
            } else {
                my_nick.to_owned()
            };
            content
                .into_iter()
                .filter(|line| !line.is_empty())
                .map(|line| {
                    rvs_prefixed(
                        nick_prefix.clone(),
                        Message::from(Command::PRIVMSG(target.clone(), line)),
                    )
                })
                .collect()
        }
    }
}

#[allow(non_snake_case)]
pub async fn rvs_handle_irc_connection_ABEIMP(
    conn: tokio::net::TcpStream,
    onebot_rx: tokio::sync::broadcast::Receiver<RenderedOnebotMessage>,
    bot: Arc<kovi::RuntimeBot>,
) {
    let codec = irc_proto::IrcCodec::new("utf-8")
        .expect("utf-8 is always a valid encoding");
    let irc_conn = codec.framed(conn);
    let (irc_tx, irc_rx) = irc_conn.split();
    let irc_tx = Arc::new(Mutex::new(irc_tx));
    let shutdown_token = CancellationToken::new();
    let (nick_tx, nick_rx) = watch::channel(String::new());

    let irc_message_task = tokio::spawn(rvs_handle_irc_messages_ABEIMP(
        irc_rx,
        irc_tx.clone(),
        nick_tx,
        bot,
        shutdown_token.clone(),
    ));
    let onebot_message_task = tokio::spawn(rvs_handle_onebot_messages_ABEIM(
        onebot_rx,
        irc_tx,
        nick_rx,
        shutdown_token.clone(),
    ));

    tokio::select! {
        result = irc_message_task => {
            if let Ok(Err(e)) = result {
                log::warn!("irc message handler error: {e}");
            }
        }
        result = onebot_message_task => {
            if let Ok(Err(e)) = result {
                log::warn!("onebot message handler error: {e}");
            }
        }
    }

    log::info!("irc connection closed, shutting down tasks");
    shutdown_token.cancel();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(name: &str, content: &str) {
        let path = format!("test_out/{}.out", name);
        std::fs::write(&path, content).unwrap();
    }

    #[test]
    fn test_20260418_parse_join_channels_single() {
        let channels = rvs_parse_join_channels("#12345");
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].raw, "#12345");
        assert_eq!(channels[0].group_id, Some(12345));
        snapshot("20260418_parse_join_channels_single", &format!("{:?}", channels));
    }

    #[test]
    fn test_20260418_parse_join_channels_multiple() {
        let channels = rvs_parse_join_channels("#12345,#67890");
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].group_id, Some(12345));
        assert_eq!(channels[1].group_id, Some(67890));
        snapshot("20260418_parse_join_channels_multiple", &format!("{:?}", channels));
    }

    #[test]
    fn test_20260418_parse_join_channels_invalid() {
        let channels = rvs_parse_join_channels("#abc,#12345");
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[0].group_id, None);
        assert_eq!(channels[0].raw, "#abc");
        assert_eq!(channels[1].group_id, Some(12345));
        snapshot("20260418_parse_join_channels_invalid", &format!("{:?}", channels));
    }

    #[test]
    fn test_20260418_parse_join_channels_empty() {
        let channels = rvs_parse_join_channels("");
        assert!(channels.is_empty());
        let channels = rvs_parse_join_channels(",,,");
        assert!(channels.is_empty());
        snapshot("20260418_parse_join_channels_empty", "(empty)");
    }

    #[test]
    fn test_20260418_parse_join_channels_no_hash() {
        let channels = rvs_parse_join_channels("12345");
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].group_id, Some(12345));
        assert_eq!(channels[0].raw, "12345");
        snapshot("20260418_parse_join_channels_no_hash", &format!("{:?}", channels));
    }

    #[test]
    fn test_20260418_plan_join_effects_all_valid() {
        let channels = rvs_parse_join_channels("#111,#222");
        let (replies, lookups) = rvs_plan_join_effects(&channels, "nick");
        assert!(replies.is_empty());
        assert_eq!(lookups.len(), 2);
        assert_eq!(lookups[0], ("#111".to_owned(), 111));
        assert_eq!(lookups[1], ("#222".to_owned(), 222));
        snapshot("20260418_plan_join_effects_all_valid", &format!("{lookups:?}"));
    }

    #[test]
    fn test_20260418_plan_join_effects_mixed() {
        let channels = rvs_parse_join_channels("#abc,#123");
        let (replies, lookups) = rvs_plan_join_effects(&channels, "nick");
        assert_eq!(replies.len(), 1);
        assert_eq!(lookups.len(), 1);
        assert_eq!(lookups[0].1, 123);
        snapshot("20260418_plan_join_effects_mixed", &format!("replies={replies:?}, lookups={lookups:?}"));
    }

    #[test]
    fn test_20260418_plan_join_effects_all_invalid() {
        let channels = rvs_parse_join_channels("#foo,#bar");
        let (replies, lookups) = rvs_plan_join_effects(&channels, "nick");
        assert_eq!(replies.len(), 2);
        assert!(lookups.is_empty());
        snapshot("20260418_plan_join_effects_all_invalid", &format!("{replies:?}"));
    }

    #[test]
    fn test_20260418_plan_join_effects_empty() {
        let channels = rvs_parse_join_channels("");
        let (replies, lookups) = rvs_plan_join_effects(&channels, "nick");
        assert!(replies.is_empty());
        assert!(lookups.is_empty());
        snapshot("20260418_plan_join_effects_empty", "(empty)");
    }
}

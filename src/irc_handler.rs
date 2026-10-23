use std::collections::HashSet;
use std::sync::Arc;

use futures::{Sink, SinkExt, Stream, StreamExt};
use irc_proto::{Command, Message};
use kovi::tokio::sync::Mutex;
use kovi::{log, tokio};
use tokio_util::codec::Decoder;

use crate::error::IrcGatewayError;
use crate::irc_proto::{
    IrcCommandEffect, PrivmsgTarget, RegistrationState, build_join_reply, build_no_such_channel,
    build_no_such_nick, current_utc_secs, extract_channel_id, format_utc_timestamp,
    plan_irc_command, prefixed, resolve_privmsg_target, server_name,
};
use crate::onebot::RenderedOnebotMessage;
use crate::onebot_types::{parse_group_info, parse_stranger_info};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedChannel {
    Group { raw: String, group_id: i64 },
    Private { raw: String, peer_id: i64 },
    Invalid { raw: String },
}

/// Parses a comma-separated IRC channel list into typed channels.
///
/// Channel naming convention:
/// - `#<digits>` → `Group` (OneBot group chat)
/// - `##<digits>` → `Private` (OneBot private chat, disguised as a channel)
/// - anything else → `Invalid`
pub fn parse_join_channels(chanlist: &str) -> Vec<ParsedChannel> {
    let mut seen = std::collections::HashSet::<&str>::new();
    chanlist
        .split(',')
        .filter_map(|ch| {
            let ch = ch.trim();
            if ch.is_empty() || !seen.insert(ch) {
                return None;
            }
            if let Some(stripped) = ch.strip_prefix("##") {
                match extract_channel_id(stripped) {
                    Some(peer_id) => Some(ParsedChannel::Private {
                        raw: ch.to_owned(),
                        peer_id,
                    }),
                    None => Some(ParsedChannel::Invalid { raw: ch.to_owned() }),
                }
            } else if let Some(stripped) = ch.strip_prefix('#') {
                match extract_channel_id(stripped) {
                    Some(group_id) => Some(ParsedChannel::Group {
                        raw: ch.to_owned(),
                        group_id,
                    }),
                    None => Some(ParsedChannel::Invalid { raw: ch.to_owned() }),
                }
            } else {
                Some(ParsedChannel::Invalid { raw: ch.to_owned() })
            }
        })
        .collect()
}

/// Sends a batch of IRC messages through the shared sink.
///
/// Never panics in practice; errors are propagated via `Result`.
pub async fn send_messages<OUT>(
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
            .map_err(IrcGatewayError::connection_broken)?;
    }
    tx.flush()
        .await
        .map_err(IrcGatewayError::connection_broken)?;
    Ok(())
}

/// Reads IRC frames from the client stream and dispatches each command.
///
/// # Panics
///
/// Never panics in practice.
pub async fn handle_irc_messages<IN, OUT, E>(
    mut irc_rx: IN,
    irc_tx: Arc<Mutex<OUT>>,
    bot: Arc<kovi::RuntimeBot>,
    shared_nick_user: Arc<Mutex<(String, String)>>,
    joined_private_channels: Arc<Mutex<HashSet<i64>>>,
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
        let next_item = irc_rx.next().await;

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

        let (reply, should_quit) = dispatch_irc_command(
            &message.command,
            &mut nick,
            &mut user,
            &mut reg_state,
            &mut cap_negotiating,
            &bot,
            &joined_private_channels,
        )
        .await;

        {
            let mut shared = shared_nick_user.lock().await;
            if shared.0 != nick {
                shared.0 = nick.clone();
            }
            if shared.1 != user {
                shared.1 = user.clone();
            }
        }

        if let Err(e) = send_messages(&irc_tx, reply).await {
            log::warn!("failed to send IRC reply: {e}");
            return Ok(());
        }

        if should_quit {
            log::info!("irc client sent QUIT, closing connection");
            return Ok(());
        }
    }
}
/// Returns the current UTC time as a formatted string.
///
/// Reads the system clock (side effect).
fn make_now_str() -> String {
    format_utc_timestamp(current_utc_secs())
}

/// Dispatches IRC command effects (replies, nick changes, joins, etc.).
///
/// # Panics
///
/// Never panics in practice.
async fn execute_irc_effects(
    effects: Vec<IrcCommandEffect>,
    nick: &mut String,
    user: &mut String,
    reg_state: &mut RegistrationState,
    cap_negotiating: &mut bool,
    bot: &Arc<kovi::RuntimeBot>,
    joined_private_channels: &Arc<Mutex<HashSet<i64>>>,
) -> (Vec<Message>, bool) {
    let mut replies = Vec::new();
    let mut should_quit = false;
    for effect in effects {
        match effect {
            IrcCommandEffect::Reply(msgs) => replies.extend(msgs),
            IrcCommandEffect::SetNick(n) => *nick = n,
            IrcCommandEffect::SetUser(u) => *user = u,
            IrcCommandEffect::SetRegState(s) => *reg_state = s,
            IrcCommandEffect::SetCapNegotiating(v) => *cap_negotiating = v,
            IrcCommandEffect::AsyncJoin(chanlist) => {
                let (join_replies, private_ids) = handle_join(nick, user, &chanlist, bot).await;
                if !private_ids.is_empty() {
                    let mut set = joined_private_channels.lock().await;
                    set.extend(private_ids);
                }
                replies.extend(join_replies);
            }
            IrcCommandEffect::SendPrivmsg { target, text } => {
                let privmsg_replies = handle_privmsg(&target, &text, bot, nick);
                replies.extend(privmsg_replies);
            }
            IrcCommandEffect::PartPrivate(peer_ids) => {
                if !peer_ids.is_empty() {
                    let mut set = joined_private_channels.lock().await;
                    set.retain(|id| !peer_ids.contains(id));
                }
            }
            IrcCommandEffect::Quit => should_quit = true,
            IrcCommandEffect::LogDebug(msg) => log::debug!("{}", msg),
        }
    }
    (replies, should_quit)
}

/// Routes an IRC command to the appropriate handler and collects reply messages.
///
/// # Panics
///
/// Never panics in practice.
async fn dispatch_irc_command(
    command: &Command,
    nick: &mut String,
    user: &mut String,
    reg_state: &mut RegistrationState,
    cap_negotiating: &mut bool,
    bot: &Arc<kovi::RuntimeBot>,
    joined_private_channels: &Arc<Mutex<HashSet<i64>>>,
) -> (Vec<Message>, bool) {
    let now_str = make_now_str();
    let effects = plan_irc_command(command, nick, user, reg_state, *cap_negotiating, &now_str);
    execute_irc_effects(
        effects,
        nick,
        user,
        reg_state,
        cap_negotiating,
        bot,
        joined_private_channels,
    )
    .await
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinLookup {
    Group { channel: String, group_id: i64 },
    Private { channel: String, peer_id: i64 },
}

fn plan_join_effects(channels: &[ParsedChannel], nick: &str) -> (Vec<Message>, Vec<JoinLookup>) {
    let mut immediate = Vec::new();
    let mut lookups = Vec::new();
    for ch in channels {
        match ch {
            ParsedChannel::Group { raw, group_id } => {
                lookups.push(JoinLookup::Group {
                    channel: raw.clone(),
                    group_id: *group_id,
                });
            }
            ParsedChannel::Private { raw, peer_id } => {
                lookups.push(JoinLookup::Private {
                    channel: raw.clone(),
                    peer_id: *peer_id,
                });
            }
            ParsedChannel::Invalid { raw } => {
                immediate.push(build_no_such_channel(nick, raw));
            }
        }
    }
    (immediate, lookups)
}

/// Handles a JOIN command: resolves group/peer names and builds join reply messages.
///
/// # Panics
///
/// Never panics in practice.
async fn handle_join(
    nick: &str,
    user: &str,
    chanlist: &str,
    bot: &Arc<kovi::RuntimeBot>,
) -> (Vec<Message>, Vec<i64>) {
    let channels = parse_join_channels(chanlist);
    let (mut reply, lookups) = plan_join_effects(&channels, nick);
    let mut private_ids = Vec::new();
    for lookup in lookups {
        let (channel, topic) = match &lookup {
            JoinLookup::Group { channel, group_id } => {
                let name = match bot.get_group_info(*group_id, false).await {
                    Ok(resp) => parse_group_info(&resp.data)
                        .map(|g| g.group_name.clone())
                        .filter(|s| !s.is_empty())
                        .unwrap_or("未知群聊".to_owned()),
                    Err(_) => "无法获取群名".to_owned(),
                };
                (channel.clone(), name)
            }
            JoinLookup::Private { channel, peer_id } => {
                private_ids.push(*peer_id);
                let name = match bot.get_stranger_info(*peer_id, false).await {
                    Ok(resp) => parse_stranger_info(&resp.data)
                        .map(|s| s.nickname.clone())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| peer_id.to_string()),
                    Err(_) => peer_id.to_string(),
                };
                (channel.clone(), name)
            }
        };
        reply.extend(build_join_reply(nick, user, &channel, &topic));
    }
    (reply, private_ids)
}

/// Dispatches a PRIVMSG to the appropriate OneBot send method based on the resolved target.
///
/// # Panics
///
/// Never panics in practice; all branches perform non-fallible sends or logging.
fn handle_privmsg(
    target: &str,
    text: &str,
    bot: &Arc<kovi::RuntimeBot>,
    nick: &str,
) -> Vec<Message> {
    match resolve_privmsg_target(target) {
        PrivmsgTarget::Group(group_id) => {
            bot.send_group_msg(group_id, text.to_owned());
            vec![]
        }
        PrivmsgTarget::Private(peer_id) => {
            bot.send_private_msg(peer_id, text.to_owned());
            vec![]
        }
        PrivmsgTarget::Invalid(t) => {
            log::warn!("PRIVMSG to invalid target: {t}");
            vec![build_no_such_nick(nick, &t)]
        }
    }
}

/// Forwards OneBot messages to the IRC client as PRIVMSG lines.
///
/// Private chats are disguised as IRC channels with the `##<peer_id>` naming convention.
/// When the first private message from a peer arrives, a virtual JOIN burst
/// (JOIN + RPL_TOPIC + RPL_NAMREPLY + RPL_ENDOFNAMES) is sent so the IRC client
/// treats the channel as joined.
///
/// # Panics
///
/// Never panics in practice.
pub async fn handle_onebot_messages<OUT>(
    mut onebot_rx: kovi::tokio::sync::broadcast::Receiver<RenderedOnebotMessage>,
    irc_tx: Arc<Mutex<OUT>>,
    shared_nick_user: Arc<Mutex<(String, String)>>,
    bot: Arc<kovi::RuntimeBot>,
    joined_private_channels: Arc<Mutex<HashSet<i64>>>,
) -> Result<(), IrcGatewayError>
where
    OUT: Sink<Message> + Unpin,
    <OUT as futures::Sink<Message>>::Error: std::error::Error + Send + Sync + 'static,
{
    loop {
        let recv_item = onebot_rx.recv().await;

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

        if let RenderedOnebotMessage::Private { sender_id, .. } = &message {
            let mut set = joined_private_channels.lock().await;
            if set.insert(*sender_id) {
                let (nick, user) = shared_nick_user.lock().await.clone();
                let join_burst = build_private_join_burst(&nick, &user, *sender_id, &bot).await;
                drop(set);
                if let Err(e) = send_messages(&irc_tx, join_burst).await {
                    log::warn!("irc connection broken during virtual join: {e}");
                    return Ok(());
                }
            }
        }

        let messages = render_onebot_to_irc(message);

        if let Err(e) = send_messages(&irc_tx, messages).await {
            log::warn!("irc connection broken while forwarding onebot message: {e}");
            return Ok(());
        }
    }
}

/// Builds a virtual JOIN burst for a private chat channel.
///
/// # Panics
///
/// Never panics in practice.
async fn build_private_join_burst(
    nick: &str,
    user: &str,
    peer_id: i64,
    bot: &kovi::RuntimeBot,
) -> Vec<Message> {
    debug_assert!(peer_id > 0, "peer_id must be positive");
    let channel = format!("##{peer_id}");
    let nickname = match bot.get_stranger_info(peer_id, false).await {
        Ok(resp) => parse_stranger_info(&resp.data)
            .map(|s| s.nickname.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| peer_id.to_string()),
        Err(_) => peer_id.to_string(),
    };
    let topic = format!("与 {nickname} 的私聊");
    build_join_reply(nick, user, &channel, &topic)
}

fn render_onebot_to_irc(message: RenderedOnebotMessage) -> Vec<Message> {
    use irc_proto::Prefix;

    match message {
        RenderedOnebotMessage::Group {
            content,
            sender_name,
            group_id,
            sender_id,
        } => {
            let nick_prefix =
                Prefix::Nickname(sender_name, sender_id.to_string(), server_name().to_owned());
            content
                .into_iter()
                .filter(|line| !line.is_empty())
                .map(|line| {
                    prefixed(
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
            let nick_prefix =
                Prefix::Nickname(sender_name, sender_id.to_string(), server_name().to_owned());
            let target = format!("##{sender_id}");
            content
                .into_iter()
                .filter(|line| !line.is_empty())
                .map(|line| {
                    prefixed(
                        nick_prefix.clone(),
                        Message::from(Command::PRIVMSG(target.clone(), line)),
                    )
                })
                .collect()
        }
    }
}

/// Sets up codec, splits the connection, and runs the IRC + OneBot message loops concurrently.
///
/// # Panics
///
/// Panics if the IRC codec cannot be created with UTF-8 encoding (should never happen).
pub async fn handle_irc_connection(
    conn: tokio_listener::Connection,
    onebot_rx: tokio::sync::broadcast::Receiver<RenderedOnebotMessage>,
    bot: Arc<kovi::RuntimeBot>,
) {
    let codec = irc_proto::IrcCodec::new("utf-8").expect("utf-8 is always a valid encoding");
    let irc_conn = codec.framed(conn);
    let (irc_tx, irc_rx) = irc_conn.split();
    let irc_tx = Arc::new(Mutex::new(irc_tx));
    let shared_nick_user: Arc<Mutex<(String, String)>> =
        Arc::new(Mutex::new((String::new(), String::new())));
    let joined_private_channels: Arc<Mutex<HashSet<i64>>> = Arc::new(Mutex::new(HashSet::new()));

    // Race the two handler loops; the first one to return ends the connection.
    // Both loops only suspend at message boundaries (broadcast `recv` /
    // `StreamExt::next`), so cancelling the loser never drops a half-read
    // message. Each block logs its own failure, so the select result carries
    // nothing we need; dropping it cancels the still-pending loop.
    let irc_loop = {
        let irc_tx = Arc::clone(&irc_tx);
        let bot = Arc::clone(&bot);
        let shared_nick_user = Arc::clone(&shared_nick_user);
        let joined_private_channels = Arc::clone(&joined_private_channels);
        async move {
            if let Err(e) = handle_irc_messages(
                irc_rx,
                irc_tx,
                bot,
                shared_nick_user,
                joined_private_channels,
            )
            .await
            {
                log::warn!("irc message handler error: {e}");
            }
        }
    };
    let onebot_loop = async move {
        if let Err(e) = handle_onebot_messages(
            onebot_rx,
            irc_tx,
            shared_nick_user,
            bot,
            joined_private_channels,
        )
        .await
        {
            log::warn!("onebot message handler error: {e}");
        }
    };
    drop(futures::future::select(Box::pin(irc_loop), Box::pin(onebot_loop)).await);

    log::info!("irc connection closed");
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "test assertions may panic")]
#[expect(clippy::indexing_slicing, reason = "test assertions use known indices")]
mod tests {
    use super::*;
    use crate::irc_proto::handle_part;

    fn snapshot(name: &str, content: &str) {
        let path = format!("test_out/test_{}.out", name);
        std::fs::write(&path, content).unwrap();
    }

    #[test]
    fn test_20260418_parse_join_channels_single() {
        let channels = parse_join_channels("#12345");
        assert_eq!(channels.len(), 1);
        assert_eq!(
            channels[0],
            ParsedChannel::Group {
                raw: "#12345".to_owned(),
                group_id: 12345,
            }
        );
        snapshot(
            "20260418_parse_join_channels_single",
            &format!("{:?}", channels),
        );
    }

    #[test]
    fn test_20260418_parse_join_channels_multiple() {
        let channels = parse_join_channels("#12345,#67890");
        assert_eq!(channels.len(), 2);
        assert_eq!(
            channels[0],
            ParsedChannel::Group {
                raw: "#12345".to_owned(),
                group_id: 12345,
            }
        );
        assert_eq!(
            channels[1],
            ParsedChannel::Group {
                raw: "#67890".to_owned(),
                group_id: 67890,
            }
        );
        snapshot(
            "20260418_parse_join_channels_multiple",
            &format!("{:?}", channels),
        );
    }

    #[test]
    fn test_20260418_parse_join_channels_invalid() {
        let channels = parse_join_channels("#abc,#12345");
        assert_eq!(channels.len(), 2);
        assert_eq!(
            channels[0],
            ParsedChannel::Invalid {
                raw: "#abc".to_owned()
            }
        );
        assert_eq!(
            channels[1],
            ParsedChannel::Group {
                raw: "#12345".to_owned(),
                group_id: 12345,
            }
        );
        snapshot(
            "20260418_parse_join_channels_invalid",
            &format!("{:?}", channels),
        );
    }

    #[test]
    fn test_20260418_parse_join_channels_private_double_hash() {
        let channels = parse_join_channels("##67890");
        assert_eq!(channels.len(), 1);
        assert_eq!(
            channels[0],
            ParsedChannel::Private {
                raw: "##67890".to_owned(),
                peer_id: 67890,
            }
        );
        snapshot(
            "20260418_parse_join_channels_private_double_hash",
            &format!("{:?}", channels),
        );
    }

    #[test]
    fn test_20260418_parse_join_channels_private_invalid() {
        let channels = parse_join_channels("##abc");
        assert_eq!(channels.len(), 1);
        assert_eq!(
            channels[0],
            ParsedChannel::Invalid {
                raw: "##abc".to_owned()
            }
        );
        snapshot(
            "20260418_parse_join_channels_private_invalid",
            &format!("{:?}", channels),
        );
    }

    #[test]
    fn test_20260418_handle_part_empty() {
        let msgs = handle_part("nick", "user", "", &None);
        assert!(msgs.is_empty());
        snapshot("20260418_handle_part_empty", &format!("{msgs:?}"));
    }

    #[test]
    fn test_20260604_parse_join_channels_rejects_mixed_digits() {
        let channels = parse_join_channels("#12a34,##56b78");
        assert_eq!(channels.len(), 2);
        assert_eq!(
            channels[0],
            ParsedChannel::Invalid {
                raw: "#12a34".to_owned()
            }
        );
        assert_eq!(
            channels[1],
            ParsedChannel::Invalid {
                raw: "##56b78".to_owned()
            }
        );
        snapshot(
            "20260604_parse_join_channels_rejects_mixed_digits",
            &format!("{:?}", channels),
        );
    }

    #[test]
    fn test_20260418_parse_join_channels_no_hash() {
        let channels = parse_join_channels("12345");
        assert_eq!(channels.len(), 1);
        assert_eq!(
            channels[0],
            ParsedChannel::Invalid {
                raw: "12345".to_owned()
            }
        );
        snapshot(
            "20260418_parse_join_channels_no_hash",
            &format!("{:?}", channels),
        );
    }

    #[test]
    fn test_20260418_plan_join_effects_all_valid() {
        let channels = parse_join_channels("#111,#222");
        let (replies, lookups) = plan_join_effects(&channels, "nick");
        assert!(replies.is_empty());
        assert_eq!(lookups.len(), 2);
        assert_eq!(
            lookups[0],
            JoinLookup::Group {
                channel: "#111".to_owned(),
                group_id: 111,
            }
        );
        assert_eq!(
            lookups[1],
            JoinLookup::Group {
                channel: "#222".to_owned(),
                group_id: 222,
            }
        );
        snapshot(
            "20260418_plan_join_effects_all_valid",
            &format!("{lookups:?}"),
        );
    }

    #[test]
    fn test_20260418_plan_join_effects_mixed() {
        let channels = parse_join_channels("#abc,#123,##456");
        let (replies, lookups) = plan_join_effects(&channels, "nick");
        assert_eq!(replies.len(), 1);
        assert_eq!(lookups.len(), 2);
        assert_eq!(
            lookups[0],
            JoinLookup::Group {
                channel: "#123".to_owned(),
                group_id: 123,
            }
        );
        assert_eq!(
            lookups[1],
            JoinLookup::Private {
                channel: "##456".to_owned(),
                peer_id: 456,
            }
        );
        snapshot(
            "20260418_plan_join_effects_mixed",
            &format!("replies={replies:?}, lookups={lookups:?}"),
        );
    }

    #[test]
    fn test_20260418_plan_join_effects_all_invalid() {
        let channels = parse_join_channels("#foo,#bar");
        let (replies, lookups) = plan_join_effects(&channels, "nick");
        assert_eq!(replies.len(), 2);
        assert!(lookups.is_empty());
        snapshot(
            "20260418_plan_join_effects_all_invalid",
            &format!("{replies:?}"),
        );
    }

    #[test]
    fn test_20260418_plan_join_effects_empty() {
        let channels = parse_join_channels("");
        let (replies, lookups) = plan_join_effects(&channels, "nick");
        assert!(replies.is_empty());
        assert!(lookups.is_empty());
        snapshot("20260418_plan_join_effects_empty", "(empty)");
    }

    #[test]
    fn test_20260528_render_onebot_to_irc_group() {
        let msg = RenderedOnebotMessage::Group {
            content: vec!["hello".to_owned()],
            sender_name: "Alice".to_owned(),
            group_id: 12345,
            sender_id: 100,
        };
        let result = render_onebot_to_irc(msg);
        assert_eq!(result.len(), 1);
        snapshot(
            "20260528_render_onebot_to_irc_group",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260528_render_onebot_to_irc_private() {
        let msg = RenderedOnebotMessage::Private {
            content: vec!["hello".to_owned()],
            sender_id: 100,
            sender_name: "Bob".to_owned(),
        };
        let result = render_onebot_to_irc(msg);
        assert_eq!(result.len(), 1);
        snapshot(
            "20260528_render_onebot_to_irc_private",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260604_parse_join_channels_dedup() {
        let channels = parse_join_channels("#12345,#12345,#67890");
        assert_eq!(channels.len(), 2);
        assert_eq!(
            channels[0],
            ParsedChannel::Group {
                raw: "#12345".to_owned(),
                group_id: 12345,
            }
        );
        assert_eq!(
            channels[1],
            ParsedChannel::Group {
                raw: "#67890".to_owned(),
                group_id: 67890,
            }
        );
        snapshot(
            "20260604_parse_join_channels_dedup",
            &format!("{:?}", channels),
        );
    }
}

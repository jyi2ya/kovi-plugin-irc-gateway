use kovi::{serde_json, tokio};

use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

use anyhow::Context;
use futures::{Sink, SinkExt, Stream, StreamExt};
use irc_proto::{CapSubCommand, Command, Message, Prefix, Response};
use kovi::log;
use tokio::sync::{Mutex, watch};
use tokio_util::codec::Decoder;
use tokio_util::sync::CancellationToken;

const SERVER_NAME: &str = "onebotirc.villv.tech";
const SERVER_VERSION: &str = "kovi-irc-gateway-0.1";

fn format_utc_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let mut year = 1970u64;
    let mut remaining_days = days;
    loop {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let month_days: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &md in &month_days {
        if remaining_days < md {
            break;
        }
        remaining_days -= md;
        month += 1;
    }
    let day = remaining_days + 1;
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

fn prefixed(prefix: Prefix, mut message: Message) -> Message {
    message.prefix = Some(prefix);
    message
}

fn server_prefix() -> Prefix {
    Prefix::ServerName(SERVER_NAME.to_owned())
}

fn server_msg(cmd: Command) -> Message {
    prefixed(server_prefix(), Message::from(cmd))
}

fn server_reply(response: Response, args: Vec<String>) -> Message {
    server_msg(Command::Response(response, args))
}

fn parse_prefix_number<N>(value: &str) -> Option<N>
where
    N: std::str::FromStr,
    <N as std::str::FromStr>::Err: std::fmt::Debug,
{
    let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

async fn send_messages<OUT>(irc_tx: &Arc<Mutex<OUT>>, messages: Vec<Message>) -> anyhow::Result<()>
where
    OUT: Sink<Message> + Unpin,
    <OUT as futures::Sink<Message>>::Error: std::error::Error + Send + Sync + Debug + 'static,
{
    if messages.is_empty() {
        return Ok(());
    }
    let mut tx = irc_tx.lock().await;
    for msg in messages {
        log::debug!("-> {msg}");
        tx.feed(msg).await.context("irc connection broken")?;
    }
    tx.flush().await.context("irc connection broken")?;
    Ok(())
}

fn build_welcome_burst(nick: &str) -> Vec<Message> {
    let now = format_utc_now();
    vec![
        server_reply(
            Response::RPL_WELCOME,
            vec![
                nick.to_owned(),
                format!("Welcome to the QQ IRC Gateway {nick}!{nick}@{SERVER_NAME}"),
            ],
        ),
        server_reply(
            Response::RPL_YOURHOST,
            vec![
                nick.to_owned(),
                format!("Your host is {SERVER_NAME}, running version {SERVER_VERSION}"),
            ],
        ),
        server_reply(
            Response::RPL_CREATED,
            vec![nick.to_owned(), format!("This server was created {now}")],
        ),
        server_reply(
            Response::RPL_MYINFO,
            vec![
                nick.to_owned(),
                SERVER_NAME.to_owned(),
                SERVER_VERSION.to_owned(),
                "io".to_owned(),
                "mnt".to_owned(),
            ],
        ),
        server_reply(
            Response::RPL_ISUPPORT,
            vec![
                nick.to_owned(),
                "CASEMAPPING=ascii".to_owned(),
                "CHANTYPES=#".to_owned(),
                "CHANMODES=,,,m".to_owned(),
                "PREFIX=(o)@".to_owned(),
                "NICKLEN=30".to_owned(),
                "are supported by this server".to_owned(),
            ],
        ),
        build_motd_start(nick),
        build_motd_line(nick, "Welcome to the QQ IRC Gateway!"),
        build_motd_line(nick, "Join a QQ group channel with: /JOIN #<group_id>"),
        build_motd_end(nick),
    ]
}

fn build_motd_start(nick: &str) -> Message {
    server_reply(
        Response::RPL_MOTDSTART,
        vec![
            nick.to_owned(),
            format!("- {SERVER_NAME} Message of the Day -"),
        ],
    )
}

fn build_motd_line(nick: &str, line: &str) -> Message {
    server_reply(
        Response::RPL_MOTD,
        vec![nick.to_owned(), format!("- {line}")],
    )
}

fn build_motd_end(nick: &str) -> Message {
    server_reply(
        Response::RPL_ENDOFMOTD,
        vec![nick.to_owned(), "End of /MOTD command.".to_owned()],
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RegistrationState {
    Pending { got_nick: bool, got_user: bool },
    Registered,
}

impl RegistrationState {
    fn is_registered(&self) -> bool {
        matches!(self, RegistrationState::Registered)
    }
}

async fn handle_irc_messages<IN, OUT, E>(
    mut irc_rx: IN,
    irc_tx: Arc<Mutex<OUT>>,
    nick_tx: watch::Sender<String>,
    bot: Arc<kovi::RuntimeBot>,
    cancel_signal: CancellationToken,
) -> anyhow::Result<()>
where
    IN: Stream<Item = Result<Message, E>> + Unpin,
    OUT: Sink<Message> + Unpin,
    E: Debug,
    <OUT as futures::Sink<Message>>::Error: std::error::Error + Send + Sync + Debug + 'static,
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

        let reply: Vec<Message> = match message.command {
            Command::CAP(_target, sub, _param1, param2) => match sub {
                CapSubCommand::LS => {
                    cap_negotiating = true;
                    vec![server_msg(Command::CAP(
                        Some("*".to_owned()),
                        CapSubCommand::LS,
                        None,
                        Some(String::new()),
                    ))]
                }
                CapSubCommand::REQ => {
                    let requested = param2.unwrap_or_default();
                    vec![server_msg(Command::CAP(
                        Some("*".to_owned()),
                        CapSubCommand::NAK,
                        None,
                        Some(requested),
                    ))]
                }
                CapSubCommand::END => {
                    cap_negotiating = false;
                    if let RegistrationState::Pending { got_nick, got_user } = reg_state {
                        if got_nick && got_user {
                            reg_state = RegistrationState::Registered;
                            let _ = nick_tx.send(nick.clone());
                            build_welcome_burst(&nick)
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    }
                }
                _ => Vec::new(),
            },

            Command::NICK(new_nick) => {
                nick = new_nick;
                match &mut reg_state {
                    RegistrationState::Pending { got_nick, got_user } => {
                        *got_nick = true;
                        if *got_user && !cap_negotiating {
                            reg_state = RegistrationState::Registered;
                            let _ = nick_tx.send(nick.clone());
                            build_welcome_burst(&nick)
                        } else {
                            Vec::new()
                        }
                    }
                    RegistrationState::Registered => {
                        let _ = nick_tx.send(nick.clone());
                        vec![prefixed(
                            Prefix::Nickname(nick.clone(), user.clone(), SERVER_NAME.to_owned()),
                            Message::from(Command::NICK(nick.clone())),
                        )]
                    }
                }
            }

            Command::USER(new_user, _mode, _realname) => {
                user = new_user;
                match &mut reg_state {
                    RegistrationState::Pending { got_nick, got_user } => {
                        *got_user = true;
                        if *got_nick && !cap_negotiating {
                            reg_state = RegistrationState::Registered;
                            let _ = nick_tx.send(nick.clone());
                            build_welcome_burst(&nick)
                        } else {
                            Vec::new()
                        }
                    }
                    RegistrationState::Registered => Vec::new(),
                }
            }

            Command::PASS(_) => Vec::new(),

            Command::PING(server1, server2) => {
                vec![server_msg(Command::PONG(server1, server2))]
            }

            Command::QUIT(_message) => {
                return Ok(());
            }

            cmd if !reg_state.is_registered() => {
                log::debug!("ignoring command before registration: {:?}", cmd);
                Vec::new()
            }

            Command::JOIN(chanlist, _chankeys, _realname) => {
                let mut reply = Vec::new();
                for channel in chanlist.split(',') {
                    let channel = channel.trim();
                    if channel.is_empty() {
                        continue;
                    }
                    let stripped = channel.strip_prefix('#').unwrap_or(channel);
                    let group_id: i64 = match parse_prefix_number(stripped) {
                        Some(id) => id,
                        None => {
                            reply.push(server_reply(
                                Response::ERR_NOSUCHCHANNEL,
                                vec![
                                    nick.clone(),
                                    channel.to_owned(),
                                    "No such channel".to_owned(),
                                ],
                            ));
                            continue;
                        }
                    };
                    let group_name = match bot.get_group_info(group_id, false).await {
                        Ok(resp) => resp.data["group_name"]
                            .as_str()
                            .unwrap_or("Unknown Group")
                            .to_owned(),
                        Err(_) => "无法获取群名".to_owned(),
                    };
                    let chan = format!("#{group_id}");
                    reply.extend([
                        prefixed(
                            Prefix::Nickname(nick.clone(), user.clone(), SERVER_NAME.to_owned()),
                            Message::from(Command::JOIN(chan.clone(), None, None)),
                        ),
                        server_reply(
                            Response::RPL_TOPIC,
                            vec![nick.clone(), chan.clone(), group_name],
                        ),
                        server_reply(
                            Response::RPL_NAMREPLY,
                            vec![
                                nick.clone(),
                                "=".to_owned(),
                                chan.clone(),
                                format!("@{nick}"),
                            ],
                        ),
                        server_reply(
                            Response::RPL_ENDOFNAMES,
                            vec![nick.clone(), chan.clone(), "End of /NAMES list.".to_owned()],
                        ),
                    ]);
                }
                reply
            }

            Command::PART(chanlist, comment) => {
                let mut reply = Vec::new();
                for channel in chanlist.split(',') {
                    let channel = channel.trim();
                    if channel.is_empty() {
                        continue;
                    }
                    reply.push(prefixed(
                        Prefix::Nickname(nick.clone(), user.clone(), SERVER_NAME.to_owned()),
                        Message::from(Command::PART(channel.to_owned(), comment.clone())),
                    ));
                }
                reply
            }

            Command::PRIVMSG(target, text) => {
                if let Some(stripped) = target.strip_prefix('#') {
                    match parse_prefix_number::<i64>(stripped) {
                        Some(group_id) => {
                            bot.send_group_msg(group_id, text);
                        }
                        None => {
                            log::warn!("PRIVMSG to invalid channel: {target}");
                        }
                    }
                } else {
                    match parse_prefix_number::<i64>(&target) {
                        Some(peer_id) => {
                            bot.send_private_msg(peer_id, text);
                        }
                        None => {
                            log::warn!("PRIVMSG to invalid target: {target}");
                        }
                    }
                }
                Vec::new()
            }

            Command::NOTICE(_target, _text) => Vec::new(),

            Command::WHO(mask, _is_op) => {
                let mask = mask.unwrap_or_else(|| "*".to_owned());
                vec![server_reply(
                    Response::RPL_ENDOFWHO,
                    vec![nick.clone(), mask, "End of /WHO list.".to_owned()],
                )]
            }

            Command::WHOIS(_target, mask) => {
                vec![
                    server_reply(
                        Response::RPL_WHOISUSER,
                        vec![
                            nick.clone(),
                            mask.clone(),
                            user.clone(),
                            SERVER_NAME.to_owned(),
                            "*".to_owned(),
                            mask.clone(),
                        ],
                    ),
                    server_reply(
                        Response::RPL_WHOISSERVER,
                        vec![
                            nick.clone(),
                            mask.clone(),
                            SERVER_NAME.to_owned(),
                            "QQ IRC Gateway".to_owned(),
                        ],
                    ),
                    server_reply(
                        Response::RPL_ENDOFWHOIS,
                        vec![nick.clone(), mask, "End of /WHOIS list.".to_owned()],
                    ),
                ]
            }

            Command::WHOWAS(nicklist, _count, _target) => {
                vec![server_reply(
                    Response::RPL_ENDOFWHOWAS,
                    vec![nick.clone(), nicklist, "End of WHOWAS.".to_owned()],
                )]
            }

            Command::USERHOST(nicknames) => {
                let replies: Vec<String> = nicknames
                    .iter()
                    .map(|n| format!("{n}=+{n}@{SERVER_NAME}"))
                    .collect();
                vec![server_reply(
                    Response::RPL_USERHOST,
                    vec![nick.clone(), replies.join(" ")],
                )]
            }

            Command::ISON(_nicklist) => {
                vec![server_reply(
                    Response::RPL_ISON,
                    vec![nick.clone(), String::new()],
                )]
            }

            Command::ChannelMODE(channel, _modes) => {
                vec![server_reply(
                    Response::RPL_CHANNELMODEIS,
                    vec![nick.clone(), channel, "+".to_owned()],
                )]
            }

            Command::UserMODE(_nickname, _modes) => Vec::new(),

            Command::TOPIC(channel, new_topic) => match new_topic {
                None => {
                    vec![server_reply(
                        Response::RPL_NOTOPIC,
                        vec![nick.clone(), channel, "No topic is set.".to_owned()],
                    )]
                }
                Some(_) => Vec::new(),
            },

            Command::NAMES(chanlist, _target) => {
                let chan = chanlist.unwrap_or_default();
                vec![server_reply(
                    Response::RPL_ENDOFNAMES,
                    vec![nick.clone(), chan, "End of /NAMES list.".to_owned()],
                )]
            }

            Command::LIST(chanlist, _target) => {
                let _ = chanlist;
                vec![server_reply(
                    Response::RPL_LISTEND,
                    vec![nick.clone(), "End of /LIST".to_owned()],
                )]
            }

            Command::MOTD(_target) => {
                vec![
                    build_motd_start(&nick),
                    build_motd_line(&nick, "Welcome to the QQ IRC Gateway!"),
                    build_motd_line(&nick, "Join a QQ group channel with: /JOIN #<group_id>"),
                    build_motd_end(&nick),
                ]
            }

            Command::VERSION(_target) => {
                vec![server_reply(
                    Response::RPL_VERSION,
                    vec![
                        nick.clone(),
                        SERVER_VERSION.to_owned(),
                        SERVER_NAME.to_owned(),
                        "QQ IRC Gateway".to_owned(),
                    ],
                )]
            }

            Command::TIME(_target) => {
                let now = format_utc_now();
                vec![server_reply(
                    Response::RPL_TIME,
                    vec![nick.clone(), SERVER_NAME.to_owned(), now],
                )]
            }

            Command::INFO(_target) => {
                vec![server_reply(
                    Response::RPL_INFO,
                    vec![nick.clone(), "QQ IRC Gateway by kovi".to_owned()],
                )]
            }

            Command::LUSERS(_mask, _target) => {
                vec![
                    server_reply(
                        Response::RPL_LUSERCLIENT,
                        vec![nick.clone(), "There is 1 user on 1 server".to_owned()],
                    ),
                    server_reply(
                        Response::RPL_LUSERME,
                        vec![nick.clone(), "I have 1 client and 0 servers".to_owned()],
                    ),
                ]
            }

            Command::STATS(_query, _target) => Vec::new(),

            Command::LINKS(_remote, _mask) => {
                vec![server_reply(
                    Response::RPL_ENDOFLINKS,
                    vec![
                        nick.clone(),
                        "*".to_owned(),
                        "End of /LINKS list.".to_owned(),
                    ],
                )]
            }

            Command::ADMIN(_target) => {
                vec![server_reply(
                    Response::RPL_ADMINME,
                    vec![nick.clone(), SERVER_NAME.to_owned()],
                )]
            }

            Command::AWAY(msg) => match msg {
                None => vec![server_reply(
                    Response::RPL_UNAWAY,
                    vec![
                        nick.clone(),
                        "You are no longer marked as being away".to_owned(),
                    ],
                )],
                Some(_) => vec![server_reply(
                    Response::RPL_NOWAWAY,
                    vec![
                        nick.clone(),
                        "You have been marked as being away".to_owned(),
                    ],
                )],
            },

            Command::INVITE(_nickname, _channel) => Vec::new(),

            Command::KICK(_chanlist, _userlist, _comment) => Vec::new(),

            unknown => {
                log::debug!("unhandled IRC command: {:?}", unknown);
                Vec::new()
            }
        };

        if let Err(e) = send_messages(&irc_tx, reply).await {
            log::warn!("failed to send IRC reply: {e}");
            return Ok(());
        }
    }
}

async fn handle_onebot_messages<OUT>(
    mut onebot_rx: tokio::sync::broadcast::Receiver<RenderedOnebotMessage>,
    irc_tx: Arc<Mutex<OUT>>,
    nick_rx: watch::Receiver<String>,
    cancel_signal: CancellationToken,
) -> anyhow::Result<()>
where
    OUT: Sink<Message> + Unpin,
    <OUT as futures::Sink<Message>>::Error: std::error::Error + Send + Sync + Debug + 'static,
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

        let messages: Vec<Message> = match message {
            RenderedOnebotMessage::Group {
                content,
                sender_name,
                group_id,
                sender_id,
            } => {
                let nick_prefix = Prefix::Nickname(
                    sender_name.clone(),
                    sender_id.to_string(),
                    SERVER_NAME.to_owned(),
                );
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
                let nick_prefix = Prefix::Nickname(
                    sender_name.clone(),
                    sender_id.to_string(),
                    SERVER_NAME.to_owned(),
                );
                let target = if my_nick.is_empty() {
                    "you".to_owned()
                } else {
                    my_nick.clone()
                };
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
        };

        if let Err(e) = send_messages(&irc_tx, messages).await {
            log::warn!("irc connection broken while forwarding onebot message: {e}");
            return Ok(());
        }
    }
}

async fn handle_irc_connection(
    conn: tokio::net::TcpStream,
    onebot_rx: tokio::sync::broadcast::Receiver<RenderedOnebotMessage>,
    bot: Arc<kovi::RuntimeBot>,
) {
    let codec = irc_proto::IrcCodec::new("utf-8").unwrap();
    let irc_conn = codec.framed(conn);
    let (irc_tx, irc_rx) = irc_conn.split();
    let irc_tx = Arc::new(Mutex::new(irc_tx));
    let shutdown_token = CancellationToken::new();
    let (nick_tx, nick_rx) = watch::channel(String::new());

    let irc_message_task = tokio::spawn(handle_irc_messages(
        irc_rx,
        irc_tx.clone(),
        nick_tx,
        bot,
        shutdown_token.clone(),
    ));
    let onebot_message_task = tokio::spawn(handle_onebot_messages(
        onebot_rx,
        irc_tx.clone(),
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

async fn irc_server_main(
    bind_addr: std::net::SocketAddr,
    broadcast_tx: tokio::sync::broadcast::Sender<RenderedOnebotMessage>,
    bot: Arc<kovi::RuntimeBot>,
) {
    let acceptor = match tokio::net::TcpListener::bind(bind_addr).await {
        Ok(l) => l,
        Err(e) => {
            log::error!("failed to bind IRC server to {bind_addr}: {e}");
            return;
        }
    };
    log::info!("IRC gateway listening on {bind_addr}");
    loop {
        match acceptor.accept().await {
            Ok((conn, peer)) => {
                log::info!("incoming IRC connection from {peer}");
                let rx = broadcast_tx.subscribe();
                let bot = Arc::clone(&bot);
                tokio::spawn(handle_irc_connection(conn, rx, bot));
            }
            Err(e) => {
                log::warn!("accept error: {e}");
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum RenderedOnebotMessage {
    Group {
        content: Vec<String>,
        sender_id: i64,
        sender_name: String,
        group_id: i64,
    },
    Private {
        content: Vec<String>,
        sender_id: i64,
        sender_name: String,
    },
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmojiInfo {
    pub emoji_id: String,
    pub describe: String,
}

static QQ_EMOJI_INFO: LazyLock<HashMap<String, EmojiInfo>> = LazyLock::new(|| {
    // https://koishi.js.org/QFace/assets/qq_emoji/_index.json
    let json = include_str!("../data/qq_emoji.json");
    let decoded: Vec<EmojiInfo> = serde_json::from_str(json).unwrap();
    let mut result = HashMap::new();
    result.extend(
        decoded
            .into_iter()
            .map(|item| (item.emoji_id.clone(), item)),
    );
    result
});

async fn render_qq_array_message(
    messages: &kovi::Message,
    bot: Arc<kovi::RuntimeBot>,
    group_id: Option<i64>,
) -> String {
    // https://283375.github.io/onebot_v11_vitepress/message/segment.html
    futures::stream::iter(messages.iter().cloned())
        .map(async |segment| match segment.type_.as_str() {
            "text" => segment.data["text"].as_str().unwrap_or("").to_owned(),
            "face" => {
                let id = segment.data["id"].as_str().unwrap_or("").to_owned();
                format!(
                    "[face:{}]",
                    QQ_EMOJI_INFO
                        .get(&id)
                        .map(|info| info.describe.as_str())
                        .unwrap_or(&id)
                )
            }
            "image" => {
                let file = match segment.data["file"].as_str() {
                    Some(f) => f,
                    None => return "[image:unknown]".to_owned(),
                };
                let path = match bot.get_image(file).await {
                    Ok(info) => match info.data["file"].as_str() {
                        Some(p) => p.to_owned(),
                        None => return "[image:no-path]".to_owned(),
                    },
                    Err(err) => return format!("[image:fetch-error:{err}]"),
                };
                log::info!("image path: {path}");
                let image_reader = match image::io::Reader::open(&path) {
                    Ok(reader) => reader,
                    Err(err) => return format!("[image:open-error:{err}]"),
                };
                let image_data = match image_reader.with_guessed_format() {
                    Ok(data) => data,
                    Err(err) => return format!("[image:format-error:{err}]"),
                };
                let image = match image_data.decode() {
                    Ok(image) => image,
                    Err(err) => return format!("[image:decode-error:{err}]"),
                };
                let mut buf = String::new();
                let render_options = rascii_art::RenderOptions::default()
                    .height(15)
                    .colored(true);
                if let Err(err) = rascii_art::render_image_to(&image, &mut buf, &render_options) {
                    return format!("[image:render-error:{err}]");
                };
                format!("\n{buf}\n")
            }
            "at" => {
                let qq = segment.data["qq"].as_str().unwrap_or("?");
                let name = if let Ok(user_id) = qq.parse::<i64>() {
                    let member = if let Some(gid) = group_id {
                        bot.get_group_member_info(gid, user_id, false).await.ok()
                    } else {
                        None
                    };
                    if let Some(ref info) = member {
                        let card = info.data["card"].as_str().unwrap_or("");
                        let nick = info.data["nickname"].as_str().unwrap_or("");
                        if !card.is_empty() {
                            card.to_owned()
                        } else if !nick.is_empty() {
                            nick.to_owned()
                        } else {
                            qq.to_owned()
                        }
                    } else {
                        bot.get_stranger_info(user_id, false)
                            .await
                            .ok()
                            .and_then(|r| r.data["nickname"].as_str().map(str::to_owned))
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| qq.to_owned())
                    }
                } else {
                    qq.to_owned()
                };
                format!("[at:{name}]")
            }
            _ => format!("[{}]", segment.type_),
        })
        .buffered(16)
        .collect::<Vec<_>>()
        .await
        .join("")
}

impl RenderedOnebotMessage {
    pub async fn from_msg_event(value: &kovi::MsgEvent, bot: Arc<kovi::RuntimeBot>) -> Self {
        if value.is_private() {
            let sender_name = value
                .get_sender_nickname()
                .chars()
                .filter(|c| !c.is_whitespace() && !c.is_ascii_punctuation())
                .collect::<String>();
            let sender_name = if sender_name.is_empty() {
                value.sender.user_id.to_string()
            } else {
                sender_name
            };
            Self::Private {
                content: render_qq_array_message(&value.message, Arc::clone(&bot), None)
                    .await
                    .split('\n')
                    .map(str::to_owned)
                    .collect(),
                sender_id: value.sender.user_id,
                sender_name,
            }
        } else {
            let gid = value.group_id.unwrap();
            let sender_name = bot
                .get_group_member_info(gid, value.sender.user_id, false)
                .await
                .ok()
                .map(|info| {
                    let title = info.data["title"].as_str().unwrap_or("").trim();
                    let card = info.data["card"].as_str().unwrap_or("").trim();
                    let nick = info.data["nickname"].as_str().unwrap_or("").trim();
                    let raw = if !title.is_empty() && !card.is_empty() {
                        format!("【{title}】{card}")
                    } else if !card.is_empty() {
                        card.to_owned()
                    } else if !nick.is_empty() {
                        nick.to_owned()
                    } else {
                        String::new()
                    };
                    raw.chars()
                        .filter(|c| !c.is_whitespace())
                        .collect::<String>()
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| value.sender.user_id.to_string());
            Self::Group {
                content: render_qq_array_message(&value.message, Arc::clone(&bot), Some(gid))
                    .await
                    .split('\n')
                    .map(str::to_owned)
                    .collect(),
                sender_id: value.sender.user_id,
                group_id: gid,
                sender_name,
            }
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Config {
    bind_addr: std::net::SocketAddr,
}

#[kovi::plugin]
async fn main() {
    let bot = kovi::PluginBuilder::get_runtime_bot();
    let data_path = bot.get_data_path();
    let config_path = data_path.join("config.toml");
    let default_config: Config = kovi::toml::toml! {
        bind_addr = "0.0.0.0:8621"
    }
    .try_into()
    .unwrap();
    let config = kovi::utils::load_toml_data(default_config, config_path).unwrap();

    let broadcast_tx = tokio::sync::broadcast::Sender::new(16);
    let _irc = kovi::spawn(irc_server_main(
        config.bind_addr,
        broadcast_tx.clone(),
        Arc::clone(&bot),
    ));

    let broadcast_tx = Arc::new(broadcast_tx);

    kovi::PluginBuilder::on_msg(move |event: Arc<kovi::MsgEvent>| {
        let broadcast_tx = Arc::clone(&broadcast_tx);
        let bot = Arc::clone(&bot);
        async move {
            let rendered = RenderedOnebotMessage::from_msg_event(event.as_ref(), bot).await;
            // send error occurs only when there are no listeners.
            let _ = broadcast_tx.send(rendered);
        }
    })
}

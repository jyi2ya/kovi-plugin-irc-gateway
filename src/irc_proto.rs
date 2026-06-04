use irc_proto::{CapSubCommand, Command, Message, Prefix, Response};

const SERVER_NAME: &str = "onebotirc.villv.tech";
const SERVER_VERSION: &str = "kovi-irc-gateway-0.1";

#[derive(Debug)]
pub enum IrcCommandEffect {
    Reply(Vec<Message>),
    SetNick(String),
    SetUser(String),
    SetRegState(RegistrationState),
    SetCapNegotiating(bool),
    AsyncJoin(String),
    SendPrivmsg { target: String, text: String },
    PartPrivate(Vec<i64>),
    Quit,
    LogDebug(String),
}

/// Returns the configured IRC server hostname.
pub fn rvs_server_name() -> &'static str {
    SERVER_NAME
}

/// Returns the configured server version string.
pub fn rvs_server_version() -> &'static str {
    SERVER_VERSION
}

/// Attaches the given prefix to a message.
pub fn rvs_prefixed(prefix: Prefix, mut message: Message) -> Message {
    message.prefix = Some(prefix);
    message
}

/// Returns a server-name prefix using the configured hostname.
pub fn rvs_server_prefix() -> Prefix {
    Prefix::ServerName(SERVER_NAME.to_owned())
}

/// Builds a message with the server prefix and the given command.
pub fn rvs_server_msg(cmd: Command) -> Message {
    rvs_prefixed(rvs_server_prefix(), Message::from(cmd))
}

/// Builds a numeric reply from the server with the given response code and arguments.
pub fn rvs_server_reply(response: Response, args: Vec<String>) -> Message {
    rvs_server_msg(Command::Response(response, args))
}

/// Builds a nickname prefix with the given nick, user, and the configured server hostname.
pub fn rvs_nick_prefix(nick: &str, user: &str) -> Prefix {
    Prefix::Nickname(nick.to_owned(), user.to_owned(), SERVER_NAME.to_owned())
}

/// Builds the RPL_MOTDSTART message.
pub fn rvs_build_motd_start(nick: &str) -> Message {
    rvs_server_reply(
        Response::RPL_MOTDSTART,
        vec![
            nick.to_owned(),
            format!("- {SERVER_NAME} Message of the Day -"),
        ],
    )
}

/// Builds a single RPL_MOTD line.
pub fn rvs_build_motd_line(nick: &str, line: &str) -> Message {
    rvs_server_reply(
        Response::RPL_MOTD,
        vec![nick.to_owned(), format!("- {line}")],
    )
}

/// Builds the RPL_ENDOFMOTD message.
pub fn rvs_build_motd_end(nick: &str) -> Message {
    rvs_server_reply(
        Response::RPL_ENDOFMOTD,
        vec![nick.to_owned(), "/MOTD 结束".to_owned()],
    )
}

/// Formats a Unix timestamp (seconds since epoch) as an ISO 8601 UTC datetime string.
///
/// # Panics
///
/// Never panics in practice; integer arithmetic is used throughout.
pub fn rvs_format_utc_timestamp(secs: u64) -> String {
    debug_assert!(secs < 253402300800, "timestamp exceeds year 9999");
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let mut year = 1970u64;
    let mut remaining_days = days;
    loop {
        let days_in_year =
            if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
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
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
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

/// Returns the current UTC time as seconds since the Unix epoch.
///
/// Reads the system clock (side effect).
pub fn rvs_current_utc_secs_S() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Builds the full registration welcome burst (001-005, ISUPPORT, MOTD).
pub fn rvs_build_welcome_burst(nick: &str, user: &str, now_str: &str) -> Vec<Message> {
    vec![
        rvs_server_reply(
            Response::RPL_WELCOME,
            vec![
                nick.to_owned(),
                format!("欢迎来到 QQ IRC 网关 {nick}!{user}@{SERVER_NAME}"),
            ],
        ),
        rvs_server_reply(
            Response::RPL_YOURHOST,
            vec![
                nick.to_owned(),
                format!("您的主机是 {SERVER_NAME}，运行版本 {SERVER_VERSION}"),
            ],
        ),
        rvs_server_reply(
            Response::RPL_CREATED,
            vec![nick.to_owned(), format!("此服务器创建于 {now_str}")],
        ),
        rvs_server_reply(
            Response::RPL_MYINFO,
            vec![
                nick.to_owned(),
                SERVER_NAME.to_owned(),
                SERVER_VERSION.to_owned(),
                "io".to_owned(),
                "mnt".to_owned(),
            ],
        ),
        rvs_server_reply(
            Response::RPL_ISUPPORT,
            vec![
                nick.to_owned(),
                "CASEMAPPING=ascii".to_owned(),
                "CHANTYPES=#".to_owned(),
                "CHANMODES=,,,m".to_owned(),
                "PREFIX=(o)@".to_owned(),
                "NICKLEN=30".to_owned(),
                "此服务器支持".to_owned(),
            ],
        ),
        rvs_build_motd_start(nick),
        rvs_build_motd_line(nick, "欢迎来到 QQ IRC 网关！"),
        rvs_build_motd_line(nick, "/JOIN #<群号> 加入群聊频道"),
        rvs_build_motd_line(nick, "/JOIN ##<QQ号> 私聊频道（首次发消息时自动加入）"),
        rvs_build_motd_end(nick),
    ]
}

/// Extracts all ASCII digits from the input string.
pub fn rvs_extract_digits(value: &str) -> String {
    value.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// Returns the numeric channel id if the entire input consists of ASCII digits.
pub fn rvs_extract_channel_id(value: &str) -> Option<i64> {
    if value.is_empty() || !value.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

#[expect(
    clippy::allow_attributes,
    reason = "dead_code needs #[allow] because tests use this function"
)]
#[allow(dead_code, reason = "utility function for parsing prefixed numbers")]
/// Parses the first contiguous digit sequence from `value` into a numeric type `N`.
pub fn rvs_parse_prefix_number<N>(value: &str) -> Option<N>
where
    N: std::str::FromStr,
    <N as std::str::FromStr>::Err: std::fmt::Debug,
{
    let digits = rvs_extract_digits(value);
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivmsgTarget {
    Group(i64),
    Private(i64),
    Invalid(String),
}

/// Resolves a PRIVMSG target string into a Group, Private, or Invalid target.
///
/// Convention:
/// - `##<digits>` → `Private(<digits>)` (OneBot private chat with a QQ user)
/// - `#<digits>` → `Group(<digits>)` (OneBot group chat)
/// - anything else → `Invalid`
pub fn rvs_resolve_privmsg_target(target: &str) -> PrivmsgTarget {
    if let Some(stripped) = target.strip_prefix("##") {
        match rvs_extract_channel_id(stripped) {
            Some(id) => PrivmsgTarget::Private(id),
            None => PrivmsgTarget::Invalid(target.to_owned()),
        }
    } else if let Some(stripped) = target.strip_prefix('#') {
        match rvs_extract_channel_id(stripped) {
            Some(id) => PrivmsgTarget::Group(id),
            None => PrivmsgTarget::Invalid(target.to_owned()),
        }
    } else {
        PrivmsgTarget::Invalid(target.to_owned())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationState {
    Pending { got_nick: bool, got_user: bool },
    Registered,
}

impl RegistrationState {
    /// Returns `true` if registration is complete.
    pub fn rvs_is_registered(&self) -> bool {
        matches!(self, RegistrationState::Registered)
    }
}

/// Strips characters that are not valid in IRC nicknames.
///
/// Keeps letters, digits, IRC special characters `[\\]^_{|}~-`, and all
/// non-ASCII characters (e.g. Chinese).
pub fn rvs_sanitize_nick(raw: &str) -> String {
    raw.chars()
        .filter(|c| {
            c.is_alphanumeric()
                || matches!(
                    *c,
                    '[' | ']' | '\\' | '_' | '^' | '{' | '}' | '|' | '~' | '-'
                )
                || !c.is_ascii()
        })
        .collect()
}

/// Builds a CAP LS reply advertising no capabilities.
pub fn rvs_build_cap_ls_reply() -> Vec<Message> {
    vec![rvs_server_msg(Command::CAP(
        Some("*".to_owned()),
        CapSubCommand::LS,
        None,
        Some(String::new()),
    ))]
}

/// Builds a CAP NAK reply rejecting the requested capabilities.
pub fn rvs_build_cap_nak_reply(requested: Option<String>) -> Vec<Message> {
    vec![rvs_server_msg(Command::CAP(
        Some("*".to_owned()),
        CapSubCommand::NAK,
        None,
        Some(requested.unwrap_or_default()),
    ))]
}

/// Builds a PONG response echoing the server tokens.
///
/// # Panics
///
/// Never panics in practice; delegates to [`rvs_server_msg`] which performs no fallible operations.
pub fn rvs_build_pong(server1: String, server2: Option<String>) -> Vec<Message> {
    vec![rvs_server_msg(Command::PONG(server1, server2))]
}

/// Builds a NICK change broadcast message from the old identity.
pub fn rvs_build_nick_change_broadcast(nick: &str, user: &str) -> Vec<Message> {
    vec![rvs_prefixed(
        rvs_nick_prefix(nick, user),
        Message::from(Command::NICK(nick.to_owned())),
    )]
}

/// Builds the JOIN reply sequence: join event, topic, names reply, and end-of-names.
///
/// `channel` is the raw IRC channel name as it should appear in the response
/// (e.g. `#12345` for groups, `##67890` for private chats disguised as channels).
/// `topic` is the display name carried in RPL_TOPIC.
pub fn rvs_build_join_reply(nick: &str, user: &str, channel: &str, topic: &str) -> Vec<Message> {
    let chan = channel.to_owned();
    vec![
        rvs_prefixed(
            rvs_nick_prefix(nick, user),
            Message::from(Command::JOIN(chan.clone(), None, None)),
        ),
        rvs_server_reply(
            Response::RPL_TOPIC,
            vec![nick.to_owned(), chan.clone(), topic.to_owned()],
        ),
        rvs_server_reply(
            Response::RPL_NAMREPLY,
            vec![
                nick.to_owned(),
                "=".to_owned(),
                chan.clone(),
                format!("@{nick}"),
            ],
        ),
        rvs_server_reply(
            Response::RPL_ENDOFNAMES,
            vec![nick.to_owned(), chan.clone(), "/NAMES 列表结束".to_owned()],
        ),
    ]
}

/// Builds an ERR_NOSUCHCHANNEL error reply.
pub fn rvs_build_no_such_channel(nick: &str, channel: &str) -> Message {
    rvs_server_reply(
        Response::ERR_NOSUCHCHANNEL,
        vec![nick.to_owned(), channel.to_owned(), "无此频道".to_owned()],
    )
}

/// Builds an ERR_NOSUCHNICK error reply.
pub fn rvs_build_no_such_nick(nick: &str, target: &str) -> Message {
    rvs_server_reply(
        Response::ERR_NOSUCHNICK,
        vec![
            nick.to_owned(),
            target.to_owned(),
            "无此昵称/频道".to_owned(),
        ],
    )
}

/// Builds a PART broadcast message with an optional part comment.
pub fn rvs_build_part_broadcast(
    nick: &str,
    user: &str,
    channel: &str,
    comment: Option<String>,
) -> Message {
    rvs_prefixed(
        rvs_nick_prefix(nick, user),
        Message::from(Command::PART(channel.to_owned(), comment)),
    )
}

/// Builds the end-of-WHO list reply.
pub fn rvs_build_who_end(nick: &str, mask: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_ENDOFWHO,
        vec![nick.to_owned(), mask.to_owned(), "/WHO 列表结束".to_owned()],
    )]
}

/// Builds WHOIS user/server info and end-of-WHOIS replies.
pub fn rvs_build_whois_reply(nick: &str, user: &str, mask: &str) -> Vec<Message> {
    vec![
        rvs_server_reply(
            Response::RPL_WHOISUSER,
            vec![
                nick.to_owned(),
                mask.to_owned(),
                user.to_owned(),
                rvs_server_name().to_owned(),
                "*".to_owned(),
                mask.to_owned(),
            ],
        ),
        rvs_server_reply(
            Response::RPL_WHOISSERVER,
            vec![
                nick.to_owned(),
                mask.to_owned(),
                rvs_server_name().to_owned(),
                "QQ IRC 网关".to_owned(),
            ],
        ),
        rvs_server_reply(
            Response::RPL_ENDOFWHOIS,
            vec![
                nick.to_owned(),
                mask.to_owned(),
                "/WHOIS 列表结束".to_owned(),
            ],
        ),
    ]
}

/// Builds the end-of-WHOWAS reply.
pub fn rvs_build_whowas_end(nick: &str, nicklist: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_ENDOFWHOWAS,
        vec![
            nick.to_owned(),
            nicklist.to_owned(),
            "WHOWAS 列表结束".to_owned(),
        ],
    )]
}

/// Builds a RPL_USERHOST reply for the given nicknames.
pub fn rvs_build_userhost_reply(nick: &str, nicknames: &[String]) -> Vec<Message> {
    let replies: Vec<String> = nicknames
        .iter()
        .map(|n| format!("{n}=+{n}@{}", rvs_server_name()))
        .collect();
    vec![rvs_server_reply(
        Response::RPL_USERHOST,
        vec![nick.to_owned(), replies.join(" ")],
    )]
}

/// Builds a RPL_ISON reply (always reports no users online).
pub fn rvs_build_ison_reply(nick: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_ISON,
        vec![nick.to_owned(), String::new()],
    )]
}

/// Builds a RPL_CHANNELMODEIS reply showing no modes set.
pub fn rvs_build_channel_mode(nick: &str, channel: String) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_CHANNELMODEIS,
        vec![nick.to_owned(), channel, "+".to_owned()],
    )]
}

/// Builds a RPL_NOTOPIC reply for a channel.
pub fn rvs_build_no_topic(nick: &str, channel: String) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_NOTOPIC,
        vec![nick.to_owned(), channel, "未设置主题".to_owned()],
    )]
}

/// Builds the end-of-NAMES list reply for a channel.
pub fn rvs_build_names_end(nick: &str, chan: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_ENDOFNAMES,
        vec![
            nick.to_owned(),
            chan.to_owned(),
            "/NAMES 列表结束".to_owned(),
        ],
    )]
}

/// Builds the end-of-LIST reply.
pub fn rvs_build_list_end(nick: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_LISTEND,
        vec![nick.to_owned(), "/LIST 列表结束".to_owned()],
    )]
}

/// Builds the full MOTD burst (start, lines, end).
pub fn rvs_build_motd_burst(nick: &str) -> Vec<Message> {
    vec![
        rvs_build_motd_start(nick),
        rvs_build_motd_line(nick, "欢迎来到 QQ IRC 网关！"),
        rvs_build_motd_line(nick, "/JOIN #<群号> 加入群聊频道"),
        rvs_build_motd_line(nick, "/JOIN ##<QQ号> 私聊频道（首次发消息时自动加入）"),
        rvs_build_motd_end(nick),
    ]
}

/// Builds the RPL_VERSION reply with server version info.
pub fn rvs_build_version_reply(nick: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_VERSION,
        vec![
            nick.to_owned(),
            rvs_server_version().to_owned(),
            rvs_server_name().to_owned(),
            "QQ IRC 网关".to_owned(),
        ],
    )]
}

/// Builds a RPL_TIME reply with the current server time.
pub fn rvs_build_time_reply(nick: &str, now_str: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_TIME,
        vec![
            nick.to_owned(),
            rvs_server_name().to_owned(),
            now_str.to_owned(),
        ],
    )]
}

/// Builds a RPL_INFO reply with server identification.
pub fn rvs_build_info_reply(nick: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_INFO,
        vec![nick.to_owned(), "QQ IRC 网关 (kovi)".to_owned()],
    )]
}

/// Builds LUSER client/me count replies (reports 1 user, 1 server).
pub fn rvs_build_lusers_reply(nick: &str) -> Vec<Message> {
    vec![
        rvs_server_reply(
            Response::RPL_LUSERCLIENT,
            vec![nick.to_owned(), "当前 1 个用户，1 个服务器".to_owned()],
        ),
        rvs_server_reply(
            Response::RPL_LUSERME,
            vec![nick.to_owned(), "1 个客户端，0 个服务器".to_owned()],
        ),
    ]
}

/// Builds the end-of-LINKS list reply.
pub fn rvs_build_links_end(nick: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_ENDOFLINKS,
        vec![
            nick.to_owned(),
            "*".to_owned(),
            "/LINKS 列表结束".to_owned(),
        ],
    )]
}

/// Builds RPL_ADMINME reply with the server name.
pub fn rvs_build_admin_reply(nick: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_ADMINME,
        vec![nick.to_owned(), rvs_server_name().to_owned()],
    )]
}

/// Builds away/unaway replies depending on whether a message was provided.
pub fn rvs_build_away_replies(nick: &str, msg: Option<&String>) -> Vec<Message> {
    match msg {
        None => vec![rvs_server_reply(
            Response::RPL_UNAWAY,
            vec![nick.to_owned(), "您已取消离开状态".to_owned()],
        )],
        Some(_) => vec![rvs_server_reply(
            Response::RPL_NOWAWAY,
            vec![nick.to_owned(), "您已被标记为离开".to_owned()],
        )],
    }
}

/// Builds PART broadcast messages for each channel in a comma-separated list.
pub fn rvs_handle_part(
    nick: &str,
    user: &str,
    chanlist: &str,
    comment: &Option<String>,
) -> Vec<Message> {
    let mut reply = Vec::new();
    for channel in chanlist.split(',') {
        let channel = channel.trim();
        if channel.is_empty() {
            continue;
        }
        reply.push(rvs_build_part_broadcast(
            nick,
            user,
            channel,
            comment.clone(),
        ));
    }
    reply
}

/// Returns the peer IDs of private-chat channels (##<digits>) in a PART channel list.
fn rvs_plan_part_private(chanlist: &str) -> Vec<i64> {
    chanlist
        .split(',')
        .filter_map(|ch| {
            let ch = ch.trim();
            let stripped = ch.strip_prefix("##")?;
            rvs_extract_channel_id(stripped)
        })
        .collect()
}

/// Pure decision function: maps an IRC command to a list of effects without performing I/O.
///
/// # Panics
///
/// Never panics in practice; all match arms produce effects without fallible operations.
pub fn rvs_plan_irc_command(
    command: &Command,
    nick: &str,
    user: &str,
    reg_state: &RegistrationState,
    cap_negotiating: bool,
    now_str: &str,
) -> Vec<IrcCommandEffect> {
    match command {
        Command::CAP(_target, sub, _param1, param2) => match sub {
            CapSubCommand::LS => vec![
                IrcCommandEffect::SetCapNegotiating(true),
                IrcCommandEffect::Reply(rvs_build_cap_ls_reply()),
            ],
            CapSubCommand::REQ => {
                vec![IrcCommandEffect::Reply(rvs_build_cap_nak_reply(
                    param2.clone(),
                ))]
            }
            CapSubCommand::END => {
                let mut effects = vec![IrcCommandEffect::SetCapNegotiating(false)];
                if let RegistrationState::Pending {
                    got_nick: true,
                    got_user: true,
                } = reg_state
                {
                    effects.push(IrcCommandEffect::SetRegState(RegistrationState::Registered));
                    effects.push(IrcCommandEffect::Reply(rvs_build_welcome_burst(
                        nick, user, now_str,
                    )));
                }
                effects
            }
            _ => vec![],
        },

        Command::NICK(new_nick) => {
            let mut effects = vec![IrcCommandEffect::SetNick(new_nick.clone())];
            match reg_state {
                RegistrationState::Pending {
                    got_nick: _,
                    got_user,
                } => {
                    if *got_user && !cap_negotiating {
                        effects.push(IrcCommandEffect::SetRegState(RegistrationState::Registered));
                        effects.push(IrcCommandEffect::Reply(rvs_build_welcome_burst(
                            new_nick, user, now_str,
                        )));
                    } else {
                        effects.push(IrcCommandEffect::SetRegState(RegistrationState::Pending {
                            got_nick: true,
                            got_user: *got_user,
                        }));
                    }
                }
                RegistrationState::Registered => {
                    effects.push(IrcCommandEffect::Reply(rvs_build_nick_change_broadcast(
                        new_nick, user,
                    )));
                }
            }
            effects
        }

        Command::USER(new_user, _mode, _realname) => {
            let mut effects = vec![IrcCommandEffect::SetUser(new_user.clone())];
            match reg_state {
                RegistrationState::Pending {
                    got_nick,
                    got_user: _,
                } => {
                    if *got_nick && !cap_negotiating {
                        effects.push(IrcCommandEffect::SetRegState(RegistrationState::Registered));
                        effects.push(IrcCommandEffect::Reply(rvs_build_welcome_burst(
                            nick, new_user, now_str,
                        )));
                    } else {
                        effects.push(IrcCommandEffect::SetRegState(RegistrationState::Pending {
                            got_nick: *got_nick,
                            got_user: true,
                        }));
                    }
                }
                RegistrationState::Registered => {}
            }
            effects
        }

        Command::PASS(_) => vec![],

        Command::PING(server1, server2) => {
            vec![IrcCommandEffect::Reply(rvs_build_pong(
                server1.clone(),
                server2.clone(),
            ))]
        }

        Command::QUIT(_) => vec![
            IrcCommandEffect::Reply(vec![Message::from(Command::ERROR("再见".to_owned()))]),
            IrcCommandEffect::Quit,
        ],

        _ if !reg_state.rvs_is_registered() => {
            vec![
                IrcCommandEffect::Reply(vec![rvs_server_reply(
                    Response::ERR_NOTREGISTERED,
                    vec!["*".to_owned(), "您尚未注册".to_owned()],
                )]),
                IrcCommandEffect::LogDebug(format!(
                    "ignoring command before registration: {:?}",
                    command
                )),
            ]
        }

        Command::JOIN(chanlist, _chankeys, _realname) => {
            vec![IrcCommandEffect::AsyncJoin(chanlist.clone())]
        }

        Command::PART(chanlist, comment) => {
            let mut effects = vec![IrcCommandEffect::Reply(rvs_handle_part(
                nick, user, chanlist, comment,
            ))];
            let peer_ids = rvs_plan_part_private(chanlist);
            if !peer_ids.is_empty() {
                effects.push(IrcCommandEffect::PartPrivate(peer_ids));
            }
            effects
        }

        Command::PRIVMSG(target, text) => {
            vec![IrcCommandEffect::SendPrivmsg {
                target: target.clone(),
                text: text.clone(),
            }]
        }

        Command::NOTICE(_target, _text) => vec![],

        Command::WHO(mask, _is_op) => {
            let mask = mask.as_deref().unwrap_or("*");
            vec![IrcCommandEffect::Reply(rvs_build_who_end(nick, mask))]
        }

        Command::WHOIS(_target, mask) => {
            vec![IrcCommandEffect::Reply(rvs_build_whois_reply(
                nick, user, mask,
            ))]
        }

        Command::WHOWAS(nicklist, _count, _target) => {
            vec![IrcCommandEffect::Reply(rvs_build_whowas_end(
                nick, nicklist,
            ))]
        }

        Command::USERHOST(nicknames) => {
            vec![IrcCommandEffect::Reply(rvs_build_userhost_reply(
                nick, nicknames,
            ))]
        }

        Command::ISON(_nicklist) => {
            vec![IrcCommandEffect::Reply(rvs_build_ison_reply(nick))]
        }

        Command::ChannelMODE(channel, _modes) => {
            vec![IrcCommandEffect::Reply(rvs_build_channel_mode(
                nick,
                channel.clone(),
            ))]
        }

        Command::UserMODE(_nickname, _modes) => vec![],

        Command::TOPIC(channel, new_topic) => match new_topic {
            None => vec![IrcCommandEffect::Reply(rvs_build_no_topic(
                nick,
                channel.clone(),
            ))],
            Some(_) => vec![IrcCommandEffect::Reply(vec![rvs_server_reply(
                Response::ERR_CHANOPRIVSNEEDED,
                vec![
                    nick.to_owned(),
                    channel.clone(),
                    "您不是频道操作员".to_owned(),
                ],
            )])],
        },

        Command::NAMES(chanlist, _target) => {
            let chan = chanlist.as_deref().unwrap_or("");
            vec![IrcCommandEffect::Reply(rvs_build_names_end(nick, chan))]
        }

        Command::LIST(_chanlist, _target) => {
            vec![IrcCommandEffect::Reply(rvs_build_list_end(nick))]
        }

        Command::MOTD(_target) => {
            vec![IrcCommandEffect::Reply(rvs_build_motd_burst(nick))]
        }

        Command::VERSION(_target) => {
            vec![IrcCommandEffect::Reply(rvs_build_version_reply(nick))]
        }

        Command::TIME(_target) => {
            vec![IrcCommandEffect::Reply(rvs_build_time_reply(nick, now_str))]
        }

        Command::INFO(_target) => {
            vec![IrcCommandEffect::Reply(rvs_build_info_reply(nick))]
        }

        Command::LUSERS(_mask, _target) => {
            vec![IrcCommandEffect::Reply(rvs_build_lusers_reply(nick))]
        }

        Command::STATS(_query, _target) => vec![],

        Command::LINKS(_remote, _mask) => {
            vec![IrcCommandEffect::Reply(rvs_build_links_end(nick))]
        }

        Command::ADMIN(_target) => {
            vec![IrcCommandEffect::Reply(rvs_build_admin_reply(nick))]
        }

        Command::AWAY(msg) => {
            vec![IrcCommandEffect::Reply(rvs_build_away_replies(
                nick,
                msg.as_ref(),
            ))]
        }

        Command::INVITE(_nickname, _channel) => vec![IrcCommandEffect::LogDebug(
            "INVITE is not supported".to_owned(),
        )],

        Command::KICK(_chanlist, _userlist, _comment) => {
            vec![IrcCommandEffect::LogDebug(
                "KICK is not supported".to_owned(),
            )]
        }

        unknown => {
            vec![IrcCommandEffect::LogDebug(format!(
                "unhandled IRC command: {:?}",
                unknown
            ))]
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "test assertions may panic")]
mod tests {
    use super::*;

    fn snapshot(name: &str, content: &str) {
        let path = format!("test_out/test_{}.out", name);
        std::fs::write(&path, content).unwrap();
    }

    #[test]
    fn test_20260418_format_utc_timestamp_epoch() {
        let result = rvs_format_utc_timestamp(0);
        snapshot("20260418_format_utc_timestamp_epoch", &result);
        assert_eq!(result, "1970-01-01T00:00:00Z");
    }

    #[test]
    fn test_20260418_format_utc_timestamp_known_date() {
        let result = rvs_format_utc_timestamp(1700000000);
        snapshot("20260418_format_utc_timestamp_known_date", &result);
        assert!(result.starts_with("202"));
        assert!(result.contains("T"));
        assert!(result.ends_with("Z"));
    }

    #[test]
    fn test_20260418_format_utc_timestamp_midnight() {
        let result = rvs_format_utc_timestamp(86400);
        assert_eq!(result, "1970-01-02T00:00:00Z");
        snapshot("20260418_format_utc_timestamp_midnight", &result);
    }

    #[test]
    fn test_20260418_format_utc_timestamp_leap_year() {
        let result = rvs_format_utc_timestamp(86400 * 59);
        assert!(result.contains("03-01") || result.contains("02-29") || result.contains("02-28"));
        snapshot("20260418_format_utc_timestamp_leap_year", &result);
    }

    #[test]
    fn test_20260418_parse_prefix_number_valid() {
        let r1 = rvs_parse_prefix_number::<i64>("12345");
        let r2 = rvs_parse_prefix_number::<i64>("#12345");
        let r3 = rvs_parse_prefix_number::<i64>("group123");
        assert_eq!(r1, Some(12345));
        assert_eq!(r2, Some(12345));
        assert_eq!(r3, Some(123));
        snapshot(
            "20260418_parse_prefix_number_valid",
            &format!("{r1:?}, {r2:?}, {r3:?}"),
        );
    }

    #[test]
    fn test_20260418_parse_prefix_number_empty() {
        let r1 = rvs_parse_prefix_number::<i64>("");
        let r2 = rvs_parse_prefix_number::<i64>("abc");
        let r3 = rvs_parse_prefix_number::<i64>("#");
        assert_eq!(r1, None);
        assert_eq!(r2, None);
        assert_eq!(r3, None);
        snapshot(
            "20260418_parse_prefix_number_empty",
            &format!("{r1:?}, {r2:?}, {r3:?}"),
        );
    }

    #[test]
    fn test_20260418_parse_prefix_number_overflow() {
        let result = rvs_parse_prefix_number::<u8>("999");
        assert_eq!(result, None);
        snapshot(
            "20260418_parse_prefix_number_overflow",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260418_sanitize_nick_normal() {
        let result = rvs_sanitize_nick("hello world");
        snapshot("20260418_sanitize_nick_normal", &result);
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn test_20260418_sanitize_nick_with_punctuation() {
        let result = rvs_sanitize_nick("Alice!@# (test)");
        snapshot("20260418_sanitize_nick_with_punctuation", &result);
        assert_eq!(result, "Alicetest");
    }

    #[test]
    fn test_20260418_sanitize_nick_all_whitespace() {
        let result = rvs_sanitize_nick("  \t\n  ");
        assert!(result.is_empty());
        snapshot(
            "20260418_sanitize_nick_all_whitespace",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260418_sanitize_nick_chinese() {
        let result = rvs_sanitize_nick("张三 123");
        snapshot("20260418_sanitize_nick_chinese", &result);
        assert_eq!(result, "张三123");
    }

    #[test]
    fn test_20260604_sanitize_nick_preserves_irc_special() {
        assert_eq!(rvs_sanitize_nick("user_1"), "user_1");
        assert_eq!(rvs_sanitize_nick("hello-world"), "hello-world");
        assert_eq!(rvs_sanitize_nick("nick[1]"), "nick[1]");
        assert_eq!(rvs_sanitize_nick("a{b}c"), "a{b}c");
        assert_eq!(rvs_sanitize_nick("x|y"), "x|y");
        assert_eq!(rvs_sanitize_nick("a^b"), "a^b");
        assert_eq!(rvs_sanitize_nick("hello~world"), "hello~world");
        assert_eq!(rvs_sanitize_nick("a\\b"), "a\\b");
        assert_eq!(rvs_sanitize_nick("a!b@c#d"), "abcd");
        snapshot(
            "20260604_sanitize_nick_preserves_irc_special",
            "all IRC special chars preserved",
        );
    }

    #[test]
    fn test_20260418_registration_state() {
        let mut state = RegistrationState::Pending {
            got_nick: false,
            got_user: false,
        };
        assert!(!state.rvs_is_registered());
        state = RegistrationState::Registered;
        assert!(state.rvs_is_registered());
        snapshot("20260418_registration_state", &format!("{state:?}"));
    }

    #[test]
    fn test_20260418_build_pong() {
        let msgs = rvs_build_pong("server1".to_owned(), Some("server2".to_owned()));
        assert_eq!(msgs.len(), 1);
        snapshot("20260418_build_pong", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_cap_ls_reply() {
        let msgs = rvs_build_cap_ls_reply();
        assert_eq!(msgs.len(), 1);
        snapshot("20260418_build_cap_ls_reply", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_welcome_burst() {
        let msgs = rvs_build_welcome_burst("testnick", "testuser", "2024-01-01T00:00:00Z");
        assert!(!msgs.is_empty());
        snapshot("20260418_build_welcome_burst", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_motd_burst() {
        let msgs = rvs_build_motd_burst("testnick");
        assert_eq!(msgs.len(), 5);
        snapshot("20260418_build_motd_burst", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_join_reply() {
        let msgs = rvs_build_join_reply("nick", "user", "#12345", "TestGroup");
        assert_eq!(msgs.len(), 4);
        snapshot("20260418_build_join_reply", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_join_reply_private() {
        let msgs = rvs_build_join_reply("nick", "user", "##67890", "Alice");
        assert_eq!(msgs.len(), 4);
        snapshot("20260418_build_join_reply_private", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_away_replies_unset() {
        let msgs = rvs_build_away_replies("nick", None);
        assert_eq!(msgs.len(), 1);
        snapshot("20260418_build_away_replies_unset", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_away_replies_set() {
        let away_msg = "brb".to_owned();
        let msgs = rvs_build_away_replies("nick", Some(&away_msg));
        assert_eq!(msgs.len(), 1);
        snapshot("20260418_build_away_replies_set", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_userhost_reply() {
        let nicks = vec!["alice".to_owned(), "bob".to_owned()];
        let msgs = rvs_build_userhost_reply("nick", &nicks);
        assert_eq!(msgs.len(), 1);
        snapshot("20260418_build_userhost_reply", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_no_such_channel() {
        let msg = rvs_build_no_such_channel("nick", "#abc");
        snapshot("20260418_build_no_such_channel", &format!("{:?}", msg));
    }

    #[test]
    fn test_20260418_build_part_broadcast() {
        let msg = rvs_build_part_broadcast("nick", "user", "#12345", Some("bye".to_owned()));
        snapshot("20260418_build_part_broadcast", &format!("{:?}", msg));
    }

    #[test]
    fn test_20260418_build_whois_reply() {
        let msgs = rvs_build_whois_reply("nick", "user", "target");
        assert_eq!(msgs.len(), 3);
        snapshot("20260418_build_whois_reply", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_lusers_reply() {
        let msgs = rvs_build_lusers_reply("nick");
        assert_eq!(msgs.len(), 2);
        snapshot("20260418_build_lusers_reply", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_version_reply() {
        let msgs = rvs_build_version_reply("nick");
        assert_eq!(msgs.len(), 1);
        snapshot("20260418_build_version_reply", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_time_reply() {
        let msgs = rvs_build_time_reply("nick", "2024-06-15T12:30:00Z");
        assert_eq!(msgs.len(), 1);
        snapshot("20260418_build_time_reply", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_extract_digits() {
        assert_eq!(rvs_extract_digits(""), "");
        assert_eq!(rvs_extract_digits("abc"), "");
        assert_eq!(rvs_extract_digits("12345"), "12345");
        assert_eq!(rvs_extract_digits("#12345"), "12345");
        assert_eq!(rvs_extract_digits("group123"), "123");
        snapshot(
            "20260418_extract_digits",
            "empty, abc, 12345, #12345, group123",
        );
    }

    #[test]
    fn test_20260604_extract_channel_id() {
        assert_eq!(rvs_extract_channel_id(""), None);
        assert_eq!(rvs_extract_channel_id("abc"), None);
        assert_eq!(rvs_extract_channel_id("12345"), Some(12345));
        assert_eq!(rvs_extract_channel_id("12abc"), None);
        assert_eq!(rvs_extract_channel_id("0"), Some(0));
        let result = format!(
            "{:?}, {:?}, {:?}, {:?}, {:?}",
            rvs_extract_channel_id(""),
            rvs_extract_channel_id("abc"),
            rvs_extract_channel_id("12345"),
            rvs_extract_channel_id("12abc"),
            rvs_extract_channel_id("0"),
        );
        snapshot("20260604_extract_channel_id", &result);
    }

    #[test]
    fn test_20260418_resolve_privmsg_target_group() {
        let result = rvs_resolve_privmsg_target("#12345");
        assert_eq!(result, PrivmsgTarget::Group(12345));
        snapshot(
            "20260418_resolve_privmsg_target_group",
            &format!("{:?}", result),
        );
    }

    #[test]
    fn test_20260418_resolve_privmsg_target_private_double_hash() {
        let result = rvs_resolve_privmsg_target("##67890");
        assert_eq!(result, PrivmsgTarget::Private(67890));
        snapshot(
            "20260418_resolve_privmsg_target_private_double_hash",
            &format!("{:?}", result),
        );
    }

    #[test]
    fn test_20260418_resolve_privmsg_target_invalid() {
        let result = rvs_resolve_privmsg_target("#abc");
        assert!(matches!(result, PrivmsgTarget::Invalid(_)));
        snapshot(
            "20260418_resolve_privmsg_target_invalid",
            &format!("{:?}", result),
        );
    }

    #[test]
    fn test_20260418_resolve_privmsg_target_invalid_no_hash() {
        let result = rvs_resolve_privmsg_target("nobody");
        assert!(matches!(result, PrivmsgTarget::Invalid(_)));
        snapshot(
            "20260418_resolve_privmsg_target_invalid_no_hash",
            &format!("{:?}", result),
        );
    }

    #[test]
    fn test_20260418_resolve_privmsg_target_invalid_double_hash() {
        let result = rvs_resolve_privmsg_target("##abc");
        assert!(matches!(result, PrivmsgTarget::Invalid(_)));
        snapshot(
            "20260418_resolve_privmsg_target_invalid_double_hash",
            &format!("{:?}", result),
        );
    }

    #[test]
    fn test_20260418_handle_part_single() {
        let msgs = rvs_handle_part("nick", "user", "#12345", &None);
        assert_eq!(msgs.len(), 1);
        snapshot("20260418_handle_part_single", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_handle_part_multiple() {
        let msgs = rvs_handle_part("nick", "user", "#12345,#67890", &Some("bye".to_owned()));
        assert_eq!(msgs.len(), 2);
        snapshot("20260418_handle_part_multiple", &format!("{:?}", msgs));
    }

    fn pending(got_nick: bool, got_user: bool) -> RegistrationState {
        RegistrationState::Pending { got_nick, got_user }
    }

    fn count_replies(effects: &[IrcCommandEffect]) -> usize {
        effects
            .iter()
            .filter(|e| matches!(e, IrcCommandEffect::Reply(_)))
            .count()
    }

    #[test]
    fn test_20260418_plan_ping() {
        let effects = rvs_plan_irc_command(
            &Command::PING("svr".to_owned(), None),
            "nick",
            "user",
            &pending(false, false),
            false,
            "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 1);
        snapshot("20260418_plan_ping", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_nick_before_user() {
        let effects = rvs_plan_irc_command(
            &Command::NICK("mynick".to_owned()),
            "nick",
            "user",
            &pending(false, false),
            false,
            "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 0);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::SetNick(_)))
        );
        snapshot("20260418_plan_nick_before_user", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_nick_after_user_completes_registration() {
        let effects = rvs_plan_irc_command(
            &Command::NICK("mynick".to_owned()),
            "nick",
            "user",
            &pending(false, true),
            false,
            "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 1);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::SetRegState(_)))
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::SetNick(_)))
        );
        snapshot(
            "20260418_plan_nick_after_user_completes_registration",
            &format!("{:?}", effects),
        );
    }

    #[test]
    fn test_20260418_plan_nick_blocked_by_cap() {
        let effects = rvs_plan_irc_command(
            &Command::NICK("mynick".to_owned()),
            "nick",
            "user",
            &pending(false, true),
            true,
            "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 0);
        snapshot(
            "20260418_plan_nick_blocked_by_cap",
            &format!("{:?}", effects),
        );
    }

    #[test]
    fn test_20260418_plan_user_after_nick_completes_registration() {
        let effects = rvs_plan_irc_command(
            &Command::USER("myuser".to_owned(), "0".to_owned(), "real".to_owned()),
            "nick",
            "user",
            &pending(true, false),
            false,
            "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 1);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::SetRegState(_)))
        );
        snapshot(
            "20260418_plan_user_after_nick_completes_registration",
            &format!("{:?}", effects),
        );
    }

    #[test]
    fn test_20260418_plan_cap_end_completes_if_both_received() {
        let effects = rvs_plan_irc_command(
            &Command::CAP(None, CapSubCommand::END, None, None),
            "nick",
            "user",
            &pending(true, true),
            false,
            "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 1);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::SetRegState(_)))
        );
        snapshot(
            "20260418_plan_cap_end_completes_if_both_received",
            &format!("{:?}", effects),
        );
    }

    #[test]
    fn test_20260418_plan_cap_end_noop_if_not_ready() {
        let effects = rvs_plan_irc_command(
            &Command::CAP(None, CapSubCommand::END, None, None),
            "nick",
            "user",
            &pending(false, true),
            false,
            "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 0);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::SetCapNegotiating(false)))
        );
        snapshot(
            "20260418_plan_cap_end_noop_if_not_ready",
            &format!("{:?}", effects),
        );
    }

    #[test]
    fn test_20260418_plan_join_returns_async_effect() {
        let effects = rvs_plan_irc_command(
            &Command::JOIN("#12345".to_owned(), None, None),
            "nick",
            "user",
            &RegistrationState::Registered,
            false,
            "",
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::AsyncJoin(_)))
        );
        snapshot(
            "20260418_plan_join_returns_async_effect",
            &format!("{:?}", effects),
        );
    }

    #[test]
    fn test_20260418_plan_join_ignored_before_registration() {
        let effects = rvs_plan_irc_command(
            &Command::JOIN("#12345".to_owned(), None, None),
            "nick",
            "user",
            &pending(false, false),
            false,
            "",
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::LogDebug(_)))
        );
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::AsyncJoin(_)))
        );
        snapshot(
            "20260418_plan_join_ignored_before_registration",
            &format!("{:?}", effects),
        );
    }

    #[test]
    fn test_20260418_plan_privmsg() {
        let effects = rvs_plan_irc_command(
            &Command::PRIVMSG("#12345".to_owned(), "hello".to_owned()),
            "nick",
            "user",
            &RegistrationState::Registered,
            false,
            "",
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::SendPrivmsg { .. }))
        );
        snapshot("20260418_plan_privmsg", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_motd() {
        let effects = rvs_plan_irc_command(
            &Command::MOTD(None),
            "nick",
            "user",
            &RegistrationState::Registered,
            false,
            "",
        );
        assert_eq!(count_replies(&effects), 1);
        snapshot("20260418_plan_motd", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_time() {
        let effects = rvs_plan_irc_command(
            &Command::TIME(None),
            "nick",
            "user",
            &RegistrationState::Registered,
            false,
            "2024-06-15T12:00:00Z",
        );
        assert_eq!(count_replies(&effects), 1);
        snapshot("20260418_plan_time", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_nick_change_when_registered() {
        let effects = rvs_plan_irc_command(
            &Command::NICK("newnick".to_owned()),
            "oldnick",
            "user",
            &RegistrationState::Registered,
            false,
            "",
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::SetNick(_)))
        );
        assert_eq!(count_replies(&effects), 1);
        snapshot(
            "20260418_plan_nick_change_when_registered",
            &format!("{:?}", effects),
        );
    }

    #[test]
    fn test_20260418_plan_unknown_command() {
        let effects = rvs_plan_irc_command(
            &Command::Raw("BOGUS test".to_owned(), vec![]),
            "nick",
            "user",
            &RegistrationState::Registered,
            false,
            "",
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::LogDebug(_)))
        );
        snapshot("20260418_plan_unknown_command", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_irssi_registration_flow() {
        let mut state = pending(false, false);
        let mut cap_neg = false;

        let effects = rvs_plan_irc_command(
            &Command::CAP(None, CapSubCommand::LS, None, None),
            "",
            "",
            &state,
            cap_neg,
            "",
        );
        apply_effects(&effects, &mut state, &mut cap_neg);
        assert!(cap_neg);

        let effects = rvs_plan_irc_command(
            &Command::NICK("mynick".to_owned()),
            "",
            "",
            &state,
            cap_neg,
            "",
        );
        apply_effects(&effects, &mut state, &mut cap_neg);
        assert!(matches!(
            state,
            RegistrationState::Pending {
                got_nick: true,
                got_user: false
            }
        ));

        let effects = rvs_plan_irc_command(
            &Command::USER("myuser".to_owned(), "0".to_owned(), "real".to_owned()),
            "mynick",
            "",
            &state,
            cap_neg,
            "",
        );
        apply_effects(&effects, &mut state, &mut cap_neg);
        assert!(matches!(
            state,
            RegistrationState::Pending {
                got_nick: true,
                got_user: true
            }
        ));

        let effects = rvs_plan_irc_command(
            &Command::CAP(None, CapSubCommand::END, None, None),
            "mynick",
            "myuser",
            &state,
            cap_neg,
            "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 1);
        assert!(effects.iter().any(|e| matches!(
            e,
            IrcCommandEffect::SetRegState(RegistrationState::Registered)
        )));
        snapshot("20260418_irssi_registration_flow", &format!("{effects:?}"));
    }

    fn apply_effects(
        effects: &[IrcCommandEffect],
        state: &mut RegistrationState,
        cap_neg: &mut bool,
    ) {
        for e in effects {
            match e {
                IrcCommandEffect::SetRegState(s) => *state = s.clone(),
                IrcCommandEffect::SetCapNegotiating(v) => *cap_neg = *v,
                _ => {}
            }
        }
    }

    #[test]
    fn test_20260528_server_name() {
        let result = rvs_server_name();
        assert!(!result.is_empty());
        snapshot("20260528_server_name", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_server_version() {
        let result = rvs_server_version();
        assert!(!result.is_empty());
        snapshot("20260528_server_version", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_server_prefix() {
        let result = rvs_server_prefix();
        assert!(matches!(result, Prefix::ServerName(_)));
        snapshot("20260528_server_prefix", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_prefixed() {
        let msg = Message::from(Command::PING("test".to_owned(), None));
        let result = rvs_prefixed(Prefix::ServerName("srv".to_owned()), msg);
        assert!(result.prefix.is_some());
        snapshot("20260528_prefixed", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_server_msg() {
        let result = rvs_server_msg(Command::PING("test".to_owned(), None));
        assert!(result.prefix.is_some());
        snapshot("20260528_server_msg", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_server_reply() {
        let result = rvs_server_reply(
            Response::RPL_WELCOME,
            vec!["nick".to_owned(), "hi".to_owned()],
        );
        assert!(result.prefix.is_some());
        snapshot("20260528_server_reply", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_nick_prefix() {
        let result = rvs_nick_prefix("alice", "user1");
        assert!(matches!(result, Prefix::Nickname(_, _, _)));
        snapshot("20260528_nick_prefix", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_motd_start() {
        let result = rvs_build_motd_start("nick");
        snapshot("20260528_build_motd_start", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_motd_line() {
        let result = rvs_build_motd_line("nick", "Hello world");
        snapshot("20260528_build_motd_line", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_motd_end() {
        let result = rvs_build_motd_end("nick");
        snapshot("20260528_build_motd_end", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_cap_nak_reply() {
        let result = rvs_build_cap_nak_reply(Some("multi-prefix".to_owned()));
        assert_eq!(result.len(), 1);
        snapshot("20260528_build_cap_nak_reply", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_nick_change_broadcast() {
        let result = rvs_build_nick_change_broadcast("oldnick", "user1");
        assert_eq!(result.len(), 1);
        snapshot(
            "20260528_build_nick_change_broadcast",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260528_build_who_end() {
        let result = rvs_build_who_end("nick", "*");
        assert_eq!(result.len(), 1);
        snapshot("20260528_build_who_end", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_whowas_end() {
        let result = rvs_build_whowas_end("nick", "someone");
        assert_eq!(result.len(), 1);
        snapshot("20260528_build_whowas_end", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_ison_reply() {
        let result = rvs_build_ison_reply("nick");
        assert_eq!(result.len(), 1);
        snapshot("20260528_build_ison_reply", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_channel_mode() {
        let result = rvs_build_channel_mode("nick", "#12345".to_owned());
        assert_eq!(result.len(), 1);
        snapshot("20260528_build_channel_mode", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_no_topic() {
        let result = rvs_build_no_topic("nick", "#12345".to_owned());
        assert_eq!(result.len(), 1);
        snapshot("20260528_build_no_topic", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_names_end() {
        let result = rvs_build_names_end("nick", "#12345");
        assert_eq!(result.len(), 1);
        snapshot("20260528_build_names_end", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_list_end() {
        let result = rvs_build_list_end("nick");
        assert_eq!(result.len(), 1);
        snapshot("20260528_build_list_end", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_links_end() {
        let result = rvs_build_links_end("nick");
        assert_eq!(result.len(), 1);
        snapshot("20260528_build_links_end", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_admin_reply() {
        let result = rvs_build_admin_reply("nick");
        assert_eq!(result.len(), 1);
        snapshot("20260528_build_admin_reply", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_build_info_reply() {
        let result = rvs_build_info_reply("nick");
        assert_eq!(result.len(), 1);
        snapshot("20260528_build_info_reply", &format!("{result:?}"));
    }

    #[test]
    fn test_20260604_plan_part_private_extracts_peers() {
        let result = rvs_plan_part_private("##111,##222");
        assert_eq!(result, vec![111, 222]);
        snapshot(
            "20260604_plan_part_private_extracts_peers",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260604_plan_part_private_mixed() {
        let result = rvs_plan_part_private("#111,##333");
        assert_eq!(result, vec![333]);
        snapshot("20260604_plan_part_private_mixed", &format!("{result:?}"));
    }

    #[test]
    fn test_20260604_plan_part_private_no_private() {
        let result = rvs_plan_part_private("#111,#222");
        assert!(result.is_empty());
        snapshot(
            "20260604_plan_part_private_no_private",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260604_plan_part_private_empty() {
        let result = rvs_plan_part_private("");
        assert!(result.is_empty());
        snapshot("20260604_plan_part_private_empty", &format!("{result:?}"));
    }

    #[test]
    fn test_20260604_plan_part_private_invalid_double_hash() {
        let result = rvs_plan_part_private("##abc");
        assert!(result.is_empty());
        snapshot(
            "20260604_plan_part_private_invalid_double_hash",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260604_plan_part_generates_effect() {
        let effects = rvs_plan_irc_command(
            &Command::PART("##123,#456".to_owned(), None),
            "nick",
            "user",
            &RegistrationState::Registered,
            false,
            "",
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::PartPrivate(ids) if *ids == vec![123]))
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::Reply(_)))
        );
        snapshot(
            "20260604_plan_part_generates_effect",
            &format!("{effects:?}"),
        );
    }

    #[test]
    fn test_20260604_plan_part_no_effect_for_group_only() {
        let effects = rvs_plan_irc_command(
            &Command::PART("#123,#456".to_owned(), None),
            "nick",
            "user",
            &RegistrationState::Registered,
            false,
            "",
        );
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, IrcCommandEffect::PartPrivate(_)))
        );
        snapshot(
            "20260604_plan_part_no_effect_for_group_only",
            &format!("{effects:?}"),
        );
    }

    #[test]
    fn test_20260604_build_no_such_nick() {
        let result = rvs_build_no_such_nick("nick", "target");
        let msg_str = result.to_string();
        assert!(msg_str.contains("401"));
        snapshot("20260604_build_no_such_nick", &format!("{result:?}"));
    }

    #[test]
    fn test_20260604_plan_quit() {
        let effects = rvs_plan_irc_command(
            &Command::QUIT(None),
            "nick",
            "user",
            &RegistrationState::Registered,
            false,
            "2024-01-01T00:00:00Z",
        );
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::Quit)));
        let reply_count: usize = effects
            .iter()
            .filter_map(|e| match e {
                IrcCommandEffect::Reply(msgs) => Some(msgs.len()),
                _ => None,
            })
            .sum();
        assert_eq!(reply_count, 1);
        snapshot("20260604_plan_quit", &format!("{effects:?}"));
    }
}

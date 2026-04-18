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
    BroadcastNick(String),
    AsyncJoin(String),
    SendPrivmsg { target: String, text: String },
    LogDebug(String),
}

#[allow(non_snake_case)]
pub fn rvs_server_name() -> &'static str {
    SERVER_NAME
}

#[allow(non_snake_case)]
pub fn rvs_server_version() -> &'static str {
    SERVER_VERSION
}

#[allow(non_snake_case)]
pub fn rvs_prefixed(prefix: Prefix, mut message: Message) -> Message {
    message.prefix = Some(prefix);
    message
}

#[allow(non_snake_case)]
pub fn rvs_server_prefix() -> Prefix {
    Prefix::ServerName(SERVER_NAME.to_owned())
}

#[allow(non_snake_case)]
pub fn rvs_server_msg(cmd: Command) -> Message {
    rvs_prefixed(rvs_server_prefix(), Message::from(cmd))
}

#[allow(non_snake_case)]
pub fn rvs_server_reply(response: Response, args: Vec<String>) -> Message {
    rvs_server_msg(Command::Response(response, args))
}

#[allow(non_snake_case)]
pub fn rvs_nick_prefix(nick: &str, user: &str) -> Prefix {
    Prefix::Nickname(nick.to_owned(), user.to_owned(), SERVER_NAME.to_owned())
}

#[allow(non_snake_case)]
pub fn rvs_build_motd_start(nick: &str) -> Message {
    rvs_server_reply(
        Response::RPL_MOTDSTART,
        vec![
            nick.to_owned(),
            format!("- {SERVER_NAME} Message of the Day -"),
        ],
    )
}

#[allow(non_snake_case)]
pub fn rvs_build_motd_line(nick: &str, line: &str) -> Message {
    rvs_server_reply(
        Response::RPL_MOTD,
        vec![nick.to_owned(), format!("- {line}")],
    )
}

#[allow(non_snake_case)]
pub fn rvs_build_motd_end(nick: &str) -> Message {
    rvs_server_reply(
        Response::RPL_ENDOFMOTD,
        vec![nick.to_owned(), "End of /MOTD command.".to_owned()],
    )
}

#[allow(non_snake_case)]
pub fn rvs_format_utc_timestamp(secs: u64) -> String {
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

#[allow(non_snake_case)]
pub fn rvs_current_utc_secs_P() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[allow(non_snake_case)]
pub fn rvs_build_welcome_burst(nick: &str, now_str: &str) -> Vec<Message> {
    vec![
        rvs_server_reply(
            Response::RPL_WELCOME,
            vec![
                nick.to_owned(),
                format!("Welcome to the QQ IRC Gateway {nick}!{nick}@{SERVER_NAME}"),
            ],
        ),
        rvs_server_reply(
            Response::RPL_YOURHOST,
            vec![
                nick.to_owned(),
                format!("Your host is {SERVER_NAME}, running version {SERVER_VERSION}"),
            ],
        ),
        rvs_server_reply(
            Response::RPL_CREATED,
            vec![nick.to_owned(), format!("This server was created {now_str}")],
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
                "are supported by this server".to_owned(),
            ],
        ),
        rvs_build_motd_start(nick),
        rvs_build_motd_line(nick, "Welcome to the QQ IRC Gateway!"),
        rvs_build_motd_line(nick, "Join a QQ group channel with: /JOIN #<group_id>"),
        rvs_build_motd_end(nick),
    ]
}

#[allow(non_snake_case)]
pub fn rvs_extract_digits(value: &str) -> String {
    value.chars().filter(|c| c.is_ascii_digit()).collect()
}

#[allow(non_snake_case, dead_code)]
pub fn rvs_parse_prefix_number_E<N>(value: &str) -> Option<N>
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

#[allow(non_snake_case)]
pub fn rvs_resolve_privmsg_target(target: &str) -> PrivmsgTarget {
    let (raw, prefixed) = match target.strip_prefix('#') {
        Some(s) => (s, true),
        None => (target, false),
    };
    let digits = rvs_extract_digits(raw);
    match digits.parse::<i64>() {
        Ok(id) if prefixed => PrivmsgTarget::Group(id),
        Ok(id) if !prefixed => PrivmsgTarget::Private(id),
        _ => PrivmsgTarget::Invalid(target.to_owned()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationState {
    Pending { got_nick: bool, got_user: bool },
    Registered,
}

impl RegistrationState {
    #[allow(non_snake_case)]
    pub fn rvs_is_registered(&self) -> bool {
        matches!(self, RegistrationState::Registered)
    }
}

#[allow(non_snake_case)]
pub fn rvs_sanitize_nick(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_whitespace() && !c.is_ascii_punctuation())
        .collect::<String>()
}

#[allow(non_snake_case)]
pub fn rvs_build_cap_ls_reply() -> Vec<Message> {
    vec![rvs_server_msg(Command::CAP(
        Some("*".to_owned()),
        CapSubCommand::LS,
        None,
        Some(String::new()),
    ))]
}

#[allow(non_snake_case)]
pub fn rvs_build_cap_nak_reply(requested: Option<String>) -> Vec<Message> {
    vec![rvs_server_msg(Command::CAP(
        Some("*".to_owned()),
        CapSubCommand::NAK,
        None,
        Some(requested.unwrap_or_default()),
    ))]
}

#[allow(non_snake_case)]
pub fn rvs_build_pong(server1: String, server2: Option<String>) -> Vec<Message> {
    vec![rvs_server_msg(Command::PONG(server1, server2))]
}

#[allow(non_snake_case)]
pub fn rvs_build_nick_change_broadcast(nick: &str, user: &str) -> Vec<Message> {
    vec![rvs_prefixed(
        rvs_nick_prefix(nick, user),
        Message::from(Command::NICK(nick.to_owned())),
    )]
}

#[allow(non_snake_case)]
pub fn rvs_build_join_reply(
    nick: &str,
    user: &str,
    group_id: i64,
    group_name: &str,
) -> Vec<Message> {
    let chan = format!("#{group_id}");
    vec![
        rvs_prefixed(
            rvs_nick_prefix(nick, user),
            Message::from(Command::JOIN(chan.clone(), None, None)),
        ),
        rvs_server_reply(
            Response::RPL_TOPIC,
            vec![nick.to_owned(), chan.clone(), group_name.to_owned()],
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
            vec![nick.to_owned(), chan.clone(), "End of /NAMES list.".to_owned()],
        ),
    ]
}

#[allow(non_snake_case)]
pub fn rvs_build_no_such_channel(nick: &str, channel: &str) -> Message {
    rvs_server_reply(
        Response::ERR_NOSUCHCHANNEL,
        vec![
            nick.to_owned(),
            channel.to_owned(),
            "No such channel".to_owned(),
        ],
    )
}

#[allow(non_snake_case)]
pub fn rvs_build_part_broadcast(nick: &str, user: &str, channel: &str, comment: Option<String>) -> Message {
    rvs_prefixed(
        rvs_nick_prefix(nick, user),
        Message::from(Command::PART(channel.to_owned(), comment)),
    )
}

#[allow(non_snake_case)]
pub fn rvs_build_who_end(nick: &str, mask: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_ENDOFWHO,
        vec![nick.to_owned(), mask.to_owned(), "End of /WHO list.".to_owned()],
    )]
}

#[allow(non_snake_case)]
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
                "QQ IRC Gateway".to_owned(),
            ],
        ),
        rvs_server_reply(
            Response::RPL_ENDOFWHOIS,
            vec![nick.to_owned(), mask.to_owned(), "End of /WHOIS list.".to_owned()],
        ),
    ]
}

#[allow(non_snake_case)]
pub fn rvs_build_whowas_end(nick: &str, nicklist: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_ENDOFWHOWAS,
        vec![nick.to_owned(), nicklist.to_owned(), "End of WHOWAS.".to_owned()],
    )]
}

#[allow(non_snake_case)]
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

#[allow(non_snake_case)]
pub fn rvs_build_ison_reply(nick: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_ISON,
        vec![nick.to_owned(), String::new()],
    )]
}

#[allow(non_snake_case)]
pub fn rvs_build_channel_mode(nick: &str, channel: String) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_CHANNELMODEIS,
        vec![nick.to_owned(), channel, "+".to_owned()],
    )]
}

#[allow(non_snake_case)]
pub fn rvs_build_no_topic(nick: &str, channel: String) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_NOTOPIC,
        vec![nick.to_owned(), channel, "No topic is set.".to_owned()],
    )]
}

#[allow(non_snake_case)]
pub fn rvs_build_names_end(nick: &str, chan: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_ENDOFNAMES,
        vec![nick.to_owned(), chan.to_owned(), "End of /NAMES list.".to_owned()],
    )]
}

#[allow(non_snake_case)]
pub fn rvs_build_list_end(nick: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_LISTEND,
        vec![nick.to_owned(), "End of /LIST".to_owned()],
    )]
}

#[allow(non_snake_case)]
pub fn rvs_build_motd_burst(nick: &str) -> Vec<Message> {
    vec![
        rvs_build_motd_start(nick),
        rvs_build_motd_line(nick, "Welcome to the QQ IRC Gateway!"),
        rvs_build_motd_line(nick, "Join a QQ group channel with: /JOIN #<group_id>"),
        rvs_build_motd_end(nick),
    ]
}

#[allow(non_snake_case)]
pub fn rvs_build_version_reply(nick: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_VERSION,
        vec![
            nick.to_owned(),
            rvs_server_version().to_owned(),
            rvs_server_name().to_owned(),
            "QQ IRC Gateway".to_owned(),
        ],
    )]
}

#[allow(non_snake_case)]
pub fn rvs_build_time_reply(nick: &str, now_str: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_TIME,
        vec![nick.to_owned(), rvs_server_name().to_owned(), now_str.to_owned()],
    )]
}

#[allow(non_snake_case)]
pub fn rvs_build_info_reply(nick: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_INFO,
        vec![nick.to_owned(), "QQ IRC Gateway by kovi".to_owned()],
    )]
}

#[allow(non_snake_case)]
pub fn rvs_build_lusers_reply(nick: &str) -> Vec<Message> {
    vec![
        rvs_server_reply(
            Response::RPL_LUSERCLIENT,
            vec![nick.to_owned(), "There is 1 user on 1 server".to_owned()],
        ),
        rvs_server_reply(
            Response::RPL_LUSERME,
            vec![nick.to_owned(), "I have 1 client and 0 servers".to_owned()],
        ),
    ]
}

#[allow(non_snake_case)]
pub fn rvs_build_links_end(nick: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_ENDOFLINKS,
        vec![
            nick.to_owned(),
            "*".to_owned(),
            "End of /LINKS list.".to_owned(),
        ],
    )]
}

#[allow(non_snake_case)]
pub fn rvs_build_admin_reply(nick: &str) -> Vec<Message> {
    vec![rvs_server_reply(
        Response::RPL_ADMINME,
        vec![nick.to_owned(), rvs_server_name().to_owned()],
    )]
}

#[allow(non_snake_case)]
pub fn rvs_build_away_replies(nick: &str, msg: Option<&String>) -> Vec<Message> {
    match msg {
        None => vec![rvs_server_reply(
            Response::RPL_UNAWAY,
            vec![
                nick.to_owned(),
                "You are no longer marked as being away".to_owned(),
            ],
        )],
        Some(_) => vec![rvs_server_reply(
            Response::RPL_NOWAWAY,
            vec![
                nick.to_owned(),
                "You have been marked as being away".to_owned(),
            ],
        )],
    }
}

#[allow(non_snake_case)]
pub fn rvs_handle_part(nick: &str, user: &str, chanlist: &str, comment: &Option<String>) -> Vec<Message> {
    let mut reply = Vec::new();
    for channel in chanlist.split(',') {
        let channel = channel.trim();
        if channel.is_empty() {
            continue;
        }
        reply.push(rvs_build_part_broadcast(nick, user, channel, comment.clone()));
    }
    reply
}

#[allow(non_snake_case)]
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
                vec![IrcCommandEffect::Reply(rvs_build_cap_nak_reply(param2.clone()))]
            }
            CapSubCommand::END => {
                let mut effects = vec![IrcCommandEffect::SetCapNegotiating(false)];
                if let RegistrationState::Pending { got_nick: true, got_user: true } = reg_state {
                    effects.push(IrcCommandEffect::SetRegState(RegistrationState::Registered));
                    effects.push(IrcCommandEffect::BroadcastNick(nick.to_owned()));
                    effects.push(IrcCommandEffect::Reply(rvs_build_welcome_burst(nick, now_str)));
                }
                effects
            }
            _ => vec![],
        },

        Command::NICK(new_nick) => {
            let mut effects = vec![IrcCommandEffect::SetNick(new_nick.clone())];
            match reg_state {
                RegistrationState::Pending { got_nick: _, got_user } => {
                    if *got_user && !cap_negotiating {
                        effects.push(IrcCommandEffect::SetRegState(RegistrationState::Registered));
                        effects.push(IrcCommandEffect::BroadcastNick(new_nick.clone()));
                        effects.push(IrcCommandEffect::Reply(rvs_build_welcome_burst(new_nick, now_str)));
                    } else {
                        effects.push(IrcCommandEffect::SetRegState(RegistrationState::Pending {
                            got_nick: true,
                            got_user: *got_user,
                        }));
                    }
                }
                RegistrationState::Registered => {
                    effects.push(IrcCommandEffect::BroadcastNick(new_nick.clone()));
                    effects.push(IrcCommandEffect::Reply(rvs_build_nick_change_broadcast(new_nick, user)));
                }
            }
            effects
        }

        Command::USER(new_user, _mode, _realname) => {
            let mut effects = vec![IrcCommandEffect::SetUser(new_user.clone())];
            match reg_state {
                RegistrationState::Pending { got_nick, got_user: _ } => {
                    if *got_nick && !cap_negotiating {
                        effects.push(IrcCommandEffect::SetRegState(RegistrationState::Registered));
                        effects.push(IrcCommandEffect::BroadcastNick(nick.to_owned()));
                        effects.push(IrcCommandEffect::Reply(rvs_build_welcome_burst(nick, now_str)));
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
            vec![IrcCommandEffect::Reply(rvs_build_pong(server1.clone(), server2.clone()))]
        }

        Command::QUIT(_) => vec![],

        _ if !reg_state.rvs_is_registered() => {
            vec![IrcCommandEffect::LogDebug(format!("ignoring command before registration: {:?}", command))]
        }

        Command::JOIN(chanlist, _chankeys, _realname) => {
            vec![IrcCommandEffect::AsyncJoin(chanlist.clone())]
        }

        Command::PART(chanlist, comment) => {
            vec![IrcCommandEffect::Reply(rvs_handle_part(nick, user, chanlist, comment))]
        }

        Command::PRIVMSG(target, text) => {
            vec![IrcCommandEffect::SendPrivmsg { target: target.clone(), text: text.clone() }]
        }

        Command::NOTICE(_target, _text) => vec![],

        Command::WHO(mask, _is_op) => {
            let mask = mask.as_deref().unwrap_or("*");
            vec![IrcCommandEffect::Reply(rvs_build_who_end(nick, mask))]
        }

        Command::WHOIS(_target, mask) => {
            vec![IrcCommandEffect::Reply(rvs_build_whois_reply(nick, user, mask))]
        }

        Command::WHOWAS(nicklist, _count, _target) => {
            vec![IrcCommandEffect::Reply(rvs_build_whowas_end(nick, nicklist))]
        }

        Command::USERHOST(nicknames) => {
            vec![IrcCommandEffect::Reply(rvs_build_userhost_reply(nick, nicknames))]
        }

        Command::ISON(_nicklist) => {
            vec![IrcCommandEffect::Reply(rvs_build_ison_reply(nick))]
        }

        Command::ChannelMODE(channel, _modes) => {
            vec![IrcCommandEffect::Reply(rvs_build_channel_mode(nick, channel.clone()))]
        }

        Command::UserMODE(_nickname, _modes) => vec![],

        Command::TOPIC(channel, new_topic) => match new_topic {
            None => vec![IrcCommandEffect::Reply(rvs_build_no_topic(nick, channel.clone()))],
            Some(_) => vec![],
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
            vec![IrcCommandEffect::Reply(rvs_build_away_replies(nick, msg.as_ref()))]
        }

        Command::INVITE(_nickname, _channel) => vec![],

        Command::KICK(_chanlist, _userlist, _comment) => vec![],

        unknown => {
            vec![IrcCommandEffect::LogDebug(format!("unhandled IRC command: {:?}", unknown))]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(name: &str, content: &str) {
        let path = format!("test_out/{}.out", name);
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
        assert_eq!(rvs_parse_prefix_number_E::<i64>("12345"), Some(12345));
        assert_eq!(rvs_parse_prefix_number_E::<i64>("#12345"), Some(12345));
        assert_eq!(rvs_parse_prefix_number_E::<i64>("group123"), Some(123));
    }

    #[test]
    fn test_20260418_parse_prefix_number_empty() {
        assert_eq!(rvs_parse_prefix_number_E::<i64>(""), None);
        assert_eq!(rvs_parse_prefix_number_E::<i64>("abc"), None);
        assert_eq!(rvs_parse_prefix_number_E::<i64>("#"), None);
    }

    #[test]
    fn test_20260418_parse_prefix_number_overflow() {
        assert_eq!(rvs_parse_prefix_number_E::<u8>("999"), None);
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
    }

    #[test]
    fn test_20260418_sanitize_nick_chinese() {
        let result = rvs_sanitize_nick("张三 123");
        snapshot("20260418_sanitize_nick_chinese", &result);
        assert_eq!(result, "张三123");
    }

    #[test]
    fn test_20260418_registration_state() {
        let mut state = RegistrationState::Pending { got_nick: false, got_user: false };
        assert!(!state.rvs_is_registered());
        state = RegistrationState::Registered;
        assert!(state.rvs_is_registered());
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
        let msgs = rvs_build_welcome_burst("testnick", "2024-01-01T00:00:00Z");
        assert!(!msgs.is_empty());
        snapshot("20260418_build_welcome_burst", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_motd_burst() {
        let msgs = rvs_build_motd_burst("testnick");
        assert_eq!(msgs.len(), 4);
        snapshot("20260418_build_motd_burst", &format!("{:?}", msgs));
    }

    #[test]
    fn test_20260418_build_join_reply() {
        let msgs = rvs_build_join_reply("nick", "user", 12345, "TestGroup");
        assert_eq!(msgs.len(), 4);
        snapshot("20260418_build_join_reply", &format!("{:?}", msgs));
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
        snapshot("20260418_extract_digits", "empty, abc, 12345, #12345, group123");
    }

    #[test]
    fn test_20260418_resolve_privmsg_target_group() {
        let result = rvs_resolve_privmsg_target("#12345");
        assert_eq!(result, PrivmsgTarget::Group(12345));
        snapshot("20260418_resolve_privmsg_target_group", &format!("{:?}", result));
    }

    #[test]
    fn test_20260418_resolve_privmsg_target_private() {
        let result = rvs_resolve_privmsg_target("67890");
        assert_eq!(result, PrivmsgTarget::Private(67890));
        snapshot("20260418_resolve_privmsg_target_private", &format!("{:?}", result));
    }

    #[test]
    fn test_20260418_resolve_privmsg_target_invalid() {
        let result = rvs_resolve_privmsg_target("#abc");
        assert!(matches!(result, PrivmsgTarget::Invalid(_)));
        snapshot("20260418_resolve_privmsg_target_invalid", &format!("{:?}", result));
    }

    #[test]
    fn test_20260418_resolve_privmsg_target_invalid_no_hash() {
        let result = rvs_resolve_privmsg_target("nobody");
        assert!(matches!(result, PrivmsgTarget::Invalid(_)));
        snapshot("20260418_resolve_privmsg_target_invalid_no_hash", &format!("{:?}", result));
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

    #[test]
    fn test_20260418_handle_part_empty() {
        let msgs = rvs_handle_part("nick", "user", "", &None);
        assert!(msgs.is_empty());
    }

    fn pending(got_nick: bool, got_user: bool) -> RegistrationState {
        RegistrationState::Pending { got_nick, got_user }
    }

    fn count_replies(effects: &[IrcCommandEffect]) -> usize {
        effects.iter().filter(|e| matches!(e, IrcCommandEffect::Reply(_))).count()
    }

    #[test]
    fn test_20260418_plan_ping() {
        let effects = rvs_plan_irc_command(
            &Command::PING("svr".to_owned(), None),
            "nick", "user", &pending(false, false), false, "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 1);
        snapshot("20260418_plan_ping", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_nick_before_user() {
        let effects = rvs_plan_irc_command(
            &Command::NICK("mynick".to_owned()),
            "nick", "user", &pending(false, false), false, "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 0);
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::SetNick(_))));
        snapshot("20260418_plan_nick_before_user", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_nick_after_user_completes_registration() {
        let effects = rvs_plan_irc_command(
            &Command::NICK("mynick".to_owned()),
            "nick", "user", &pending(false, true), false, "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 1);
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::SetRegState(_))));
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::BroadcastNick(_))));
        snapshot("20260418_plan_nick_after_user", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_nick_blocked_by_cap() {
        let effects = rvs_plan_irc_command(
            &Command::NICK("mynick".to_owned()),
            "nick", "user", &pending(false, true), true, "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 0);
        snapshot("20260418_plan_nick_blocked_by_cap", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_user_after_nick_completes_registration() {
        let effects = rvs_plan_irc_command(
            &Command::USER("myuser".to_owned(), "0".to_owned(), "real".to_owned()),
            "nick", "user", &pending(true, false), false, "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 1);
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::SetRegState(_))));
        snapshot("20260418_plan_user_after_nick", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_cap_end_completes_if_both_received() {
        let effects = rvs_plan_irc_command(
            &Command::CAP(None, CapSubCommand::END, None, None),
            "nick", "user", &pending(true, true), false, "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 1);
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::SetRegState(_))));
        snapshot("20260418_plan_cap_end_complete", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_cap_end_noop_if_not_ready() {
        let effects = rvs_plan_irc_command(
            &Command::CAP(None, CapSubCommand::END, None, None),
            "nick", "user", &pending(false, true), false, "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 0);
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::SetCapNegotiating(false))));
    }

    #[test]
    fn test_20260418_plan_join_returns_async_effect() {
        let effects = rvs_plan_irc_command(
            &Command::JOIN("#12345".to_owned(), None, None),
            "nick", "user", &RegistrationState::Registered, false, "",
        );
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::AsyncJoin(_))));
        snapshot("20260418_plan_join", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_join_ignored_before_registration() {
        let effects = rvs_plan_irc_command(
            &Command::JOIN("#12345".to_owned(), None, None),
            "nick", "user", &pending(false, false), false, "",
        );
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::LogDebug(_))));
        assert!(!effects.iter().any(|e| matches!(e, IrcCommandEffect::AsyncJoin(_))));
    }

    #[test]
    fn test_20260418_plan_privmsg() {
        let effects = rvs_plan_irc_command(
            &Command::PRIVMSG("#12345".to_owned(), "hello".to_owned()),
            "nick", "user", &RegistrationState::Registered, false, "",
        );
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::SendPrivmsg { .. })));
        snapshot("20260418_plan_privmsg", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_motd() {
        let effects = rvs_plan_irc_command(
            &Command::MOTD(None),
            "nick", "user", &RegistrationState::Registered, false, "",
        );
        assert_eq!(count_replies(&effects), 1);
        snapshot("20260418_plan_motd", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_time() {
        let effects = rvs_plan_irc_command(
            &Command::TIME(None),
            "nick", "user", &RegistrationState::Registered, false, "2024-06-15T12:00:00Z",
        );
        assert_eq!(count_replies(&effects), 1);
        snapshot("20260418_plan_time", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_nick_change_when_registered() {
        let effects = rvs_plan_irc_command(
            &Command::NICK("newnick".to_owned()),
            "oldnick", "user", &RegistrationState::Registered, false, "",
        );
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::BroadcastNick(_))));
        assert_eq!(count_replies(&effects), 1);
        snapshot("20260418_plan_nick_change_registered", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_plan_unknown_command() {
        let effects = rvs_plan_irc_command(
            &Command::Raw("BOGUS test".to_owned(), vec![]),
            "nick", "user", &RegistrationState::Registered, false, "",
        );
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::LogDebug(_))));
        snapshot("20260418_plan_unknown_command", &format!("{:?}", effects));
    }

    #[test]
    fn test_20260418_irssi_registration_flow() {
        let mut state = pending(false, false);
        let mut cap_neg = false;

        let effects = rvs_plan_irc_command(
            &Command::CAP(None, CapSubCommand::LS, None, None),
            "", "", &state, cap_neg, "",
        );
        apply_effects(&effects, &mut state, &mut cap_neg);
        assert!(cap_neg);

        let effects = rvs_plan_irc_command(
            &Command::NICK("mynick".to_owned()),
            "", "", &state, cap_neg, "",
        );
        apply_effects(&effects, &mut state, &mut cap_neg);
        assert!(matches!(state, RegistrationState::Pending { got_nick: true, got_user: false }));

        let effects = rvs_plan_irc_command(
            &Command::USER("myuser".to_owned(), "0".to_owned(), "real".to_owned()),
            "mynick", "", &state, cap_neg, "",
        );
        apply_effects(&effects, &mut state, &mut cap_neg);
        assert!(matches!(state, RegistrationState::Pending { got_nick: true, got_user: true }));

        let effects = rvs_plan_irc_command(
            &Command::CAP(None, CapSubCommand::END, None, None),
            "mynick", "myuser", &state, cap_neg, "2024-01-01T00:00:00Z",
        );
        assert_eq!(count_replies(&effects), 1);
        assert!(effects.iter().any(|e| matches!(e, IrcCommandEffect::SetRegState(RegistrationState::Registered))));
        snapshot("20260418_irssi_registration_flow", &format!("{effects:?}"));
    }

    fn apply_effects(effects: &[IrcCommandEffect], state: &mut RegistrationState, cap_neg: &mut bool) {
        for e in effects {
            match e {
                IrcCommandEffect::SetRegState(s) => *state = s.clone(),
                IrcCommandEffect::SetCapNegotiating(v) => *cap_neg = *v,
                _ => {}
            }
        }
    }
}

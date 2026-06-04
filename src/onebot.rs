use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use kovi::log;
use kovi::serde_json;

use crate::config::ImageMode;
use crate::irc_proto::rvs_sanitize_nick;

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
    let json = include_str!("../data/qq_emoji.json");
    let decoded: Vec<EmojiInfo> =
        serde_json::from_str(json).expect("never: embedded emoji data is valid JSON");
    decoded
        .into_iter()
        .map(|item| (item.emoji_id.clone(), item))
        .collect()
});

fn rvs_segment_display_name(type_: &str) -> String {
    match type_ {
        "record" => "语音".to_owned(),
        "video" => "视频".to_owned(),
        "forward" => "转发消息".to_owned(),
        "json" => "卡片消息".to_owned(),
        "xml" => "卡片消息".to_owned(),
        "music" => "音乐分享".to_owned(),
        "location" => "位置分享".to_owned(),
        "shake" => "窗口抖动".to_owned(),
        "poke" => "戳一戳".to_owned(),
        "dice" => "骰子".to_owned(),
        "rps" => "猜拳".to_owned(),
        _ => type_.to_owned(),
    }
}

fn rvs_lookup_emoji_name<'a>(emoji_map: &'a HashMap<String, EmojiInfo>, id: &'a str) -> &'a str {
    emoji_map
        .get(id)
        .map(|info| info.describe.as_str())
        .unwrap_or_else(|| id)
}

fn rvs_render_pure_segment(
    emoji_map: &HashMap<String, EmojiInfo>,
    type_: &str,
    data: &serde_json::Value,
) -> Option<String> {
    match type_ {
        "text" => Some(
            data.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned(),
        ),
        "face" => {
            let id = data
                .get("id")
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.as_i64().map(|n| n.to_string()).unwrap_or_default())
                })
                .unwrap_or_default();
            Some(format!("[{}]", rvs_lookup_emoji_name(emoji_map, &id)))
        }
        "image" | "at" | "reply" => None,
        _ => Some(format!("[{}]", rvs_segment_display_name(type_))),
    }
}

/// Renders a OneBot message array into a single IRC-friendly string.
///
/// # Panics
///
/// Never panics in practice; the `P` suffix is inherited from downstream renderers.
pub async fn rvs_render_qq_array_message_ABIPS(
    messages: &kovi::Message,
    bot: Arc<kovi::RuntimeBot>,
    group_id: Option<i64>,
    image_mode: &ImageMode,
) -> String {
    use futures::{StreamExt, stream};

    stream::iter(messages.iter().cloned())
        .map(async |segment| {
            if let Some(rendered) =
                rvs_render_pure_segment(&QQ_EMOJI_INFO, &segment.type_, &segment.data)
            {
                return rendered;
            }
            match segment.type_.as_str() {
                "image" => rvs_render_image_segment_ABIP(&segment, &bot, image_mode).await,
                "reply" => rvs_render_reply_segment_AIP(&segment, group_id, &bot).await,
                "at" => {
                    let qq = segment
                        .data
                        .get("qq")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    rvs_render_at_mention_AIP(qq, group_id, &bot).await
                }
                _ => format!("[{}]", rvs_segment_display_name(&segment.type_)),
            }
        })
        .buffered(16)
        .collect::<Vec<_>>()
        .await
        .join("")
}

/// Renders a reply segment into a quoted preview showing the original sender and a content hint.
///
/// # Panics
///
/// Never panics in practice; the `P` suffix is inherited from downstream helpers.
async fn rvs_render_reply_segment_AIP(
    segment: &kovi::bot::message::Segment,
    group_id: Option<i64>,
    bot: &kovi::RuntimeBot,
) -> String {
    let message_id: i32 = segment
        .data
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if message_id == 0 {
        return "[回复]".to_owned();
    }
    let info = match bot.get_msg(message_id).await {
        Ok(info) => info,
        Err(_) => return "[回复]".to_owned(),
    };
    let sender_id = match info
        .data
        .get("sender")
        .and_then(|s| s.get("user_id"))
        .and_then(|v| v.as_i64())
    {
        Some(id) => id,
        None => return "[回复]".to_owned(),
    };
    let sender_nick = match group_id {
        Some(gid) => match bot.get_group_member_info(gid, sender_id, false).await {
            Ok(m) => rvs_extract_group_sender_name(&m).unwrap_or_else(|| sender_id.to_string()),
            Err(_) => sender_id.to_string(),
        },
        None => match bot.get_stranger_info(sender_id, false).await {
            Ok(m) => m
                .data
                .get("nickname")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| sender_id.to_string()),
            Err(_) => sender_id.to_string(),
        },
    };
    let sanitized = rvs_sanitize_nick(&sender_nick);
    let sender_name = rvs_pick_sender_name(&sanitized, sender_id);
    let content_hint = rvs_reply_content_hint(&info);
    format!("> {sender_name}: {content_hint}\n")
}

fn rvs_reply_content_hint(info: &kovi::ApiReturn) -> String {
    let text = info
        .data
        .get("raw_message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(20)
        .collect::<String>();
    if text.is_empty() {
        "...".to_owned()
    } else {
        format!("{text}...")
    }
}

/// Renders an image segment as a placeholder tag or ASCII art depending on `image_mode`.
///
/// # Panics
///
/// Never panics in practice; the `P` suffix is inherited from the ASCII-art renderer.
async fn rvs_render_image_segment_ABIP(
    segment: &kovi::bot::message::Segment,
    bot: &kovi::RuntimeBot,
    image_mode: &ImageMode,
) -> String {
    let file = match segment.data.get("file").and_then(|v| v.as_str()) {
        Some(f) => f,
        None => return "[图片：未知]".to_owned(),
    };
    match image_mode {
        ImageMode::Placeholder => {
            if let Some(summary) = segment
                .data
                .get("summary")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                return format!("[表情包：{summary}]");
            }
            if let Some(url) = segment.data.get("url").and_then(|v| v.as_str()) {
                let path_part = url.split('?').next().unwrap_or(url);
                if path_part.to_lowercase().ends_with(".gif") {
                    return "[表情包]".to_owned();
                }
            }
            return "[图片]".to_owned();
        }
        ImageMode::AsciiArt => {}
    }
    let path = match bot.get_image(file).await {
        Ok(info) => match info.data.get("file").and_then(|v| v.as_str()) {
            Some(p) => p.to_owned(),
            None => return "[图片：无路径]".to_owned(),
        },
        Err(err) => {
            log::warn!("image fetch error: {err}");
            return "[图片：获取失败]".to_owned();
        }
    };
    log::info!("image path: {path}");
    rvs_render_image_to_ascii_BIP(&path)
}
/// Renders an image to colored ASCII art.
///
/// # Panics
///
/// Never panics in practice; P marker inherited from callees.
fn rvs_render_ascii_art_IP(image: &image::DynamicImage) -> String {
    let mut buf = String::new();
    let render_options = rascii_art::RenderOptions::default()
        .height(15)
        .colored(true);
    if let Err(err) = rascii_art::render_image_to(image, &mut buf, &render_options) {
        log::warn!("image render error: {err}");
        return "[图片：渲染失败]".to_owned();
    };
    format!("\n{buf}\n")
}

/// Loads an image from disk and renders it as ASCII art.
///
/// # Panics
///
/// Never panics in practice; errors are returned as placeholder strings.
fn rvs_render_image_to_ascii_BIP(path: &str) -> String {
    let image_reader = match image::io::Reader::open(path) {
        Ok(reader) => reader,
        Err(err) => {
            log::warn!("image open error: {err}");
            return "[图片：打开失败]".to_owned();
        }
    };
    let image_data = match image_reader.with_guessed_format() {
        Ok(data) => data,
        Err(err) => {
            log::warn!("image format error: {err}");
            return "[图片：格式错误]".to_owned();
        }
    };
    let image = match image_data.decode() {
        Ok(image) => image,
        Err(err) => {
            log::warn!("image decode error: {err}");
            return "[图片：解码失败]".to_owned();
        }
    };
    rvs_render_ascii_art_IP(&image)
}

fn rvs_pick_display_name(info: &kovi::ApiReturn, fallback: &str) -> String {
    let card = info
        .data
        .get("card")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if !card.is_empty() {
        return card.to_owned();
    }
    let nick = info
        .data
        .get("nickname")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if !nick.is_empty() {
        return nick.to_owned();
    }
    fallback.to_owned()
}

/// Renders an @-mention segment, resolving the QQ number to a display name via the bot API.
///
/// # Panics
///
/// Never panics in practice; the `P` suffix is inherited from downstream helpers.
async fn rvs_render_at_mention_AIP(
    qq: &str,
    group_id: Option<i64>,
    bot: &kovi::RuntimeBot,
) -> String {
    let name = if let Ok(user_id) = qq.parse::<i64>() {
        let member: Option<kovi::ApiReturn> = if let Some(gid) = group_id {
            bot.get_group_member_info(gid, user_id, false).await.ok()
        } else {
            None
        };
        if let Some(ref info) = member {
            rvs_pick_display_name(info, qq)
        } else {
            match bot.get_stranger_info(user_id, false).await {
                Ok(r) => r
                    .data
                    .get("nickname")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| qq.to_owned()),
                Err(_) => qq.to_owned(),
            }
        }
    } else {
        qq.to_owned()
    };
    format!("@{name}")
}

/// Extracts a group sender display name from API info, combining title and card when both are present.
pub fn rvs_extract_group_sender_name(info: &kovi::ApiReturn) -> Option<String> {
    let title = info
        .data
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let card = info
        .data
        .get("card")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let nick = info
        .data
        .get("nickname")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let raw = if !title.is_empty() && !card.is_empty() {
        format!("【{title}】{card}")
    } else if !card.is_empty() {
        card.to_owned()
    } else if !nick.is_empty() {
        nick.to_owned()
    } else {
        return None;
    };
    let sanitized: String = raw.chars().filter(|c: &char| !c.is_whitespace()).collect();
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

/// Returns the sanitized name if non-empty, otherwise falls back to the numeric sender id.
pub fn rvs_pick_sender_name(sanitized: &str, fallback_id: i64) -> String {
    debug_assert!(fallback_id > 0, "fallback_id must be positive");
    if sanitized.is_empty() {
        fallback_id.to_string()
    } else {
        sanitized.to_owned()
    }
}

/// Splits a multi-line string into individual lines.
pub fn rvs_split_lines(content: &str) -> Vec<String> {
    content.split('\n').map(str::to_owned).collect()
}

impl RenderedOnebotMessage {
    /// Constructs a `RenderedOnebotMessage` from a `MsgEvent`, resolving sender names and rendering all segments.
    ///
    /// # Panics
    ///
    /// Panics if a group message event lacks `group_id` (never panics in practice).
    pub async fn rvs_from_msg_event_ABIPS(
        value: &kovi::MsgEvent,
        bot: Arc<kovi::RuntimeBot>,
        image_mode: &ImageMode,
    ) -> Self {
        if value.message_type == "private" {
            let raw_name = match bot.get_stranger_info(value.sender.user_id, false).await {
                Ok(info) => info
                    .data
                    .get("nickname")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| value.get_sender_nickname()),
                Err(_) => value.get_sender_nickname(),
            };
            let sender_name =
                rvs_pick_sender_name(&rvs_sanitize_nick(&raw_name), value.sender.user_id);
            let content = rvs_split_lines(
                &rvs_render_qq_array_message_ABIPS(
                    &value.message,
                    Arc::clone(&bot),
                    None,
                    image_mode,
                )
                .await,
            );
            Self::Private {
                content,
                sender_id: value.sender.user_id,
                sender_name,
            }
        } else {
            let gid = match value.group_id {
                Some(id) => id,
                None => {
                    log::warn!(
                        "non-private message without group_id, message_type={}",
                        value.message_type
                    );
                    let content = rvs_split_lines(
                        &rvs_render_qq_array_message_ABIPS(
                            &value.message,
                            Arc::clone(&bot),
                            None,
                            image_mode,
                        )
                        .await,
                    );
                    return Self::Group {
                        content,
                        sender_id: value.sender.user_id,
                        group_id: 0,
                        sender_name: value.sender.user_id.to_string(),
                    };
                }
            };
            let sender_name = match bot
                .get_group_member_info(gid, value.sender.user_id, false)
                .await
            {
                Ok(info) => rvs_extract_group_sender_name(&info),
                Err(_) => None,
            }
            .unwrap_or_else(|| value.sender.user_id.to_string());
            let content = rvs_split_lines(
                &rvs_render_qq_array_message_ABIPS(
                    &value.message,
                    Arc::clone(&bot),
                    Some(gid),
                    image_mode,
                )
                .await,
            );
            Self::Group {
                content,
                sender_id: value.sender.user_id,
                group_id: gid,
                sender_name,
            }
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
    fn test_20260418_render_pure_segment_text() {
        let data = serde_json::json!({"text": "hello world"});
        let result = rvs_render_pure_segment(&HashMap::new(), "text", &data);
        assert_eq!(result, Some("hello world".to_owned()));
        snapshot("20260418_render_pure_segment_text", &format!("{result:?}"));
    }

    #[test]
    fn test_20260418_render_pure_segment_text_empty() {
        let data = serde_json::json!({});
        let result = rvs_render_pure_segment(&HashMap::new(), "text", &data);
        assert_eq!(result, Some("".to_owned()));
        snapshot(
            "20260418_render_pure_segment_text_empty",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260418_render_pure_segment_face() {
        let data = serde_json::json!({"id": "178"});
        let mut map = HashMap::new();
        map.insert(
            "178".to_owned(),
            EmojiInfo {
                emoji_id: "178".to_owned(),
                describe: "微笑".to_owned(),
            },
        );
        let result = rvs_render_pure_segment(&map, "face", &data);
        assert_eq!(result, Some("[微笑]".to_owned()));
        snapshot("20260418_render_pure_segment_face", &format!("{result:?}"));
    }

    #[test]
    fn test_20260604_render_pure_segment_face_integer_id() {
        let data = serde_json::json!({"id": 178});
        let mut map = HashMap::new();
        map.insert(
            "178".to_owned(),
            EmojiInfo {
                emoji_id: "178".to_owned(),
                describe: "微笑".to_owned(),
            },
        );
        let result = rvs_render_pure_segment(&map, "face", &data);
        assert_eq!(result, Some("[微笑]".to_owned()));
        snapshot(
            "20260604_render_pure_segment_face_integer_id",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260418_render_pure_segment_face_missing() {
        let data = serde_json::json!({"id": "999"});
        let result = rvs_render_pure_segment(&HashMap::new(), "face", &data);
        assert_eq!(result, Some("[999]".to_owned()));
        snapshot(
            "20260418_render_pure_segment_face_missing",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260418_render_pure_segment_image_returns_none() {
        let data = serde_json::json!({"file": "test.jpg"});
        let result = rvs_render_pure_segment(&HashMap::new(), "image", &data);
        assert!(result.is_none());
        snapshot("20260418_render_pure_segment_image_returns_none", "None");
    }

    #[test]
    fn test_20260418_render_pure_segment_at_returns_none() {
        let data = serde_json::json!({"qq": "12345"});
        let result = rvs_render_pure_segment(&HashMap::new(), "at", &data);
        assert!(result.is_none());
        snapshot("20260418_render_pure_segment_at_returns_none", "None");
    }

    #[test]
    fn test_20260418_render_pure_segment_unknown() {
        let data = serde_json::json!({});
        let result = rvs_render_pure_segment(&HashMap::new(), "record", &data);
        assert_eq!(result, Some("[语音]".to_owned()));
        snapshot(
            "20260418_render_pure_segment_unknown",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260528_extract_group_sender_name_with_card() {
        let info = kovi::ApiReturn {
            status: "ok".to_owned(),
            retcode: 0,
            data: serde_json::json!({
                "card": "Alice",
                "nickname": "alice_nick",
                "title": ""
            }),
            echo: String::new(),
        };
        let result = rvs_extract_group_sender_name(&info);
        assert_eq!(result, Some("Alice".to_owned()));
        snapshot(
            "20260528_extract_group_sender_name_with_card",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260528_extract_group_sender_name_with_title_and_card() {
        let info = kovi::ApiReturn {
            status: "ok".to_owned(),
            retcode: 0,
            data: serde_json::json!({
                "card": "Bob",
                "nickname": "bob_nick",
                "title": "管理员"
            }),
            echo: String::new(),
        };
        let result = rvs_extract_group_sender_name(&info);
        assert_eq!(result, Some("【管理员】Bob".to_owned()));
        snapshot(
            "20260528_extract_group_sender_name_with_title_and_card",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260528_extract_group_sender_name_nickname_only() {
        let info = kovi::ApiReturn {
            status: "ok".to_owned(),
            retcode: 0,
            data: serde_json::json!({
                "card": "",
                "nickname": "charlie",
                "title": ""
            }),
            echo: String::new(),
        };
        let result = rvs_extract_group_sender_name(&info);
        assert_eq!(result, Some("charlie".to_owned()));
        snapshot(
            "20260528_extract_group_sender_name_nickname_only",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260528_extract_group_sender_name_all_empty() {
        let info = kovi::ApiReturn {
            status: "ok".to_owned(),
            retcode: 0,
            data: serde_json::json!({
                "card": "",
                "nickname": "",
                "title": ""
            }),
            echo: String::new(),
        };
        let result = rvs_extract_group_sender_name(&info);
        assert!(result.is_none());
        snapshot(
            "20260528_extract_group_sender_name_all_empty",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260528_pick_sender_name_nonempty() {
        let result = rvs_pick_sender_name("alice", 12345);
        assert_eq!(result, "alice");
        snapshot("20260528_pick_sender_name_nonempty", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_pick_sender_name_fallback() {
        let result = rvs_pick_sender_name("", 12345);
        assert_eq!(result, "12345");
        snapshot("20260528_pick_sender_name_fallback", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_split_lines_single() {
        let result = rvs_split_lines("hello");
        assert_eq!(result, vec!["hello"]);
        snapshot("20260528_split_lines_single", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_split_lines_multiple() {
        let result = rvs_split_lines("hello\nworld\nfoo");
        assert_eq!(result, vec!["hello", "world", "foo"]);
        snapshot("20260528_split_lines_multiple", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_split_lines_empty() {
        let result = rvs_split_lines("");
        assert_eq!(result, vec![""]);
        snapshot("20260528_split_lines_empty", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_lookup_emoji_name_found() {
        let mut map = HashMap::new();
        map.insert(
            "123".to_owned(),
            EmojiInfo {
                emoji_id: "123".to_owned(),
                describe: "smile".to_owned(),
            },
        );
        let result = rvs_lookup_emoji_name(&map, "123");
        assert_eq!(result, "smile");
        snapshot("20260528_lookup_emoji_name_found", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_lookup_emoji_name_not_found() {
        let map = HashMap::new();
        let result = rvs_lookup_emoji_name(&map, "456");
        assert_eq!(result, "456");
        snapshot(
            "20260528_lookup_emoji_name_not_found",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260528_pick_display_name_card() {
        let info = kovi::ApiReturn {
            status: "ok".to_owned(),
            retcode: 0,
            data: serde_json::json!({"card": "CardName", "nickname": "NickName"}),
            echo: String::new(),
        };
        let result = rvs_pick_display_name(&info, "fallback");
        assert_eq!(result, "CardName");
        snapshot("20260528_pick_display_name_card", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_pick_display_name_nick() {
        let info = kovi::ApiReturn {
            status: "ok".to_owned(),
            retcode: 0,
            data: serde_json::json!({"nickname": "NickName"}),
            echo: String::new(),
        };
        let result = rvs_pick_display_name(&info, "fallback");
        assert_eq!(result, "NickName");
        snapshot("20260528_pick_display_name_nick", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_pick_display_name_fallback() {
        let info = kovi::ApiReturn {
            status: "ok".to_owned(),
            retcode: 0,
            data: serde_json::json!({}),
            echo: String::new(),
        };
        let result = rvs_pick_display_name(&info, "fallback");
        assert_eq!(result, "fallback");
        snapshot(
            "20260528_pick_display_name_fallback",
            &format!("{result:?}"),
        );
    }

    #[test]
    fn test_20260528_reply_content_hint_normal() {
        let info = kovi::ApiReturn {
            status: "ok".to_owned(),
            retcode: 0,
            data: serde_json::json!({"raw_message": "hello world this is a test"}),
            echo: String::new(),
        };
        let result = rvs_reply_content_hint(&info);
        assert!(result.contains("hello"));
        snapshot("20260528_reply_content_hint_normal", &format!("{result:?}"));
    }

    #[test]
    fn test_20260528_reply_content_hint_empty() {
        let info = kovi::ApiReturn {
            status: "ok".to_owned(),
            retcode: 0,
            data: serde_json::json!({}),
            echo: String::new(),
        };
        let result = rvs_reply_content_hint(&info);
        assert_eq!(result, "...");
        snapshot("20260528_reply_content_hint_empty", &format!("{result:?}"));
    }

    #[test]
    fn test_20260604_segment_display_name_known() {
        assert_eq!(rvs_segment_display_name("record"), "语音");
        assert_eq!(rvs_segment_display_name("video"), "视频");
        assert_eq!(rvs_segment_display_name("forward"), "转发消息");
        assert_eq!(rvs_segment_display_name("json"), "卡片消息");
        assert_eq!(rvs_segment_display_name("xml"), "卡片消息");
        assert_eq!(rvs_segment_display_name("music"), "音乐分享");
        assert_eq!(rvs_segment_display_name("location"), "位置分享");
        assert_eq!(rvs_segment_display_name("shake"), "窗口抖动");
        assert_eq!(rvs_segment_display_name("poke"), "戳一戳");
        assert_eq!(rvs_segment_display_name("dice"), "骰子");
        assert_eq!(rvs_segment_display_name("rps"), "猜拳");
        snapshot(
            "20260604_segment_display_name_known",
            "all known mappings verified",
        );
    }

    #[test]
    fn test_20260604_segment_display_name_unknown() {
        assert_eq!(rvs_segment_display_name("custom_type"), "custom_type");
        assert_eq!(rvs_segment_display_name("something"), "something");
        snapshot(
            "20260604_segment_display_name_unknown",
            "unknown types pass through unchanged",
        );
    }
}

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use kovi::log;
use kovi::serde_json;

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
    let decoded: Vec<EmojiInfo> = serde_json::from_str(json).unwrap();
    decoded
        .into_iter()
        .map(|item| (item.emoji_id.clone(), item))
        .collect()
});

#[allow(non_snake_case)]
fn rvs_lookup_emoji_name<'a>(emoji_map: &'a HashMap<String, EmojiInfo>, id: &'a str) -> &'a str {
    emoji_map
        .get(id)
        .map(|info| info.describe.as_str())
        .unwrap_or_else(|| id)
}

#[allow(non_snake_case)]
fn rvs_render_pure_segment(emoji_map: &HashMap<String, EmojiInfo>, type_: &str, data: &serde_json::Value) -> Option<String> {
    match type_ {
        "text" => Some(data["text"].as_str().unwrap_or("").to_owned()),
        "face" => {
            let id = data["id"].as_str().unwrap_or("").to_owned();
            Some(format!("[face:{}]", rvs_lookup_emoji_name(emoji_map, &id)))
        }
        "image" | "at" => None,
        _ => Some(format!("[{type_}]")),
    }
}

#[allow(non_snake_case)]
pub async fn rvs_render_qq_array_message_AEIP(
    messages: &kovi::Message,
    bot: Arc<kovi::RuntimeBot>,
    group_id: Option<i64>,
) -> String {
    use futures::{StreamExt, stream};

    stream::iter(messages.iter().cloned())
        .map(async |segment| {
            if let Some(rendered) = rvs_render_pure_segment(&QQ_EMOJI_INFO, &segment.type_, &segment.data) {
                return rendered;
            }
            match segment.type_.as_str() {
                "image" => rvs_render_image_segment_AEIP(&segment, &bot).await,
                "at" => {
                    let qq = segment.data["qq"].as_str().unwrap_or("?");
                    rvs_render_at_mention_AEIP(qq, group_id, &bot).await
                }
                _ => format!("[{}]", segment.type_),
            }
        })
        .buffered(16)
        .collect::<Vec<_>>()
        .await
        .join("")
}

#[allow(non_snake_case)]
async fn rvs_render_image_segment_AEIP(
    segment: &kovi::bot::message::Segment,
    bot: &kovi::RuntimeBot,
) -> String {
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
    rvs_render_image_to_ascii_E(&path)
}

#[allow(non_snake_case)]
fn rvs_render_ascii_art_E(image: &image::DynamicImage) -> String {
    let mut buf = String::new();
    let render_options = rascii_art::RenderOptions::default()
        .height(15)
        .colored(true);
    if let Err(err) = rascii_art::render_image_to(image, &mut buf, &render_options) {
        return format!("[image:render-error:{err}]");
    };
    format!("\n{buf}\n")
}

#[allow(non_snake_case)]
fn rvs_render_image_to_ascii_E(path: &str) -> String {
    let image_reader = match image::io::Reader::open(path) {
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
    rvs_render_ascii_art_E(&image)
}

#[allow(non_snake_case)]
fn rvs_pick_display_name(info: &kovi::ApiReturn, fallback: &str) -> String {
    let card = info.data["card"].as_str().unwrap_or("").trim();
    if !card.is_empty() {
        return card.to_owned();
    }
    let nick = info.data["nickname"].as_str().unwrap_or("").trim();
    if !nick.is_empty() {
        return nick.to_owned();
    }
    fallback.to_owned()
}

#[allow(non_snake_case)]
async fn rvs_render_at_mention_AEIP(
    qq: &str,
    group_id: Option<i64>,
    bot: &kovi::RuntimeBot,
) -> String {
    let name = if let Ok(user_id) = qq.parse::<i64>() {
        let member = if let Some(gid) = group_id {
            bot.get_group_member_info(gid, user_id, false).await.ok()
        } else {
            None
        };
        if let Some(ref info) = member {
            rvs_pick_display_name(info, qq)
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

#[allow(non_snake_case)]
pub fn rvs_extract_group_sender_name(info: &kovi::ApiReturn) -> Option<String> {
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
        return None;
    };
    let sanitized: String = raw.chars().filter(|c: &char| !c.is_whitespace()).collect();
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

#[allow(non_snake_case)]
pub fn rvs_pick_sender_name(sanitized: &str, fallback_id: i64) -> String {
    if sanitized.is_empty() {
        fallback_id.to_string()
    } else {
        sanitized.to_owned()
    }
}

#[allow(non_snake_case)]
pub fn rvs_split_lines(content: &str) -> Vec<String> {
    content.split('\n').map(str::to_owned).collect()
}

impl RenderedOnebotMessage {
    #[allow(non_snake_case)]
    pub async fn rvs_from_msg_event_AEIP(
        value: &kovi::MsgEvent,
        bot: Arc<kovi::RuntimeBot>,
    ) -> Self {
        if value.is_private() {
            let sender_name = rvs_pick_sender_name(
                &rvs_sanitize_nick(&value.get_sender_nickname()),
                value.sender.user_id,
            );
            let content = rvs_split_lines(
                &rvs_render_qq_array_message_AEIP(&value.message, Arc::clone(&bot), None).await,
            );
            Self::Private {
                content,
                sender_id: value.sender.user_id,
                sender_name,
            }
        } else {
            let gid = value.group_id.unwrap();
            let sender_name = bot
                .get_group_member_info(gid, value.sender.user_id, false)
                .await
                .ok()
                .and_then(|info| rvs_extract_group_sender_name(&info))
                .unwrap_or_else(|| value.sender.user_id.to_string());
            let content = rvs_split_lines(
                &rvs_render_qq_array_message_AEIP(&value.message, Arc::clone(&bot), Some(gid))
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
mod tests {
    use super::*;

    fn snapshot(name: &str, content: &str) {
        let path = format!("test_out/{}.out", name);
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
        snapshot("20260418_render_pure_segment_text_empty", &format!("{result:?}"));
    }

    #[test]
    fn test_20260418_render_pure_segment_face() {
        let data = serde_json::json!({"id": "178"});
        let mut map = HashMap::new();
        map.insert("178".to_owned(), EmojiInfo { emoji_id: "178".to_owned(), describe: "微笑".to_owned() });
        let result = rvs_render_pure_segment(&map, "face", &data);
        assert_eq!(result, Some("[face:微笑]".to_owned()));
        snapshot("20260418_render_pure_segment_face", &format!("{result:?}"));
    }

    #[test]
    fn test_20260418_render_pure_segment_face_missing() {
        let data = serde_json::json!({"id": "999"});
        let result = rvs_render_pure_segment(&HashMap::new(), "face", &data);
        assert_eq!(result, Some("[face:999]".to_owned()));
        snapshot("20260418_render_pure_segment_face_missing", &format!("{result:?}"));
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
        assert_eq!(result, Some("[record]".to_owned()));
        snapshot("20260418_render_pure_segment_unknown", &format!("{result:?}"));
    }
}

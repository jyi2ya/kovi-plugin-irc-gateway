use std::collections::HashMap;

use kovi::serde_json;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmojiInfo {
    pub emoji_id: String,
    pub describe: String,
}

pub static QQ_EMOJI_INFO: std::sync::LazyLock<HashMap<String, EmojiInfo>> =
    std::sync::LazyLock::new(|| {
        let json = include_str!("../data/qq_emoji.json");
        let decoded: Vec<EmojiInfo> =
            serde_json::from_str(json).expect("never: embedded emoji data is valid JSON");
        decoded
            .into_iter()
            .map(|item| (item.emoji_id.clone(), item))
            .collect()
    });

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GroupInfoData {
    #[serde(default)]
    pub group_name: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct GroupMemberInfoData {
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub card: String,
    #[serde(default)]
    pub title: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct StrangerInfoData {
    #[serde(default)]
    pub nickname: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct GetImageData {
    #[serde(default)]
    pub file: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct GetMsgSender {
    #[serde(default)]
    pub user_id: i64,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct GetMsgData {
    #[serde(default)]
    pub sender: GetMsgSender,
    #[serde(default)]
    pub raw_message: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct TextSegmentData {
    #[serde(default)]
    pub text: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct FaceSegmentData {
    #[serde(default)]
    pub id: FaceId,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ImageSegmentData {
    #[serde(default)]
    pub file: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct AtSegmentData {
    #[serde(default)]
    pub qq: String,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ReplySegmentData {
    #[serde(default)]
    pub id: ReplyId,
}

#[derive(Debug, Clone)]
pub enum FaceId {
    String(String),
    Integer(i64),
}

impl Default for FaceId {
    fn default() -> Self {
        Self::String(String::new())
    }
}

impl<'de> serde::Deserialize<'de> for FaceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;
        struct FaceIdVisitor;

        impl de::Visitor<'_> for FaceIdVisitor {
            type Value = FaceId;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a string or integer")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<FaceId, E> {
                Ok(FaceId::String(v.to_owned()))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<FaceId, E> {
                Ok(FaceId::Integer(v))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<FaceId, E> {
                Ok(FaceId::Integer(v as i64))
            }
        }

        deserializer.deserialize_any(FaceIdVisitor)
    }
}

impl FaceId {
    /// Converts the face ID to its string representation.
    pub fn rvs_to_string(&self) -> String {
        match self {
            FaceId::String(s) => s.clone(),
            FaceId::Integer(n) => n.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ReplyId {
    String(String),
    Integer(i32),
}

impl Default for ReplyId {
    fn default() -> Self {
        Self::Integer(0)
    }
}

impl<'de> serde::Deserialize<'de> for ReplyId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;
        struct ReplyIdVisitor;

        impl de::Visitor<'_> for ReplyIdVisitor {
            type Value = ReplyId;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a string or integer")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<ReplyId, E> {
                Ok(ReplyId::String(v.to_owned()))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<ReplyId, E> {
                Ok(ReplyId::Integer(v as i32))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<ReplyId, E> {
                Ok(ReplyId::Integer(v as i32))
            }
        }

        deserializer.deserialize_any(ReplyIdVisitor)
    }
}

impl ReplyId {
    /// Converts the reply ID to an i32 message ID.
    pub fn rvs_to_i32(&self) -> i32 {
        match self {
            ReplyId::String(s) => s.parse().unwrap_or(0),
            ReplyId::Integer(n) => *n,
        }
    }
}

/// Deserializes a `serde_json::Value` into a typed struct, returning `None` on failure.
fn rvs_from_value<T: serde::de::DeserializeOwned>(value: &serde_json::Value) -> Option<T> {
    serde_json::from_value(value.clone()).ok()
}

/// Parses a `get_group_info` API response data into a typed struct.
pub fn rvs_parse_group_info(value: &serde_json::Value) -> Option<GroupInfoData> {
    rvs_from_value(value)
}

/// Parses a `get_group_member_info` API response data into a typed struct.
pub fn rvs_parse_group_member_info(value: &serde_json::Value) -> Option<GroupMemberInfoData> {
    rvs_from_value(value)
}

/// Parses a `get_stranger_info` API response data into a typed struct.
pub fn rvs_parse_stranger_info(value: &serde_json::Value) -> Option<StrangerInfoData> {
    rvs_from_value(value)
}

/// Parses a `get_image` API response data into a typed struct.
pub fn rvs_parse_get_image(value: &serde_json::Value) -> Option<GetImageData> {
    rvs_from_value(value)
}

/// Parses a `get_msg` API response data into a typed struct.
pub fn rvs_parse_get_msg(value: &serde_json::Value) -> Option<GetMsgData> {
    rvs_from_value(value)
}

/// Parses a text segment's data into a typed struct.
pub fn rvs_parse_text_data(value: &serde_json::Value) -> TextSegmentData {
    rvs_from_value(value).unwrap_or_default()
}

/// Parses a face segment's data into a typed struct.
pub fn rvs_parse_face_data(value: &serde_json::Value) -> FaceSegmentData {
    rvs_from_value(value).unwrap_or_default()
}

/// Parses an image segment's data into a typed struct.
pub fn rvs_parse_image_data(value: &serde_json::Value) -> ImageSegmentData {
    rvs_from_value(value).unwrap_or_default()
}

/// Parses an at-mention segment's data into a typed struct.
pub fn rvs_parse_at_data(value: &serde_json::Value) -> AtSegmentData {
    rvs_from_value(value).unwrap_or_default()
}

/// Parses a reply segment's data into a typed struct.
pub fn rvs_parse_reply_data(value: &serde_json::Value) -> ReplySegmentData {
    rvs_from_value(value).unwrap_or_default()
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
    fn test_20260605_parse_group_info() {
        let v = serde_json::json!({"group_id": 123, "group_name": "test群", "member_count": 10});
        let info = rvs_parse_group_info(&v).unwrap();
        assert_eq!(info.group_name, "test群");
        snapshot("20260605_parse_group_info", &format!("{info:?}"));
    }

    #[test]
    fn test_20260605_parse_group_member_info() {
        let v = serde_json::json!({"nickname": "alice", "card": "Alice", "title": "管理员"});
        let info = rvs_parse_group_member_info(&v).unwrap();
        assert_eq!(info.nickname, "alice");
        assert_eq!(info.card, "Alice");
        assert_eq!(info.title, "管理员");
        snapshot("20260605_parse_group_member_info", &format!("{info:?}"));
    }

    #[test]
    fn test_20260605_parse_stranger_info() {
        let v = serde_json::json!({"user_id": 100, "nickname": "bob"});
        let info = rvs_parse_stranger_info(&v).unwrap();
        assert_eq!(info.nickname, "bob");
        snapshot("20260605_parse_stranger_info", &format!("{info:?}"));
    }

    #[test]
    fn test_20260605_parse_get_image() {
        let v = serde_json::json!({"file": "/tmp/img.jpg"});
        let info = rvs_parse_get_image(&v).unwrap();
        assert_eq!(info.file, "/tmp/img.jpg");
        snapshot("20260605_parse_get_image", &format!("{info:?}"));
    }

    #[test]
    fn test_20260605_parse_get_msg() {
        let v = serde_json::json!({"sender": {"user_id": 42}, "raw_message": "hello"});
        let info = rvs_parse_get_msg(&v).unwrap();
        assert_eq!(info.sender.user_id, 42);
        assert_eq!(info.raw_message, "hello");
        snapshot("20260605_parse_get_msg", &format!("{info:?}"));
    }

    #[test]
    fn test_20260605_parse_text_data() {
        let v = serde_json::json!({"text": "hello world"});
        let data = rvs_parse_text_data(&v);
        assert_eq!(data.text, "hello world");
        snapshot("20260605_parse_text_data", &format!("{data:?}"));
    }

    #[test]
    fn test_20260605_parse_text_data_empty() {
        let v = serde_json::json!({});
        let data = rvs_parse_text_data(&v);
        assert_eq!(data.text, "");
        snapshot("20260605_parse_text_data_empty", &format!("{data:?}"));
    }

    #[test]
    fn test_20260605_parse_face_data_string_id() {
        let v = serde_json::json!({"id": "178"});
        let data = rvs_parse_face_data(&v);
        assert_eq!(data.id.rvs_to_string(), "178");
        snapshot("20260605_parse_face_data_string_id", &format!("{data:?}"));
    }

    #[test]
    fn test_20260605_parse_face_data_integer_id() {
        let v = serde_json::json!({"id": 178});
        let data = rvs_parse_face_data(&v);
        assert_eq!(data.id.rvs_to_string(), "178");
        snapshot("20260605_parse_face_data_integer_id", &format!("{data:?}"));
    }

    #[test]
    fn test_20260605_parse_at_data() {
        let v = serde_json::json!({"qq": "12345"});
        let data = rvs_parse_at_data(&v);
        assert_eq!(data.qq, "12345");
        snapshot("20260605_parse_at_data", &format!("{data:?}"));
    }

    #[test]
    fn test_20260605_parse_reply_data() {
        let v = serde_json::json!({"id": "12345"});
        let data = rvs_parse_reply_data(&v);
        assert_eq!(data.id.rvs_to_i32(), 12345);
        snapshot("20260605_parse_reply_data", &format!("{data:?}"));
    }

    #[test]
    fn test_20260605_parse_reply_data_integer() {
        let v = serde_json::json!({"id": 67890});
        let data = rvs_parse_reply_data(&v);
        assert_eq!(data.id.rvs_to_i32(), 67890);
        snapshot("20260605_parse_reply_data_integer", &format!("{data:?}"));
    }

    #[test]
    fn test_20260605_parse_image_data() {
        let v = serde_json::json!({"file": "test.jpg", "url": "http://example.com/test.jpg", "summary": "表情包"});
        let data = rvs_parse_image_data(&v);
        assert_eq!(data.file, "test.jpg");
        assert_eq!(data.url, "http://example.com/test.jpg");
        assert_eq!(data.summary, "表情包");
        snapshot("20260605_parse_image_data", &format!("{data:?}"));
    }
}

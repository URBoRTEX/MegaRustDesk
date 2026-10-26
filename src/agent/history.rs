//! Portable history codec: records from different peers must never be mixed.
use serde_json::{json, Value};
use std::collections::VecDeque;
pub const MAX_BYTES: usize = 2 * 1024 * 1024;

pub struct SavedHistory {
    pub thread: Option<String>,
    pub messages: VecDeque<String>,
}

pub fn decode(peer: &str, data: &[u8]) -> Result<SavedHistory, String> {
    if data.len() > MAX_BYTES {
        return Err("История превышает допустимый размер".into());
    }
    let record: Value = serde_json::from_slice(data).map_err(|e| e.to_string())?;
    if record["peer"].as_str() != Some(peer) {
        return Err("История другого компьютера".into());
    }
    let thread = match &record["threadId"] {
        Value::Null => None,
        Value::String(s) if !s.is_empty() && s.len() < 256 => Some(s.clone()),
        _ => return Err("Некорректный диалог".into()),
    };
    let entries = record["messages"]
        .as_array()
        .ok_or("Некорректные сообщения")?;
    if entries.len() > 100 {
        return Err("Слишком много сообщений".into());
    }
    let mut messages = VecDeque::new();
    for item in entries {
        messages.push_back(
            item.as_str()
                .ok_or("Некорректный текст сообщения")?
                .to_owned(),
        );
    }
    Ok(SavedHistory { thread, messages })
}

pub fn encode(peer: &str, thread: Option<&str>, messages: &VecDeque<String>) -> Vec<u8> {
    let mut recent = messages.iter().rev().take(100).cloned().collect::<Vec<_>>();
    recent.reverse();
    loop {
        let bytes = json!({"peer":peer,"threadId":thread,"messages":recent})
            .to_string()
            .into_bytes();
        if bytes.len() <= MAX_BYTES {
            return bytes;
        }
        if recent.len() > 1 {
            recent.remove(0);
        } else {
            recent[0] = recent[0].chars().take(64 * 1024).collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_history_roundtrip() {
        let mut messages = VecDeque::new();
        messages.push_back("Проверка — агент работает".into());
        let saved = decode("peer-a", &encode("peer-a", Some("thread-test"), &messages)).unwrap();
        assert_eq!(saved.messages, messages);
        assert_eq!(saved.thread.as_deref(), Some("thread-test"));
    }
    #[test]
    fn rejects_other_peer() {
        assert!(decode("b", br#"{"peer":"a","threadId":null,"messages":[]}"#).is_err());
    }
    #[test]
    fn rejects_corrupt_json() {
        assert!(decode("a", b"incomplete{").is_err());
    }
    #[test]
    fn rejects_oversize_input() {
        assert!(decode("a", &vec![b' '; MAX_BYTES + 1]).is_err());
    }
    #[test]
    fn rejects_non_string_messages() {
        assert!(decode("a", br#"{"peer":"a","threadId":null,"messages":[true]}"#).is_err());
    }
    #[test]
    fn stopped_task_has_no_resume_id() {
        let saved = decode("a", &encode("a", None, &VecDeque::new())).unwrap();
        assert!(saved.thread.is_none());
    }
    #[test]
    fn limits_history_to_newest_hundred() {
        let messages = (0..110).map(|i| i.to_string()).collect();
        let saved = decode("a", &encode("a", None, &messages)).unwrap();
        assert_eq!(saved.messages.len(), 100);
        assert_eq!(saved.messages.front().unwrap(), "10");
        assert_eq!(saved.messages.back().unwrap(), "109");
    }
    #[test]
    fn bounded_encoding_stays_readable() {
        let messages = (0..100).map(|_| "Ж".repeat(100000)).collect();
        let encoded = encode("a", Some("thread-test"), &messages);
        assert!(encoded.len() <= MAX_BYTES);
        assert!(decode("a", &encoded).is_ok());
    }
}

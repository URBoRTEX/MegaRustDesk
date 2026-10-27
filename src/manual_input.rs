//! Explicit human paste preparation. No MCP entrypoint or background clipboard sync.
use crate::agent::gate::Gate;

pub const MAX_TEXT_BYTES: usize = 1024 * 1024;

#[cfg(feature = "agent")]
use hbb_common::message_proto::*;

#[cfg(feature = "agent")]
pub const MODIFIER_KEYS: [ControlKey; 8] = [ControlKey::Control, ControlKey::RControl,
    ControlKey::Shift, ControlKey::RShift, ControlKey::Alt, ControlKey::RAlt,
    ControlKey::Meta, ControlKey::RWin];

#[cfg(feature = "agent")]
pub fn text_messages(text: String, held: [bool; 8]) -> Vec<Message> {
    let mut messages = Vec::new();
    for key in MODIFIER_KEYS {
        let mut event=KeyEvent::new(); event.set_control_key(key);
        let mut msg=Message::new(); msg.set_key_event(event); messages.push(msg);
    }
    let mut event=KeyEvent::new(); event.set_seq(text);
    let mut msg=Message::new(); msg.set_key_event(event); messages.push(msg);
    for (index,key) in MODIFIER_KEYS.into_iter().enumerate() {
        if !held[index] { continue; }
        let mut event=KeyEvent::new(); event.set_control_key(key); event.down=true;
        // Receiver fix_modifiers uses generic groups for both left/right keys.
        // Exclude the key's own group until its other side has been restored:
        // otherwise press_modifiers temporarily presses/releases this same key.
        event.modifiers = [ControlKey::Control, ControlKey::Shift, ControlKey::Alt, ControlKey::Meta]
            .into_iter().enumerate().filter_map(|(group,key)| {
                let any_held = held[group*2] || held[group*2+1];
                let own_group = group == index/2;
                let other_restored = index%2 == 1 && held[index-1];
                if any_held && (!own_group || other_restored) { Some(key.into()) } else { None }
            }).collect();
        let mut msg=Message::new(); msg.set_key_event(event); messages.push(msg);
    }
    messages
}

#[cfg(feature = "agent")]
pub fn neutral_release(mut event: KeyEvent) -> Option<KeyEvent> {
    if !matches!(event.union, Some(key_event::Union::ControlKey(_)) | Some(key_event::Union::Chr(_)) | Some(key_event::Union::Win2winHotkey(_))) {
        return None;
    }
    event.down=false; event.press=false; event.modifiers.clear();
    Some(event)
}

pub fn prepare_text(
    gate: &Gate,
    keyboard_allowed: bool,
    read: impl FnOnce() -> Result<String, String>,
) -> Result<(u64, String), String> {
    if !gate.permits_manual(gate.epoch) || !keyboard_allowed {
        return Err("Вставка доступна только при вашем управлении подключённым ПК".into());
    }
    let text = read()?;
    if text.is_empty() {
        return Err("В буфере нет текста".into());
    }
    if text.len() > MAX_TEXT_BYTES {
        return Err("Текст в буфере больше 1 МБ".into());
    }
    Ok((gate.epoch, text))
}

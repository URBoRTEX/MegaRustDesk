// Real RustDesk message construction, synthetic input only, no OS/clipboard APIs.
#![allow(dead_code)]
#[path = "../../src/agent/gate.rs"] pub mod gate;
mod agent { pub use crate::gate; }
#[path = "../../src/manual_input.rs"] mod manual_input;
use hbb_common::message_proto::*;
fn main() {
    // Receiver fix_modifiers/press_modifiers contract, all 256 left/right combinations.
    for mask in 0..256 {
        let held=std::array::from_fn(|i| mask & (1<<i) != 0);
        let batch=manual_input::text_messages("paste".into(),held);
        let mut remote=[false;8];
        for msg in batch {
            let event=msg.key_event();
            if let Some(key_event::Union::Seq(_))=event.union {
                assert!(!remote.iter().any(|v|*v));continue;
            }
            let key=event.control_key();let index=manual_input::MODIFIER_KEYS.iter().position(|k|*k==key).unwrap();
            let mut temporary=Vec::new();
            if event.down {
                let groups=[ControlKey::Control,ControlKey::Shift,ControlKey::Alt,ControlKey::Meta];
                for i in 0..8 {
                    if i!=index && !event.modifiers.contains(&groups[i/2].into()) {remote[i]=false;}
                }
                for (g,key) in groups.into_iter().enumerate() {
                    if event.modifiers.contains(&key.into()) && !remote[g*2] && !remote[g*2+1] {
                        remote[g*2]=true;temporary.push(g*2);
                    }
                }
            }
            remote[index]=event.down;
            for i in temporary {remote[i]=false;}
        }
        assert_eq!(remote,held,"receiver modifier contract, mask={mask}");
    }
    let mut held=[false;8]; held[0]=true; held[2]=true;
    let batch=manual_input::text_messages("Текст\n行 🚀".into(),held);
    assert_eq!(batch.len(),11);
    for msg in &batch[..8] {
        let event=msg.key_event();assert!(!event.down && !event.press);
        assert!(matches!(event.union,Some(key_event::Union::ControlKey(_))));
        assert!(event.modifiers.is_empty());
    }
    assert_eq!(batch[8].key_event().seq(),"Текст\n行 🚀");
    let mut remote_modifiers=[false;8];
    for (index,msg) in batch.iter().enumerate() {
        if index==8 { assert!(!remote_modifiers.iter().any(|v|*v));continue; }
        let event=msg.key_event();
        let key=event.control_key();
        let position=manual_input::MODIFIER_KEYS.iter().position(|k|*k==key).unwrap();
        remote_modifiers[position]=event.down;
    }
    assert_eq!(remote_modifiers,held,"Ctrl/Shift must remain held for the next Map-mode key");
    assert_eq!(manual_input::text_messages("x".into(),[false;8]).len(),9);
    let mut key=KeyEvent::new();key.set_chr(0x11);key.down=true;key.press=true;key.mode=KeyboardMode::Map.into();key.modifiers.push(ControlKey::Control.into());
    let release=manual_input::neutral_release(key).unwrap();
    assert!(!release.down && !release.press && release.modifiers.is_empty());assert_eq!(release.chr(),0x11);
    assert_eq!(release.mode.enum_value().unwrap(),KeyboardMode::Map);
    let mut legacy=KeyEvent::new();legacy.set_control_key(ControlKey::RControl);legacy.down=true;
    let legacy=manual_input::neutral_release(legacy).unwrap();assert!(!legacy.down && legacy.control_key()==ControlKey::RControl);
    let mut text=KeyEvent::new();text.set_seq("must not become an unconditional release".into());
    assert!(manual_input::neutral_release(text).is_none());
    let mut translated=KeyEvent::new();translated.set_win2win_hotkey(0x0057_0000);translated.down=true;translated.mode=KeyboardMode::Translate.into();
    let translated=manual_input::neutral_release(translated).unwrap();
    assert!(!translated.down && translated.mode.enum_value().unwrap()==KeyboardMode::Translate);
    println!("manual protocol: Unicode + held modifiers + neutral raw key release: PASS");
}

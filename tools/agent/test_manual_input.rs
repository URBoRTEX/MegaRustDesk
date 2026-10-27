// Standalone, no OS clipboard or desktop input. Compiles with rustc --test.
#[path = "../../src/agent/gate.rs"] pub mod gate;
mod agent { pub use crate::gate; }
#[path = "../../src/manual_input.rs"] mod manual_input;

#[cfg(test)] mod tests {
    use super::{agent::gate::*, manual_input::*};
    use std::{cell::Cell, time::Instant};
    fn manual() -> Gate {
        let mut gate = Gate::default();
        gate.frame(Geometry { display:0, width:800, height:600, x:0, y:0 }, Instant::now());
        gate
    }
    #[test] fn starts_with_human_control() { assert_eq!(Gate::default().mode, Mode::Manual); }
    #[test] fn reads_unicode_once_only_for_manual_connected_keyboard() {
        let gate=manual(); let reads=Cell::new(0);
        let (epoch,text)=prepare_text(&gate,true,|| { reads.set(reads.get()+1); Ok("Текст\n第二行 🚀".into()) }).unwrap();
        assert_eq!(text,"Текст\n第二行 🚀"); assert_eq!(reads.get(),1); assert!(gate.permits_manual(epoch));
    }
    #[test] fn denied_modes_never_read_clipboard() {
        for mode in [Mode::Ready,Mode::Running,Mode::Paused,Mode::Stopped] {
            let mut gate=manual(); gate.mode=mode;
            assert!(prepare_text(&gate,true,||panic!("clipboard read in agent mode")).is_err());
        }
        assert!(prepare_text(&Gate::default(),true,||panic!("disconnected read")).is_err());
        assert!(prepare_text(&manual(),false,||panic!("read without keyboard permission")).is_err());
    }
    #[test] fn queued_paste_is_revoked_by_handoff_pause_disconnect_and_reconnect() {
        let mut gate=manual(); let epoch=gate.epoch;
        gate.start().unwrap(); assert!(!gate.permits_manual(epoch));
        gate.manual(); assert!(!gate.permits_manual(epoch));
        let epoch=gate.epoch; gate.pause(); assert!(!gate.permits_manual(epoch));
        gate.manual(); let epoch=gate.epoch; gate.disconnect(); assert!(!gate.permits_manual(epoch));
        gate.frame(Geometry { display:0,width:800,height:600,x:0,y:0 },Instant::now());
        assert!(!gate.permits_manual(epoch));
    }
    #[test] fn empty_oversized_and_read_error_do_not_queue_text() {
        let gate=manual();
        assert!(prepare_text(&gate,true,||Ok(String::new())).is_err());
        assert!(prepare_text(&gate,true,||Ok("x".repeat(MAX_TEXT_BYTES+1))).is_err());
        assert!(prepare_text(&gate,true,||Err("clipboard busy".into())).is_err());
        assert!(prepare_text(&gate,true,||Ok(" \t\n".into())).is_ok());
    }
}

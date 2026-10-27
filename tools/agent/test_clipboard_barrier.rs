#[path = "../../src/manual_clipboard.rs"] mod manual_clipboard;
use manual_clipboard::{PasteBarrier, Ack};
use std::time::{Duration, Instant};

#[test]
fn paste_waits_for_its_own_apply_ack_and_keeps_key_order() {
    let now = Instant::now();
    let mut barrier = PasteBarrier::new(4, 15, vec!["V down"], now);
    barrier.announce(12);
    barrier.push("V up").unwrap();
    barrier.push("Control up").unwrap();
    assert!(matches!(barrier.ack(11, true), Ack::Ignored));
    match barrier.ack(12, true) {
        Ack::Ready(events) => assert_eq!(events, vec!["V down", "V up", "Control up"]),
        _ => panic!("matching application ACK did not release ordered keys"),
    }
    assert!(matches!(barrier.ack(12, true), Ack::Ignored));
}

#[test]
fn revoke_reclaim_timeout_and_failed_apply_never_flush_input() {
    let now = Instant::now();
    let mut barrier = PasteBarrier::new(4, 15, vec!["V down"], now);
    barrier.announce(12);
    assert!(barrier.invalid(5, 23, now));
    assert!(barrier.invalid(4, 23, now));
    assert!(barrier.invalid(4, 15, now + Duration::from_secs(6)));
    assert!(!barrier.invalid(4, 15, now));
    assert!(matches!(barrier.ack(12, false), Ack::Failed));
    assert!(matches!(barrier.ack(12, true), Ack::Ignored));
}

#[test]
fn queue_is_bounded_and_legacy_ack_cannot_release_paste() {
    let mut barrier = PasteBarrier::new(1, 7, Vec::new(), Instant::now());
    barrier.announce(12);
    for i in 0..256 { barrier.push(i).unwrap(); }
    assert!(barrier.push(257).is_err());
    assert!(matches!(barrier.ack(0, true), Ack::Ignored));
}

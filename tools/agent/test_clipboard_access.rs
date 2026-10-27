#[path = "../../libs/clipboard/src/access.rs"]
mod access;

#[test]
fn revoke_fences_old_callbacks_even_after_regrant() {
    let authority = access::Authority::default();
    let old = authority.grant(true, true);
    assert!(authority.accepts(old));
    authority.revoke();
    assert!(!authority.accepts(old));
    let new = authority.grant(true, true);
    assert_ne!(old, new);
    assert!(!authority.accepts(old));
    assert!(authority.accepts(new));
}

#[test]
fn text_and_file_permissions_are_independent() {
    let authority = access::Authority::default();
    authority.grant(true, false);
    assert!(authority.text_token().is_some());
    assert!(authority.file_token().is_none());
    authority.grant(false, true);
    assert!(authority.text_token().is_none());
    assert!(authority.file_token().is_some());
    authority.revoke();
    assert!(authority.text_token().is_none());
    assert!(authority.file_token().is_none());
}

#[test]
fn unchanged_permissions_preserve_an_in_progress_file_operation() {
    let authority = access::Authority::default();
    let token = authority.grant(true, true);
    assert_eq!(token, authority.grant(true, true));
    assert!(authority.accepts(token));
}

#[test]
fn application_ack_roundtrips_full_sequence_and_standard_flag_bits() {
    for sequence in [0, 1, 91, 16384, 0x3fffffff] {
        for flags in [1, 2] {
            assert_eq!(access::decode_apply_ack(access::encode_apply_ack(sequence, flags)), (sequence, flags));
        }
    }
}

#[test]
fn human_viewer_starts_suspended_and_keeps_lifecycle_ownership() {
    assert!(!access::human_managed());
    assert!(access::AUTHORITY.file_token().is_some());
    access::manage_human_viewer();
    assert!(access::human_managed());
    assert!(access::AUTHORITY.text_token().is_none());
    assert!(access::AUTHORITY.file_token().is_none());
    let suspended = access::AUTHORITY.current();
    access::AUTHORITY.grant(true, true);
    assert!(!access::AUTHORITY.accepts(suspended));
    assert!(access::human_managed());
    access::AUTHORITY.revoke();
}

//! Process-local clipboard authority. No clipboard access or transport lives here.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub struct Authority(AtomicU64);

impl Default for Authority {
    fn default() -> Self { Self::new() }
}

impl Authority {
    // Upstream/connection-manager behavior stays enabled. The agent viewer revokes
    // this before creating a context and grants only from its human input path.
    pub const fn new() -> Self { Self(AtomicU64::new(3)) }

    pub fn grant(&self, text: bool, files: bool) -> u64 {
        let flags = u64::from(text) | (u64::from(files) << 1);
        let mut old = self.0.load(Ordering::Acquire);
        loop {
            if old & 3 == flags { return old; }
            // Exhaustion fails closed; a generation must never wrap and revive.
            let next = (old & !3).checked_add(4).map_or(old & !3, |n| n | flags);
            match self.0.compare_exchange_weak(old, next, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => return next,
                Err(actual) => old = actual,
            }
        }
    }

    pub fn current(&self) -> u64 { self.0.load(Ordering::Acquire) }
    pub fn revoke(&self) { self.grant(false, false); }
    pub fn accepts(&self, token: u64) -> bool {
        token & 3 != 0 && self.0.load(Ordering::Acquire) == token
    }
    pub fn text_token(&self) -> Option<u64> { self.token(1) }
    pub fn file_token(&self) -> Option<u64> { self.token(2) }
    fn token(&self, flag: u64) -> Option<u64> {
        let value = self.0.load(Ordering::Acquire);
        (value & flag != 0).then_some(value)
    }
}

pub static AUTHORITY: Authority = Authority::new();

pub const SEQUENCE_FORMAT: &str = "MegaRustDesk-Clipboard-Sequence";
pub fn encode_apply_ack(sequence: u32, flags: i32) -> i32 {
    (((sequence & 0x3fffffff) << 2) | (flags as u32 & 3)) as i32
}
pub fn decode_apply_ack(flags: i32) -> (u32, i32) {
    ((flags as u32) >> 2, flags & 3)
}

// A Sciter viewer has explicit human grants. The connection manager keeps its
// upstream lifecycle, but revokes native access before destroying its context.
static HUMAN_MANAGED: AtomicBool = AtomicBool::new(false);
pub fn manage_human_viewer() { HUMAN_MANAGED.store(true, Ordering::Release); AUTHORITY.revoke(); }
pub fn human_managed() -> bool { HUMAN_MANAGED.load(Ordering::Acquire) }

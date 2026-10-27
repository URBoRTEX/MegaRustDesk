//! Ordered physical paste waiting for a particular native clipboard application.
use std::time::{Duration, Instant};

pub enum Ack<T> { Ignored, Ready(Vec<T>), Failed }

pub struct PasteBarrier<T> {
    pub epoch: u64,
    pub token: u64,
    sequence: u32,
    deadline: Instant,
    events: Vec<T>,
    finished: bool,
}

impl<T> PasteBarrier<T> {
    pub fn new(epoch: u64, token: u64, events: Vec<T>, now: Instant) -> Self {
        Self { epoch, token, sequence: 0, deadline: now + Duration::from_secs(5), events, finished: false }
    }
    pub fn announce(&mut self, sequence: u32) { self.sequence = sequence; }
    pub fn push(&mut self, event: T) -> Result<(), ()> {
        if self.finished || self.events.len() >= 256 { return Err(()); }
        self.events.push(event);
        Ok(())
    }
    pub fn invalid(&self, epoch: u64, token: u64, now: Instant) -> bool {
        self.finished || self.epoch != epoch || self.token != token || now >= self.deadline
    }
    pub fn ack(&mut self, sequence: u32, accepted: bool) -> Ack<T> {
        if self.finished || sequence == 0 || self.sequence != sequence { return Ack::Ignored; }
        self.finished = true;
        if accepted { Ack::Ready(std::mem::take(&mut self.events)) } else { Ack::Failed }
    }
}

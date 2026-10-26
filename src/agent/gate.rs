//! Session ownership and observation leases. No desktop/OS APIs are available here.
use std::time::{Duration, Instant};

pub const MAX_FRAME_AGE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Manual,
    Ready,
    Running,
    Paused,
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub display: usize,
    pub width: usize,
    pub height: usize,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug)]
pub struct Gate {
    pub mode: Mode,
    pub epoch: u64,
    pub connected: bool,
    pub geometry: Option<Geometry>,
    pub sequence: u64,
    pub frame_at: Option<Instant>,
    observed: Option<(String, Instant, u64, Geometry)>,
}

impl Default for Gate {
    fn default() -> Self {
        Self {
            mode: Mode::Ready,
            epoch: 1,
            connected: false,
            geometry: None,
            sequence: 0,
            frame_at: None,
            observed: None,
        }
    }
}

impl Gate {
    fn invalidate(&mut self) {
        self.epoch += 1;
        self.observed = None;
    }
    pub fn frame(&mut self, geometry: Geometry, now: Instant) {
        if self.geometry != Some(geometry) {
            self.invalidate();
        }
        self.geometry = Some(geometry);
        self.connected = true;
        self.sequence += 1;
        self.frame_at = Some(now);
    }
    pub fn start(&mut self) -> Result<(), &'static str> {
        if !self.connected || self.frame_at.is_none() {
            return Err("Нет актуального изображения удалённого компьютера");
        }
        self.invalidate();
        self.mode = Mode::Running;
        Ok(())
    }
    pub fn pause(&mut self) {
        self.invalidate();
        self.mode = Mode::Paused;
    }
    pub fn stop(&mut self) {
        self.invalidate();
        self.mode = Mode::Stopped;
    }
    pub fn manual(&mut self) {
        self.invalidate();
        self.mode = Mode::Manual;
    }
    pub fn disconnect(&mut self) {
        self.pause();
        self.connected = false;
        self.frame_at = None;
        self.geometry = None;
    }
    pub fn observe(&mut self, session: &str, now: Instant) -> Result<String, &'static str> {
        if !self.connected
            || self
                .frame_at
                .map_or(true, |t| now.saturating_duration_since(t) > MAX_FRAME_AGE)
        {
            return Err("Изображение устарело. Запросите новый кадр");
        }
        let geometry = self.geometry.ok_or("Нет дисплея")?;
        let token = format!(
            "{}:{}:{}:{}",
            session, self.epoch, geometry.display, self.sequence
        );
        self.observed = Some((token.clone(), now, self.epoch, geometry));
        Ok(token)
    }
    pub fn authorize(&mut self, token: &str, now: Instant) -> Result<u64, &'static str> {
        if self.mode != Mode::Running || !self.connected {
            return Err("Ввод агента остановлен");
        }
        let (expected, at, epoch, geometry) = self
            .observed
            .as_ref()
            .ok_or("Сначала получите снимок экрана")?;
        if token != expected
            || *epoch != self.epoch
            || self.geometry != Some(*geometry)
            || now.saturating_duration_since(*at) > MAX_FRAME_AGE
            || !self.permits(*epoch, now)
        {
            return Err("Кадр или сессия изменились. Получите новый снимок");
        }
        let epoch = *epoch;
        self.observed = None;
        Ok(epoch)
    }
    pub fn permits(&self, epoch: u64, now: Instant) -> bool {
        self.mode == Mode::Running
            && self.connected
            && self.epoch == epoch
            && self
                .frame_at
                .map_or(false, |t| now.saturating_duration_since(t) <= MAX_FRAME_AGE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn geometry() -> Geometry {
        Geometry {
            display: 0,
            width: 1920,
            height: 1080,
            x: -1920,
            y: 0,
        }
    }
    fn running() -> (Gate, Instant) {
        let t = Instant::now();
        let mut g = Gate::default();
        g.frame(geometry(), t);
        g.start().unwrap();
        (g, t)
    }
    #[test]
    fn requires_connection() {
        assert!(Gate::default().start().is_err());
    }
    #[test]
    fn observation_is_single_use() {
        let (mut g, t) = running();
        let token = g.observe("peer-session", t).unwrap();
        assert!(g.authorize(&token, t).is_ok());
        assert!(g.authorize(&token, t).is_err());
    }
    #[test]
    fn rejects_stale_observation() {
        let (mut g, t) = running();
        let token = g.observe("s", t).unwrap();
        assert!(g.authorize(&token, t + Duration::from_secs(6)).is_err());
    }
    #[test]
    fn pause_invalidates_queued_input() {
        let (mut g, t) = running();
        let token = g.observe("s", t).unwrap();
        let epoch = g.authorize(&token, t).unwrap();
        g.pause();
        assert!(!g.permits(epoch, t));
        g.start().unwrap();
        assert!(!g.permits(epoch, t));
    }
    #[test]
    fn stop_invalidates_queued_input() {
        let (mut g, t) = running();
        let token = g.observe("s", t).unwrap();
        let epoch = g.authorize(&token, t).unwrap();
        g.stop();
        assert!(!g.permits(epoch, t));
    }
    #[test]
    fn geometry_change_invalidates_observation() {
        let (mut g, t) = running();
        let token = g.observe("s", t).unwrap();
        let mut geo = geometry();
        geo.width = 1280;
        g.frame(geo, t);
        assert!(g.authorize(&token, t).is_err());
    }
    #[test]
    fn reconnect_requires_explicit_resume_and_new_frame() {
        let (mut g, t) = running();
        let token = g.observe("s", t).unwrap();
        g.disconnect();
        assert!(g.start().is_err());
        g.frame(geometry(), t);
        assert_eq!(g.mode, Mode::Paused);
        assert!(g.authorize(&token, t).is_err());
        g.start().unwrap();
        assert!(g.observe("s", t).is_ok());
    }
    #[test]
    fn manual_excludes_agent() {
        let (mut g, t) = running();
        let token = g.observe("s", t).unwrap();
        g.manual();
        assert!(g.authorize(&token, t).is_err());
        assert_eq!(g.mode, Mode::Manual);
    }
    #[test]
    fn frames_do_not_resume_stopped_task() {
        let (mut g, t) = running();
        g.stop();
        g.frame(geometry(), t);
        assert_eq!(g.mode, Mode::Stopped);
    }
    #[test]
    fn wrong_session_token_rejected() {
        let (mut g, t) = running();
        let token = g.observe("s1", t).unwrap();
        assert!(g.authorize(&token.replace("s1", "s2"), t).is_err());
    }
    #[test]
    fn video_frames_dont_invalidate_same_geometry_lease() {
        let (mut g, t) = running();
        let token = g.observe("s", t).unwrap();
        g.frame(geometry(), t + Duration::from_millis(20));
        assert!(g.authorize(&token, t + Duration::from_millis(21)).is_ok());
    }
}

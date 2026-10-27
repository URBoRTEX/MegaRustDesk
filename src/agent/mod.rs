//! Private per-session remote tools. This module never captures or injects into the local desktop.
pub mod gate;
mod mcp;
mod portable;
mod transport;
pub mod updater;
mod update_manifest;
mod update_install;
pub mod update_process;
#[cfg(windows)]
pub mod single_instance;

use crate::{
    client::{Data, Interface},
    ui::remote::SciterHandler,
    ui_session_interface::Session,
};
use gate::{Gate, Geometry, Mode};
use hbb_common::message_proto::*;
use serde_json::{json, Value};
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

pub use mcp::run_stdio;
pub use portable::initialize;

pub fn network_policy() -> Value {
    use hbb_common::config::{self, Config};
    let mut login = crate::client::LoginConfigHandler::default();
    login.enforce_agent_isolation();
    let relay_only = crate::is_udp_disabled()
        && !crate::get_udp_punch_enabled()
        && !crate::get_ipv6_punch_enabled()
        && config::is_disable_tcp_listen()
        && !config::option2bool("direct-server", &Config::get_option("direct-server"))
        && !config::option2bool("enable-lan-discovery", &Config::get_option("enable-lan-discovery"))
        && login.force_relay;
    json!({"outboundRelayOnly":relay_only,"firewallPolicyChanged":false})
}

/// Runs the real receiver startup for external socket observation, without a GUI,
/// authentication, model turn, input, or changing system security settings.
pub fn diagnose_network() -> Value {
    hbb_common::init_log(false, "agent-network-test");
    std::thread::spawn(|| crate::start_server(true, false));
    std::thread::sleep(Duration::from_secs(25));
    network_policy()
}

/// Developer diagnostic: actual packaged runtime and MCP, with no model turn or desktop input.
pub fn diagnose() -> Result<Value, String> {
    let mut session: Session<SciterHandler> = Session::default();
    session.ui_handler.agent = session.agent.clone();
    attach(session.clone());
    let result = (|| {
        let disconnected = call_tool(
            &session,
            "click",
            json!({"frame_id":"synthetic-invalid-frame","x":0,"y":0}),
        )
        .is_err();
        if !disconnected {
            return Err("Отключённая сессия приняла ввод".into());
        }
        let mut result = mcp::diagnose(&session.agent)?;
        result["disconnectedInputBlocked"] = json!(true);
        result["outboundRelayOnly"] = network_policy()["outboundRelayOnly"].clone();
        Ok(result)
    })();
    session.agent.shutdown.store(true, Ordering::SeqCst);
    mcp::unpublish(&session.agent);
    result
}

pub struct Agent {
    pub gate: Mutex<Gate>,
    pub session_id: String,
    pub peer: Mutex<String>,
    frame: Mutex<Vec<u8>>,
    pub displays: Mutex<Vec<DisplayInfo>>,
    pub log: Mutex<VecDeque<String>>,
    pub shutdown: AtomicBool,
    pub receive_frames: AtomicBool,
    endpoint_started: AtomicBool,
    pub expected_display: AtomicUsize,
    pub endpoint: Mutex<Option<(u16, String)>>,
    pub controller: Mutex<Option<String>>,
    pub controller_pid: AtomicUsize,
}

impl Default for Agent {
    fn default() -> Self {
        Self {
            gate: Mutex::new(Gate::default()),
            session_id: uuid::Uuid::new_v4().to_string(),
            peer: Mutex::new(String::new()),
            frame: Mutex::new(Vec::new()),
            displays: Mutex::new(Vec::new()),
            log: Mutex::new(VecDeque::new()),
            shutdown: AtomicBool::new(false),
            receive_frames: AtomicBool::new(false),
            endpoint_started: AtomicBool::new(false),
            expected_display: AtomicUsize::new(usize::MAX),
            endpoint: Mutex::new(None),
            controller: Mutex::new(None),
            controller_pid: AtomicUsize::new(0),
        }
    }
}

impl Agent {
    pub fn message(&self, text: impl Into<String>) {
        let mut log = self.log.lock().unwrap();
        log.push_back(text.into());
        while log.len() > 100 {
            log.pop_front();
        }
        drop(log);
    }
    pub fn on_frame(&self, display: usize, rgba: &scrap::ImageRgb) {
        if rgba.w == 0 || rgba.h == 0 || rgba.w > 16384 || rgba.h > 16384 {
            return;
        }
        let mut g = self.gate.lock().unwrap();
        if !self.receive_frames.load(Ordering::SeqCst) {
            return;
        }
        let expected = self.expected_display.load(Ordering::SeqCst);
        if expected != usize::MAX && expected != display {
            return;
        }
        if rgba.w.saturating_mul(rgba.h) > 33_554_432 {
            g.pause();
            return;
        }
        if g.frame_at
            .map_or(false, |t| t.elapsed() < Duration::from_millis(100))
            && g.geometry.map_or(false, |v| {
                v.display == display && v.width == rgba.w && v.height == rgba.h
            })
        {
            return;
        }
        let displays = self.displays.lock().unwrap();
        let (x, y) = displays.get(display).map(|d| (d.x, d.y)).unwrap_or((0, 0));
        let stride = if rgba.align == 0 {
            rgba.w * 4
        } else {
            (rgba.w * 4 + rgba.align - 1) & !(rgba.align - 1)
        };
        if rgba.raw.len() < stride * rgba.h {
            return;
        }
        let mut frame = self.frame.lock().unwrap();
        frame.clear();
        frame.reserve(rgba.w * rgba.h * 4);
        for row in 0..rgba.h {
            for px in rgba.raw[row * stride..row * stride + rgba.w * 4].chunks_exact(4) {
                frame.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
        }
        g.frame(
            Geometry {
                display,
                width: rgba.w,
                height: rgba.h,
                x,
                y,
            },
            Instant::now(),
        );
    }
    pub fn disconnect(&self) {
        self.receive_frames.store(false, Ordering::SeqCst);
        self.gate.lock().unwrap().disconnect();
        self.message(
            "Связь потеряна. Ввод остановлен; последнее действие автоматически не повторяется.",
        );
    }
    pub fn status(&self) -> Value {
        let (mode, connected, display) = {
            let g = self.gate.lock().unwrap();
            (g.mode, g.connected, g.geometry.map(|x| x.display))
        };
        let status = match mode {
            Mode::Manual => "Вы управляете",
            Mode::Ready => "Управление не передано",
            Mode::Running => "Управляет Codex",
            Mode::Paused => "Управление приостановлено",
            Mode::Stopped => "Управление остановлено",
        };
        let peer = self.peer.lock().unwrap().clone();
        let messages = self.log.lock().unwrap().iter().cloned().collect::<Vec<_>>();
        let controller_connected = self.controller.lock().unwrap().is_some();
        let mcp_ready = self.endpoint.lock().unwrap().is_some();
        json!({"status":status,"mode":format!("{:?}",mode),"connected":connected,"peer":peer,"display":display,"session_id":self.session_id,"controllerConnected":controller_connected,"mcpReady":mcp_ready,"messages":messages})
    }
}

pub fn attach(session: Session<SciterHandler>) {
    if !session.agent.endpoint_started.swap(true, Ordering::SeqCst) {
        mcp::serve(session);
    }
}

fn release(session: &Session<SciterHandler>) {
    let mut messages = Vec::new();
    for button in [1, 2, 4] {
        messages.push(mouse((button << 3) | 2, 0, 0));
    }
    for key in [
        ControlKey::Alt,
        ControlKey::Control,
        ControlKey::Shift,
        ControlKey::Meta,
    ] {
        let mut event = KeyEvent::new();
        event.set_control_key(key);
        event.down = false;
        let mut msg = Message::new();
        msg.set_key_event(event);
        messages.push(msg);
    }
    session.send(Data::AgentRelease(messages));
}

pub fn control(session: &Session<SciterHandler>, command: &str, _task: &str) -> Result<(), String> {
    attach(session.clone());
    match command {
        "start" | "resume" => {
            {
                let mut gate = session.agent.gate.lock().unwrap();
                gate.start().map_err(str::to_owned)?;
                *session.agent.controller.lock().unwrap() = None;
                session.agent.controller_pid.store(0,Ordering::SeqCst);
                // Queue releases before MCP can acquire the newly running gate.
                session.leave(session.get_keyboard_mode());
                release(session);
            }
        }
        "pause" | "stop" | "manual" => {
            {
                let mut g = session.agent.gate.lock().unwrap();
                match command {
                    "pause" => g.pause(),
                    "manual" => g.manual(),
                    _ => g.stop(),
                }
            }
            release(session);
            session.leave(session.get_keyboard_mode());
        }
        "exit" => {
            session.agent.gate.lock().unwrap().stop();
            release(session);
            session.agent.shutdown.store(true, Ordering::SeqCst);
            mcp::unpublish(&session.agent);
            session.close();
        }
        _ => return Err("Неизвестное действие".into()),
    }
    Ok(())
}

fn mouse(mask: i32, x: i32, y: i32) -> Message {
    let mut m = Message::new();
    m.set_mouse_event(MouseEvent {
        mask,
        x,
        y,
        ..Default::default()
    });
    m
}

fn integer(args: &Value, key: &str, min: i64, max: i64) -> Result<i32, String> {
    let n = args[key]
        .as_i64()
        .ok_or_else(|| format!("Не задано целое {key}"))?;
    if n < min || n > max {
        return Err(format!("Координата/значение {key} вне границ"));
    }
    Ok(n as i32)
}

pub fn call_tool(
    session: &Session<SciterHandler>,
    name: &str,
    args: Value,
) -> Result<Value, String> {
    let agent = &session.agent;
    if agent.shutdown.load(Ordering::SeqCst) {
        return Err("Сессия завершена".into());
    }
    match name {
        "session_status" => {
            return Ok(json!({"content":[{"type":"text","text":agent.status().to_string()}]}))
        }
        "pause" | "stop" => {
            control(session, name, "")?;
            return Ok(json!({"content":[{"type":"text","text":"Приостановлено"}]}));
        }
        "screenshot" => {
            // Ask the existing session for a new encoded frame, including when its window is hidden.
            let old = agent.gate.lock().unwrap().sequence;
            session.refresh_video(0);
            let deadline = Instant::now() + Duration::from_secs(4);
            while Instant::now() < deadline {
                if agent.gate.lock().unwrap().sequence > old {
                    break;
                }
                std::thread::sleep(Duration::from_millis(30));
            }
            let mut g = agent.gate.lock().unwrap();
            let token = g
                .observe(&agent.session_id, Instant::now())
                .map_err(str::to_owned)?;
            let geo = g.geometry.ok_or("Нет изображения")?;
            let frame = agent.frame.lock().unwrap().clone();
            drop(g);
            let mut png = Vec::new();
            image::codecs::png::PngEncoder::new(&mut png)
                .encode(
                    &frame,
                    geo.width as u32,
                    geo.height as u32,
                    image::ColorType::Rgba8,
                )
                .map_err(|e| e.to_string())?;
            let metadata = json!({"frame_id":token,"session":agent.session_id,"peer":*agent.peer.lock().unwrap(),"display":geo.display,"width":geo.width,"height":geo.height,"coordinates":"physical pixels relative to selected display","max_age_ms":5000});
            let mut mime = "image/png";
            if png.len() > 12 * 1024 * 1024 {
                let rgb = frame
                    .chunks_exact(4)
                    .flat_map(|p| p[..3].iter().copied())
                    .collect::<Vec<_>>();
                png.clear();
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut png, 80)
                    .encode(
                        &rgb,
                        geo.width as u32,
                        geo.height as u32,
                        image::ColorType::Rgb8,
                    )
                    .map_err(|e| e.to_string())?;
                mime = "image/jpeg";
            }
            if png.len() > 16 * 1024 * 1024 {
                return Err(
                    "Изображение слишком велико; уменьшите разрешение удалённого дисплея".into(),
                );
            }
            return Ok(
                json!({"content":[{"type":"text","text":metadata.to_string()},{"type":"image","mimeType":mime,"data":base64(&png)}]}),
            );
        }
        "select_display" => {
            let display = integer(&args, "display", 0, 63)?;
            if agent
                .displays
                .lock()
                .unwrap()
                .get(display as usize)
                .is_none()
            {
                return Err("Дисплей отсутствует".into());
            }
            let mut g = agent.gate.lock().unwrap();
            if g.mode != Mode::Running {
                return Err("Ввод агента остановлен".into());
            }
            g.frame_at = None;
            g.geometry = None;
            g.epoch += 1;
            agent
                .expected_display
                .store(display as usize, Ordering::SeqCst);
            drop(g);
            session.switch_display(display);
            return Ok(
                json!({"content":[{"type":"text","text":"Дисплей выбран; запросите новый screenshot"}]}),
            );
        }
        _ => {}
    }
    let mut g = agent.gate.lock().unwrap();
    let geo = g.geometry.ok_or("Нет дисплея")?;
    let token = args["frame_id"]
        .as_str()
        .ok_or("Нужен frame_id последнего screenshot")?;
    let mut messages = Vec::new();
    let point = |key_x: &str, key_y: &str| -> Result<(i32, i32), String> {
        Ok((
            integer(&args, key_x, 0, geo.width as i64 - 1)?
                .checked_add(geo.x)
                .ok_or("Некорректная геометрия дисплея")?,
            integer(&args, key_y, 0, geo.height as i64 - 1)?
                .checked_add(geo.y)
                .ok_or("Некорректная геометрия дисплея")?,
        ))
    };
    match name {
        "click" | "double_click" => {
            let (x, y) = point("x", "y")?;
            messages.push(mouse(0, x, y));
            let button = match args["button"].as_str().unwrap_or("left") {
                "left" => 1,
                "right" => 2,
                "middle" => 4,
                _ => return Err("Неизвестная кнопка мыши".into()),
            };
            for _ in 0..if name == "double_click" { 2 } else { 1 } {
                messages.push(mouse((button << 3) | 1, x, y));
                messages.push(mouse((button << 3) | 2, x, y));
            }
        }
        "scroll" => {
            let (x, y) = point("x", "y")?;
            messages.push(mouse(0, x, y));
            messages.push(mouse(
                3,
                integer(&args, "dx", -20, 20)?,
                integer(&args, "dy", -20, 20)?,
            ));
        }
        "drag" => {
            let (x, y) = point("x", "y")?;
            let (tx, ty) = point("to_x", "to_y")?;
            messages.push(mouse(0, x, y));
            messages.push(mouse(9, x, y));
            for i in 1..=12 {
                messages.push(mouse(0, x + (tx - x) * i / 12, y + (ty - y) * i / 12));
            }
            messages.push(mouse(10, tx, ty));
        }
        "type_text" => {
            let text = args["text"].as_str().ok_or("Нужен текст")?;
            if text.len() > 32768 {
                return Err("Текст превышает 32 КБ".into());
            }
            let mut event = KeyEvent::new();
            event.set_seq(text.into());
            let mut msg = Message::new();
            msg.set_key_event(event);
            messages.push(msg);
        }
        "hotkey" => {
            let keys = args["keys"].as_array().ok_or("Нужен список keys")?;
            if keys.is_empty() || keys.len() > 5 {
                return Err("От 1 до 5 клавиш".into());
            }
            let mut event = KeyEvent::new();
            event.press = true;
            for (idx, key) in keys.iter().enumerate() {
                let key = key.as_str().ok_or("Клавиши должны быть строками")?;
                let ctrl = match key.to_lowercase().as_str() {
                    "ctrl" | "control" => Some(ControlKey::Control),
                    "alt" => Some(ControlKey::Alt),
                    "shift" => Some(ControlKey::Shift),
                    "win" | "meta" => Some(ControlKey::Meta),
                    "enter" | "return" => Some(ControlKey::Return),
                    "tab" => Some(ControlKey::Tab),
                    "esc" | "escape" => Some(ControlKey::Escape),
                    "backspace" => Some(ControlKey::Backspace),
                    "delete" => Some(ControlKey::Delete),
                    "up" => Some(ControlKey::UpArrow),
                    "down" => Some(ControlKey::DownArrow),
                    "left" => Some(ControlKey::LeftArrow),
                    "right" => Some(ControlKey::RightArrow),
                    "home" => Some(ControlKey::Home),
                    "end" => Some(ControlKey::End),
                    "space" => Some(ControlKey::Space),
                    "f4" => Some(ControlKey::F4),
                    "f1" => Some(ControlKey::F1),
                    "f2" => Some(ControlKey::F2),
                    "f3" => Some(ControlKey::F3),
                    "f5" => Some(ControlKey::F5),
                    "f6" => Some(ControlKey::F6),
                    "f7" => Some(ControlKey::F7),
                    "f8" => Some(ControlKey::F8),
                    "f9" => Some(ControlKey::F9),
                    "f10" => Some(ControlKey::F10),
                    "f11" => Some(ControlKey::F11),
                    "f12" => Some(ControlKey::F12),
                    "pageup" => Some(ControlKey::PageUp),
                    "pagedown" => Some(ControlKey::PageDown),
                    "insert" => Some(ControlKey::Insert),
                    _ => None,
                };
                if idx + 1 < keys.len() {
                    let c = ctrl.ok_or("Модификатор неизвестен")?;
                    if ![
                        ControlKey::Control,
                        ControlKey::Alt,
                        ControlKey::Shift,
                        ControlKey::Meta,
                    ]
                    .contains(&c)
                    {
                        return Err("Ожидается модификатор".into());
                    }
                    event.modifiers.push(c.into());
                } else if let Some(c) = ctrl {
                    event.set_control_key(c);
                } else if key.chars().count() == 1 {
                    event.set_chr(key.chars().next().unwrap().to_ascii_lowercase() as u32);
                } else {
                    return Err("Неизвестная клавиша".into());
                }
            }
            let mut msg = Message::new();
            msg.set_key_event(event);
            messages.push(msg);
        }
        _ => return Err("Инструмент не разрешён".into()),
    }
    if !*session.server_keyboard_enabled.read().unwrap() {
        return Err("Удалённый компьютер запретил ввод".into());
    }
    let epoch = g.authorize(token, Instant::now()).map_err(str::to_owned)?;
    drop(g);
    session.send(Data::AgentBatch(epoch, messages));
    Ok(
        json!({"content":[{"type":"text","text":"Действие поставлено в очередь сессии. Получите новый screenshot и проверьте результат; при потере связи не повторяйте действие автоматически."}]}),
    )
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let v = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        s.push(TABLE[(v >> 18) as usize] as char);
        s.push(TABLE[((v >> 12) & 63) as usize] as char);
        s.push(if chunk.len() > 1 {
            TABLE[((v >> 6) & 63) as usize] as char
        } else {
            '='
        });
        s.push(if chunk.len() > 2 {
            TABLE[(v & 63) as usize] as char
        } else {
            '='
        });
    }
    s
}

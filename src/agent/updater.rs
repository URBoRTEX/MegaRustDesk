//! Manual update coordinator. Downloads never replace the running application.
use super::{
    portable,
    update_manifest::{self, Manifest, MAX_ARCHIVE, MAX_EXPANDED},
    update_process,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};

lazy_static::lazy_static! {
    static ref CONFIG: Value = serde_json::from_str(include_str!("../../tools/agent/release_config.json")).expect("embedded release config");
    static ref STATE: Mutex<State> = Mutex::new(State::default());
    static ref ADMISSION: Mutex<(bool,usize)> = Mutex::new((false,0));
}
static BUSY: AtomicBool = AtomicBool::new(false);
#[derive(Default)]
struct State {
    phase: String,
    message: String,
    latest: String,
    progress: u64,
    manifest: Option<Manifest>,
    bytes: Vec<u8>,
    signature: String,
    work: Option<PathBuf>,
}
pub fn config() -> &'static Value {
    &CONFIG
}
pub fn product_version() -> &'static str {
    CONFIG["version"].as_str().unwrap()
}
pub struct Incoming;
impl Drop for Incoming {
    fn drop(&mut self) {
        let mut state = ADMISSION.lock().unwrap();
        state.1 = state.1.saturating_sub(1);
    }
}
pub fn incoming() -> Option<Incoming> {
    let mut state = ADMISSION.lock().unwrap();
    if state.0 {
        None
    } else {
        state.1 += 1;
        Some(Incoming)
    }
}
pub fn freeze_incoming() -> Result<(), String> {
    let mut state = ADMISSION.lock().unwrap();
    if state.1 > 0 {
        return Err("Завершите входящие удалённые подключения перед обновлением".into());
    }
    state.0 = true;
    Ok(())
}
pub fn unfreeze_incoming() {
    ADMISSION.lock().unwrap().0 = false;
}
pub(super) fn update_finished(message: &str) {
    unfreeze_incoming();
    state("updated", message);
}
fn state(phase: &str, message: &str) {
    let mut s = STATE.lock().unwrap();
    s.phase = phase.into();
    s.message = message.into();
}
pub fn status() -> String {
    let s = STATE.lock().unwrap();
    let mut phase = if s.phase.is_empty() {
        "idle".to_owned()
    } else {
        s.phase.clone()
    };
    let mut message = s.message.clone();
    if phase == "idle" {
        if let Ok(root) = portable::root() {
            if let Ok(bytes) = fs::read(root.join(".updates/result.json")) {
                if bytes.len() < 16384 {
                    if let Ok(result) = serde_json::from_slice::<Value>(&bytes) {
                        if result["version"] == product_version()
                            || result["state"] == "rolled_back"
                        {
                            phase = result["state"].as_str().unwrap_or("idle").into();
                            message = result["message"].as_str().unwrap_or("").into();
                        }
                    }
                }
            }
        }
    }
    let action = if BUSY.load(Ordering::SeqCst) {
        "none"
    } else if s.work.is_some() {
        "apply"
    } else if s.manifest.is_some() {
        "download"
    } else {
        "check"
    };
    json!({"version":product_version(),"state":phase,"message":message,"latestVersion":s.latest,"progress":s.progress,"action":action}).to_string()
}

fn spawn<F: FnOnce() -> Result<(), String> + Send + 'static>(work: F) {
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        if let Err(error) = work() {
            state("failed", &error);
        }
        BUSY.store(false, Ordering::SeqCst);
    });
}
fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(45))
        .user_agent(format!("MegaRustDesk/{}", product_version()))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            let host = url.host_str().unwrap_or("");
            if attempt.previous().len() > 5
                || url.scheme() != "https"
                || ![
                    "github.com",
                    "api.github.com",
                    "objects.githubusercontent.com",
                    "release-assets.githubusercontent.com",
                    "github-releases.githubusercontent.com",
                ]
                .contains(&host)
            {
                attempt.error("Unsupported update redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| "Не удалось настроить защищённое соединение с GitHub".into())
}
fn bounded(response: reqwest::blocking::Response, limit: u64) -> Result<Vec<u8>, String> {
    if !response.status().is_success() {
        return Err("GitHub не отдал файл обновления".into());
    }
    if response.content_length().map_or(false, |n| n > limit) {
        return Err("Файл обновления превышает лимит".into());
    }
    let mut bytes = Vec::new();
    response
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Ошибка защищённой загрузки обновления")?;
    if bytes.len() as u64 > limit {
        return Err("Превышен лимит ответа GitHub".into());
    }
    Ok(bytes)
}
fn request(client: &reqwest::blocking::Client, url: &str, limit: u64) -> Result<Vec<u8>, String> {
    bounded(
        client
            .get(url)
            .send()
            .map_err(|_| "Нет защищённого доступа к GitHub. Проверьте подключение")?,
        limit,
    )
}
pub fn check() {
    spawn(|| {
        state("checking", "Проверяю обновления…");
        let client = client()?;
        let response = client
            .get(format!(
                "https://api.github.com/repos/{}/releases/latest",
                update_manifest::REPOSITORY
            ))
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|_| "Нет защищённого доступа к GitHub")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            state("current", "Опубликованных обновлений пока нет");
            return Ok(());
        }
        if response.status() == reqwest::StatusCode::FORBIDDEN
            || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
        {
            return Err("GitHub временно ограничил запросы. Повторите позже".into());
        }
        let release: Value = serde_json::from_slice(&bounded(response, 1_048_576)?)
            .map_err(|_| "Неверный ответ GitHub")?;
        if release["draft"] != false || release["prerelease"] != false {
            return Err("Предварительный релиз не входит в stable-канал".into());
        }
        let tag = release["tag_name"]
            .as_str()
            .ok_or("В релизе отсутствует тег")?;
        let version = tag
            .strip_prefix("agent-v")
            .ok_or("Неподдерживаемый тег релиза")?;
        if update_manifest::version(version)? <= update_manifest::version(product_version())? {
            state("current", "Установлена последняя версия");
            return Ok(());
        }
        let base = format!(
            "https://github.com/{}/releases/download/{tag}",
            update_manifest::REPOSITORY
        );
        let bytes = request(&client, &format!("{base}/update-manifest.json"), 262_144)?;
        let signature = String::from_utf8(request(
            &client,
            &format!("{base}/update-manifest.sig"),
            256,
        )?)
        .map_err(|_| "Неверная подпись релиза")?;
        let manifest = update_manifest::verify_manifest(
            &bytes,
            &signature,
            CONFIG["public_key"].as_str().unwrap(),
            product_version(),
        )?;
        if manifest.tag != tag {
            return Err("Manifest не соответствует выбранному релизу".into());
        }
        let mut s = STATE.lock().unwrap();
        s.phase = "available".into();
        s.message = "Доступна новая версия".into();
        s.latest = manifest.version.clone();
        s.manifest = Some(manifest);
        s.bytes = bytes;
        s.signature = signature;
        Ok(())
    });
}
pub fn download() {
    spawn(|| {
        let (manifest, bytes, signature) = {
            let s = STATE.lock().unwrap();
            (
                s.manifest.clone().ok_or("Сначала проверьте обновления")?,
                s.bytes.clone(),
                s.signature.clone(),
            )
        };
        let root = portable::root()?;
        update_process::check_space(&root, MAX_EXPANDED * 3 + manifest.size)?;
        let work = root
            .join(".updates")
            .join(format!("incoming-{}", uuid::Uuid::new_v4()));
        update_manifest::plain_path(&work)?;
        fs::create_dir_all(&work).map_err(|_| "Папка приложения недоступна для записи")?;
        fs::write(work.join("update-manifest.json"), bytes).map_err(|e| e.to_string())?;
        fs::write(work.join("update-manifest.sig"), signature).map_err(|e| e.to_string())?;
        state("downloading", "Скачиваю обновление…");
        let client = client()?;
        let response = client
            .get(&manifest.asset_url)
            .send()
            .map_err(|_| "Не удалось скачать обновление")?;
        if !response.status().is_success()
            || response
                .content_length()
                .map_or(false, |n| n > manifest.size)
        {
            return Err("GitHub не отдал ожидаемый пакет".into());
        }
        let archive = work.join("download.zip");
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&archive)
            .map_err(|e| e.to_string())?;
        let mut reader = response.take(manifest.size + 1);
        let mut buffer = [0u8; 65536];
        let mut count = 0u64;
        loop {
            let n = reader.read(&mut buffer).map_err(|_| "Загрузка прервана")?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > manifest.size || count > MAX_ARCHIVE {
                return Err("Пакет превышает ожидаемый размер".into());
            }
            file.write_all(&buffer[..n])
                .map_err(|_| "Не удалось сохранить обновление")?;
            STATE.lock().unwrap().progress = count * 100 / manifest.size;
        }
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        state("verifying", "Проверяю подпись и файлы…");
        update_manifest::extract_archive(&archive, &manifest, &work.join("payload"))?;
        let mut s = STATE.lock().unwrap();
        s.work = Some(work);
        s.phase = "ready".into();
        s.message = "Пакет проверен. Можно перезапустить приложение".into();
        Ok(())
    });
}
pub fn apply() {
    spawn(|| {
        let work = STATE
            .lock()
            .unwrap()
            .work
            .clone()
            .ok_or("Нет подготовленного обновления")?;
        freeze_incoming()?;
        if let Err(error) = update_process::prepare(&portable::root()?, &work) {
            unfreeze_incoming();
            state("blocked", &error);
            return Ok(());
        }
        std::process::exit(0);
    });
}
pub fn early_dispatch() -> bool {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("--agent-product-version") {
        println!("{}", product_version());
        return true;
    }
    if args.get(1).map(String::as_str) == Some("--agent-apply-update") {
        let result = args
            .get(2)
            .ok_or_else(|| "Отсутствует план обновления".to_owned())
            .and_then(|path| update_process::run_helper(std::path::Path::new(path)));
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return true;
    }
    if args.get(1).map(String::as_str) == Some("--agent-verify-update") {
        let result = (|| -> Result<(), String> {
            if args.len() != 6 {
                return Err("Expected manifest signature archive current-version".into());
            }
            let bytes = fs::read(&args[2]).map_err(|e| e.to_string())?;
            let sig = fs::read_to_string(&args[3]).map_err(|e| e.to_string())?;
            let m = update_manifest::verify_manifest(
                &bytes,
                &sig,
                CONFIG["public_key"].as_str().unwrap(),
                &args[5],
            )?;
            update_manifest::verify_archive(std::path::Path::new(&args[4]), &m)?;
            println!(
                "{}",
                json!({"version":m.version,"signatureVerified":true,"inventoryVerified":true,"files":m.files.len()})
            );
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return true;
    }
    false
}

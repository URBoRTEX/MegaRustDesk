//! Windows-only process coordination for the signed portable updater.
use super::{
    portable, update_install,
    update_manifest::{self, Manifest},
    updater,
};
use serde_json::{json, Value};
use std::{
    fs,
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use winapi::{
    shared::minwindef::FILETIME,
    um::{
        handleapi::{CloseHandle, INVALID_HANDLE_VALUE},
        processthreadsapi::{GetProcessTimes, OpenProcess},
        synchapi::WaitForSingleObject,
        tlhelp32::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        winbase::QueryFullProcessImageNameW,
        winnt::{PROCESS_QUERY_LIMITED_INFORMATION, SYNCHRONIZE},
    },
};
const NO_WINDOW: u32 = 0x08000000;
fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_lowercase()
}
fn load(path: &Path, limit: u64) -> Result<Value, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > limit {
        return Err("Слишком большой файл updater".into());
    }
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|_| "Неверный файл updater".into())
}
fn write(path: &Path, value: &Value) -> Result<(), String> {
    update_manifest::plain_path(path)?;
    fs::write(path, value.to_string()).map_err(|e| e.to_string())
}
fn birth(pid: u32) -> Result<u64, String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return Err("Не удалось проверить процесс приложения".into());
        }
        let mut created: FILETIME = std::mem::zeroed();
        let mut exit = created;
        let mut kernel = created;
        let mut user = created;
        let ok = GetProcessTimes(h, &mut created, &mut exit, &mut kernel, &mut user);
        CloseHandle(h);
        if ok == 0 {
            return Err("Не удалось проверить время создания процесса".into());
        }
        Ok(((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
    }
}
fn image(pid: u32) -> Result<PathBuf, String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return Err("Не удалось проверить процесс RustDesk".into());
        }
        let mut buffer = vec![0u16; 32768];
        let mut size = buffer.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buffer.as_mut_ptr(), &mut size);
        CloseHandle(h);
        if ok == 0 {
            return Err("Не удалось проверить путь RustDesk".into());
        }
        Ok(PathBuf::from(String::from_utf16_lossy(
            &buffer[..size as usize],
        )))
    }
}
fn others(root: &Path, exclude: u32) -> Result<Vec<u32>, String> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err("Не удалось проверить запущенные приложения".into());
        }
        let result = (|| {
            let mut found = Vec::new();
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = Process32FirstW(snapshot, &mut entry);
            while ok != 0 {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
                if entry.th32ProcessID != exclude && name.starts_with("rustdesk-agent") {
                    let path = image(entry.th32ProcessID)?;
                    if path.parent().map(|p| normalized(p)) == Some(normalized(root)) {
                        found.push(entry.th32ProcessID);
                    }
                }
                ok = Process32NextW(snapshot, &mut entry);
            }
            Ok(found)
        })();
        CloseHandle(snapshot);
        result
    }
}
pub fn check_space(root: &Path, required: u64) -> Result<(), String> {
    unsafe {
        let path = root
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let mut available = 0u64;
        if winapi::um::fileapi::GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut available as *mut u64 as *mut _,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        ) == 0
        {
            return Err("Не удалось проверить свободное место".into());
        }
        if available < required {
            return Err("Недостаточно свободного места для обновления и отката".into());
        }
        Ok(())
    }
}
fn launch(root: &Path, work: &Path, recovery: bool) -> Result<(), String> {
    if !others(root, std::process::id())?.is_empty() {
        return Err("Закройте другие окна RustDesk и использующие эту папку Codex/MCP-клиенты, затем повторите".into());
    }
    update_manifest::plain_path(work)?;
    let runner = work.join("runner.exe");
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let hash = update_manifest::file_hash(&current)?;
    if runner.exists() {
        if update_manifest::file_hash(&runner)? != hash {
            return Err("Неизвестный updater runner сохранён".into());
        }
    } else {
        fs::copy(&current, &runner).map_err(|_| "Не удалось подготовить updater")?;
    }
    let nonce = uuid::Uuid::new_v4().to_string();
    let ready = work.join("ready.json");
    if ready.exists() {
        fs::remove_file(&ready).map_err(|e| e.to_string())?;
    }
    let plan = json!({"root":root,"work":work,"parentPid":std::process::id(),"parentBirth":birth(std::process::id())?,"nonce":nonce,"runnerSha256":hash,"mode":if recovery {"recover"}else{"apply"}});
    let path = work.join("plan.json");
    write(&path, &plan)?;
    let mut child=Command::new(&runner).arg("--agent-apply-update").arg(&path).creation_flags(super::process_job::spawn_flags()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|_|"Windows не разрешила запустить updater вне процесса приложения. Текущая версия сохранена")?;
    if let Err(error)=super::process_job::check_child(&child) {
        let _=child.kill();let _=child.wait();return Err(error);
    }
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(8) {
        if let Ok(value) = load(&ready, 4096) {
            if value["nonce"] == nonce && value["pid"].as_u64() == Some(child.id() as u64) {
                if value["ok"] == true {
                    if let Err(error)=super::process_job::check_child(&child) {
                        let _=child.kill();let _=child.wait();return Err(error);
                    }
                    return Ok(());
                }
                return Err(value["message"]
                    .as_str()
                    .unwrap_or("Updater не готов")
                    .to_owned());
            }
        }
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            return Err("Updater завершился до готовности; текущая версия сохранена".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
    Err("Updater не подтвердил готовность; текущая версия сохранена".into())
}
fn lock(root: &Path) -> Result<fs::File, String> {
    let dir = root.join(".updates");
    update_manifest::plain_path(&dir)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("apply.lock");
    update_manifest::plain_path(&path)?;
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .share_mode(0)
        .open(path)
        .map_err(|_| "Другое обновление уже выполняется".into())
}
fn work_manifest(work: &Path, current: &str) -> Result<Manifest, String> {
    let path = work.join("update-manifest.json");
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 262144 {
        return Err("Manifest превышает лимит".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let signature =
        fs::read_to_string(work.join("update-manifest.sig")).map_err(|e| e.to_string())?;
    update_manifest::verify_manifest(
        &bytes,
        &signature,
        updater::config()["public_key"].as_str().unwrap(),
        current,
    )
}
pub fn run_helper(plan_path: &Path) -> Result<(), String> {
    update_manifest::plain_path(plan_path)?;
    let mut plan = load(plan_path, 16384)?;
    let root = PathBuf::from(plan["root"].as_str().ok_or("Нет каталога приложения")?);
    let work = PathBuf::from(plan["work"].as_str().ok_or("Нет staging")?);
    update_manifest::plain_path(&root)?;
    update_manifest::plain_path(&work)?;
    let recovering = plan["mode"] == "recover";
    let work_name = work.file_name().unwrap_or_default().to_string_lossy();
    if normalized(work.parent().ok_or("Нет каталога staging")?)
        != normalized(&root.join(".updates"))
        || !(if recovering {
            work_name.starts_with("recovery-")
        } else {
            work_name.starts_with("incoming-")
        })
        || normalized(plan_path) != normalized(&work.join("plan.json"))
    {
        return Err("План не принадлежит portable-папке".into());
    }
    let nonce = plan["nonce"].as_str().ok_or("Нет nonce updater")?;
    uuid::Uuid::parse_str(nonce).map_err(|_| "Неверный nonce")?;
    let own = std::env::current_exe().map_err(|e| e.to_string())?;
    if normalized(&own) != normalized(&work.join("runner.exe"))
        || update_manifest::file_hash(&own)?
            != plan["runnerSha256"].as_str().ok_or("Нет hash updater")?
    {
        return Err("Updater runner не подтверждён".into());
    }
    let parent = plan["parentPid"].as_u64().ok_or("Нет parent PID")? as u32;
    let parent_birth = plan["parentBirth"].as_u64().unwrap_or(0);
    let apply_lock = lock(&root)?;
    let mut restart_after_unlock = false;
    let result = (|| -> Result<(), String> {
        if parent != 0 {
            if birth(parent)? != parent_birth
                || image(parent)?.parent().map(normalized) != Some(normalized(&root))
            {
                return Err("Родительский процесс изменился".into());
            }
        }
        if recovering {
            let guard = Parent::open(parent, parent_birth, &root)?;
            update_install::validate_recovery(&root)?;
            write(
                &work.join("ready.json"),
                &json!({"ok":true,"pid":std::process::id(),"birth":birth(std::process::id())?,"nonce":nonce}),
            )?;
            guard.wait()?;
            wait_others(&root)?;
            update_install::recover(&root)?;
            write(
                &root.join(".updates/result.json"),
                &json!({"state":"rolled_back","message":"Предыдущая версия восстановлена после прерванного обновления"}),
            )?;
            restart_after_unlock = true;
            return Ok(());
        }
        let guard = Parent::open(parent, parent_birth, &root)?;
        let installed = load(&root.join("manifest.json"), 524288)?;
        let current = installed["version"].as_str().ok_or("Нет текущей версии")?;
        let manifest = work_manifest(&work, current)?;
        update_manifest::verify_archive(&work.join("download.zip"), &manifest)?;
        write(
            &work.join("ready.json"),
            &json!({"ok":true,"pid":std::process::id(),"birth":birth(std::process::id())?,"nonce":nonce}),
        )?;
        guard.wait()?;
        wait_others(&root)?;
        let health = work.join("gui-health.json");
        if health.exists() {
            fs::remove_file(&health).map_err(|e| e.to_string())?;
        }
        update_install::apply(&root, &work.join("payload"), &manifest, |folder| {
            let mut child = Command::new(folder.join("RustDesk-Agent.exe"))
                .current_dir(folder)
                .env("MEGARUSTDESK_UPDATE_HEALTH", &health)
                .env("MEGARUSTDESK_UPDATE_NONCE", nonce)
                .env("MEGARUSTDESK_UPDATE_HELPER", std::process::id().to_string())
                .env("MEGARUSTDESK_UPDATE_OLD_VERSION", current)
                .creation_flags(NO_WINDOW)
                .spawn()
                .map_err(|_| "Не удалось запустить новую версию")?;
            let timer = Instant::now();
            while timer.elapsed() < Duration::from_secs(20) {
                if child.try_wait().map_err(|e| e.to_string())?.is_some() {
                    return Err("Новая версия завершилась при запуске".into());
                }
                if let Ok(value) = load(&health, 4096) {
                    if value["nonce"] == nonce && value["version"] == manifest.version {
                        thread::sleep(Duration::from_millis(500));
                        if child.try_wait().map_err(|e| e.to_string())?.is_none() {
                            return Ok(());
                        }
                    }
                }
                thread::sleep(Duration::from_millis(100));
            }
            let _ = child.kill();
            let _ = child.wait();
            Err("Новая версия не подтвердила запуск".into())
        })?;
        let _ = write(
            &root.join(".updates/result.json"),
            &json!({"state":"updated","version":manifest.version,"message":"Обновление установлено","nonce":nonce}),
        );
        Ok(())
    })();
    if let Err(error) = &result {
        let _ = write(
            &work.join("ready.json"),
            &json!({"ok":false,"pid":std::process::id(),"nonce":nonce,"message":error}),
        );
        let _ = write(
            &root.join(".updates/result.json"),
            &json!({"state":"failed","message":error}),
        );
        if !recovering
            && (parent == 0 || birth(parent).ok() != Some(parent_birth))
            && !pending_recovery(&root)
        {
            restart_after_unlock = true;
        }
    }
    update_install::after_unlock(apply_lock, || {
        if restart_after_unlock {
            Command::new(root.join("RustDesk-Agent.exe"))
                .current_dir(&root)
                .creation_flags(NO_WINDOW)
                .spawn()
                .map_err(|_| "Восстановление выполнено, запустите приложение вручную")?;
        }
        Ok(())
    })?;
    result
}
pub fn recover_startup() -> Result<(), String> {
    let root = portable::root()?;
    if health_context(&root) {
        return Ok(());
    }
    if pending_recovery(&root) {
        let work = root
            .join(".updates")
            .join(format!("recovery-{}", uuid::Uuid::new_v4()));
        update_manifest::plain_path(&work)?;
        fs::create_dir(&work).map_err(|e| e.to_string())?;
        launch(&root, &work, true)?;
        std::process::exit(0);
    }
    if root.join(".updates/transaction.json").exists() {
        let _guard = lock(&root)?;
        update_install::recover(&root)?;
    }
    Ok(())
}

pub fn mark_gui_started() {
    let Some(health) = std::env::var_os("MEGARUSTDESK_UPDATE_HEALTH") else {
        return;
    };
    let path = PathBuf::from(health);
    let nonce = std::env::var("MEGARUSTDESK_UPDATE_NONCE").unwrap_or_default();
    let helper = std::env::var("MEGARUSTDESK_UPDATE_HELPER")
        .ok()
        .and_then(|v| v.parse::<u32>().ok());
    let old_version = std::env::var("MEGARUSTDESK_UPDATE_OLD_VERSION").unwrap_or_default();
    if let Ok(root) = portable::root() {
        if path.starts_with(root.join(".updates"))
            && path.file_name().and_then(|n| n.to_str()) == Some("gui-health.json")
        {
            let _ = write(
                &path,
                &json!({"nonce":nonce,"version":updater::product_version()}),
            );
            if let (Some(pid), Some(work)) = (helper, path.parent().map(Path::to_path_buf)) {
                let ready = load(&work.join("ready.json"), 4096).ok();
                let expected = ready.as_ref().and_then(|r| r["birth"].as_u64());
                let token = nonce.clone();
                thread::spawn(move || {
                    if let Some(stamp) = expected {
                        if birth(pid).ok() == Some(stamp) {
                            unsafe {
                                let h = OpenProcess(SYNCHRONIZE, 0, pid);
                                if !h.is_null() {
                                    let waited = WaitForSingleObject(h, 60000);
                                    CloseHandle(h);
                                    if waited != 0 {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    let result_path = root.join(".updates/result.json");
                    let committed = load(&root.join(".updates/transaction.json"), 1048576)
                        .ok()
                        .map_or(false, |v| {
                            v["phase"] == "committed"
                                && v["targetVersion"] == updater::product_version()
                        });
                    if committed {
                        let mut report=load(&result_path,16384).unwrap_or_else(|_|json!({"state":"updated","version":updater::product_version(),"nonce":token}));
                        {
                            let result = cleanup_work(&work, &old_version, &token);
                            let rollback_clean =
                                load(&root.join(".updates/transaction.json"), 1048576)
                                    .ok()
                                    .map_or(false, |v| v["cleanupCompleted"] == true);
                            report["cleanupCompleted"] = json!(result.is_ok() && rollback_clean);
                            report["message"] = json!("Обновление установлено");
                            if report["cleanupCompleted"] != true {
                                report["message"]=json!("Обновление установлено; временные файлы сохранены для проверки");
                            }
                            updater::update_finished(
                                report["message"]
                                    .as_str()
                                    .unwrap_or("Обновление установлено"),
                            );
                            let _ = write(&result_path, &report);
                        }
                    }
                });
            }
        }
    }
    for name in [
        "MEGARUSTDESK_UPDATE_HEALTH",
        "MEGARUSTDESK_UPDATE_NONCE",
        "MEGARUSTDESK_UPDATE_HELPER",
        "MEGARUSTDESK_UPDATE_OLD_VERSION",
    ] {
        std::env::remove_var(name);
    }
}

pub fn prepare(root: &Path, work: &Path) -> Result<(), String> {
    launch(root, work, false)
}
fn pending_recovery(root: &Path) -> bool {
    load(&root.join(".updates/transaction.json"), 1048576)
        .ok()
        .map_or(false, |v| {
            matches!(v["phase"].as_str(), Some("replacing") | Some("health"))
        })
}
fn health_context(root: &Path) -> bool {
    if std::env::args().len() != 1 {
        return false;
    }
    let Some(path) = std::env::var_os("MEGARUSTDESK_UPDATE_HEALTH").map(PathBuf::from) else {
        return false;
    };
    if path.file_name().and_then(|p| p.to_str()) != Some("gui-health.json")
        || path.parent().and_then(Path::parent).map(normalized)
            != Some(normalized(&root.join(".updates")))
    {
        return false;
    }
    let Some(work) = path.parent() else {
        return false;
    };
    let Ok(ready) = load(&work.join("ready.json"), 4096) else {
        return false;
    };
    let nonce = std::env::var("MEGARUSTDESK_UPDATE_NONCE").unwrap_or_default();
    let Some(pid) = ready["pid"].as_u64().map(|p| p as u32) else {
        return false;
    };
    ready["ok"] == true
        && ready["nonce"] == nonce
        && std::env::var("MEGARUSTDESK_UPDATE_HELPER")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            == Some(pid as u64)
        && ready["birth"].as_u64() == birth(pid).ok()
        && image(pid).ok().map(|p| normalized(&p)) == Some(normalized(&work.join("runner.exe")))
}
pub fn startup_admitted() -> bool {
    let Ok(root) = portable::root() else {
        return false;
    };
    if root.join(".updates/apply.lock").exists() && lock(&root).is_err() {
        if health_context(&root) {
            return updater::freeze_incoming().is_ok();
        }
        return false;
    }
    if std::env::args().any(|a| a == "--agent-mcp") && pending_recovery(&root) {
        return false;
    }
    true
}
struct Parent(Option<winapi::shared::ntdef::HANDLE>);
impl Drop for Parent {
    fn drop(&mut self) {
        if let Some(handle) = self.0 {
            unsafe {
                CloseHandle(handle);
            }
        }
    }
}
impl Parent {
    fn open(pid: u32, expected: u64, root: &Path) -> Result<Self, String> {
        if pid == 0 {
            return Ok(Self(None));
        }
        unsafe {
            let handle = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return Err("Не удалось удержать identity родителя".into());
            }
            let guard = Self(Some(handle));
            if birth(pid)? != expected
                || image(pid)?.parent().map(normalized) != Some(normalized(root))
            {
                return Err("Родительский процесс изменился".into());
            }
            Ok(guard)
        }
    }
    fn wait(&self) -> Result<(), String> {
        if let Some(handle) = self.0 {
            if unsafe { WaitForSingleObject(handle, 20000) } != 0 {
                return Err("Приложение не завершилось; файлы не заменены".into());
            }
        }
        Ok(())
    }
}
fn wait_others(root: &Path) -> Result<(), String> {
    let started = Instant::now();
    loop {
        if others(root, 0)?.is_empty() {
            return Ok(());
        }
        if started.elapsed() > Duration::from_secs(2) {
            return Err("Другой процесс приложения удерживает файлы. Обновление отменено".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn cleanup_work(work: &Path, old_version: &str, nonce: &str) -> Result<(), String> {
    let manifest = work_manifest(work, old_version)?;
    update_manifest::verify_archive(&work.join("download.zip"), &manifest)?;
    let plan = load(&work.join("plan.json"), 16384)?;
    let ready = load(&work.join("ready.json"), 4096)?;
    let health = load(&work.join("gui-health.json"), 4096)?;
    if plan["nonce"] != nonce
        || ready["nonce"] != nonce
        || health["nonce"] != nonce
        || update_manifest::file_hash(&work.join("runner.exe"))?
            != plan["runnerSha256"].as_str().ok_or("Нет hash runner")?
    {
        return Err("Содержимое updater изменилось".into());
    }
    fn collect(
        folder: &Path,
        root: &Path,
        m: &Manifest,
        files: &mut Vec<PathBuf>,
        dirs: &mut Vec<PathBuf>,
    ) -> Result<(), String> {
        update_manifest::plain_path(folder)?;
        for entry in fs::read_dir(folder).map_err(|e| e.to_string())? {
            let p = entry.map_err(|e| e.to_string())?.path();
            update_manifest::plain_path(&p)?;
            let relative = p
                .strip_prefix(root)
                .map_err(|_| "Неверный staging")?
                .to_string_lossy()
                .replace('\\', "/");
            if p.is_dir() {
                if !m
                    .files
                    .keys()
                    .any(|name| format!("payload/{name}").starts_with(&format!("{relative}/")))
                {
                    return Err("Посторонняя папка staging сохранена".into());
                }
                collect(&p, root, m, files, dirs)?;
                dirs.push(p);
            } else {
                if let Some(name) = relative.strip_prefix("payload/") {
                    if m.files.get(name) != Some(&update_manifest::file_hash(&p)?) {
                        return Err("Payload изменён; staging сохранён".into());
                    }
                } else if ![
                    "update-manifest.json",
                    "update-manifest.sig",
                    "download.zip",
                    "runner.exe",
                    "plan.json",
                    "ready.json",
                    "gui-health.json",
                ]
                .contains(&relative.as_str())
                {
                    return Err("Посторонний файл staging сохранён".into());
                }
                files.push(p);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    collect(work, work, &manifest, &mut files, &mut dirs)?;
    for p in files {
        fs::remove_file(p).map_err(|e| e.to_string())?;
    }
    for p in dirs {
        fs::remove_dir(p).map_err(|e| e.to_string())?;
    }
    fs::remove_dir(work).map_err(|e| e.to_string())
}

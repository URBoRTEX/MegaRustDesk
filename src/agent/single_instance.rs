//! Main-window takeover for portable copies owned by the same Windows user/session.
#[cfg(not(singleton_standalone))]
use hbb_common::sysinfo;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
};
use winapi::{
    shared::{
        minwindef::{BOOL, FILETIME, LPARAM},
        windef::HWND,
    },
    um::{
        handleapi::CloseHandle,
        processthreadsapi::{GetCurrentProcess, GetProcessTimes, OpenProcess, TerminateProcess},
        synchapi::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
        winbase::QueryFullProcessImageNameW,
        winnt::{HANDLE, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, SYNCHRONIZE},
        winuser::{EnumWindows, GetWindowThreadProcessId, PostMessageW, WM_CLOSE},
    },
};

pub fn is_main_args(args: &[String]) -> bool {
    args.is_empty() || args == ["--noinstall"]
}
fn is_host_args(args: &[String]) -> bool {
    is_main_args(args) || args == ["--server"] || args == ["--tray"]
}
fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .to_lowercase()
}
fn executable_hash(exe: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut file = fs::File::open(exe).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}
fn verified_product(exe: &Path) -> Result<bool, String> {
    let name = exe
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if !name.starts_with("rustdesk-agent") || !name.ends_with(".exe") {
        return Ok(false);
    }
    let Some(root) = exe.parent() else {
        return Ok(false);
    };
    let path = root.join("manifest.json");
    if fs::metadata(&path).map_or(true, |m| m.len() > 524288) {
        return Ok(false);
    }
    let Ok(bytes) = fs::read(path) else {
        return Ok(false);
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Ok(false);
    };
    if value["product"] != "RustDesk Agent Portable" || value["platform"] != "windows-x64" {
        return Ok(false);
    }
    let expected = value["files"]["RustDesk-Agent.exe"]
        .as_str()
        .ok_or("Нет hash прежнего EXE")?;
    if expected != executable_hash(exe)? {
        return Err("Прежний EXE не соответствует своему manifest".into());
    }
    Ok(true)
}
struct ProcessHandle(HANDLE);
impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn creation(handle: HANDLE) -> Result<u64, String> {
    unsafe {
        let mut born: FILETIME = std::mem::zeroed();
        let mut exit = born;
        let mut kernel = born;
        let mut user = born;
        if GetProcessTimes(handle, &mut born, &mut exit, &mut kernel, &mut user) == 0 {
            return Err("Не удалось проверить время создания экземпляра".into());
        }
        Ok(((born.dwHighDateTime as u64) << 32) | born.dwLowDateTime as u64)
    }
}
fn image(handle: HANDLE) -> Result<PathBuf, String> {
    unsafe {
        let mut path = vec![0u16; 32768];
        let mut len = path.len() as u32;
        if QueryFullProcessImageNameW(handle, 0, path.as_mut_ptr(), &mut len) == 0 {
            return Err("Не удалось проверить путь экземпляра".into());
        }
        Ok(PathBuf::from(String::from_utf16_lossy(
            &path[..len as usize],
        )))
    }
}
pub struct Admission(HANDLE);
impl Admission {
    fn acquire(name: &str) -> Result<Self, String> {
        let wide = std::ffi::OsStr::new(name)
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        unsafe {
            let handle = CreateMutexW(std::ptr::null_mut(), 0, wide.as_ptr());
            if handle.is_null() {
                return Err("Не создана блокировка запуска приложения".into());
            }
            match WaitForSingleObject(handle, 10000) {
                0 | 0x80 => Ok(Self(handle)),
                _ => {
                    CloseHandle(handle);
                    Err("Другой экземпляр ещё запускается. Повторите запуск".into())
                }
            }
        }
    }
}
impl Drop for Admission {
    fn drop(&mut self) {
        unsafe {
            ReleaseMutex(self.0);
            CloseHandle(self.0);
        }
    }
}
unsafe extern "system" fn close_window(hwnd: HWND, param: LPARAM) -> BOOL {
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == param as u32 {
        PostMessageW(hwnd, WM_CLOSE, 0, 0);
    }
    1
}
fn close_host(pid: u32, handle: &ProcessHandle) -> Result<(), String> {
    unsafe {
        if WaitForSingleObject(handle.0, 0) == 0 {
            return Ok(());
        }
        EnumWindows(Some(close_window), pid as LPARAM);
        if WaitForSingleObject(handle.0, 1500) == 0 {
            return Ok(());
        }
        if TerminateProcess(handle.0, 0) == 0 && WaitForSingleObject(handle.0, 0) != 0 {
            return Err(format!(
                "Не удалось закрыть прежний экземпляр PID {pid}. Закройте его вручную"
            ));
        }
        if WaitForSingleObject(handle.0, 4000) != 0 {
            return Err(format!("Прежний экземпляр PID {pid} ещё завершается"));
        }
    }
    Ok(())
}
// Scope is supplied only by the standalone test harness; production always uses None.
fn take_over(scope: Option<&Path>) -> Result<Option<Admission>, String> {
    let my_pid = std::process::id();
    let my_born = creation(unsafe { GetCurrentProcess() })?;
    let mut sys = sysinfo::System::new();
    sys.refresh_processes();
    let me = sys
        .process(sysinfo::Pid::from_u32(my_pid))
        .ok_or("Не найден текущий процесс")?;
    let uid = me
        .user_id()
        .cloned()
        .ok_or("Не удалось проверить пользователя Windows")?;
    let session = me
        .session_id()
        .ok_or("Не удалось проверить сеанс Windows")?;
    let key = format!(
        "{uid:?}:{session:?}:{}",
        scope.map(normalized).unwrap_or_default()
    );
    let hash = Sha256::digest(key.as_bytes())
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let admission = Admission::acquire(&format!(r"Local\MegaRustDesk.MainAdmissionV1.{hash}"))?;
    sys.refresh_processes();
    let mut ancestors = Vec::new();
    let mut parent = sys
        .process(sysinfo::Pid::from_u32(my_pid))
        .and_then(|p| p.parent());
    for _ in 0..64 {
        let Some(pid) = parent else {
            break;
        };
        if ancestors.contains(&pid) {
            break;
        }
        ancestors.push(pid);
        parent = sys.process(pid).and_then(|p| p.parent());
    }
    let mut targets = Vec::new();
    for (pid, process) in sys.processes() {
        if pid.as_u32() == my_pid
            || process.user_id() != Some(&uid)
            || process.session_id() != Some(session)
            || process.cmd().is_empty()
            || !is_host_args(&process.cmd()[1..])
        {
            continue;
        }
        let exe = process.exe();
        if scope.map_or(false, |root| !exe.starts_with(root)) {
            continue;
        }
        if !verified_product(exe).map_err(|_|format!("Не подтверждены файлы прежнего экземпляра PID {}. Закройте его вручную и повторите запуск",pid.as_u32()))? {continue;}
        if Path::new(&process.cmd()[0])
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            != exe.file_name().map(|n| n.to_string_lossy().to_lowercase())
        {
            continue;
        }
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE | PROCESS_TERMINATE,
                0,
                pid.as_u32(),
            )
        };
        if handle.is_null() {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(87) {
                continue;
            }
            return Err(format!(
                "Нет доступа к прежнему экземпляру PID {}. Закройте его вручную",
                pid.as_u32()
            ));
        }
        let handle = ProcessHandle(handle);
        if unsafe { WaitForSingleObject(handle.0, 0) } == 0 {
            continue;
        }
        let born = creation(handle.0)?;
        if normalized(&image(handle.0)?) != normalized(exe) {
            continue;
        }
        if (born, pid.as_u32()) > (my_born, my_pid) {
            return Ok(None);
        }
        if ancestors.contains(pid) {
            return Err(
                "Новый экземпляр запущен старым приложением. Запустите EXE напрямую из папки"
                    .into(),
            );
        }
        targets.push((pid.as_u32(), handle));
    }
    for (pid, handle) in targets {
        close_host(pid, &handle)?;
    }
    Ok(Some(admission))
}
#[cfg(not(singleton_standalone))]
pub fn enter() -> Result<Option<Admission>, String> {
    let root = super::portable::root()?;
    super::update_manifest::verify_installation(
        &root,
        &std::env::current_exe().map_err(|e| e.to_string())?,
        super::updater::product_version(),
    )?;
    take_over(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }
    fn host_roles_only() {
        for role in [
            vec![],
            args(&["--noinstall"]),
            args(&["--server"]),
            args(&["--tray"]),
        ] {
            assert!(is_host_args(&role), "Host role was not selected: {role:?}");
        }
        for role in [
            args(&["--agent-mcp"]),
            args(&["--connect", "fixture-peer"]),
            args(&["--cm"]),
            args(&["--agent-ui-test"]),
            args(&["--agent-apply-update", "plan.json"]),
            args(&["--agent-self-test"]),
            args(&["--future-mode"]),
        ] {
            assert!(!is_host_args(&role), "Non-host role selected: {role:?}");
            assert!(!is_main_args(&role));
        }
        assert!(!is_main_args(&args(&["--server"])));
        assert!(!is_main_args(&args(&["--tray"])));
    }
    #[cfg(singleton_standalone)]
    struct Fixtures {
        root: PathBuf,
        children: Vec<std::process::Child>,
    }
    #[cfg(singleton_standalone)]
    impl Fixtures {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "megarustdesk-singleton-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            Self {
                root,
                children: Vec::new(),
            }
        }
        fn executable(&self, name: &str, product: bool) -> PathBuf {
            let folder = self.root.join(name);
            fs::create_dir(&folder).unwrap();
            let exe = folder.join("RustDesk-Agent.exe");
            fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
            fs::write(folder.join("manifest.json"),serde_json::json!({"product":if product{"RustDesk Agent Portable"}else{"Other Product"},"platform":"windows-x64","files":{"RustDesk-Agent.exe":executable_hash(&exe).unwrap()}}).to_string()).unwrap();
            fs::create_dir(folder.join("data")).unwrap();
            fs::write(folder.join("data/keep"), b"task-owned synthetic data").unwrap();
            exe
        }
        fn spawn(&mut self, exe: &Path, role: &str, arguments: &[&str]) -> usize {
            use std::os::windows::process::CommandExt;
            let child = std::process::Command::new(exe)
                .args(arguments)
                .env("MEGA_SINGLETON_FIXTURE", role)
                .env("MEGA_SINGLETON_SCOPE", &self.root)
                .env(
                    "MEGA_SINGLETON_CHILD",
                    self.root.join("child/RustDesk-Agent.exe"),
                )
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .creation_flags(0x08000000)
                .spawn()
                .unwrap();
            self.children.push(child);
            self.children.len() - 1
        }
        fn ready(&self, index: usize) -> Result<(), String> {
            let pid = self.children[index].id();
            let path = self.root.join(format!("ready-{pid}"));
            let timer = std::time::Instant::now();
            while timer.elapsed() < std::time::Duration::from_secs(12) {
                if path.exists() {
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            Err(format!("Fixture {pid} did not start"))
        }
        fn alive(&mut self, index: usize) -> bool {
            self.children[index].try_wait().unwrap().is_none()
        }
        fn cleanup(mut self) {
            for child in &mut self.children {
                let _ = child.kill();
                let _ = child.wait();
            }
            let mut sys = sysinfo::System::new();
            sys.refresh_processes();
            for (pid, process) in sys.processes() {
                if process.exe().starts_with(&self.root) {
                    unsafe {
                        let handle = OpenProcess(
                            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | SYNCHRONIZE,
                            0,
                            pid.as_u32(),
                        );
                        if !handle.is_null() {
                            let handle = ProcessHandle(handle);
                            if image(handle.0).map_or(false, |p| p.starts_with(&self.root)) {
                                TerminateProcess(handle.0, 0);
                                WaitForSingleObject(handle.0, 3000);
                            }
                        }
                    }
                }
            }
            assert_eq!(self.root.parent(), Some(std::env::temp_dir().as_path()));
            assert!(self
                .root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&format!("megarustdesk-singleton-{}-", std::process::id())));
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
    #[cfg(singleton_standalone)]
    fn takes_old_hosts_and_keeps_workers() {
        let mut fixtures = Fixtures::new();
        let old = fixtures.executable("old", true);
        let new = fixtures.executable("new", true);
        let tray = fixtures.executable("tray", true);
        let server = fixtures.executable("server", true);
        let other = fixtures.executable("other", false);
        let a = fixtures.spawn(&old, "hold", &[]);
        let mcp = fixtures.spawn(&old, "admit", &["--agent-mcp"]);
        let viewer = fixtures.spawn(&old, "admit", &["--connect", "fixture-peer"]);
        let t = fixtures.spawn(&tray, "hold", &["--tray"]);
        let s = fixtures.spawn(&server, "hold", &["--server"]);
        let o = fixtures.spawn(&other, "hold", &[]);
        let result = (|| -> Result<(), String> {
            for i in [a, mcp, viewer, t, s, o] {
                fixtures.ready(i)?;
            }
            if !fixtures.alive(a) {
                return Err("MCP or viewer took over main".into());
            }
            let b = fixtures.spawn(&new, "admit", &[]);
            fixtures.ready(b)?;
            for i in [a, t, s] {
                if fixtures.alive(i) {
                    return Err("Older host survived takeover".into());
                }
            }
            for i in [mcp, viewer, o, b] {
                if !fixtures.alive(i) {
                    return Err("Worker or unrelated product terminated".into());
                }
            }
            if fs::read(old.parent().unwrap().join("data/keep")).unwrap()
                != b"task-owned synthetic data"
            {
                return Err("Data changed".into());
            }
            let repeat = fixtures.spawn(&new, "admit", &[]);
            fixtures.ready(repeat)?;
            if fixtures.alive(b)
                || !fixtures.alive(repeat)
                || !fixtures.alive(mcp)
                || !fixtures.alive(viewer)
            {
                return Err("Same-folder restart lost ownership or workers".into());
            }
            Ok(())
        })();
        fixtures.cleanup();
        assert!(result.is_ok(), "{result:?}");
    }
    #[cfg(singleton_standalone)]
    fn parallel_start_leaves_one_host() {
        let mut fixtures = Fixtures::new();
        let a = fixtures.executable("a", true);
        let b = fixtures.executable("b", true);
        let old = fixtures.spawn(&a, "hold", &[]);
        let result = (|| -> Result<(), String> {
            fixtures.ready(old)?;
            let x = fixtures.spawn(&a, "admit", &[]);
            let y = fixtures.spawn(&b, "admit", &[]);
            let timer = std::time::Instant::now();
            while timer.elapsed() < std::time::Duration::from_secs(14) {
                let ready_x = fixtures
                    .root
                    .join(format!("ready-{}", fixtures.children[x].id()))
                    .exists();
                let ready_y = fixtures
                    .root
                    .join(format!("ready-{}", fixtures.children[y].id()))
                    .exists();
                if (ready_x || ready_y)
                    && !fixtures.alive(old)
                    && fixtures.alive(x) != fixtures.alive(y)
                {
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err("Concurrent starts left zero or multiple main processes".into())
        })();
        fixtures.cleanup();
        assert!(result.is_ok(), "{result:?}");
    }
    #[cfg(singleton_standalone)]
    fn mismatched_image_is_not_closed() {
        let mut fixtures = Fixtures::new();
        let bad = fixtures.executable("unverified", true);
        let new = fixtures.executable("new", true);
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(bad.parent().unwrap().join("manifest.json")).unwrap())
                .unwrap();
        manifest["files"]["RustDesk-Agent.exe"] = serde_json::json!("0".repeat(64));
        fs::write(
            bad.parent().unwrap().join("manifest.json"),
            manifest.to_string(),
        )
        .unwrap();
        let old = fixtures.spawn(&bad, "hold", &[]);
        let result = (|| -> Result<(), String> {
            fixtures.ready(old)?;
            let next = fixtures.spawn(&new, "admit", &[]);
            let timer = std::time::Instant::now();
            while timer.elapsed() < std::time::Duration::from_secs(10) && fixtures.alive(next) {
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            if !fixtures.alive(old) {
                return Err("Unverified executable was terminated".into());
            }
            if fixtures.alive(next) {
                return Err("New main ignored an unverified host".into());
            }
            Ok(())
        })();
        fixtures.cleanup();
        assert!(result.is_ok(), "{result:?}");
    }
    #[cfg(singleton_standalone)]
    fn child_of_old_host_is_refused_safely() {
        let mut fixtures = Fixtures::new();
        let parent = fixtures.executable("parent", true);
        let _child = fixtures.executable("child", true);
        let old = fixtures.executable("other-old", true);
        let p = fixtures.spawn(&parent, "parent", &[]);
        let o = fixtures.spawn(&old, "hold", &[]);
        let result = (|| -> Result<(), String> {
            fixtures.ready(p)?;
            fixtures.ready(o)?;
            fs::write(fixtures.root.join("spawn-child"), b"go").unwrap();
            let timer = std::time::Instant::now();
            while timer.elapsed() < std::time::Duration::from_secs(12) {
                if fixtures.root.join("child-exit").exists() {
                    if fs::read(fixtures.root.join("child-exit")).unwrap() != b"2" {
                        return Err("Child launch was not refused".into());
                    }
                    if !fixtures.alive(p) || !fixtures.alive(o) {
                        return Err("Refused child killed an older host".into());
                    }
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
            }
            Err("Child fixture did not finish".into())
        })();
        fixtures.cleanup();
        assert!(result.is_ok(), "{result:?}");
    }
    pub fn run_all() {
        host_roles_only();
        println!("PASS host_roles_only");
        #[cfg(singleton_standalone)]
        {
            mismatched_image_is_not_closed();
            println!("PASS mismatched_image_is_not_closed");
            child_of_old_host_is_refused_safely();
            println!("PASS child_of_old_host_is_refused_safely");
            takes_old_hosts_and_keeps_workers();
            println!("PASS takes_old_hosts_and_keeps_workers");
            parallel_start_leaves_one_host();
            println!("PASS parallel_start_leaves_one_host");
        }
    }
}
#[cfg(all(test, singleton_standalone))]
fn main() {
    if let Ok(role) = std::env::var("MEGA_SINGLETON_FIXTURE") {
        let root = PathBuf::from(std::env::var_os("MEGA_SINGLETON_SCOPE").unwrap());
        if role == "admit" && is_main_args(&std::env::args().skip(1).collect::<Vec<_>>()) {
            match take_over(Some(&root)) {
                Ok(Some(admission)) => drop(admission),
                Ok(None) => return,
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(2);
                }
            }
        }
        fs::write(root.join(format!("ready-{}", std::process::id())), b"ready").unwrap();
        let mut spawned = false;
        loop {
            if role == "parent" && !spawned && root.join("spawn-child").exists() {
                use std::os::windows::process::CommandExt;
                let mut child =
                    std::process::Command::new(std::env::var_os("MEGA_SINGLETON_CHILD").unwrap())
                        .env("MEGA_SINGLETON_FIXTURE", "admit")
                        .creation_flags(0x08000000)
                        .spawn()
                        .unwrap();
                let status = child.wait().unwrap();
                fs::write(
                    root.join("child-exit"),
                    status.code().unwrap_or(-1).to_string(),
                )
                .unwrap();
                spawned = true;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    tests::run_all();
}

//! File transaction and rollback. Does not stop processes or read user data.
#[cfg(not(update_standalone))]
use super::update_manifest::{file_hash, plain_path, safe_relative, Manifest};
#[cfg(update_standalone)]
use super::{file_hash, plain_path, safe_relative, Manifest};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};

pub fn after_unlock<T, F: FnOnce() -> Result<(), String>>(lock: T, start: F) -> Result<(), String> {
    drop(lock);
    start()
}

fn load(path: &Path) -> Result<Value, String> {
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|_| "Повреждённый журнал обновления".into())
}
fn save(path: &Path, value: &Value) -> Result<(), String> {
    use std::io::Write;
    let serial = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temp = path.with_extension(format!("json.tmp-{}-{serial}", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    file.write_all(value.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    fs::rename(temp, path).map_err(|e| e.to_string())
}
fn map(value: &Value) -> Result<BTreeMap<String, String>, String> {
    let object = value
        .as_object()
        .filter(|v| v.len() <= 4096)
        .ok_or("Неверный inventory транзакции")?;
    let mut result = BTreeMap::new();
    for (name, hash) in object {
        safe_relative(name)?;
        let hash = hash
            .as_str()
            .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or("Неверный hash транзакции")?;
        result.insert(name.clone(), hash.to_owned());
    }
    Ok(result)
}
fn copy_atomic(from: &Path, to: &Path) -> Result<(), String> {
    plain_path(to)?;
    fs::create_dir_all(to.parent().ok_or("Нет каталога")?).map_err(|e| e.to_string())?;
    let serial = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temp = to.with_file_name(format!(
        "{}.update-tmp-{}-{serial}",
        to.file_name().ok_or("Нет имени")?.to_string_lossy(),
        std::process::id()
    ));
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|_| "Незавершённый временный файл сохранён")?;
    let mut source = fs::File::open(from).map_err(|e| e.to_string())?;
    std::io::copy(&mut source, &mut output).map_err(|e| e.to_string())?;
    output.sync_all().map_err(|e| e.to_string())?;
    drop(output);
    drop(source);
    let started = std::time::Instant::now();
    loop {
        match fs::rename(&temp, to) {
            Ok(()) => return Ok(()),
            Err(error) => {
                if started.elapsed() < std::time::Duration::from_secs(2) {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    continue;
                }
                if let (Ok(actual), Ok(expected)) = (file_hash(&temp), file_hash(from)) {
                    if actual == expected {
                        let _ = fs::remove_file(&temp);
                    }
                }
                return Err(error.to_string());
            }
        }
    }
}
fn files_under(path: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    plain_path(path)?;
    let mut result = Vec::new();
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let p = entry.map_err(|e| e.to_string())?.path();
        plain_path(&p)?;
        if p.is_dir() {
            result.extend(files_under(&p)?);
        } else {
            result.push(p);
        }
    }
    Ok(result)
}
fn verify_backup(path: &Path, partial: bool) -> Result<BTreeMap<String, String>, String> {
    let inventory = load(&path.join("backup-inventory.json"))?;
    let expected = map(&inventory["files"])?;
    for file in files_under(path)? {
        let name = file
            .strip_prefix(path)
            .map_err(|_| "Неверный путь backup")?
            .to_string_lossy()
            .replace('\\', "/");
        if name == "backup-inventory.json" {
            continue;
        }
        let hash = expected
            .get(&name)
            .ok_or("В backup есть посторонние файлы")?;
        if &file_hash(&file)? != hash {
            let root = path
                .parent()
                .and_then(Path::parent)
                .ok_or("Нет исходной папки backup")?;
            let source = root.join(&name);
            if !partial
                || file_hash(&source)? != *hash
                || !fs::read(source)
                    .map_err(|e| e.to_string())?
                    .starts_with(&fs::read(&file).map_err(|e| e.to_string())?)
            {
                return Err("Backup изменён; он сохранён".into());
            }
        }
    }
    if !partial {
        for (name, hash) in &expected {
            if file_hash(&path.join(name))? != *hash {
                return Err("Backup неполный".into());
            }
        }
    }
    Ok(expected)
}
fn remove_empty_tree(path: &Path) -> Result<(), String> {
    plain_path(path)?;
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let p = entry.map_err(|e| e.to_string())?.path();
        if p.is_dir() {
            remove_empty_tree(&p)?;
        } else {
            return Err("Непроверенный файл сохранён".into());
        }
    }
    fs::remove_dir(path).map_err(|e| e.to_string())
}
fn discard_backup(path: &Path, partial: bool) -> Result<(), String> {
    verify_backup(path, partial)?;
    for file in files_under(path)? {
        fs::remove_file(file).map_err(|e| e.to_string())?;
    }
    remove_empty_tree(path)
}
fn restore(root: &Path, journal: &Value) -> Result<(), String> {
    let backup = root.join(".updates/rollback-next");
    let old = verify_backup(&backup, false)?;
    let new = map(&journal["new"])?;
    for name in new.keys() {
        if !old.contains_key(name) {
            let p = root.join(name);
            if p.exists() {
                plain_path(&p)?;
                if file_hash(&p)? != new[name] {
                    return Err("Новый файл изменён; автоматический rollback остановлен".into());
                }
                fs::remove_file(p).map_err(|e| e.to_string())?;
            }
        }
    }
    for (name, hash) in &old {
        if file_hash(&root.join(name)).ok().as_ref() != Some(hash) {
            copy_atomic(&backup.join(name), &root.join(name))?;
        }
    }
    Ok(())
}
pub fn recover(root: &Path) -> Result<bool, String> {
    plain_path(root)?;
    let path = root.join(".updates/transaction.json");
    if !path.exists() {
        return Ok(false);
    }
    let mut journal = load(&path)?;
    match journal["phase"].as_str().ok_or("Нет фазы транзакции")? {
        "preparing" => {
            let backup = root.join(".updates/rollback-next");
            if backup.exists() {
                discard_backup(&backup, true)?;
            }
            journal["phase"] = json!("rolled_back");
            save(&path, &journal)?;
            Ok(true)
        }
        "replacing" | "health" => {
            restore(root, &journal)?;
            journal["phase"] = json!("rolled_back");
            save(&path, &journal)?;
            Ok(true)
        }
        "committed" | "rolled_back" => Ok(false),
        _ => Err("Неизвестная фаза обновления".into()),
    }
}
pub fn validate_recovery(root: &Path) -> Result<(), String> {
    let journal = load(&root.join(".updates/transaction.json"))?;
    if !matches!(
        journal["phase"].as_str(),
        Some("replacing") | Some("health")
    ) {
        return Err("Восстановление не требуется".into());
    }
    map(&journal["new"])?;
    verify_backup(&root.join(".updates/rollback-next"), false)?;
    Ok(())
}
pub fn apply<F: FnMut(&Path) -> Result<(), String>>(
    root: &Path,
    payload: &Path,
    manifest: &Manifest,
    mut health: F,
) -> Result<(), String> {
    plain_path(root)?;
    plain_path(payload)?;
    recover(root)?;
    for (name, hash) in &manifest.files {
        safe_relative(name)?;
        if file_hash(&payload.join(name))? != *hash {
            return Err("Подготовленный payload изменён".into());
        }
    }
    if files_under(payload)?.len() != manifest.files.len() {
        return Err("Посторонний файл в staging".into());
    }
    let old_manifest = load(&root.join("manifest.json"))?;
    if old_manifest["product"] != "RustDesk Agent Portable" {
        return Err("Выбран другой продукт".into());
    }
    let mut old = map(&old_manifest["files"])?;
    old.insert(
        "manifest.json".into(),
        file_hash(&root.join("manifest.json"))?,
    );
    for (name, hash) in &old {
        plain_path(&root.join(name))?;
        if file_hash(&root.join(name))? != *hash {
            return Err("Установленные файлы изменены; обновление остановлено".into());
        }
    }
    let update = root.join(".updates");
    fs::create_dir_all(&update).map_err(|e| e.to_string())?;
    plain_path(&update)?;
    let previous = update.join("previous");
    if previous.exists() {
        verify_backup(&previous, false)?;
    }
    let backup = update.join("rollback-next");
    if backup.exists() {
        discard_backup(&backup, true)?;
    }
    fs::create_dir(&backup).map_err(|e| e.to_string())?;
    save(&backup.join("backup-inventory.json"), &json!({"files":old}))?;
    let journal_path = update.join("transaction.json");
    let mut journal = json!({"schema":1,"phase":"preparing","old":old,"new":manifest.files,"targetVersion":manifest.version});
    save(&journal_path, &journal)?;
    for name in old.keys() {
        let path = backup.join(name);
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::copy(root.join(name), path).map_err(|e| e.to_string())?;
    }
    verify_backup(&backup, false)?;
    journal["phase"] = json!("replacing");
    save(&journal_path, &journal)?;
    let result = (|| -> Result<(), String> {
        for name in manifest.files.keys() {
            copy_atomic(&payload.join(name), &root.join(name))?;
        }
        for name in old.keys() {
            if !manifest.files.contains_key(name) {
                fs::remove_file(root.join(name)).map_err(|e| e.to_string())?;
            }
        }
        journal["phase"] = json!("health");
        save(&journal_path, &journal)?;
        health(root)?;
        Ok(())
    })();
    if let Err(error) = result {
        match restore(root, &journal) {
            Ok(()) => {
                journal["phase"] = json!("rolled_back");
                save(&journal_path, &journal)?;
                Err(format!("{error}. Предыдущая версия восстановлена"))
            }
            Err(rollback) => Err(format!("{error}. Rollback требует внимания: {rollback}")),
        }
    } else {
        journal["phase"] = json!("committed");
        save(&journal_path, &journal)?;
        let cleanup = (|| -> Result<(), String> {
            if previous.exists() {
                discard_backup(&previous, false)?;
            }
            fs::rename(&backup, &previous).map_err(|e| e.to_string())
        })();
        journal["cleanupCompleted"] = json!(cleanup.is_ok());
        if cleanup.is_err() {
            journal["cleanupWarning"] = json!("Предыдущий rollback сохранён: очистка не завершена");
        }
        save(&journal_path, &journal)?;
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::path::PathBuf;
    fn fixture() -> (PathBuf, PathBuf, Manifest) {
        let root = std::env::temp_dir().join(format!(
            "megarustdesk-install-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let payload = root.join(".updates/incoming/payload");
        fs::create_dir_all(&payload).unwrap();
        fs::create_dir(root.join("data")).unwrap();
        fs::write(root.join("data/keep.txt"), b"synthetic-user-data").unwrap();
        let mut old = BTreeMap::new();
        let mut new = BTreeMap::new();
        for name in ["RustDesk-Agent.exe", "sciter.dll"] {
            fs::write(root.join(name), b"old").unwrap();
            old.insert(name.to_owned(), file_hash(&root.join(name)).unwrap());
            fs::write(payload.join(name), b"new").unwrap();
            new.insert(name.to_owned(), file_hash(&payload.join(name)).unwrap());
        }
        fs::write(
            root.join("manifest.json"),
            json!({"product":"RustDesk Agent Portable","version":"0.2.3","files":old}).to_string(),
        )
        .unwrap();
        fs::write(
            payload.join("manifest.json"),
            json!({"product":"RustDesk Agent Portable","version":"0.3.0","files":new}).to_string(),
        )
        .unwrap();
        new.insert(
            "manifest.json".into(),
            file_hash(&payload.join("manifest.json")).unwrap(),
        );
        let manifest = Manifest {
            version: "0.3.0".into(),
            tag: "agent-v0.3.0".into(),
            asset_name: String::new(),
            asset_url: String::new(),
            size: 0,
            sha256: String::new(),
            files: new,
            raw: json!({}),
        };
        (root, payload, manifest)
    }
    fn cleanup(root: PathBuf) {
        assert!(
            root.starts_with(std::env::temp_dir())
                && root
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("megarustdesk-install-")
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg_attr(not(update_standalone), test)]
    fn applies_verified_files_and_preserves_data() {
        let (r, p, m) = fixture();
        apply(&r, &p, &m, |_| Ok(())).unwrap();
        assert_eq!(fs::read(r.join("RustDesk-Agent.exe")).unwrap(), b"new");
        assert_eq!(
            fs::read(r.join("data/keep.txt")).unwrap(),
            b"synthetic-user-data"
        );
        assert_eq!(
            fs::read(r.join(".updates/previous/RustDesk-Agent.exe")).unwrap(),
            b"old"
        );
        cleanup(r);
    }
    #[cfg_attr(not(update_standalone), test)]
    fn failed_health_restores_all_original_files() {
        let (r, p, m) = fixture();
        assert!(apply(&r, &p, &m, |_| Err("synthetic health failure".into())).is_err());
        assert_eq!(fs::read(r.join("RustDesk-Agent.exe")).unwrap(), b"old");
        assert_eq!(fs::read(r.join("sciter.dll")).unwrap(), b"old");
        assert_eq!(
            fs::read(r.join("data/keep.txt")).unwrap(),
            b"synthetic-user-data"
        );
        cleanup(r);
    }
    #[cfg_attr(not(update_standalone), test)]
    fn tampered_payload_never_replaces_current_version() {
        let (r, p, m) = fixture();
        fs::write(p.join("sciter.dll"), b"tampered").unwrap();
        assert!(apply(&r, &p, &m, |_| Ok(())).is_err());
        assert_eq!(fs::read(r.join("RustDesk-Agent.exe")).unwrap(), b"old");
        cleanup(r);
    }
    #[cfg_attr(not(update_standalone), test)]
    fn locked_file_rolls_back_without_stale_temporary() {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let (r, p, m) = fixture();
            let lock = fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(r.join("sciter.dll"))
                .unwrap();
            let error = apply(&r, &p, &m, |_| Ok(())).unwrap_err();
            drop(lock);
            assert!(error.contains("Предыдущая версия восстановлена"), "{error}");
            assert_eq!(fs::read(r.join("RustDesk-Agent.exe")).unwrap(), b"old");
            assert!(!r.join("sciter.dll.update-tmp").exists());
            cleanup(r);
        }
    }
    #[cfg_attr(not(update_standalone), test)]
    fn interrupted_replacement_is_recovered() {
        let (r, p, m) = fixture();
        apply(&r, &p, &m, |_| Ok(())).unwrap();
        let path = r.join(".updates/transaction.json");
        let mut journal = load(&path).unwrap();
        journal["phase"] = json!("health");
        fs::write(&path, journal.to_string()).unwrap();
        fs::rename(
            r.join(".updates/previous"),
            r.join(".updates/rollback-next"),
        )
        .unwrap();
        assert!(recover(&r).unwrap());
        assert_eq!(fs::read(r.join("RustDesk-Agent.exe")).unwrap(), b"old");
        cleanup(r);
    }
    #[cfg_attr(not(update_standalone), test)]
    fn interrupted_journal_temp_does_not_block_recovery() {
        let (r, p, m) = fixture();
        apply(&r, &p, &m, |_| Ok(())).unwrap();
        let path = r.join(".updates/transaction.json");
        let mut journal = load(&path).unwrap();
        journal["phase"] = json!("health");
        fs::write(&path, journal.to_string()).unwrap();
        fs::rename(
            r.join(".updates/previous"),
            r.join(".updates/rollback-next"),
        )
        .unwrap();
        fs::write(r.join(".updates/transaction.json.tmp"), b"partial journal").unwrap();
        let result = recover(&r);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(fs::read(r.join("RustDesk-Agent.exe")).unwrap(), b"old");
        cleanup(r);
    }
    #[cfg_attr(not(update_standalone), test)]
    fn partial_backup_keeps_unmodified_version_startable() {
        let (r, _p, _m) = fixture();
        let old = load(&r.join("manifest.json")).unwrap();
        let mut files = map(&old["files"]).unwrap();
        files.insert(
            "manifest.json".into(),
            file_hash(&r.join("manifest.json")).unwrap(),
        );
        let b = r.join(".updates/rollback-next");
        fs::create_dir(&b).unwrap();
        fs::write(
            b.join("backup-inventory.json"),
            json!({"files":files}).to_string(),
        )
        .unwrap();
        fs::write(b.join("RustDesk-Agent.exe"), b"o").unwrap();
        fs::write(
            r.join(".updates/transaction.json"),
            json!({"phase":"preparing"}).to_string(),
        )
        .unwrap();
        let result = recover(&r);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(fs::read(r.join("RustDesk-Agent.exe")).unwrap(), b"old");
        cleanup(r);
    }
    #[cfg_attr(not(update_standalone), test)]
    fn releases_lock_before_restart() {
        use std::os::windows::fs::OpenOptionsExt;
        let (r, _, _) = fixture();
        let path = r.join(".updates/dispatch.lock");
        let lock = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .share_mode(0)
            .open(&path)
            .unwrap();
        let result = after_unlock(lock, || {
            fs::OpenOptions::new()
                .write(true)
                .share_mode(0)
                .open(&path)
                .map(|_| ())
                .map_err(|e| e.to_string())
        });
        assert!(result.is_ok(), "{result:?}");
        cleanup(r);
    }
    #[cfg(update_standalone)]
    pub fn run_all() {
        let filter = std::env::var("UPDATER_TEST_FILTER").unwrap_or_default();
        let mut count = 0;
        for (name, test) in [
            (
                "unlock_before_restart",
                releases_lock_before_restart as fn(),
            ),
            (
                "apply_and_preserve",
                applies_verified_files_and_preserves_data as fn(),
            ),
            ("health_rollback", failed_health_restores_all_original_files),
            (
                "tampered_payload",
                tampered_payload_never_replaces_current_version,
            ),
            (
                "locked_file",
                locked_file_rolls_back_without_stale_temporary,
            ),
            ("journal_recovery", interrupted_replacement_is_recovered),
            (
                "journal_tmp",
                interrupted_journal_temp_does_not_block_recovery,
            ),
            (
                "partial_backup",
                partial_backup_keeps_unmodified_version_startable,
            ),
        ] {
            if !filter.is_empty() && filter != name {
                continue;
            }
            println!("RUN {name}");
            test();
            count += 1;
        }
        println!("Installer tests passed: {count}");
    }
}

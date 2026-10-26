//! Signed release metadata and strict portable ZIP inventory. No desktop or installation APIs.
#[cfg(not(update_standalone))]
use hbb_common::sodiumoxide;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sodiumoxide::crypto::sign;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

pub const REPOSITORY: &str = "URBoRTEX/MegaRustDesk";
pub const PREFIX: &str = "RustDesk-Agent-Portable-win-x64/";
pub const MAX_ARCHIVE: u64 = 128 * 1024 * 1024;
pub const MAX_EXPANDED: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Manifest {
    pub version: String,
    pub tag: String,
    pub asset_name: String,
    pub asset_url: String,
    pub size: u64,
    pub sha256: String,
    pub files: BTreeMap<String, String>,
    pub raw: Value,
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn decode_hex(text: &str, size: usize) -> Result<Vec<u8>, String> {
    if text.len() != size * 2 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Неверный формат подписи или hash".into());
    }
    (0..size)
        .map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).map_err(|_| "Неверный hex".into()))
        .collect()
}
pub fn version(value: &str) -> Result<[u32; 3], String> {
    let items = value.split('.').collect::<Vec<_>>();
    if items.len() != 3 {
        return Err("Неподдерживаемая версия".into());
    }
    let mut result = [0; 3];
    for (i, p) in items.iter().enumerate() {
        if p.is_empty()
            || !p.bytes().all(|c| c.is_ascii_digit())
            || (p.len() > 1 && p.starts_with('0'))
        {
            return Err("Неподдерживаемая версия".into());
        }
        result[i] = p.parse().map_err(|_| "Неподдерживаемая версия")?;
    }
    Ok(result)
}
pub fn safe_relative(name: &str) -> Result<(), String> {
    let roots = [
        "RustDesk-Agent.exe",
        "sciter.dll",
        "manifest.json",
        "source-state.json",
        "ПРОЧИТАЙТЕ.md",
        "Подключить MCP к Codex.cmd",
    ];
    if roots.contains(&name) {
        return Ok(());
    }
    if !name.starts_with("licenses/")
        || name.len() > 220
        || !name.is_ascii()
        || name.contains('\\')
        || name.contains(':')
    {
        return Err("Недопустимый путь в пакете".into());
    }
    for part in name.split('/') {
        let lower = part.to_ascii_lowercase();
        let base = lower.split('.').next().unwrap_or("");
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with('.')
            || part.ends_with(' ')
            || part
                .chars()
                .any(|c| c.is_control() || "<>\"|?*".contains(c))
            || ["con", "prn", "aux", "nul"].contains(&base)
            || (base.len() == 4
                && (base.starts_with("com") || base.starts_with("lpt"))
                && matches!(base.as_bytes()[3], b'1'..=b'9'))
            || [
                "data",
                "runtime",
                "auth.json",
                "config.toml",
                "history.jsonl",
                ".env",
            ]
            .contains(&lower.as_str())
            || [
                ".exe", ".dll", ".ps1", ".cmd", ".bat", ".sqlite", ".toml", ".log", ".png",
            ]
            .iter()
            .any(|extension| lower.ends_with(extension))
        {
            return Err("Запрещённый файл в пакете".into());
        }
    }
    Ok(())
}
fn string(v: &Value, key: &str) -> Result<String, String> {
    v[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("В manifest отсутствует {key}"))
}
pub fn verify_manifest(
    bytes: &[u8],
    signature: &str,
    public_key: &str,
    current: &str,
) -> Result<Manifest, String> {
    sodiumoxide::init().map_err(|_| "Не удалось инициализировать проверку подписи")?;
    if bytes.len() > 262_144 {
        return Err("Manifest превышает лимит".into());
    }
    let pk = sign::PublicKey::from_slice(&decode_hex(public_key.trim(), 32)?)
        .ok_or("Неверный ключ обновлений")?;
    let sig = sign::Signature::from_bytes(&decode_hex(signature.trim(), 64)?)
        .map_err(|_| "Неверная подпись обновления")?;
    if !sign::verify_detached(&sig, bytes, &pk) {
        return Err("Подпись обновления не подтверждена".into());
    }
    let raw: Value = serde_json::from_slice(bytes).map_err(|_| "Неверный JSON manifest")?;
    if raw["schema"] != 1
        || raw["product"] != "RustDesk Agent Portable"
        || raw["platform"] != "windows-x64"
        || raw["repository"] != REPOSITORY
    {
        return Err("Обновление предназначено другому продукту".into());
    }
    let target = string(&raw, "version")?;
    if version(&target)? <= version(current)? {
        return Err("Версия обновления не новее установленной".into());
    }
    let tag = string(&raw, "tag")?;
    if tag != format!("agent-v{target}") {
        return Err("Версия и тег не совпадают".into());
    }
    decode_hex(&string(&raw, "sourceCommit")?, 20)?;
    decode_hex(&string(&raw, "sourceBuildId")?, 32)?;
    let source = &raw["sourceAsset"];
    let source_name = format!("MegaRustDesk-{target}-source.zip");
    if string(source, "name")? != source_name
        || string(source, "url")?
            != format!("https://github.com/{REPOSITORY}/releases/download/{tag}/{source_name}")
        || source["size"]
            .as_u64()
            .filter(|n| *n > 0 && *n <= 512 * 1024 * 1024)
            .is_none()
    {
        return Err("Неверный архив соответствующих исходников".into());
    }
    decode_hex(&string(source, "sha256")?, 32)?;
    let asset = &raw["asset"];
    let asset_name = string(asset, "name")?;
    if asset_name != format!("MegaRustDesk-{target}-windows-x64.zip") {
        return Err("Неверное имя update asset".into());
    }
    let asset_url = string(asset, "url")?;
    if asset_url != format!("https://github.com/{REPOSITORY}/releases/download/{tag}/{asset_name}")
    {
        return Err("Неподтверждённый источник обновления".into());
    }
    let size = asset["size"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= MAX_ARCHIVE)
        .ok_or("Недопустимый размер обновления")?;
    let sha256 = string(asset, "sha256")?;
    decode_hex(&sha256, 32)?;
    let mut files = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let inventory = raw["files"]
        .as_object()
        .filter(|o| !o.is_empty() && o.len() <= 4096)
        .ok_or("Неверный inventory")?;
    for (name, hash) in inventory {
        safe_relative(name)?;
        if !seen.insert(name.to_lowercase()) {
            return Err("Повтор файла в inventory".into());
        }
        let hash = hash.as_str().ok_or("Неверный hash файла")?;
        decode_hex(hash, 32)?;
        files.insert(name.clone(), hash.to_owned());
    }
    for required in ["RustDesk-Agent.exe", "sciter.dll", "manifest.json"] {
        if !files.contains_key(required) {
            return Err("Неполный update package".into());
        }
    }
    Ok(Manifest {
        version: target,
        tag,
        asset_name,
        asset_url,
        size,
        sha256,
        files,
        raw,
    })
}
pub fn file_hash(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let size = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(hex(&hash.finalize()))
}
fn inventory(path: &Path, manifest: &Manifest, destination: Option<&Path>) -> Result<(), String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| "Не удалось открыть update ZIP")?;
    if archive.len() != manifest.files.len() || archive.len() > 4096 {
        return Err("В ZIP есть посторонние или отсутствующие файлы".into());
    }
    let mut seen = BTreeSet::new();
    let mut expanded = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|_| "Повреждённый ZIP entry")?;
        let name = entry
            .name()
            .strip_prefix(PREFIX)
            .ok_or("Неверный корень ZIP")?
            .to_owned();
        safe_relative(&name)?;
        let hash = manifest
            .files
            .get(&name)
            .ok_or("В ZIP есть файл вне подписанного inventory")?;
        if !seen.insert(name.to_lowercase())
            || entry.is_dir()
            || entry
                .unix_mode()
                .map_or(false, |mode| mode & 0o170000 == 0o120000)
            || entry.size() > MAX_ARCHIVE
        {
            return Err("Недопустимый ZIP entry")?;
        }
        expanded = expanded
            .checked_add(entry.size())
            .ok_or("ZIP size overflow")?;
        if expanded > MAX_EXPANDED {
            return Err("Распакованный пакет превышает лимит".into());
        }
        let declared = entry.size();
        let mut data = Vec::new();
        entry
            .by_ref()
            .take(declared + 1)
            .read_to_end(&mut data)
            .map_err(|_| "Ошибка чтения ZIP")?;
        if data.len() as u64 != declared || &hex(&Sha256::digest(&data)) != hash {
            return Err("Hash файла обновления не совпал".into());
        }
        if let Some(folder) = destination {
            let target = folder.join(&name);
            fs::create_dir_all(target.parent().ok_or("Нет каталога файла")?)
                .map_err(|e| e.to_string())?;
            let mut output = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&target)
                .map_err(|e| e.to_string())?;
            std::io::Write::write_all(&mut output, &data).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
pub fn verify_archive(path: &Path, manifest: &Manifest) -> Result<(), String> {
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size != manifest.size || size > MAX_ARCHIVE || file_hash(path)? != manifest.sha256 {
        return Err("Размер или SHA-256 update ZIP не совпал".into());
    }
    inventory(path, manifest, None)
}
pub fn plain_path(path: &Path) -> Result<(), String> {
    for ancestor in path.ancestors() {
        if let Ok(metadata) = fs::symlink_metadata(ancestor) {
            if metadata.file_type().is_symlink() {
                return Err("Обновление через ссылку запрещено".into());
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err("Каталог обновления содержит reparse point".into());
                }
            }
        }
    }
    Ok(())
}
pub fn extract_archive(path: &Path, manifest: &Manifest, destination: &Path) -> Result<(), String> {
    plain_path(destination)?;
    if destination.exists() {
        return Err("Update staging уже существует".into());
    }
    verify_archive(path, manifest)?;
    fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    inventory(path, manifest, Some(destination))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{io::Write, path::PathBuf};
    fn sample() -> Value {
        json!({"schema":1,"product":"RustDesk Agent Portable","platform":"windows-x64","repository":REPOSITORY,"version":"0.3.0","tag":"agent-v0.3.0","sourceCommit":"a".repeat(40),"sourceBuildId":"b".repeat(64),"sourceAsset":{"name":"MegaRustDesk-0.3.0-source.zip","url":"https://github.com/URBoRTEX/MegaRustDesk/releases/download/agent-v0.3.0/MegaRustDesk-0.3.0-source.zip","size":100,"sha256":"b".repeat(64)},"asset":{"name":"MegaRustDesk-0.3.0-windows-x64.zip","url":"https://github.com/URBoRTEX/MegaRustDesk/releases/download/agent-v0.3.0/MegaRustDesk-0.3.0-windows-x64.zip","size":100,"sha256":"a".repeat(64)},"files":{"RustDesk-Agent.exe":"a".repeat(64),"sciter.dll":"a".repeat(64),"manifest.json":"a".repeat(64)}})
    }
    fn signed(v: &Value, current: &str) -> Result<Manifest, String> {
        let (pk, sk) = sign::gen_keypair();
        let bytes = serde_json::to_vec(v).unwrap();
        verify_manifest(
            &bytes,
            &hex(&sign::sign_detached(&bytes, &sk).to_bytes()),
            &hex(&pk.0),
            current,
        )
    }
    #[cfg_attr(not(update_standalone), test)]
    fn signed_newer_release_is_accepted() {
        assert_eq!(signed(&sample(), "0.2.3").unwrap().version, "0.3.0");
    }
    #[cfg_attr(not(update_standalone), test)]
    fn unsigned_tampered_wrong_key_are_rejected() {
        let (pk, sk) = sign::gen_keypair();
        let bytes = serde_json::to_vec(&sample()).unwrap();
        let sig = hex(&sign::sign_detached(&bytes, &sk).to_bytes());
        assert!(verify_manifest(b"{}", &sig, &hex(&pk.0), "0.2.3").is_err());
        assert!(verify_manifest(&bytes, &"00".repeat(64), &hex(&pk.0), "0.2.3").is_err());
        assert!(verify_manifest(&bytes, &sig, &"00".repeat(32), "0.2.3").is_err());
    }
    #[cfg_attr(not(update_standalone), test)]
    fn wrong_identity_tag_url_and_downgrade_are_rejected() {
        for field in ["repository", "product", "platform", "tag", "version"] {
            let mut v = sample();
            v[field] = json!("wrong");
            assert!(signed(&v, "0.2.3").is_err());
        }
        let mut v = sample();
        v["asset"]["url"] = json!("https://example.test/evil.zip");
        assert!(signed(&v, "0.2.3").is_err());
        v = sample();
        v["sourceAsset"]["url"] = json!("https://example.test/source.zip");
        assert!(signed(&v, "0.2.3").is_err());
        v = sample();
        v["sourceAsset"] = Value::Null;
        assert!(signed(&v, "0.2.3").is_err());
        assert!(signed(&sample(), "0.3.0").is_err());
        assert!(signed(&sample(), "1.0.0").is_err());
    }
    #[cfg_attr(not(update_standalone), test)]
    fn unsafe_windows_paths_and_user_data_are_rejected() {
        for name in [
            "../evil",
            "C:/evil",
            "data/x",
            "runtime/codex.exe",
            "auth.json",
            "licenses/../x",
            "licenses/a:stream",
            "licenses/CON.txt",
            "licenses/file.",
            "licenses/foo\\bar",
            "licenses/a.exe",
            "licenses/foo/Config.toml",
            "licenses/aux",
        ] {
            assert!(safe_relative(name).is_err(), "accepted {name}");
        }
        for name in [
            "RustDesk-Agent.exe",
            "sciter.dll",
            "manifest.json",
            "licenses/serde/LICENSE-MIT",
            "ПРОЧИТАЙТЕ.md",
        ] {
            assert!(safe_relative(name).is_ok(), "rejected {name}");
        }
    }
    fn zip_fixture(extra: Option<(&str, &[u8])>) -> (PathBuf, Manifest) {
        let path = std::env::temp_dir().join(format!(
            "megarustdesk-zip-{}-{}.zip",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut m = Manifest {
            version: "0.3.0".into(),
            tag: "agent-v0.3.0".into(),
            asset_name: "fixture.zip".into(),
            asset_url: String::new(),
            size: 0,
            sha256: String::new(),
            files: BTreeMap::new(),
            raw: json!({}),
        };
        let mut z = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, bytes) in [
            ("RustDesk-Agent.exe", b"synthetic-exe".as_slice()),
            ("sciter.dll", b"synthetic-dll".as_slice()),
            ("manifest.json", b"{}".as_slice()),
        ] {
            z.start_file(
                format!("{PREFIX}{name}"),
                zip::write::FileOptions::default(),
            )
            .unwrap();
            z.write_all(bytes).unwrap();
            m.files.insert(name.into(), hex(&Sha256::digest(bytes)));
        }
        if let Some((name, bytes)) = extra {
            z.start_file(
                format!("{PREFIX}{name}"),
                zip::write::FileOptions::default(),
            )
            .unwrap();
            z.write_all(bytes).unwrap();
        }
        z.finish().unwrap();
        m.size = fs::metadata(&path).unwrap().len();
        m.sha256 = hex(&Sha256::digest(fs::read(&path).unwrap()));
        (path, m)
    }
    #[cfg_attr(not(update_standalone), test)]
    fn clean_inventory_passes_and_contamination_is_rejected() {
        let (p, m) = zip_fixture(None);
        assert!(verify_archive(&p, &m).is_ok());
        fs::remove_file(p).unwrap();
        for name in [
            "data/auth.json",
            "runtime/codex.exe",
            "licenses/extra.txt",
            "../escape",
            "RUSTDESK-AGENT.EXE",
        ] {
            let (p, m) = zip_fixture(Some((name, b"untrusted")));
            let result = verify_archive(&p, &m);
            fs::remove_file(p).unwrap();
            assert!(result.is_err(), "accepted {name}");
        }
    }
    #[cfg_attr(not(update_standalone), test)]
    fn hash_size_and_missing_files_fail() {
        let (p, mut m) = zip_fixture(None);
        m.sha256 = "0".repeat(64);
        assert!(verify_archive(&p, &m).is_err());
        m.sha256 = hex(&Sha256::digest(fs::read(&p).unwrap()));
        m.size += 1;
        assert!(verify_archive(&p, &m).is_err());
        m.size -= 1;
        m.files
            .insert("licenses/missing.txt".into(), "a".repeat(64));
        assert!(verify_archive(&p, &m).is_err());
        fs::remove_file(p).unwrap();
    }
    #[cfg_attr(not(update_standalone), test)]
    fn version_comparison_is_numeric() {
        assert!(version("0.10.0").unwrap() > version("0.9.0").unwrap());
        for v in ["0.3.0-beta", "03.0.0", "1.2", "1.2.3.4"] {
            assert!(version(v).is_err());
        }
    }
    #[cfg(update_standalone)]
    pub fn run_all() {
        let tests: &[(&str, fn())] = &[
            (
                "signed_newer_release_is_accepted",
                signed_newer_release_is_accepted,
            ),
            (
                "unsigned_tampered_wrong_key_are_rejected",
                unsigned_tampered_wrong_key_are_rejected,
            ),
            (
                "wrong_identity_tag_url_and_downgrade_are_rejected",
                wrong_identity_tag_url_and_downgrade_are_rejected,
            ),
            (
                "unsafe_windows_paths_and_user_data_are_rejected",
                unsafe_windows_paths_and_user_data_are_rejected,
            ),
            (
                "clean_inventory_passes_and_contamination_is_rejected",
                clean_inventory_passes_and_contamination_is_rejected,
            ),
            (
                "hash_size_and_missing_files_fail",
                hash_size_and_missing_files_fail,
            ),
            (
                "version_comparison_is_numeric",
                version_comparison_is_numeric,
            ),
        ];
        for (name, test) in tests {
            println!("RUN {name}");
            test();
        }
        println!("Updater security tests passed: {}", tests.len());
    }
}

#[cfg(all(update_standalone, test))]
#[path = "update_install.rs"]
mod update_install;
#[cfg(all(update_standalone, test))]
fn main() {
    tests::run_all();
    update_install::tests::run_all();
}

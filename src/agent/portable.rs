use std::path::PathBuf;

pub fn root() -> Result<PathBuf, String> {
    std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or("Не найдена папка приложения".into())
}

pub fn initialize() -> Result<(), String> {
    let root = root()?;
    super::update_manifest::verify_installation(
        &root,
        &std::env::current_exe().map_err(|e| e.to_string())?,
        super::updater::product_version(),
    )?;
    // Configure the absolute DLL path before any RustDesk/Sciter value is initialized.
    // Windows' hardened DLL search intentionally excludes PATH and the working directory.
    if !std::env::args().any(|a| a == "--agent-self-test") {
        sciter::set_library(&root.join("sciter.dll").to_string_lossy())
            .map_err(|e| format!("Не загружен интерфейс приложения: {e}"))?;
    }
    let data = root.join("data");
    std::fs::create_dir_all(&data)
        .map_err(|e| format!("Папка приложения недоступна для записи: {e}"))?;
    std::env::set_var("RUSTDESK_AGENT_DATA", &data);
    *hbb_common::config::APP_NAME.write().unwrap() = "RustDeskAgent".into();
    // Hard settings prevent accidental installation through the inherited RustDesk UI.
    for key in [
        "disable-installation",
        "disable-check-update",
        "disable-tcp-listen",
    ] {
        hbb_common::config::HARD_SETTINGS
            .write()
            .unwrap()
            .insert(key.into(), "Y".into());
    }
    // Portable access uses outbound relay connections instead of requesting an
    // inbound Windows Firewall exception. Never changes the firewall policy.
    for key in ["direct-server", "enable-lan-discovery"] {
        hbb_common::config::OVERWRITE_SETTINGS
            .write()
            .unwrap()
            .insert(key.into(), "N".into());
    }
    hbb_common::config::LocalConfig::set_option("lang".into(), "ru".into());
    #[cfg(windows)]
    unsafe {
        create_process_job()?;
    }
    Ok(())
}

#[cfg(windows)]
unsafe fn create_process_job() -> Result<(), String> {
    super::process_job::create()
}

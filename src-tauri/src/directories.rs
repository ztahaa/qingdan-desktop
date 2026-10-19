use std::{path::PathBuf, process::Command};

#[tauri::command]
pub async fn directory_pick() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new().set_title("选择任务关联的文件夹").pick_folder()
            .map(|path| path.to_string_lossy().into_owned())
    }).await.map_err(|_| "无法打开目录选择器".to_string())
}

#[tauri::command]
pub fn directory_open(path: String) -> Result<(), String> {
    let folder = PathBuf::from(path.trim());
    if !folder.is_absolute() || !folder.is_dir() {
        return Err("请输入存在的文件夹完整路径".into());
    }
    let folder = folder.canonicalize().map_err(|_| "无法访问此文件夹".to_string())?;
    // A single process argument; never interpret the path as shell commands or a URL.
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let folder = {
        let value = folder.to_string_lossy().into_owned();
        if let Some(unc) = value.strip_prefix(r"\\?\UNC\") { PathBuf::from(format!(r"\\{}", unc)) }
        else if let Some(local) = value.strip_prefix(r"\\?\") { PathBuf::from(local) }
        else { PathBuf::from(value) }
    };
    command.arg(folder).spawn().map_err(|_| "无法打开资源管理器".to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn tasks_export_save(content: String, format: String) -> Result<Option<String>, String> {
    if !matches!(format.as_str(), "csv" | "json") { return Err("不支持的导出格式".into()); }
    tauri::async_runtime::spawn_blocking(move || {
        let name = format!("清单事项-{}.{}", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"), format);
        let selected = rfd::FileDialog::new().set_title("导出清单事项").set_file_name(&name)
            .add_filter(if format == "csv" { "Excel CSV" } else { "完整事项 JSON" }, &[&format]).save_file();
        let path = match selected { Some(path) => path, None => return Ok(None) };
        let temp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        std::fs::write(&temp, content.as_bytes()).map_err(|_| "无法写入导出文件，请检查文件夹权限和磁盘空间".to_string())?;
        if std::fs::rename(&temp, &path).is_err() {
            let _ = std::fs::remove_file(&temp);
            return Err("无法保存导出文件，请关闭占用它的 Excel 后重试".into());
        }
        Ok(Some(path.to_string_lossy().into_owned()))
    }).await.map_err(|_| "导出任务未能完成".to_string())?
}

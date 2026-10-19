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
        let value = folder.to_string_lossy();
        if let Some(unc) = value.strip_prefix(r"\\?\UNC\") { PathBuf::from(format!(r"\\{}", unc)) }
        else if let Some(local) = value.strip_prefix(r"\\?\") { PathBuf::from(local) }
        else { folder }
    };
    command.arg(folder).spawn().map_err(|_| "无法打开资源管理器".to_string())?;
    Ok(())
}

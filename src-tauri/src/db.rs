use chrono::{DateTime, Local, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};
use tauri::State;
use uuid::Uuid;

pub struct AppState {
    pub db_path: PathBuf,
    pub backup_dir: PathBuf,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    id: String,
    name: String,
    color: String,
    sort_order: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInput {
    id: Option<String>,
    name: String,
    color: String,
    sort_order: Option<i64>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    id: String,
    title: String,
    description: String,
    status: String,
    priority: String,
    due_at: Option<String>,
    category_id: Option<String>,
    category_name: Option<String>,
    category_color: Option<String>,
    created_at: String,
    updated_at: String,
    completed_at: Option<String>,
    deleted_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    title: String,
    description: String,
    priority: String,
    due_at: Option<String>,
    category_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFilter {
    view: String,
    search: String,
    category_id: Option<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    today: usize,
    inbox: usize,
    upcoming: usize,
    all: usize,
    completed: usize,
    trash: usize,
    overdue: usize,
    completed_today: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    start_view: String,
    theme: String,
    daily_backup: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            start_view: "today".into(),
            theme: "light".into(),
            daily_backup: true,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    name: String,
    path: String,
    created_at: String,
    size: u64,
}

fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.pragma_update(None, "foreign_keys", "ON").map_err(|e| e.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL").map_err(|e| e.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(|e| e.to_string())?;
    Ok(conn)
}

pub fn initialize(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut conn = open(path)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS categories (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            color TEXT NOT NULL,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS tasks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'todo' CHECK(status IN ('todo','completed')),
            priority TEXT NOT NULL DEFAULT 'medium' CHECK(priority IN ('low','medium','high')),
            due_at TEXT,
            category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            completed_at TEXT,
            deleted_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status, deleted_at);
        CREATE INDEX IF NOT EXISTS idx_tasks_due ON tasks(due_at);
        CREATE INDEX IF NOT EXISTS idx_tasks_category ON tasks(category_id);
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );"
    ).map_err(|e| e.to_string())?;

    let migration_count: i64 = tx.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    let existing: i64 = tx.query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    if existing == 0 {
        let created = Utc::now().to_rfc3339();
        for (id, name, color, order) in [
            ("cat-work", "工作", "#5B8FF9", 0),
            ("cat-life", "生活", "#57C6A1", 1),
            ("cat-study", "学习", "#A078E8", 2),
        ] {
            tx.execute("INSERT INTO categories(id,name,color,sort_order,created_at) VALUES(?1,?2,?3,?4,?5)", params![id, name, color, order, created])
                .map_err(|e| e.to_string())?;
        }
    }
    let task_count: i64 = tx.query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    if task_count == 0 && migration_count == 0 {
        let now = Utc::now();
        let samples = [
            ("欢迎使用清单", "点击任务可以查看详情，也可以直接勾选完成。", "medium", now + chrono::Duration::hours(3), Some("cat-work")),
            ("创建你的第一个任务", "点击左侧的新建任务，设置截止时间和分类。", "high", now + chrono::Duration::days(1), Some("cat-life")),
            ("试试本地备份", "右上角的下载图标可以创建和恢复本地备份。", "low", now + chrono::Duration::days(3), Some("cat-study")),
        ];
        for (title, description, priority, due_at, category_id) in samples {
            let id = Uuid::new_v4().to_string();
            let stamp = now.to_rfc3339();
            tx.execute(
                "INSERT INTO tasks(id,title,description,status,priority,due_at,category_id,created_at,updated_at) VALUES(?1,?2,?3,'todo',?4,?5,?6,?7,?7)",
                params![id, title, description, priority, due_at.to_rfc3339(), category_id, stamp]
            ).map_err(|e| e.to_string())?;
        }
    }
    tx.execute("INSERT OR IGNORE INTO schema_migrations(version,applied_at) VALUES(1,?1)", [Utc::now().to_rfc3339()]).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

fn row_to_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?, title: row.get(1)?, description: row.get(2)?, status: row.get(3)?, priority: row.get(4)?,
        due_at: row.get(5)?, category_id: row.get(6)?, category_name: row.get(7)?, category_color: row.get(8)?,
        created_at: row.get(9)?, updated_at: row.get(10)?, completed_at: row.get(11)?, deleted_at: row.get(12)?,
    })
}

fn all_tasks(conn: &Connection) -> Result<Vec<Task>, String> {
    let mut stmt = conn.prepare(
        "SELECT t.id,t.title,t.description,t.status,t.priority,t.due_at,t.category_id,c.name,c.color,
                t.created_at,t.updated_at,t.completed_at,t.deleted_at
         FROM tasks t LEFT JOIN categories c ON c.id=t.category_id"
    ).map_err(|e| e.to_string())?;
    stmt.query_map([], row_to_task).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

fn local_date(value: &str) -> Option<chrono::NaiveDate> {
    DateTime::parse_from_rfc3339(value).ok().map(|date| date.with_timezone(&Local).date_naive())
}

fn local_time(value: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_rfc3339(value).ok().map(|date| date.with_timezone(&Local))
}

fn validate_input(input: &TaskInput) -> Result<(), String> {
    if input.title.trim().is_empty() { return Err("任务名称不能为空".into()); }
    if input.title.chars().count() > 120 { return Err("任务名称不能超过 120 个字符".into()); }
    if input.description.chars().count() > 1000 { return Err("任务说明不能超过 1000 个字符".into()); }
    if !matches!(input.priority.as_str(), "low" | "medium" | "high") { return Err("无效的优先级".into()); }
    if let Some(value) = &input.due_at { DateTime::parse_from_rfc3339(value).map_err(|_| "截止时间格式无效".to_string())?; }
    Ok(())
}

fn load_task(conn: &Connection, id: &str) -> Result<Task, String> {
    conn.query_row(
        "SELECT t.id,t.title,t.description,t.status,t.priority,t.due_at,t.category_id,c.name,c.color,
                t.created_at,t.updated_at,t.completed_at,t.deleted_at
         FROM tasks t LEFT JOIN categories c ON c.id=t.category_id WHERE t.id=?1",
        [id], row_to_task
    ).optional().map_err(|e| e.to_string())?.ok_or_else(|| "找不到任务".into())
}

#[tauri::command]
pub fn task_list(filter: TaskFilter, state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    let conn = open(&state.db_path)?;
    let now = Local::now();
    let today = now.date_naive();
    let next_week = today + chrono::Days::new(7);
    let query = filter.search.trim().to_lowercase();
    let mut tasks: Vec<Task> = all_tasks(&conn)?.into_iter().filter(|task| {
        let active = task.deleted_at.is_none() && task.status == "todo";
        let visible = match filter.view.as_str() {
            "today" => active && task.due_at.as_deref().and_then(local_date) == Some(today),
            "inbox" => active && task.category_id.is_none(),
            "upcoming" => active && task.due_at.as_deref().and_then(local_date).is_some_and(|d| d > today && d <= next_week),
            "all" => active,
            "completed" => task.deleted_at.is_none() && task.status == "completed",
            "trash" => task.deleted_at.is_some(),
            "category" => active && task.category_id == filter.category_id,
            _ => active,
        };
        let searchable = format!("{} {} {}", task.title, task.description, task.category_name.as_deref().unwrap_or("")).to_lowercase();
        visible && (query.is_empty() || searchable.contains(&query))
    }).collect();
    tasks.sort_by(|a, b| a.due_at.is_none().cmp(&b.due_at.is_none()).then_with(|| a.due_at.cmp(&b.due_at)).then_with(|| b.created_at.cmp(&a.created_at)));
    Ok(tasks)
}

#[tauri::command]
pub fn task_create(input: TaskInput, state: State<'_, AppState>) -> Result<Task, String> {
    validate_input(&input)?;
    let conn = open(&state.db_path)?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tasks(id,title,description,status,priority,due_at,category_id,created_at,updated_at) VALUES(?1,?2,?3,'todo',?4,?5,?6,?7,?7)",
        params![id, input.title.trim(), input.description.trim(), input.priority, input.due_at, input.category_id, now]
    ).map_err(|e| e.to_string())?;
    load_task(&conn, &id)
}

#[tauri::command]
pub fn task_update(task_id: String, input: TaskInput, state: State<'_, AppState>) -> Result<Task, String> {
    validate_input(&input)?;
    let conn = open(&state.db_path)?;
    let changed = conn.execute(
        "UPDATE tasks SET title=?2,description=?3,priority=?4,due_at=?5,category_id=?6,updated_at=?7 WHERE id=?1",
        params![task_id, input.title.trim(), input.description.trim(), input.priority, input.due_at, input.category_id, Utc::now().to_rfc3339()]
    ).map_err(|e| e.to_string())?;
    if changed == 0 { return Err("找不到任务".into()); }
    load_task(&conn, &task_id)
}

#[tauri::command]
pub fn task_complete(task_id: String, completed: bool, state: State<'_, AppState>) -> Result<(), String> {
    let conn = open(&state.db_path)?;
    let now = Utc::now().to_rfc3339();
    let changed = conn.execute(
        "UPDATE tasks SET status=?2,completed_at=?3,updated_at=?4 WHERE id=?1 AND deleted_at IS NULL",
        params![task_id, if completed { "completed" } else { "todo" }, if completed { Some(now.clone()) } else { None }, now]
    ).map_err(|e| e.to_string())?;
    if changed == 0 { Err("找不到任务".into()) } else { Ok(()) }
}

#[tauri::command]
pub fn task_delete(task_id: String, permanent: bool, state: State<'_, AppState>) -> Result<(), String> {
    let conn = open(&state.db_path)?;
    let changed = if permanent {
        conn.execute("DELETE FROM tasks WHERE id=?1 AND deleted_at IS NOT NULL", [&task_id])
    } else {
        conn.execute("UPDATE tasks SET deleted_at=?2,updated_at=?2 WHERE id=?1", params![task_id, Utc::now().to_rfc3339()])
    }.map_err(|e| e.to_string())?;
    if changed == 0 { Err("找不到任务，或任务不在回收站".into()) } else { Ok(()) }
}

#[tauri::command]
pub fn task_restore(task_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = open(&state.db_path)?;
    let changed = conn.execute("UPDATE tasks SET deleted_at=NULL,updated_at=?2 WHERE id=?1", params![task_id, Utc::now().to_rfc3339()]).map_err(|e| e.to_string())?;
    if changed == 0 { Err("找不到任务".into()) } else { Ok(()) }
}

#[tauri::command]
pub fn stats_get(state: State<'_, AppState>) -> Result<Stats, String> {
    let conn = open(&state.db_path)?;
    let now = Local::now();
    let today = now.date_naive();
    let next_week = today + chrono::Days::new(7);
    let mut stats = Stats::default();
    for task in all_tasks(&conn)? {
        if task.deleted_at.is_some() { stats.trash += 1; continue; }
        if task.status == "completed" {
            stats.completed += 1;
            if task.completed_at.as_deref().and_then(local_date) == Some(today) { stats.completed_today += 1; }
            continue;
        }
        stats.all += 1;
        if task.category_id.is_none() { stats.inbox += 1; }
        if let Some(due) = task.due_at.as_deref().and_then(local_time) {
            if due.date_naive() == today { stats.today += 1; }
            if due.date_naive() > today && due.date_naive() <= next_week { stats.upcoming += 1; }
            if due < now && due.date_naive() < today { stats.overdue += 1; }
        }
    }
    Ok(stats)
}

#[tauri::command]
pub fn category_list(state: State<'_, AppState>) -> Result<Vec<Category>, String> {
    let conn = open(&state.db_path)?;
    let mut stmt = conn.prepare("SELECT id,name,color,sort_order FROM categories ORDER BY sort_order,name").map_err(|e| e.to_string())?;
    stmt.query_map([], |row| Ok(Category { id: row.get(0)?, name: row.get(1)?, color: row.get(2)?, sort_order: row.get(3)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn category_save(input: CategoryInput, state: State<'_, AppState>) -> Result<Category, String> {
    if input.name.trim().is_empty() { return Err("分类名称不能为空".into()); }
    if !input.color.starts_with('#') || input.color.len() != 7 { return Err("分类颜色格式无效".into()); }
    let conn = open(&state.db_path)?;
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let sort_order = match input.sort_order {
        Some(value) => value,
        None => conn.query_row("SELECT sort_order FROM categories WHERE id=?1", [&id], |r| r.get(0))
            .optional().map_err(|e| e.to_string())?
            .unwrap_or(conn.query_row("SELECT COALESCE(MAX(sort_order),-1)+1 FROM categories", [], |r| r.get(0)).unwrap_or(0)),
    };
    conn.execute(
        "INSERT INTO categories(id,name,color,sort_order,created_at) VALUES(?1,?2,?3,?4,?5)
         ON CONFLICT(id) DO UPDATE SET name=excluded.name,color=excluded.color,sort_order=excluded.sort_order",
        params![id, input.name.trim(), input.color, sort_order, Utc::now().to_rfc3339()]
    ).map_err(|e| if e.to_string().contains("UNIQUE") { String::from("分类名称已存在") } else { e.to_string() })?;
    Ok(Category { id, name: input.name.trim().into(), color: input.color, sort_order })
}

#[tauri::command]
pub fn category_delete(category_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = open(&state.db_path)?;
    conn.execute("DELETE FROM categories WHERE id=?1", [category_id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let conn = open(&state.db_path)?;
    let value: Option<String> = conn.query_row("SELECT value FROM settings WHERE key='app'", [], |row| row.get(0)).optional().map_err(|e| e.to_string())?;
    value.map(|raw| serde_json::from_str(&raw).map_err(|e| e.to_string())).unwrap_or(Ok(AppSettings::default()))
}

pub fn daily_backup_enabled(path: &Path) -> Result<bool, String> {
    let conn = open(path)?;
    let value: Option<String> = conn.query_row("SELECT value FROM settings WHERE key='app'", [], |row| row.get(0)).optional().map_err(|e| e.to_string())?;
    Ok(value.and_then(|raw| serde_json::from_str::<AppSettings>(&raw).ok()).unwrap_or_default().daily_backup)
}

#[tauri::command]
pub fn settings_save(settings: AppSettings, state: State<'_, AppState>) -> Result<(), String> {
    let conn = open(&state.db_path)?;
    let value = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO settings(key,value) VALUES('app',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [value]).map_err(|e| e.to_string())?;
    Ok(())
}

fn create_backup(db_path: &Path, backup_dir: &Path) -> Result<BackupInfo, String> {
    fs::create_dir_all(backup_dir).map_err(|e| e.to_string())?;
    {
        let conn = open(db_path)?;
        conn.execute_batch("PRAGMA wal_checkpoint(FULL);").map_err(|e| e.to_string())?;
    }
    let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    let name = format!("qingdan-backup-{stamp}.sqlite3");
    let target = backup_dir.join(&name);
    fs::copy(db_path, &target).map_err(|e| e.to_string())?;
    let metadata = fs::metadata(&target).map_err(|e| e.to_string())?;
    Ok(BackupInfo { name, path: target.to_string_lossy().into(), created_at: Utc::now().to_rfc3339(), size: metadata.len() })
}

pub fn create_daily_backup_if_needed(db_path: &Path, backup_dir: &Path) -> Result<(), String> {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let exists = fs::read_dir(backup_dir).map_err(|e| e.to_string())?.filter_map(Result::ok)
        .any(|entry| entry.file_name().to_string_lossy().starts_with(&format!("qingdan-backup-{today}")));
    if !exists { create_backup(db_path, backup_dir)?; }
    Ok(())
}

#[tauri::command]
pub fn backup_create(state: State<'_, AppState>) -> Result<BackupInfo, String> {
    create_backup(&state.db_path, &state.backup_dir)
}

#[tauri::command]
pub fn backup_list(state: State<'_, AppState>) -> Result<Vec<BackupInfo>, String> {
    let backup_root = state.backup_dir.canonicalize().map_err(|e| e.to_string())?;
    let mut list = Vec::new();
    for entry in fs::read_dir(&backup_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("sqlite3") { continue; }
        let metadata = entry.metadata().map_err(|e| e.to_string())?;
        let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let created: DateTime<Utc> = modified.into();
        list.push(BackupInfo { name: entry.file_name().to_string_lossy().into(), path: path.to_string_lossy().into(), created_at: created.to_rfc3339(), size: metadata.len() });
    }
    list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(list)
}

#[tauri::command]
pub fn backup_restore(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let candidate = PathBuf::from(path).canonicalize().map_err(|e| e.to_string())?;
    let root = state.backup_dir.canonicalize().map_err(|e| e.to_string())?;
    if !candidate.starts_with(&root) || candidate.extension().and_then(|s| s.to_str()) != Some("sqlite3") {
        return Err("只能恢复应用备份目录中的 SQLite 文件".into());
    }
    let check = Connection::open(&candidate).map_err(|e| e.to_string())?;
    let integrity: String = check.query_row("PRAGMA quick_check", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    if integrity != "ok" { return Err("备份文件校验失败".into()); }
    drop(check);
    create_backup(&state.db_path, &state.backup_dir)?;
    let temp = state.db_path.with_extension("restore.tmp");
    fs::copy(&candidate, &temp).map_err(|e| e.to_string())?;
    for suffix in ["-wal", "-shm"] {
        let sidecar = PathBuf::from(format!("{}{}", state.db_path.display(), suffix));
        if sidecar.exists() { fs::remove_file(sidecar).map_err(|e| e.to_string())?; }
    }
    fs::copy(&temp, &state.db_path).map_err(|e| e.to_string())?;
    fs::remove_file(&temp).map_err(|e| e.to_string())?;
    initialize(&state.db_path)
}

#[tauri::command]
pub fn demo_reset(state: State<'_, AppState>) -> Result<(), String> {
    let conn = open(&state.db_path)?;
    conn.execute("UPDATE tasks SET deleted_at=?1", [Utc::now().to_rfc3339()]).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_schema_and_defaults() {
        let path = std::env::temp_dir().join(format!("qingdan-test-{}.sqlite3", Uuid::new_v4()));
        initialize(&path).unwrap();
        let conn = open(&path).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 3);
        drop(conn);
        let _ = fs::remove_file(path);
    }
}

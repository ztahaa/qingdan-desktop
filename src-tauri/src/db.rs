use chrono::{DateTime, Local, Utc};
use rusqlite::{params, Connection, DatabaseName, OptionalExtension, Row};
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
    pub operations: std::sync::Mutex<()>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskImage {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) data_url: String,
    pub(crate) created_at: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) status: String,
    pub(crate) priority: String,
    pub(crate) due_at: Option<String>,
    pub(crate) category_id: Option<String>,
    pub(crate) category_name: Option<String>,
    pub(crate) category_color: Option<String>,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) completed_at: Option<String>,
    pub(crate) deleted_at: Option<String>,
    pub(crate) images: Vec<TaskImage>,
    pub(crate) local_path: Option<String>,
    pub(crate) sms_reminder: bool,
    #[serde(default)]
    pub(crate) email_reminder: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    title: String,
    description: String,
    priority: String,
    due_at: Option<String>,
    category_id: Option<String>,
    #[serde(default)]
    images: Vec<TaskImage>,
    local_path: Option<String>,
    #[serde(default)]
    sms_reminder: bool,
    #[serde(default)]
    email_reminder: bool,
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
    unfinished: usize,
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
    #[serde(default)]
    pub sms_enabled: bool,
    #[serde(default)]
    pub sms_phone: String,
    #[serde(default)]
    pub sms_webhook_url: String,
    #[serde(default)]
    pub email_enabled: bool,
    #[serde(default)]
    pub email_address: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            start_view: "today".into(),
            theme: "light".into(),
            daily_backup: true,
            sms_enabled: false,
            sms_phone: String::new(),
            sms_webhook_url: String::new(),
            email_enabled: false,
            email_address: String::new(),
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

pub(crate) fn open(path: &Path) -> Result<Connection, String> {
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

    let columns: Vec<String> = {
        let mut statement = tx.prepare("PRAGMA table_info(tasks)").map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| row.get(1)).map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    for (name, sql) in [
        ("images_json", "ALTER TABLE tasks ADD COLUMN images_json TEXT NOT NULL DEFAULT '[]'"),
        ("local_path", "ALTER TABLE tasks ADD COLUMN local_path TEXT"),
        ("sms_reminder", "ALTER TABLE tasks ADD COLUMN sms_reminder INTEGER NOT NULL DEFAULT 0"),
        ("email_reminder", "ALTER TABLE tasks ADD COLUMN email_reminder INTEGER NOT NULL DEFAULT 0"),
    ] {
        if !columns.iter().any(|column| column == name) {
            tx.execute_batch(sql).map_err(|e| e.to_string())?;
        }
    }
    tx.execute_batch("CREATE TABLE IF NOT EXISTS sms_deliveries (
        delivery_key TEXT PRIMARY KEY, task_id TEXT NOT NULL, attempts INTEGER NOT NULL DEFAULT 0,
        sent_at TEXT, next_attempt TEXT NOT NULL, last_error TEXT
    );").map_err(|e| e.to_string())?;

    tx.execute_batch("CREATE TABLE IF NOT EXISTS email_deliveries (
        delivery_key TEXT PRIMARY KEY, task_id TEXT NOT NULL, recipient TEXT NOT NULL,
        state TEXT NOT NULL DEFAULT 'pending', attempts INTEGER NOT NULL DEFAULT 0,
        next_attempt TEXT NOT NULL, sent_at TEXT, last_error TEXT, message_id TEXT NOT NULL
    );").map_err(|e| e.to_string())?;

    let migration_count: i64 = tx.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    let existing: i64 = tx.query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    if existing == 0 && migration_count == 0 {
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
    tx.execute("INSERT OR IGNORE INTO schema_migrations(version,applied_at) VALUES(2,?1)", [Utc::now().to_rfc3339()]).map_err(|e| e.to_string())?;
    tx.execute("INSERT OR IGNORE INTO schema_migrations(version,applied_at) VALUES(3,?1)", [Utc::now().to_rfc3339()]).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

fn row_to_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?, title: row.get(1)?, description: row.get(2)?, status: row.get(3)?, priority: row.get(4)?,
        due_at: row.get(5)?, category_id: row.get(6)?, category_name: row.get(7)?, category_color: row.get(8)?,
        created_at: row.get(9)?, updated_at: row.get(10)?, completed_at: row.get(11)?, deleted_at: row.get(12)?,
        images: serde_json::from_str(&row.get::<_, String>(13)?).map_err(|e| rusqlite::Error::FromSqlConversionFailure(13, rusqlite::types::Type::Text, Box::new(e)))?,
        local_path: row.get(14)?, sms_reminder: row.get(15)?, email_reminder: row.get(16)?,
    })
}

fn all_tasks(conn: &Connection) -> Result<Vec<Task>, String> {
    let mut stmt = conn.prepare(
        "SELECT t.id,t.title,t.description,t.status,t.priority,t.due_at,t.category_id,c.name,c.color,
                t.created_at,t.updated_at,t.completed_at,t.deleted_at,t.images_json,t.local_path,t.sms_reminder,t.email_reminder
         FROM tasks t LEFT JOIN categories c ON c.id=t.category_id"
    ).map_err(|e| e.to_string())?;
    let tasks = stmt.query_map([], row_to_task).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(tasks)
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
    if input.images.len() > 6 { return Err("每项任务最多 6 张图片".into()); }
    for image in &input.images {
        if !image.data_url.starts_with("data:image/jpeg;base64,") || image.data_url.len() > 1024 * 1024 {
            return Err("图片格式或大小无效".into());
        }
    }
    if input.email_reminder && input.due_at.is_none() { return Err("邮件提醒需要截止时间".into()); }
    Ok(())
}

pub(crate) fn load_task(conn: &Connection, id: &str) -> Result<Task, String> {
    conn.query_row(
        "SELECT t.id,t.title,t.description,t.status,t.priority,t.due_at,t.category_id,c.name,c.color,
                t.created_at,t.updated_at,t.completed_at,t.deleted_at,t.images_json,t.local_path,t.sms_reminder,t.email_reminder
         FROM tasks t LEFT JOIN categories c ON c.id=t.category_id WHERE t.id=?1",
        [id], row_to_task
    ).optional().map_err(|e| e.to_string())?.ok_or_else(|| "找不到任务".into())
}

#[tauri::command]
pub fn task_list(filter: TaskFilter, state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
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
            "unfinished" => active,
            "all" => task.deleted_at.is_none(),
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
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
    let conn = open(&state.db_path)?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tasks(id,title,description,status,priority,due_at,category_id,created_at,updated_at,images_json,local_path,sms_reminder,email_reminder) VALUES(?1,?2,?3,'todo',?4,?5,?6,?7,?7,?8,?9,?10,?11)",
        params![id, input.title.trim(), input.description.trim(), input.priority, input.due_at, input.category_id, now, serde_json::to_string(&input.images).map_err(|e| e.to_string())?, input.local_path, input.sms_reminder, input.email_reminder]
    ).map_err(|e| e.to_string())?;
    load_task(&conn, &id)
}

#[tauri::command]
pub fn task_update(task_id: String, input: TaskInput, state: State<'_, AppState>) -> Result<Task, String> {
    validate_input(&input)?;
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
    let conn = open(&state.db_path)?;
    let changed = conn.execute(
        "UPDATE tasks SET title=?2,description=?3,priority=?4,due_at=?5,category_id=?6,updated_at=?7,images_json=?8,local_path=?9,sms_reminder=?10,email_reminder=?11 WHERE id=?1",
        params![task_id, input.title.trim(), input.description.trim(), input.priority, input.due_at, input.category_id, Utc::now().to_rfc3339(), serde_json::to_string(&input.images).map_err(|e| e.to_string())?, input.local_path, input.sms_reminder, input.email_reminder]
    ).map_err(|e| e.to_string())?;
    if changed == 0 { return Err("找不到任务".into()); }
    load_task(&conn, &task_id)
}

#[tauri::command]
pub fn task_complete(task_id: String, completed: bool, state: State<'_, AppState>) -> Result<(), String> {
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
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
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
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
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
    let conn = open(&state.db_path)?;
    let changed = conn.execute("UPDATE tasks SET deleted_at=NULL,updated_at=?2 WHERE id=?1", params![task_id, Utc::now().to_rfc3339()]).map_err(|e| e.to_string())?;
    if changed == 0 { Err("找不到任务".into()) } else { Ok(()) }
}

#[tauri::command]
pub fn stats_get(state: State<'_, AppState>) -> Result<Stats, String> {
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
    let conn = open(&state.db_path)?;
    let now = Local::now();
    let today = now.date_naive();
    let next_week = today + chrono::Days::new(7);
    let mut stats = Stats::default();
    for task in all_tasks(&conn)? {
        if task.deleted_at.is_some() { stats.trash += 1; continue; }
        stats.all += 1;
        if task.status == "completed" {
            stats.completed += 1;
            if task.completed_at.as_deref().and_then(local_date) == Some(today) { stats.completed_today += 1; }
            continue;
        }
        stats.unfinished += 1;
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
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
    let conn = open(&state.db_path)?;
    let mut stmt = conn.prepare("SELECT id,name,color,sort_order FROM categories ORDER BY sort_order,name").map_err(|e| e.to_string())?;
    let categories = stmt.query_map([], |row| Ok(Category { id: row.get(0)?, name: row.get(1)?, color: row.get(2)?, sort_order: row.get(3)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(categories)
}

#[tauri::command]
pub fn category_save(input: CategoryInput, state: State<'_, AppState>) -> Result<Category, String> {
    if input.name.trim().is_empty() { return Err("分类名称不能为空".into()); }
    if !input.color.starts_with('#') || input.color.len() != 7 { return Err("分类颜色格式无效".into()); }
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
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
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
    let conn = open(&state.db_path)?;
    conn.execute("DELETE FROM categories WHERE id=?1", [category_id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
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
    crate::reminders::validate_settings(&settings)?;
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
    let conn = open(&state.db_path)?;
    let value = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO settings(key,value) VALUES('app',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [value]).map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn create_backup(db_path: &Path, backup_dir: &Path) -> Result<BackupInfo, String> {
    fs::create_dir_all(backup_dir).map_err(|e| e.to_string())?;
    let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S-%f");
    let name = format!("qingdan-backup-{stamp}.sqlite3");
    let target = backup_dir.join(&name);
    let conn = open(db_path)?;
    conn.backup(DatabaseName::Main, &target, None).map_err(|e| e.to_string())?;
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
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
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
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
    let candidate = PathBuf::from(path).canonicalize().map_err(|e| e.to_string())?;
    let root = state.backup_dir.canonicalize().map_err(|e| e.to_string())?;
    if !candidate.starts_with(&root) || candidate.extension().and_then(|s| s.to_str()) != Some("sqlite3") {
        return Err("只能恢复应用备份目录中的 SQLite 文件".into());
    }
    let check = Connection::open(&candidate).map_err(|e| e.to_string())?;
    let integrity: String = check.query_row("PRAGMA quick_check", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    if integrity != "ok" { return Err("备份文件校验失败".into()); }
    check.prepare("SELECT id,title,description,status,priority,due_at,category_id,created_at,updated_at,completed_at,deleted_at FROM tasks LIMIT 0").map_err(|_| "备份不是兼容的清单数据库".to_string())?;
    check.prepare("SELECT id,name,color,sort_order FROM categories LIMIT 0").map_err(|_| "备份缺少分类数据".to_string())?;
    drop(check);
    create_backup(&state.db_path, &state.backup_dir)?;
    let mut conn = open(&state.db_path)?;
    conn.restore(DatabaseName::Main, &candidate, None::<fn(rusqlite::backup::Progress)>).map_err(|e| e.to_string())?;
    drop(conn);
    initialize(&state.db_path)?;
    crate::reminders::recover_interrupted(&open(&state.db_path)?)
}

#[tauri::command]
pub fn demo_reset(state: State<'_, AppState>) -> Result<(), String> {
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
    let conn = open(&state.db_path)?;
    conn.execute("UPDATE tasks SET deleted_at=?1", [Utc::now().to_rfc3339()]).map_err(|e| e.to_string())?;
    Ok(())
}


#[tauri::command]
pub fn task_delete_many(task_ids: Vec<String>, permanent: bool, state: State<'_, AppState>) -> Result<(), String> {
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败".to_string())?;
    let mut conn = open(&state.db_path)?;
    delete_many(&mut conn, &task_ids, permanent)
}

fn delete_many(conn: &mut Connection, task_ids: &[String], permanent: bool) -> Result<(), String> {
    if task_ids.len() > 10000 { return Err("一次最多删除 10000 项".into()); }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    for id in task_ids.iter().collect::<std::collections::HashSet<_>>() {
        let changed = if permanent {
            tx.execute("DELETE FROM tasks WHERE id=?1 AND deleted_at IS NOT NULL", [id])
        } else {
            tx.execute("UPDATE tasks SET deleted_at=?2,updated_at=?2 WHERE id=?1 AND deleted_at IS NULL", params![id, Utc::now().to_rfc3339()])
        }.map_err(|e| e.to_string())?;
        if changed != 1 { return Err("任务不存在或状态已变化，请刷新后重试".into()); }
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn prepare_upgrade(db_path: &Path, backup_dir: &Path) -> Result<(), String> {
    if !db_path.is_file() { return Ok(()); }
    let conn = open(db_path)?;
    let version: i64 = conn.query_row("SELECT COALESCE(MAX(version),0) FROM schema_migrations", [], |row| row.get(0)).unwrap_or(0);
    drop(conn);
    if version < 3 { create_backup(db_path, backup_dir)?; }
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

    #[test]
    fn upgrade_keeps_v1_records_and_backups_original_schema() {
        let root = std::env::temp_dir().join(format!("qingdan-upgrade-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("backups")).unwrap();
        let path = root.join("qingdan.sqlite3");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE categories(id TEXT PRIMARY KEY,name TEXT UNIQUE,color TEXT,sort_order INTEGER,created_at TEXT);
            CREATE TABLE tasks(id TEXT PRIMARY KEY,title TEXT,description TEXT,status TEXT,priority TEXT,due_at TEXT,category_id TEXT,created_at TEXT,updated_at TEXT,completed_at TEXT,deleted_at TEXT);
            CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT);
            CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY,applied_at TEXT);
            INSERT INTO schema_migrations VALUES(1,'2026-10-16');
            INSERT INTO categories VALUES('personal','我的项目','#123456',0,'2026-10-16');
            INSERT INTO tasks VALUES('keep','旧任务','完成了一半','todo','high',NULL,'personal','created','updated',NULL,NULL);
            INSERT INTO tasks VALUES('done','已完成的旧任务','','completed','low',NULL,NULL,'created','updated','completed',NULL);
            INSERT INTO tasks VALUES('trash','回收站旧任务','','todo','medium',NULL,NULL,'created','updated',NULL,'deleted');").unwrap();
        drop(conn);
        prepare_upgrade(&path, &root.join("backups")).unwrap();
        let backup = fs::read_dir(root.join("backups")).unwrap().next().unwrap().unwrap().path();
        let old = Connection::open(backup).unwrap();
        assert_eq!(old.query_row("SELECT MAX(version) FROM schema_migrations", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
        drop(old);
        initialize(&path).unwrap();
        initialize(&path).unwrap();
        let conn = open(&path).unwrap();
        let tasks = all_tasks(&conn).unwrap();
        assert_eq!(tasks.len(), 3);
        let keep = tasks.iter().find(|task| task.id == "keep").unwrap();
        assert_eq!(keep.title, "旧任务");
        assert_eq!(keep.category_id.as_deref(), Some("personal"));
        assert!(keep.images.is_empty());
        assert!(!keep.sms_reminder);
        assert_eq!(tasks.iter().filter(|t| t.status == "completed").count(), 1);
        assert_eq!(tasks.iter().filter(|t| t.deleted_at.is_some()).count(), 1);
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
        drop(conn);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn batch_delete_rolls_back_and_backups_include_attachments() {
        let root = std::env::temp_dir().join(format!("qingdan-batch-{}", Uuid::new_v4()));
        fs::create_dir_all(root.join("backups")).unwrap();
        let path = root.join("qingdan.sqlite3");
        initialize(&path).unwrap();
        let mut conn = open(&path).unwrap();
        let id = all_tasks(&conn).unwrap()[0].id.clone();
        let images = r#"[{"id":"img","name":"快照.jpg","dataUrl":"data:image/jpeg;base64,aA==","createdAt":"2026-10-19"}]"#;
        conn.execute("UPDATE tasks SET images_json=?2,local_path=?3,sms_reminder=1 WHERE id=?1", params![id, images, r"D:\项目资料"]).unwrap();
        assert!(delete_many(&mut conn, &[id.clone(), "missing".into()], false).is_err());
        assert!(load_task(&conn, &id).unwrap().deleted_at.is_none());
        assert!(delete_many(&mut conn, &[id.clone()], true).is_err());
        delete_many(&mut conn, &[id.clone()], false).unwrap();
        let backup = create_backup(&path, &root.join("backups")).unwrap();
        delete_many(&mut conn, &[id.clone()], true).unwrap();
        assert!(load_task(&conn, &id).is_err());
        conn.restore(DatabaseName::Main, &backup.path, None::<fn(rusqlite::backup::Progress)>).unwrap();
        let restored = load_task(&conn, &id).unwrap();
        assert_eq!(restored.images[0].name, "快照.jpg");
        assert_eq!(restored.local_path.as_deref(), Some(r"D:\项目资料"));
        assert!(restored.sms_reminder);
        assert!(restored.deleted_at.is_some());
        drop(conn);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn deleted_all_categories_are_not_reseeded_on_upgrade() {
        let path = std::env::temp_dir().join(format!("qingdan-empty-{}.sqlite3", Uuid::new_v4()));
        initialize(&path).unwrap();
        let conn = open(&path).unwrap();
        conn.execute("DELETE FROM tasks", []).unwrap();
        conn.execute("DELETE FROM categories", []).unwrap();
        drop(conn);
        initialize(&path).unwrap();
        let conn = open(&path).unwrap();
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get::<_, i64>(0)).unwrap(), 0);
        assert!(all_tasks(&conn).unwrap().is_empty());
        drop(conn);
        let _ = fs::remove_file(path);
    }


    #[test]
    fn upgrade_v11_preserves_snapshots_without_converting_sms_consent() {
        let root = std::env::temp_dir().join(format!("qingdan-v11-{}", Uuid::new_v4()));
        let path = root.join("qingdan.sqlite3");
        initialize(&path).unwrap();
        let conn = open(&path).unwrap();
        let id = all_tasks(&conn).unwrap()[0].id.clone();
        conn.execute("UPDATE tasks SET sms_reminder=1,local_path='D:/keep' WHERE id=?1", [&id]).unwrap();
        conn.execute_batch("ALTER TABLE tasks DROP COLUMN email_reminder; DROP TABLE email_deliveries; DELETE FROM schema_migrations WHERE version=3;").unwrap();
        drop(conn);
        prepare_upgrade(&path, &root.join("backups")).unwrap();
        initialize(&path).unwrap();
        initialize(&path).unwrap();
        let conn = open(&path).unwrap();
        let task = load_task(&conn, &id).unwrap();
        assert!(task.sms_reminder);
        assert!(!task.email_reminder);
        assert_eq!(task.local_path.as_deref(), Some("D:/keep"));
        assert_eq!(conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| row.get::<_, i64>(0)).unwrap(), 3);
        assert!(!AppSettings::default().email_enabled);
        drop(conn);
        fs::remove_dir_all(root).unwrap();
    }

}


use crate::db::{self, AppSettings, AppState};
use chrono::{DateTime, Duration, Utc};
use lettre::{message::header::ContentType, transport::smtp::authentication::Credentials, Message, SmtpTransport, Transport};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::{Manager, State};
use uuid::Uuid;

const SERVICE: &str = "com.qingdan.desktop.qq-smtp";

pub fn normalize_address(value: &str) -> Result<String, String> {
    let address = value.trim().to_ascii_lowercase();
    let parsed: lettre::Address = address.parse().map_err(|_| "请输入完整 QQ 邮箱地址，例如 123456@qq.com".to_string())?;
    if parsed.domain() != "qq.com" { return Err("此版本请使用 @qq.com 邮箱".into()); }
    Ok(address)
}

fn normalize_code(value: &str) -> Result<String, String> {
    let code = value.trim();
    if !(8..=128).contains(&code.len()) || !code.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err("请输入 QQ 邮箱生成的 SMTP 授权码，不是登录密码".into());
    }
    Ok(code.to_owned())
}

fn read_code(address: &str) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let entry = keyring::Entry::new(SERVICE, address).map_err(|_| "无法访问 Windows 凭据管理器")?;
        entry.get_password().map_err(|_| "此邮箱尚未保存授权码，或凭据无法读取，请重新填写授权码".into())
    }
    #[cfg(not(target_os = "windows"))]
    { let _ = (SERVICE, address); Err("邮件发送需要 Windows 桌面版".into()) }
}

pub fn validate_settings(settings: &AppSettings) -> Result<(), String> {
    if !settings.email_address.trim().is_empty() { normalize_address(&settings.email_address)?; }
    if settings.email_enabled { read_code(&normalize_address(&settings.email_address)?)?; }
    Ok(())
}

#[tauri::command]
pub fn email_auth_save(address: String, auth_code: String, state: State<'_, AppState>) -> Result<(), String> {
    let address = normalize_address(&address)?;
    let code = normalize_code(&auth_code)?;
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败")?;
    #[cfg(target_os = "windows")]
    {
        keyring::Entry::new(SERVICE, &address).map_err(|_| "无法访问 Windows 凭据管理器")?
            .set_password(&code).map_err(|_| "授权码未能保存到 Windows 凭据管理器")?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    { let _ = (address, code); Err("授权码保存需要 Windows 桌面版".into()) }
}

#[tauri::command]
pub fn email_auth_clear(address: String, state: State<'_, AppState>) -> Result<(), String> {
    let address = normalize_address(&address)?;
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败")?;
    #[cfg(target_os = "windows")]
    {
        let entry = keyring::Entry::new(SERVICE, &address).map_err(|_| "无法访问 Windows 凭据管理器")?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("无法删除已保存的授权码".into()),
        }
    }
    #[cfg(not(target_os = "windows"))]
    { let _ = address; Err("授权码管理需要 Windows 桌面版".into()) }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailStatus {
    configured: bool,
    sent: i64,
    failed: i64,
    uncertain: i64,
    last_error: Option<String>,
}

#[tauri::command]
pub fn email_status(address: String, state: State<'_, AppState>) -> Result<EmailStatus, String> {
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败")?;
    let conn = db::open(&state.db_path)?;
    let address = normalize_address(&address).unwrap_or_default();
    let count = |kind: &str| conn.query_row("SELECT COUNT(*) FROM email_deliveries WHERE state=?1 AND recipient=?2", params![kind, address], |row| row.get(0)).map_err(|e| e.to_string());
    let last_error = conn.query_row("SELECT last_error FROM email_deliveries WHERE recipient=?1 AND last_error IS NOT NULL ORDER BY rowid DESC LIMIT 1", [&address], |row| row.get(0)).optional().map_err(|e| e.to_string())?.flatten();
    Ok(EmailStatus { configured: !address.is_empty() && read_code(&address).is_ok(), sent: count("sent")?, failed: count("failed")?, uncertain: count("uncertain")?, last_error })
}

#[tauri::command]
pub fn email_retry_failed(state: State<'_, AppState>) -> Result<usize, String> {
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败")?;
    let conn = db::open(&state.db_path)?;
    let settings = load_settings(&conn)?;
    let address = normalize_address(&settings.email_address)?;
    conn.execute("UPDATE email_deliveries SET state='pending',attempts=0,next_attempt=?1,last_error=NULL WHERE state='failed' AND recipient=?2", params![Utc::now().to_rfc3339(), address]).map_err(|e| e.to_string())
}

#[derive(Debug, Clone)]
enum SendFailure { Temporary, Permanent, Uncertain, Content(crate::mail_content::ContentError) }
impl SendFailure {
    fn message(&self) -> &'static str {
        match self {
            Self::Temporary => "邮件服务暂时拒绝，将自动重试；请检查网络或稍后再试",
            Self::Permanent => "邮箱服务拒绝发送，请检查 SMTP 是否开启、授权码是否正确及邮箱发送限制",
            Self::Uncertain => "连接中断或超时，发送结果不确定；请先检查收件箱，系统不会自动重发以避免重复",
            Self::Content(error) => error.message(),
        }
    }
}
trait Sender { fn send(&self, address: &str, code: &str, message: &Message) -> Result<(), SendFailure>; }
struct QqSender;
impl Sender for QqSender {
    fn send(&self, address: &str, code: &str, message: &Message) -> Result<(), SendFailure> {
        let mailer = SmtpTransport::relay("smtp.qq.com").map_err(|_| SendFailure::Permanent)?
            .port(465).timeout(Some(std::time::Duration::from_secs(15)))
            .credentials(Credentials::new(address.to_owned(), code.to_owned())).build();
        mailer.send(message).map(|_| ()).map_err(|error| {
            if error.is_transient() { SendFailure::Temporary }
            else if error.is_permanent() || error.is_client() || error.is_tls() { SendFailure::Permanent }
            else { SendFailure::Uncertain }
        })
    }
}

fn message(address: &str, title: &str, body: &str, message_id: &str) -> Result<Message, String> {
    let mailbox = address.parse::<lettre::message::Mailbox>().map_err(|_| "邮箱地址无效")?;
    Message::builder().from(mailbox.clone()).to(mailbox)
        .message_id(Some(format!("<qingdan-{message_id}@qingdan.local>")))
        .subject(title.replace(['\r', '\n'], " ")).header(ContentType::TEXT_PLAIN)
        .body(body.to_owned()).map_err(|_| "无法生成提醒邮件".into())
}

#[tauri::command]
pub async fn email_test(state: State<'_, AppState>) -> Result<String, String> {
    let (address, code) = {
        let _guard = state.operations.lock().map_err(|_| "数据操作锁失败")?;
        let settings = load_settings(&db::open(&state.db_path)?)?;
        let address = normalize_address(&settings.email_address)?;
        let code = read_code(&address)?;
        (address, code)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let mail = message(&address, "清单 · 邮件提醒测试", "这是一封由你在清单中点击“发送测试邮件”产生的测试邮件。收到此邮件说明 SMTP 连接与授权码可用。任务提醒需要保持电脑和清单运行，并在任务中勾选邮件提醒。", &Uuid::new_v4().to_string())?;
        QqSender.send(&address, &code, &mail).map_err(|error| error.message().to_string())?;
        Ok("QQ 邮箱已接受测试邮件，请检查收件箱和垃圾邮件；手机提示取决于邮箱 App 的通知设置".into())
    }).await.map_err(|_| "测试发送任务未能完成".to_string())?
}

fn load_settings(conn: &Connection) -> Result<AppSettings, String> {
    let raw: Option<String> = conn.query_row("SELECT value FROM settings WHERE key='app'", [], |row| row.get(0)).optional().map_err(|e| e.to_string())?;
    raw.map(|value| serde_json::from_str(&value).map_err(|e| e.to_string())).unwrap_or(Ok(AppSettings::default()))
}

struct Delivery { key: String, task_id: String, message_id: String }
fn claim_next(conn: &Connection, address: &str, now: DateTime<Utc>) -> Result<Option<Delivery>, String> {
    let candidates: Vec<(String, String, String)> = {
        let mut statement = conn.prepare("SELECT id,title,due_at FROM tasks WHERE status='todo' AND deleted_at IS NULL AND email_reminder=1 AND due_at IS NOT NULL ORDER BY due_at").map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    for (task_id, _title, due_at) in candidates {
        let due = match DateTime::parse_from_rfc3339(&due_at) { Ok(value) => value.with_timezone(&Utc), Err(_) => continue };
        if due > now || due < now - Duration::hours(24) { continue; }
        let key = format!("qq-email:{task_id}:{due_at}:{address}");
        conn.execute("INSERT OR IGNORE INTO email_deliveries(delivery_key,task_id,recipient,next_attempt,message_id) VALUES(?1,?2,?3,?4,?5)", params![key, task_id, address, now.to_rfc3339(), Uuid::new_v4().to_string()]).map_err(|e| e.to_string())?;
        let (kind, attempts, next, message_id): (String, i64, String, String) = conn.query_row("SELECT state,attempts,next_attempt,message_id FROM email_deliveries WHERE delivery_key=?1", [&key], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))).map_err(|e| e.to_string())?;
        if kind != "pending" || attempts >= 3 || next > now.to_rfc3339() { continue; }
        let changed = conn.execute("UPDATE email_deliveries SET state='sending',attempts=attempts+1 WHERE delivery_key=?1 AND state='pending'", [&key]).map_err(|e| e.to_string())?;
        if changed == 1 { return Ok(Some(Delivery { key, task_id, message_id })); }
    }
    Ok(None)
}

fn finish(conn: &Connection, delivery: &Delivery, result: Result<(), SendFailure>, now: DateTime<Utc>) -> Result<(), String> {
    let (kind, error, sent) = match result {
        Ok(()) => ("sent", None, Some(now.to_rfc3339())),
        Err(SendFailure::Temporary) => {
            let attempts: i64 = conn.query_row("SELECT attempts FROM email_deliveries WHERE delivery_key=?1", [&delivery.key], |row| row.get(0)).map_err(|e| e.to_string())?;
            (if attempts < 3 { "pending" } else { "failed" }, Some(SendFailure::Temporary.message()), None)
        }
        Err(SendFailure::Permanent) => ("failed", Some(SendFailure::Permanent.message()), None),
        Err(SendFailure::Uncertain) => ("uncertain", Some(SendFailure::Uncertain.message()), None),
        Err(SendFailure::Content(ref error)) => ("failed", Some(error.message()), None),
    };
    conn.execute("UPDATE email_deliveries SET state=?2,last_error=?3,sent_at=?4,next_attempt=?5 WHERE delivery_key=?1", params![delivery.key, kind, error, sent, (now + Duration::seconds(60)).to_rfc3339()]).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        // Never re-send an interrupted SMTP transaction automatically: it may have reached QQ.
        {
            let state = app.state::<AppState>();
            if let Ok(_guard) = state.operations.lock() {
                if let Ok(conn) = db::open(&state.db_path) {
                    let _ = recover_interrupted(&conn);
                }
            };
        }
        loop {
            let _ = deliver_next(&app);
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
    });
}
pub(crate) fn recover_interrupted(conn: &Connection) -> Result<(), String> {
    conn.execute("UPDATE email_deliveries SET state='uncertain',last_error='上次发送时应用中断，请先检查收件箱；不自动重发' WHERE state='sending'", []).map_err(|e| e.to_string())?;
    Ok(())
}
fn deliver_next(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (delivery, address, code, task) = {
        let _guard = state.operations.lock().map_err(|_| "数据操作锁失败")?;
        let conn = db::open(&state.db_path)?;
        let settings = load_settings(&conn)?;
        if !settings.email_enabled { return Ok(()); }
        let address = normalize_address(&settings.email_address)?;
        let code = read_code(&address)?;
        let delivery = match claim_next(&conn, &address, Utc::now())? { Some(value) => value, None => return Ok(()) };
        let task = db::load_task(&conn, &delivery.task_id)?;
        (delivery, address, code, task)
    };
    // A complete task snapshot is loaded under the data lock, then encoded/sent outside it.
    let result = match crate::mail_content::task_message(&address, &task, &delivery.message_id) {
        Ok(mail) => QqSender.send(&address, &code, &mail),
        Err(error) => Err(SendFailure::Content(error)),
    };
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败")?;
    finish(&db::open(&state.db_path)?, &delivery, result, Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture() -> (PathBuf, Connection) {
        let path = std::env::temp_dir().join(format!("qingdan-mail-{}.sqlite3", Uuid::new_v4()));
        db::initialize(&path).unwrap();
        let conn = db::open(&path).unwrap();
        conn.execute("DELETE FROM tasks", []).unwrap();
        (path, conn)
    }
    fn add(conn: &Connection, id: &str, due: DateTime<Utc>, status: &str, deleted: bool, enabled: bool) {
        conn.execute("INSERT INTO tasks(id,title,description,status,priority,due_at,created_at,updated_at,deleted_at,email_reminder) VALUES(?1,?2,'',?3,'medium',?4,?4,?4,?5,?6)", params![id, format!("事项-{id}"), status, due.to_rfc3339(), if deleted { Some("deleted") } else { None }, enabled]).unwrap();
    }
    fn cleanup(path: PathBuf, conn: Connection) {
        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn only_opted_in_due_active_tasks_are_claimed_and_sent_once() {
        let (path, conn) = fixture();
        let now = Utc::now();
        add(&conn, "future", now + Duration::hours(1), "todo", false, true);
        add(&conn, "old", now - Duration::hours(25), "todo", false, true);
        add(&conn, "done", now - Duration::minutes(1), "completed", false, true);
        add(&conn, "trash", now - Duration::minutes(1), "todo", true, true);
        add(&conn, "disabled", now - Duration::minutes(1), "todo", false, false);
        add(&conn, "due", now - Duration::minutes(1), "todo", false, true);
        let delivery = claim_next(&conn, "123456@qq.com", now).unwrap().unwrap();
        assert!(delivery.key.contains(":due:"));
        assert!(claim_next(&conn, "123456@qq.com", now).unwrap().is_none());
        finish(&conn, &delivery, Ok(()), now).unwrap();
        assert!(claim_next(&conn, "123456@qq.com", now + Duration::minutes(1)).unwrap().is_none());
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM email_deliveries WHERE state='sent'", [], |row| row.get::<_, i64>(0)).unwrap(), 1);
        cleanup(path, conn);
    }

    #[test]
    fn transient_retries_stop_after_three_and_reuse_message_id() {
        let (path, conn) = fixture();
        let now = Utc::now();
        add(&conn, "retry", now - Duration::minutes(1), "todo", false, true);
        let first = claim_next(&conn, "123456@qq.com", now).unwrap().unwrap();
        let mut previous = first;
        for attempt in 0..3 {
            let time = now + Duration::minutes(attempt);
            finish(&conn, &previous, Err(SendFailure::Temporary), time).unwrap();
            assert!(claim_next(&conn, "123456@qq.com", time).unwrap().is_none());
            if attempt < 2 {
                let next = claim_next(&conn, "123456@qq.com", time + Duration::minutes(1)).unwrap().unwrap();
                assert_eq!(next.message_id, previous.message_id);
                previous = next;
            }
        }
        assert!(claim_next(&conn, "123456@qq.com", now + Duration::minutes(5)).unwrap().is_none());
        assert_eq!(conn.query_row("SELECT state FROM email_deliveries", [], |row| row.get::<_, String>(0)).unwrap(), "failed");
        cleanup(path, conn);
    }

    #[test]
    fn uncertain_or_interrupted_smtp_is_not_automatically_duplicated() {
        let (path, conn) = fixture();
        let now = Utc::now();
        add(&conn, "uncertain", now - Duration::minutes(1), "todo", false, true);
        let delivery = claim_next(&conn, "123456@qq.com", now).unwrap().unwrap();
        finish(&conn, &delivery, Err(SendFailure::Uncertain), now).unwrap();
        assert!(claim_next(&conn, "123456@qq.com", now + Duration::minutes(5)).unwrap().is_none());
        // A changed due time is a new user-requested reminder.
        conn.execute("UPDATE tasks SET due_at=?1 WHERE id='uncertain'", [(now - Duration::seconds(30)).to_rfc3339()]).unwrap();
        let next = claim_next(&conn, "123456@qq.com", now).unwrap().unwrap();
        assert_ne!(next.key, delivery.key);
        recover_interrupted(&conn).unwrap();
        assert!(claim_next(&conn, "123456@qq.com", now + Duration::minutes(5)).unwrap().is_none());
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM email_deliveries WHERE state='uncertain'", [], |row| row.get::<_, i64>(0)).unwrap(), 2);
        cleanup(path, conn);
    }

    #[test]
    fn validates_qq_accounts_and_builds_self_addressed_test_mail() {
        assert_eq!(normalize_address(" 123456@QQ.COM ").unwrap(), "123456@qq.com");
        assert!(normalize_address("name@example.com").is_err());
        assert!(normalize_address("123@qq.com\r\nBcc: other@qq.com").is_err());
        assert!(normalize_code(" ").is_err());
        let mail = message("123456@qq.com", "test\r\nsubject", "hello", "unit-test").unwrap();
        let bytes = String::from_utf8(mail.formatted()).unwrap();
        assert!(bytes.contains("To: 123456@qq.com"));
        assert!(bytes.contains("From: 123456@qq.com"));
        assert!(bytes.contains("<qingdan-unit-test@qingdan.local>"));
        assert!(!bytes.contains("\r\nBcc:"));
    }

    #[test]
    fn failed_full_content_is_recorded_as_failed_and_not_sent() {
        let (path, conn) = fixture();
        let now = Utc::now();
        add(&conn, "bad-content", now - Duration::minutes(1), "todo", false, true);
        let delivery = claim_next(&conn, "123456@qq.com", now).unwrap().unwrap();
        finish(&conn, &delivery, Err(SendFailure::Content(crate::mail_content::ContentError::InvalidImage)), now).unwrap();
        let (state, error, sent): (String, String, Option<String>) = conn.query_row("SELECT state,last_error,sent_at FROM email_deliveries", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap();
        assert_eq!(state, "failed");
        assert!(error.contains("图片"));
        assert!(sent.is_none());
        assert!(claim_next(&conn, "123456@qq.com", now + Duration::minutes(5)).unwrap().is_none());
        cleanup(path, conn);
    }

}

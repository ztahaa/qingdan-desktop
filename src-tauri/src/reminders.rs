use crate::db::{self, AppSettings, AppState};
use chrono::{DateTime, Duration, Utc};
use reqwest::{blocking::Client, redirect::Policy, Url};
use rusqlite::{params, OptionalExtension};
use tauri::{Manager, State};

pub fn validate_settings(settings: &AppSettings) -> Result<(), String> {
    if !settings.sms_enabled { return Ok(()); }
    let phone = settings.sms_phone.strip_prefix('+').ok_or("手机号需带国际区号，例如 +8613800000000")?;
    if !(8..=15).contains(&phone.len()) || !phone.bytes().all(|b| b.is_ascii_digit()) {
        return Err("手机号格式无效".into());
    }
    let url = Url::parse(&settings.sms_webhook_url).map_err(|_| "短信服务地址无效")?;
    if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        return Err("短信中转服务需使用 HTTPS 地址，请勿在地址中放入账号密码".into());
    }
    if std::env::var("QINGDAN_SMS_TOKEN").unwrap_or_default().trim().is_empty() {
        return Err("请先设置电脑环境变量 QINGDAN_SMS_TOKEN，再重启应用".into());
    }
    Ok(())
}

#[tauri::command]
pub fn sms_status(state: State<'_, AppState>) -> Result<String, String> {
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败")?;
    let conn = db::open(&state.db_path)?;
    let failed: i64 = conn.query_row("SELECT COUNT(*) FROM sms_deliveries WHERE sent_at IS NULL AND attempts >= 3", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    let sent: i64 = conn.query_row("SELECT COUNT(*) FROM sms_deliveries WHERE sent_at IS NOT NULL", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    let token = if std::env::var("QINGDAN_SMS_TOKEN").unwrap_or_default().trim().is_empty() { "密钥未配置" } else { "密钥已配置" };
    Ok(format!("{token}；中转服务已接收 {sent} 条，失败 {failed} 条。最终送达以短信服务商回执为准。关闭应用或电脑后不发送；重新打开仅补发 24 小时内到期的任务。"))
}

pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let client = match Client::builder().timeout(std::time::Duration::from_secs(15)).redirect(Policy::none()).build() {
            Ok(client) => client,
            Err(_) => return,
        };
        loop {
            // SQLite connections and the operation lock are dropped before sleeping.
            let _ = deliver_next(&app, &client);
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
    });
}

fn deliver_next(app: &tauri::AppHandle, client: &Client) -> Result<(), String> {
    let state = app.state::<AppState>();
    let _guard = state.operations.lock().map_err(|_| "数据操作锁失败")?;
    let conn = db::open(&state.db_path)?;
    let raw: Option<String> = conn.query_row("SELECT value FROM settings WHERE key='app'", [], |row| row.get(0)).optional().map_err(|e| e.to_string())?;
    let settings: AppSettings = match raw { Some(raw) => serde_json::from_str(&raw).map_err(|e| e.to_string())?, None => return Ok(()) };
    if !settings.sms_enabled || validate_settings(&settings).is_err() { return Ok(()); }
    let now = Utc::now();
    let tasks: Vec<(String, String, String)> = {
        let mut statement = conn.prepare("SELECT id,title,due_at FROM tasks WHERE status='todo' AND deleted_at IS NULL AND sms_reminder=1 AND due_at IS NOT NULL ORDER BY due_at").map_err(|e| e.to_string())?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).map_err(|e| e.to_string())?;
        rows.collect::<Result<_, _>>().map_err(|e| e.to_string())?
    };
    for (task_id, title, due_at) in tasks {
        let due = match DateTime::parse_from_rfc3339(&due_at) { Ok(due) => due.with_timezone(&Utc), Err(_) => continue };
        if due > now || due < now - Duration::hours(24) { continue; }
        // Stable across restarts, retries and backup restores. The relay MUST deduplicate this key.
        let key = format!("qingdan:{task_id}:{due_at}:{}", settings.sms_phone);
        conn.execute("INSERT OR IGNORE INTO sms_deliveries(delivery_key,task_id,next_attempt) VALUES(?1,?2,?3)", params![key, task_id, now.to_rfc3339()]).map_err(|e| e.to_string())?;
        let (attempts, sent, next): (i64, Option<String>, String) = conn.query_row("SELECT attempts,sent_at,next_attempt FROM sms_deliveries WHERE delivery_key=?1", [&key], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).map_err(|e| e.to_string())?;
        if sent.is_some() || attempts >= 3 || next > now.to_rfc3339() { continue; }
        // Record attempts before sending, including a delay after a crash or uncertain network result.
        let next_attempt = (now + Duration::seconds(60 * (attempts + 1))).to_rfc3339();
        conn.execute("UPDATE sms_deliveries SET attempts=attempts+1,next_attempt=?2 WHERE delivery_key=?1", params![key, next_attempt]).map_err(|e| e.to_string())?;
        let response = client.post(&settings.sms_webhook_url)
            .bearer_auth(std::env::var("QINGDAN_SMS_TOKEN").unwrap_or_default())
            .json(&serde_json::json!({"idempotencyKey": key, "phone": settings.sms_phone, "taskId": task_id, "title": title, "dueAt": due_at}))
            .send();
        // Only explicit acceptance is success. No redirects or response-body logging (could contain secrets).
        let accepted = response.as_ref().is_ok_and(|response| matches!(response.status().as_u16(), 200 | 201 | 202 | 204));
        if accepted {
            conn.execute("UPDATE sms_deliveries SET sent_at=?2,last_error=NULL WHERE delivery_key=?1", params![key, Utc::now().to_rfc3339()]).map_err(|e| e.to_string())?;
        } else {
            conn.execute("UPDATE sms_deliveries SET last_error='中转服务未确认接收' WHERE delivery_key=?1", [&key]).map_err(|e| e.to_string())?;
        }
        break;
    }
    Ok(())
}

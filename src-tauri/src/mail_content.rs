use crate::db::Task;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Local};
use lettre::{message::{header::ContentType, Attachment, MultiPart, SinglePart}, Message};

// Application guard, not a promise about the provider's delivery limit. Never omit attachments to fit.
const MAX_MAIL_BYTES: usize = 25 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) enum ContentError { InvalidImage, ImageTooLarge, TooLarge, Build }
impl ContentError {
    pub(crate) fn message(&self) -> &'static str {
        match self {
            Self::InvalidImage => "任务中有图片数据损坏或格式不支持，邮件未发送；请重新添加图片后重试失败提醒",
            Self::ImageTooLarge => "任务单张图片超过保存格式允许的大小，邮件未发送；请压缩并重新添加该图片后重试",
            Self::TooLarge => "完整邮件超过应用的 25 MB 上限，邮件未发送；请压缩任务图片后重试，系统不会省略图片",
            Self::Build => "无法生成完整任务邮件，邮件未发送；请检查任务内容后重试",
        }
    }
}

fn escaped(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}
fn text(value: &str) -> String { if value.is_empty() { "（空）".into() } else { value.into() } }
fn optional(value: &Option<String>) -> String { value.as_deref().map(text).unwrap_or_else(|| "（未设置）".into()) }
fn time(value: &str) -> String {
    DateTime::parse_from_rfc3339(value).map(|date| format!("{}（原值：{}）", date.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S %:z"), value)).unwrap_or_else(|_| text(value))
}
fn optional_time(value: &Option<String>) -> String { value.as_deref().map(time).unwrap_or_else(|| "（未设置）".into()) }

pub(crate) fn task_fields(task: &Task) -> Vec<(&'static str, String)> {
    vec![
        ("事项 ID", text(&task.id)), ("事项名称", text(&task.title)), ("事项说明", text(&task.description)),
        ("完成状态", match task.status.as_str() { "completed" => "已完成（completed）".into(), "todo" => "未完成（todo）".into(), _ => text(&task.status) }),
        ("优先级", match task.priority.as_str() { "high" => "高（high）".into(), "medium" => "中（medium）".into(), "low" => "低（low）".into(), _ => text(&task.priority) }),
        ("截止时间", optional_time(&task.due_at)), ("分类", task.category_name.clone().unwrap_or_else(|| "收集箱".into())),
        ("分类 ID", optional(&task.category_id)), ("分类颜色", optional(&task.category_color)),
        ("创建时间", time(&task.created_at)), ("更新时间", time(&task.updated_at)),
        ("完成时间", optional_time(&task.completed_at)), ("删除时间", optional_time(&task.deleted_at)),
        ("截止时邮件提醒", if task.email_reminder { "已开启".into() } else { "未开启".into() }),
        ("旧版短信标记（仅兼容记录）", if task.sms_reminder { "是".into() } else { "否".into() }),
        ("关联本地目录（完整路径）", optional(&task.local_path)), ("图片数量", task.images.len().to_string()),
    ]
}

fn image_bytes(data_url: &str) -> Result<(Vec<u8>, ContentType, &'static str), ContentError> {
    if data_url.len() > 1024 * 1024 { return Err(ContentError::ImageTooLarge); }
    let (header, payload) = data_url.split_once(",").ok_or(ContentError::InvalidImage)?;
    let (mime, extension) = match header {
        "data:image/jpeg;base64" => ("image/jpeg", "jpg"),
        "data:image/png;base64" => ("image/png", "png"),
        "data:image/webp;base64" => ("image/webp", "webp"),
        "data:image/gif;base64" => ("image/gif", "gif"),
        "data:image/bmp;base64" => ("image/bmp", "bmp"),
        _ => return Err(ContentError::InvalidImage),
    };
    let bytes = STANDARD.decode(payload).map_err(|_| ContentError::InvalidImage)?;
    let recognizable = match extension {
        "jpg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "webp" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(&b"WEBP"[..]),
        "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "bmp" => bytes.starts_with(b"BM"),
        _ => false,
    };
    if !recognizable { return Err(ContentError::InvalidImage); }
    Ok((bytes, ContentType::parse(mime).map_err(|_| ContentError::InvalidImage)?, extension))
}

// Visible HTML and plain text are generated from exactly the same field list.
// Only task data is accepted here, so application settings/SMTP secrets cannot be serialized by accident.
pub(crate) fn task_message(address: &str, task: &Task, message_id: &str) -> Result<Message, ContentError> {
    let fields = task_fields(task);
    let mut plain = String::from("你在清单中设置的事项已到期。以下为发送时保存的完整任务内容：\n\n");
    let mut html = String::from("<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"></head><body style=\"font-family:Arial,Microsoft YaHei,sans-serif;color:#24334b;line-height:1.6\"><h2>清单 · 完整事项提醒</h2><p>以下为发送时保存的完整任务内容：</p><table style=\"border-collapse:collapse;width:100%;max-width:850px\">");
    for (label, value) in &fields {
        plain.push_str(&format!("{label}：{value}\n\n"));
        html.push_str(&format!("<tr><th style=\"border:1px solid #dce3ed;padding:9px;text-align:left;vertical-align:top;white-space:nowrap\">{}</th><td style=\"border:1px solid #dce3ed;padding:9px;white-space:pre-wrap;overflow-wrap:anywhere\">{}</td></tr>", escaped(label), escaped(value)));
    }
    html.push_str("</table><p style=\"color:#778399\">目录为电脑上的关联路径；手机不能直接访问该电脑目录。图片使用任务中已保存的版本，完整任务 JSON 和所有图片均随邮件附带。</p>");
    plain.push_str("目录为电脑上的关联路径，手机不能直接访问该电脑目录。附件包含完整任务 JSON 和所有已保存图片。\n\n");
    let mut images = Vec::new();
    for (index, image) in task.images.iter().enumerate() {
        let (bytes, mime, extension) = image_bytes(&image.data_url)?;
        // Generated names avoid path traversal, duplicate names and attachment header injection.
        let filename = format!("task-image-{:02}.{}", index + 1, extension);
        let cid = format!("qingdan-image-{}@qingdan.local", index + 1);
        let caption = format!("图片 {}：{}\n图片 ID：{}\n保存时间：{}\n附件文件名：{}", index + 1, text(&image.name), image.id, time(&image.created_at), filename);
        plain.push_str(&format!("{caption}\n\n"));
        html.push_str(&format!("<section><h3>图片 {}</h3><p style=\"white-space:pre-wrap;overflow-wrap:anywhere\">{}</p><img src=\"cid:{}\" alt=\"{}\" style=\"max-width:100%;height:auto;border:1px solid #dce3ed\"></section>", index + 1, escaped(&caption), cid, escaped(&image.name)));
        images.push((filename, cid, bytes, mime));
    }
    html.push_str("</body></html>");
    let mut related = MultiPart::related().singlepart(SinglePart::html(html));
    for (filename, cid, bytes, mime) in &images {
        related = related.singlepart(Attachment::new_inline_with_name(cid.clone(), filename.clone()).body(bytes.clone(), mime.clone()));
    }
    let alternatives = MultiPart::alternative().singlepart(SinglePart::plain(plain)).multipart(related);
    let mut mixed = MultiPart::mixed().multipart(alternatives);
    // Keep a normal downloadable attachment even when the client hides or blocks inline images.
    for (filename, _, bytes, mime) in images {
        mixed = mixed.singlepart(Attachment::new(filename).body(bytes, mime));
    }
    let json = serde_json::to_vec_pretty(task).map_err(|_| ContentError::Build)?;
    mixed = mixed.singlepart(Attachment::new("task-details.json".into()).body(json, ContentType::parse("application/json").map_err(|_| ContentError::Build)?));
    let mailbox = address.parse::<lettre::message::Mailbox>().map_err(|_| ContentError::Build)?;
    let mail = Message::builder().from(mailbox.clone()).to(mailbox)
        .subject(format!("清单到期提醒：{}", task.title.replace(['\r', '\n'], " ")))
        .message_id(Some(format!("<qingdan-{message_id}@qingdan.local>")))
        .multipart(mixed).map_err(|_| ContentError::Build)?;
    if mail.formatted().len() > MAX_MAIL_BYTES { return Err(ContentError::TooLarge); }
    Ok(mail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::TaskImage;
    use mailparse::MailHeaderMap;

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=";
    fn task() -> Task {
        Task {
            id: "full-task-001".into(), title: "中文任务 <script>alert(1)</script>".into(),
            description: "第一行：工作做到一半\n第二行：含引号\" 和 & 以及 <img src=\"https://invalid/\">".into(),
            status: "todo".into(), priority: "high".into(), due_at: Some("2026-10-19T09:00:00+08:00".into()),
            category_id: Some("cat-personal".into()), category_name: Some("我的分类".into()), category_color: Some("#123ABC".into()),
            created_at: "2026-10-18T08:00:00+08:00".into(), updated_at: "2026-10-19T08:59:00+08:00".into(),
            completed_at: None, deleted_at: None, local_path: Some(r"D:\我的项目\阶段 1".into()), sms_reminder: true, email_reminder: true,
            images: vec![
                TaskImage { id: "image-one".into(), name: "同名快照.png".into(), data_url: format!("data:image/png;base64,{PNG}"), created_at: "2026-10-19T08:30:00+08:00".into() },
                TaskImage { id: "image-two".into(), name: "同名快照.png".into(), data_url: format!("data:image/png;base64,{PNG}"), created_at: "2026-10-19T08:40:00+08:00".into() },
            ],
        }
    }
    struct Part { mime: String, body: Vec<u8>, disposition: mailparse::ParsedContentDisposition, cid: Option<String> }
    fn leaves(mail: &mailparse::ParsedMail<'_>) -> Vec<Part> {
        if mail.subparts.is_empty() {
            vec![Part { mime: mail.ctype.mimetype.clone(), body: mail.get_body_raw().unwrap(), disposition: mail.get_content_disposition(), cid: mail.headers.get_first_value("Content-ID") }]
        } else { mail.subparts.iter().flat_map(leaves).collect() }
    }

    #[test]
    fn full_task_mime_preserves_every_field_directory_and_all_images() {
        let task = task();
        let mail = task_message("123456@qq.com", &task, "complete-test").unwrap();
        let formatted = mail.formatted();
        let parsed = mailparse::parse_mail(&formatted).unwrap();
        let parts = leaves(&parsed);
        let plain = String::from_utf8(parts.iter().find(|p| p.mime == "text/plain").unwrap().body.clone()).unwrap();
        let html = String::from_utf8(parts.iter().find(|p| p.mime == "text/html").unwrap().body.clone()).unwrap();
        for (label, value) in task_fields(&task) {
            assert!(plain.contains(label), "missing field: {label}");
            assert!(plain.contains(&value.replace('\n', "\r\n")) || plain.contains(&value), "missing field value: {label}");
        }
        assert!(plain.contains(r"D:\我的项目\阶段 1"));
        assert!(plain.contains("第一行：工作做到一半"));
        assert!(plain.contains("第二行：含引号"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<img src=\"https://invalid/\">"));
        let picture_parts: Vec<_> = parts.iter().filter(|p| p.mime == "image/png").collect();
        assert_eq!(picture_parts.len(), 4); // Two inline and two normal attachments.
        let expected = STANDARD.decode(PNG).unwrap();
        for part in &picture_parts { assert_eq!(part.body, expected); }
        for index in 1..=2 {
            let filename = format!("task-image-{index:02}.png");
            let downloaded = picture_parts.iter().filter(|p| p.disposition.disposition == mailparse::DispositionType::Attachment && p.disposition.params.get("filename") == Some(&filename)).count();
            assert_eq!(downloaded, 1);
            let cid = format!("qingdan-image-{index}@qingdan.local");
            assert!(html.contains(&format!("cid:{cid}")));
            let expected_cid = format!("<{cid}>");
            assert!(picture_parts.iter().any(|p| p.cid.as_deref() == Some(expected_cid.as_str())));
        }
        let details = parts.iter().find(|p| p.mime == "application/json").unwrap();
        assert_eq!(details.disposition.params.get("filename").map(String::as_str), Some("task-details.json"));
        let value: serde_json::Value = serde_json::from_slice(&details.body).unwrap();
        assert_eq!(value, serde_json::to_value(&task).unwrap());
        assert_eq!(value["images"].as_array().unwrap().len(), 2);
        assert!(value.get("settings").is_none());
        assert!(value.get("authCode").is_none());
        assert!(value.get("emailAddress").is_none());
    }

    #[test]
    fn no_images_or_directory_still_produces_complete_valid_mime() {
        let mut task = task();
        task.images.clear();
        task.local_path = None;
        task.description.clear();
        let mail = task_message("123456@qq.com", &task, "empty-test").unwrap();
        let bytes = mail.formatted();
        let parsed = mailparse::parse_mail(&bytes).unwrap();
        let parts = leaves(&parsed);
        assert!(!parts.iter().any(|p| p.mime.starts_with("image/")));
        assert!(parts.iter().any(|p| p.mime == "application/json"));
        let text = String::from_utf8(parts.iter().find(|p| p.mime == "text/plain").unwrap().body.clone()).unwrap();
        assert!(text.contains("关联本地目录（完整路径）"));
        assert!(text.contains("（未设置）"));
        assert!(text.contains("（空）"));
    }

    #[test]
    fn corrupted_or_oversized_images_fail_instead_of_being_dropped() {
        let mut task = task();
        task.images[1].data_url = "data:image/png;base64,not valid".into();
        assert!(matches!(task_message("123456@qq.com", &task, "broken"), Err(ContentError::InvalidImage)));
        task.images[1].data_url = "data:text/html;base64,PGgxPnRlc3Q8L2gxPg==".into();
        assert!(matches!(task_message("123456@qq.com", &task, "type"), Err(ContentError::InvalidImage)));
        task.images[1].data_url = "data:image/jpeg;base64,YQ==".into();
        assert!(matches!(task_message("123456@qq.com", &task, "not-image"), Err(ContentError::InvalidImage)));
        task.images[1].data_url = "x".repeat(1024 * 1024 + 1);
        assert!(matches!(task_message("123456@qq.com", &task, "large"), Err(ContentError::ImageTooLarge)));
    }
}

# 清单 v1.2.0：QQ 邮件提醒与一键导出

- 用 QQ 邮箱 SMTP（smtp.qq.com:465，SSL/TLS）直接给自己发送到期提醒，不需要短信中转服务。
- 设置中填写 QQ 邮箱和 SMTP 授权码，启用邮件提醒，点击“保存并发送测试邮件”。授权码不是 QQ 登录密码，必须先在 QQ 邮箱开启 SMTP 并生成授权码。
- 授权码保存在当前 Windows 用户的凭据管理器中；不写入 SQLite、localStorage、任务备份或导出文件。更换电脑或 Windows 用户后需重新输入。删除授权码按钮可从本机移除它。
- 编辑任务，设置截止时间并勾选“截止时邮件提醒”。电脑和清单要保持运行，每 30 秒处理一项到期提醒；重新打开仅补发最近 24 小时到期任务。邮件受理不保证收件箱位置、手机通知或实时送达，手机需开启 QQ 邮箱通知。
- 旧版短信设置保留在旧数据中用于兼容备份，但本版本不再启动短信发送；旧短信勾选不会自动授权邮件发送，需自行勾选邮件提醒。
- 页面右上角“一键导出事项”默认导出全部未删除事项（含已完成）为 UTF-8 BOM CSV，Excel 可直接打开。导出不受当前分类、搜索、视图限制；旁边下拉可导出当前列表，或导出完整 JSON（含图片）。CSV 包含名称、说明、状态、优先级、ISO 截止时间、分类、各时间字段、邮件提醒、本地目录和图片名称/数量，图片内容请选 JSON。
- CSV 中可能被当作公式的文本会添加前导单引号，防止 Excel 执行事项内容。JSON 保留原始任务字段和图片，不含应用设置和邮箱授权码。导出文件只保存在用户选择的本机位置，不上传服务器。
- 数据库沿用原路径，首次升级到 v1.2.0 自动备份，再事务增补邮件字段和记录。安装版覆盖安装；便携版关闭旧程序后只替换 exe 并保留 data 和 portable.mode。

配置详情：[QQ 邮箱提醒](docs/qq-email.md)。本版本为 v1.1.0 全部改动的后续版本，以下旧版说明中的短信功能已被邮件替代。

---

# v1.1.0 修改说明

- 左侧新增“未完成”；“全部任务”包含未完成和已完成，均不包含回收站。
- 列表点击“批量删除”，勾选或全选当前搜索结果，确认一次即可删除。正常列表移入回收站，可恢复；回收站内永久删除需再次确认。
- 高、中、低优先级分别使用浅红、浅黄、浅蓝整张卡片背景；选中轮廓不会盖掉优先级底色。
- 编辑任务可上传图片，也可用 Win+Shift+S 截图后在编辑窗口 Ctrl+V 粘贴，记录工作进度快照。每任务最多 6 张，原图上限 12 MB，自动缩至最长边 1600 像素，转成 JPEG（静图，不保留 GIF 动画），随任务和备份保存；点击缩略图可放大。
- 可为任务输入本地文件夹路径；桌面版还可选择文件夹。详情点击“打开本地文件夹”跳转资源管理器。纯浏览器预览无法启动本地资源管理器，需桌面版或便携网页服务。
- 新增截止时短信提醒与服务状态，详情见 [短信配置](docs/sms.md)。需自备 HTTPS 中转服务、短信服务商账号和模板；本项目未包含付费短信账号，未配置不会发送。桌面应用关闭、电脑睡眠或关机时无法发送，重开后仅补发 24 小时内到期的任务。

## 更新时保留原数据

安装版直接运行新版本安装包，安装到原位置。应用标识仍为 `com.qingdan.desktop`，不改变原用户数据路径。无需卸载旧版，更不要勾选清除应用数据。

便携版先关闭旧版，把新压缩包解压到另一个文件夹，在新文件夹运行 `update-portable.ps1`，输入旧应用文件夹（例如 `D:\Qingdan`）。只替换 `Qingdan.exe`，不覆盖 `data`、`backups` 或 `portable.mode`，并保留 `Qingdan.exe.previous`。也可以关闭旧版后手动只替换 exe。不要把旧版 data 文件夹删掉，也不要在新空目录打开程序后误认为旧数据丢失。

首次升级到 v1.1.0 会先备份旧 SQLite，再以事务增补图片、目录、短信字段，不重建数据库。SQLite 备份包含图片、目录和短信开关。旧版备份也能恢复并自动迁移；恢复前先备份当前数据。

安装版与便携版是原本就不同的数据模式，互换模式不自动搬移数据。便携网页版仍使用 `data/state.json`，不要拿 SQLite 文件覆盖它。浏览器预览继续沿用 v1 存储键，不清空旧数据；浏览器容量有限，大量图片建议使用桌面 SQLite 版。

GitHub Actions 会在修改分支上运行前端构建、数据库迁移/备份测试，并生成 Windows 安装包和便携包。修改分支不会覆盖原 v1.0.0 发布。

---

# 清单 · 本地待办

一款采用方案 A「清爽蓝白」设计的离线待办桌面应用。应用不需要账号，任务、分类、图片和备份默认均保存在使用者自己的电脑上。可选的短信功能启用后，仅向用户配置的中转服务发送手机号、任务标题和截止时间。

## 已实现功能

- 今日任务、收集箱、即将到期、全部任务、已完成、回收站
- 新建、编辑、完成、软删除、恢复和永久删除任务
- 截止时间、三级优先级、描述和自定义分类
- 实时搜索、今日统计、逾期提醒和详情面板
- SQLite 本地数据库、参数化查询、外键和索引
- 手动备份、每日自动备份、备份校验和恢复前保护性备份
- 浏览器开发预览：未运行在 Tauri 中时，自动使用 localStorage，不影响界面开发
- Windows NSIS `.exe` 安装包配置
- GitHub Actions 自动构建 Windows 安装包

## 技术栈

| 部分 | 技术 |
| --- | --- |
| 界面 | Vue 3、TypeScript、Vite |
| 桌面容器 | Tauri 2 |
| 本地数据 | Rust、SQLite（rusqlite bundled） |
| 安装包 | Tauri NSIS |

## 本地开发

仅预览界面和交互（数据写入当前浏览器的 localStorage）：

```bash
npm install
npm run dev
```

运行完整桌面版前，安装 Node.js 20+、Rust stable 和 Tauri 的系统依赖，然后执行：

```bash
npm install
npm run desktop:dev
```

## 构建 Windows 安装包

在 Windows 10/11 上安装：

1. Node.js 20 或更高版本
2. Rust stable（MSVC toolchain）
3. Microsoft C++ Build Tools 和 WebView2（Windows 11 通常已包含 WebView2）

之后双击或在 PowerShell 运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-windows.ps1
```

生成结果会复制到 `release` 目录。也可以把项目推送到 GitHub，在 Actions 中手动运行“Build Qingdan for Windows”，无需本地配置 Rust。

## 一键部署到 D 盘

如果希望程序、SQLite 数据库和备份都位于 D 盘：

1. 把源码压缩包下载到 Windows 并解压。
2. 确保已安装 Node.js、Rust 和 Microsoft C++ Build Tools。
3. 进入 `scripts` 目录，双击 `deploy-to-D.bat`。
4. 脚本会构建程序、复制到 `D:\Qingdan`、创建桌面快捷方式并启动应用。

便携部署后的目录：

```text
D:\Qingdan\Qingdan.exe
D:\Qingdan\portable.mode
D:\Qingdan\data\qingdan.sqlite3
D:\Qingdan\data\backups\
```

再次运行部署脚本可更新程序，不会删除 `D:\Qingdan\data` 中的原有任务。

如果电脑缺少基础环境，可先在 PowerShell 执行：

```powershell
winget install OpenJS.NodeJS.LTS
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

## 数据保存位置

Windows 默认位置：

```text
%APPDATA%\com.qingdan.desktop\qingdan.sqlite3
%APPDATA%\com.qingdan.desktop\backups\
```

通过 `deploy-to-D.bat` 部署时，程序会检测同目录的 `portable.mode` 标记，并改用 `D:\Qingdan\data`，不再把任务数据库写到系统盘。部署脚本采用纯英文 ASCII 编码，以兼容 Windows PowerShell 5.1。

卸载应用不会主动删除该目录，因此重新安装后数据仍可继续使用。用户需要彻底清除数据时，应先退出应用，再手动删除此目录。

## 发布给其他人

推荐流程：

1. 把本项目推送到 GitHub。
2. 在 Actions 中运行“Build Qingdan for Windows”。
3. 下载 `Qingdan-Windows-Portable`（免安装便携版）或 `Qingdan-Windows-Installers`（安装包）。
4. 将 `.exe` 或 `.msi` 上传到 GitHub Release、网盘或自己的下载页。
5. 正式公开发布前购买 Windows 代码签名证书，给安装包签名，降低 SmartScreen 警告概率。

`download-site/index.html` 是可独立部署的静态下载页，仓库地址为 `ztahaa/qingdan-desktop`。

## 隐私和安全

- 应用功能不依赖云端接口，默认不会上传任务数据。
- SQL 操作全部使用参数绑定。
- 恢复功能只允许读取应用备份目录中的 `.sqlite3` 文件，并在恢复前执行完整性校验和当前数据备份。
- 如需跨设备同步，应作为后续独立功能加入端到端加密，不建议直接上传原始数据库。

## 项目结构

```text
src/                 Vue 界面与双模式数据适配层
src-tauri/src/       Rust + SQLite 原生后端
src-tauri/icons/     应用图标
download-site/       对外下载页
scripts/             Windows 构建脚本
.github/workflows/   云端构建配置
```

## 许可证

MIT



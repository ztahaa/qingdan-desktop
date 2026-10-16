# 清单 · 本地待办

一款采用方案 A「清爽蓝白」设计的离线待办桌面应用。应用不需要账号，不连接业务服务器，任务、分类、设置和备份均保存在使用者自己的电脑上。

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


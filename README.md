# Reclaude 控制台

[![Version](https://img.shields.io/badge/version-1.0.0-blue.svg)](https://github.com/Voun8/ReClaude-Monitor-Desktop/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB.svg)](https://tauri.app)
[![SvelteKit](https://img.shields.io/badge/SvelteKit-2-FF3E00.svg)](https://kit.svelte.dev)

一个 **Tauri 2 + SvelteKit** 桌面应用，把 reclaude 用户的两件高频操作合成一个图形界面：

1. **账号热切换** —— 在多个 reclaude 账号间秒切，**不打断当前正在跑的 claude 对话**。
2. **拼车额度 & 服务监控** —— 实时显示当前账号 + 所有档案的剩余额度、错误率、延迟，以及用量明细。

## 核心特性

### 账号管理
- **快照式档案**：把 `~/.reclaude/device.{json,key}` + Claude 桌面 App 数据整套保存到 `~/.reclaude-profiles/<name>`
- **热切换**：基于 `reclaude stop` 的优雅停止机制，正在 streaming 的请求会自然完成，新请求自动走新账号，切换窗口约 1.5s
- **路径校验**：档案名拒绝 `..` `/` `\` 及前导 `.`，杜绝路径穿越
- **凭证文件 chmod 600**（Unix）：本地存储的密码、device.key 仅本人可读

### 额度监控
- **当前账号 Hero 卡片**：环形进度 + 剩余额度 + 重置倒计时 + 服务状态药丸（正常 / 抖动 / 故障 / 额度告急）
- **多账号横向对比**：每个档案一行，并行拉取并显示各自余额（最多 3 并发，避免触发限流）
- **后台自动跟随**：每 10s 同步当前 reclaude 登录账号，切换后 UI 立即跟上
- **指数退避**：托盘后台循环遇到鉴权失败或网络错误时退避到 5 分钟，不会高频撞 API

### 用量统计
- **KPI 卡片**：会话数 / 消息数 / 总用量（USD）/ 总 Tokens / 活跃天数 / 连续天数 / 常用模型
- **时间范围**：7 天 / 30 天 / 全部（年度热力图）
- **模型分布**：按模型分项的成本、Tokens、占比
- **设备过滤**：按客户端设备筛选

### 极简最小化
两种最小化形态，可随时切换：
- **悬浮球**：圆形小窗，水波纹动画展示用量水位，鼠标穿透圆外，尺寸 30–600px 自定义
- **菜单栏圆环**：tiny-skia 自绘的环形 + 中央百分比数字，**主窗口可全程隐藏**（零电耗）

### 关闭行为
关闭按钮可配置为「退出程序」或「后台运行」，后者自动收进设定的最小化形态。

## 安装

> ⚠️ 包未签名。**macOS** 首次打开请右键→打开（绕过 Gatekeeper）；**Windows** 首次会触发 SmartScreen，点「更多信息」→「仍要运行」即可。

从 [Releases](https://github.com/Voun8/ReClaude-Monitor-Desktop/releases) 下载对应平台的包：

| 平台 | 文件 |
|---|---|
| macOS (Apple Silicon) | `*.dmg` 或 `*.app.tar.gz` |
| Windows | `*.msi` 或 `*.exe` |

### 前置依赖

- **reclaude CLI** —— 账号切换功能依赖它。未安装时监控仍可用，只是无法切换。
  - 安装：[`reclaude` 官方](https://reclaude.ai)（默认安装到 `~/.local/bin/reclaude` 或 Windows `%LOCALAPPDATA%\Programs\reclaude\bin`）
- **Claude 桌面 App**（可选）—— 若想切换时同步桌面会话，需先安装；否则只切 CLI 凭据。

### 关键约束（保留账号）

> 每个 reclaude 账号 **只 login 一次** 就保存档案。之后只切换、**不要再对同一账号 `reclaude login`** —— 再次 login 会吊销旧设备 key，使已存档案失效。

## 开发

需要 Node 20+、pnpm 9+、Rust 工具链（Tauri 2 要求 1.77.2+）。

```bash
pnpm install
pnpm tauri dev        # 热重载
pnpm tauri build      # 当前平台安装包，输出在 src-tauri/target/release/bundle/

# 仅前端
pnpm dev              # Vite dev（不含 Tauri 原生能力）
pnpm check            # svelte-check 类型检查
```

### 跨平台打包

仓库附带 `.github/workflows/release.yml`，**推 `v*` tag 自动构建 mac + win 包并创建 Draft Release**：

```bash
git tag v1.0.0
git push origin v1.0.0
# 几分钟后到 Releases 页面发布 Draft
```

## 架构

```
┌─────────────────────────── SvelteKit (前端) ───────────────────────────┐
│  +page.svelte         主面板：监控 + 用量 Tab + 切换列表 + 设置弹窗     │
│  FloatWidget.svelte   悬浮球：水波纹 + 错误率 + 用量                   │
│  UsageView.svelte     用量页：KPI + 热力图 + ActivityBars + 模型分布   │
└──────────────────────────────┬────────────────────────────────────────┘
                               │ invoke (Tauri IPC)
┌──────────────────────────────▼────────────────────────────────────────┐
│                          Tauri (Rust 后端)                            │
│                                                                       │
│  switcher.rs   档案快照 / 热切换 / 端口探活 / 凭证文件管理              │
│  monitor.rs    reclaude.ai 登录 / 拼车额度 / 服务指标 / 用量统计       │
│  tray_ring.rs  tiny-skia 自绘菜单栏圆环（环 + 中央百分比 + alpha 混合）│
│  lib.rs        Tauri 命令注册 + cookie 缓存 + 托盘后台循环（指数退避） │
└──────────────────────────────┬────────────────────────────────────────┘
                               │
                               ▼
              ┌──────────────────────────────┐
              │ ~/.reclaude/                 │  device.json / device.key / state.json
              │ ~/.reclaude-profiles/<name>/ │  device.{json,key} + claude-app-data/
              │                              │  + monitor.json (邮箱密码 org_id)
              │ reclaude _daemon → 49154     │  本地 HTTPS 中转代理
              │ reclaude.ai                  │  拼车账户 / 用量 API
              └──────────────────────────────┘
```

### 热切换原理

`reclaude _daemon` 是本地 HTTPS 中转代理（CA 注入 `NODE_EXTRA_CA_CERTS=~/.reclaude/ca.pem`），所有 Claude CLI 的请求经它鉴权后转给上游。

切换账号的关键发现：**`reclaude stop` 是优雅停止** —— 正在 streaming 的连接不被中断，daemon 处理完手头请求才退出。所以切换流程能做到：

1. `reclaude stop`（已有 streaming 继续完成）
2. 端口探活，等 daemon 真正退出（最多 2s）
3. 覆盖 `device.{json,key}`（chmod 600）
4. 启动新 daemon，端口 LISTEN 后返回（约 1s）

切换总耗时 ~1.5s，对正在跑的 claude 对话**几乎无感**，下一条消息自动用新账号。

## 配置文件位置

| 文件 | 用途 |
|---|---|
| `~/.reclaude/device.json` | 当前账号身份 |
| `~/.reclaude/device.key` | 当前账号 SK（敏感） |
| `~/.reclaude/state.json` | daemon 端口与状态 |
| `~/.reclaude/ui.json` | 桌面 App 设置（最小化模式 / 悬浮球尺寸 / 刷新间隔） |
| `~/.reclaude-profiles/<name>/device.{json,key}` | 档案凭证快照 |
| `~/.reclaude-profiles/<name>/claude-app-data/` | 桌面 App 会话快照 |
| `~/.reclaude-profiles/<name>/monitor.json` | 该档案的监控凭证（chmod 600） |
| `~/.reclaude-profiles/.monitor-creds.json` | 按 email 索引的根映射（chmod 600） |

## License

MIT

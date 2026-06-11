# Reclaude 控制台

[![Version](https://img.shields.io/badge/version-1.1.0-blue.svg)](https://github.com/Voun8/ReClaude-Monitor-Desktop/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB.svg)](https://tauri.app)
[![SvelteKit](https://img.shields.io/badge/SvelteKit-2-FF3E00.svg)](https://kit.svelte.dev)

一个 **Tauri 2 + SvelteKit** 桌面应用，把 reclaude 用户的两件高频操作合成一个图形界面：

1. **账号热切换** —— 在多个 reclaude 账号间秒切，**不打断当前正在跑的 claude 对话**。
2. **拼车额度 & 服务监控** —— 实时显示当前账号 + 所有档案的剩余额度、错误率、延迟，以及用量明细。

## 核心特性

### 账号管理
- **快照式档案**：把账号身份（`~/.reclaude/device.json`、Windows 的 `device.key` 或 macOS Keychain 里的签名 seed）+ Claude 桌面 App 数据整套保存到 `~/.reclaude-profiles/<name>`
- **热切换**：切换时覆盖凭据 + 配套签名私钥再重启 daemon；配合固定端口转发器，正在运行的 claude 会话切换后可自愈，无需 exit/重开
- **固定端口转发器**：App 绑 `127.0.0.1:47600` 实时转发到 daemon 当前端口，屏蔽 daemon 每次重启换随机端口对运行中会话的影响
- **路径校验**：档案名拒绝 `..` `/` `\` 及前导 `.`，杜绝路径穿越
- **凭证文件 chmod 600**（Unix）：本地存储的密码、签名私钥仅本人可读

### 额度监控
- **当前账号 Hero 卡片**：环形进度 + 剩余额度 + 重置倒计时 + 服务状态药丸（正常 / 抖动 / 故障 / 额度告急）
- **多账号横向对比**：每个档案一行，并行拉取并显示各自余额（最多 3 并发，避免触发限流）
- **后台自动跟随**：每 10s 同步当前 reclaude 登录账号，切换后 UI 立即跟上
- **指数退避**：托盘后台循环遇到鉴权失败或网络错误时退避到 5 分钟，不会高频撞 API
- **API 地址可配置**：设置里可填写 API 根地址；留空优先使用 `https://reclaude.ai`，默认地址不可用时自动切换到 `https://www.recode.cat`

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

> 每个 reclaude 账号 **login 一次** 后就在 App 里保存档案。之后只切换、尽量**不要再对同一账号 `reclaude login`** —— 再次 login 会重新生成设备签名私钥，使已存档案失效。
>
> 若确实重新 login 了某账号，**务必回到 App 重新保存该档案**（捕获新私钥），否则切回去会因签名指纹不匹配而失败（502 / 无限重试）。

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
│  switcher.rs   档案快照 / 热切换 / 端口探活 / 凭证 + 签名私钥管理       │
│  forwarder.rs  固定端口转发器（47600 → daemon 当前端口，撑热切换）     │
│  monitor.rs    可配置 API 登录 / 拼车额度 / 服务指标 / 用量统计        │
│  tray_ring.rs  tiny-skia 自绘菜单栏圆环（环 + 中央百分比 + alpha 混合）│
│  lib.rs        Tauri 命令注册 + cookie 缓存 + 托盘后台循环（指数退避） │
└──────────────────────────────┬────────────────────────────────────────┘
                               │
                               ▼
              ┌──────────────────────────────┐
              │ ~/.reclaude/                 │  device.json / state.json (+ Win device.key)
              │ ~/.reclaude-profiles/<name>/ │  device.json + device.seed(mac)/key(win)
              │                              │  + claude-app-data/ + monitor.json
              │ App forwarder → :47600       │  固定端口转发到当前 daemon（撑热切换）
              │ reclaude _daemon → dynamic   │  本地 HTTPS 中转代理 + 验签 device 身份
              │ reclaude.ai / recode.cat     │  拼车账户 / 用量 API（可配置）
              └──────────────────────────────┘
```

### 热切换原理

`reclaude _daemon` 是本地 HTTPS 中转代理（CA 注入 `NODE_EXTRA_CA_CERTS=~/.reclaude/ca.pem`），所有 Claude CLI 请求经它用**设备签名私钥**签名鉴权后转给上游。两个关键事实决定了热切换怎么做：

- **daemon 每次启动绑随机端口**（写在 `state.json`）。会话启动时把端口读死进 `HTTPS_PROXY`，一旦切换重启 daemon 换了端口，正在跑的会话就连着旧端口卡死、无限重试。
- **签名私钥不在 `device.json` 里**：Windows 是 `device.key` 文件，**macOS 在 Keychain**（service `Claude Code-device-key`）。只换 `device.json`、不换私钥，daemon 会用旧账号的钥匙签新账号请求，网关验签失败（502）。

对应两层解法：

1. **固定端口转发器**（`forwarder.rs`，`127.0.0.1:47600`）：每个连接实时读 `state.json` 转发到 daemon 当前端口。把会话的 `HTTPS_PROXY` 指向 47600（而非动态端口），daemon 换端口对会话透明 —— 切换后会话重连一次即落到新 daemon。
2. **签名私钥配套切换**：切换时把档案里的私钥（Win `device.key` / mac `device.seed` 写回 Keychain）一并还原，**再**重启 daemon，验签才对得上。

切换流程：`reclaude stop`（优雅停止，已有 streaming 继续完成）→ 端口探活等退出 → 回收残留 daemon → 覆盖 `device.json` + 还原签名私钥 → 启动新 daemon。配合 47600 转发器，正在跑的 claude 对话切换后自动用新账号，无需 exit/重开。

> 让会话走固定端口（zsh 示例）：启动 claude 前检测 `127.0.0.1:47600` 是否在监听，是则把 `HTTPS_PROXY` 指向它，否则回退 `state.json` 的动态端口。

## 配置文件位置

| 文件 | 用途 |
|---|---|
| `~/.reclaude/device.json` | 当前账号身份（含公钥指纹） |
| `~/.reclaude/device.key` | 当前账号签名私钥（**仅 Windows**；macOS 在 Keychain `Claude Code-device-key`） |
| `~/.reclaude/state.json` | daemon 端口与状态 |
| `~/.reclaude/ui.json` | 桌面 App 设置（最小化模式 / 悬浮球尺寸 / 刷新间隔） |
| `~/.reclaude-profiles/<name>/device.json` | 档案账号身份快照 |
| `~/.reclaude-profiles/<name>/device.{key,seed}` | 签名私钥快照（Win `device.key` / mac `device.seed`，chmod 600） |
| `~/.reclaude-profiles/<name>/claude-app-data/` | 桌面 App 会话快照 |
| `~/.reclaude-profiles/<name>/monitor.json` | 该档案的监控凭证（chmod 600） |
| `~/.reclaude-profiles/.monitor-creds.json` | 按 email 索引的根映射（chmod 600） |

## License

MIT

# Reclaude 控制台

一个 **Tauri + SvelteKit** 桌面应用，把两件事合并成一个图形界面：

1. **账号一键切换**（移植自 `rec-switch.ps1`）—— 基于快照在多个 reclaude 账号间切换。
2. **额度 / 服务监控**（移植自 `reclaude-monitor` VS Code 插件）—— 实时显示拼车额度、用量与服务可用性。

## 功能

### 账号档案（切换）

- 每个账号 = `~/.reclaude/device.json` + `device.key` 的身份，外加 `%APPDATA%\Claude` 桌面 App 登录态。
- **保存当前账号**：把上述两部分整套快照到 `~/.reclaude-profiles/<name>`（用 robocopy 镜像，跳过纯缓存目录）。
- **一键切换**：停桌面 App + `reclaude stop` → 写入目标账号凭证 → 恢复 App 会话 → 拉起 daemon / 桌面 App，全程不走浏览器授权。
- **只换凭证不开 App**：勾选后切换仅拉起 daemon（纯 CLI 用户）。
- 列表显示每个档案的邮箱、是否含 App 会话、是否已配监控；当前登录的账号高亮为「当前」。

> 关键约束（沿用原脚本）：每个账号只 `reclaude login` 登录一次 → 立刻保存档案。之后只切换、不要再对同一账号 login（再 login 会吊销旧设备，使已存档案失效）。

### 监控

- **拼车额度**：已用 / 总额、彩色进度条、剩余、重置倒计时。
- **服务可用性**：错误率、平均延迟、请求/分、令牌/分，带迷你折线图，60s 窗口。
- 定时刷新（额度+指标 30s，倒计时每秒走字），并每 10s 跟随 `~/.reclaude/device.json` 当前账号自动同步。
- 顶部状态药丸汇总当前健康度（正常 / 抖动 / 故障 / 额度告急）。

### 凭证衔接

- 切换档案 **不需要** 密码（纯文件快照）。
- 监控额度 **需要** 邮箱密码登录 reclaude.ai。两者通过 **每档案可选的 `monitor.json`** 衔接：
  - 保存档案时可顺带填邮箱 / 密码 / 组织 ID（org_id），写入 `~/.reclaude-profiles/<name>/monitor.json`。
  - 组织 ID 可留空，登录后自动探测（多个套餐时在界面里选）。
  - 切到某账号后，若它有监控凭证就自动显示额度；没有则一键「配置监控凭证」。

## 开发

需要 Node、pnpm 与 Rust 工具链（Tauri 2）。

```bash
pnpm install
pnpm tauri dev        # 开发模式，热重载
pnpm tauri build      # 打包安装程序 / 可执行文件
```

仅前端：`pnpm dev`（不含原生能力）/ `pnpm check`（类型检查）。

## 架构

- `src-tauri/src/switcher.rs` —— 档案快照 / 切换 / 监控凭证存储（robocopy、进程管理、文件操作）。
- `src-tauri/src/monitor.rs` —— reclaude.ai 登录、拼车额度、服务指标（reqwest）。
- `src-tauri/src/lib.rs` —— Tauri 命令 + HTTP 客户端 / Cookie 缓存状态。
- `src/routes/+page.svelte` —— 主界面（轮询、跟随当前账号、弹窗操作）。
- `src/lib/components/` —— `QuotaCard` / `MetricsCard` / `ProfilesCard` / `Modal` / `Sparkline`。

## License

MIT

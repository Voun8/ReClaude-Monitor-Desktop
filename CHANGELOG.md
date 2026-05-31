# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.0] - 2026-05-31

聚焦账号热切换的可靠性:修复 macOS 上切换账号后 502 / 无限重试的核心问题,并新增固定端口转发器,让正在运行的 claude 会话在切换后能自愈。

### 新增

- **固定端口转发器**(`forwarder.rs`):App 内常驻线程绑定 `127.0.0.1:47600`,每个连接实时读 `state.json` 转发到 daemon 的当前端口。把会话代理指向这个固定端口(而非每次重启都变的 daemon 动态端口),切换账号后正在跑的会话只需重连一次即落到新 daemon —— 真正的热切换。需把启动器的 `HTTPS_PROXY` 指向 `127.0.0.1:47600`(转发器未运行时回退到动态端口,即原行为)。
- **macOS 签名私钥(Keychain seed)配套切换**:`save_profile` 把 Keychain 里的设备 Ed25519 签名 seed 一并快照进 `<档案>/device.seed`;`use_profile` 切换时先把 seed 写回 Keychain(service `Claude Code-device-key`)再重启 daemon。

### 修复

- **macOS 切换账号后 502 / 无限重试**:根因是 macOS 上设备签名私钥存在 Keychain(而非 `device.key` 文件),原快照只换 `device.json`、漏了私钥 → daemon 用旧账号的钥匙去签新账号请求 → 网关验签失败。现已连 Keychain seed 一起切换。
- **daemon 僵尸进程累积**:历次切换残留的 `reclaude daemon` 进程从不回收、会占住旧端口与路由,导致连新开的会话也连不上干净 daemon。`use_profile` 现在每次切换前回收所有残留 daemon(`stop_reclaude_daemons`)。

### 已知约束

- **对某账号重新 `reclaude login` 后,必须在 App 里重新保存该档案** —— login 会重新生成签名 seed,旧快照随之失效(切回去会 502)。
- 转发器仅在 App 运行时提供;首次保存/切换会弹 macOS Keychain 授权框,点「始终允许」。
- Windows 上签名私钥的存储位置(`device.key` 文件 vs 凭据管理器)尚未在真机确认;若为后者,Windows 切换需补对应实现。

## [1.0.0] - 2026-05-31

首个公开版本。把 reclaude 用户日常的两件高频操作(账号切换、额度监控)合成一个干净的桌面 GUI,基于 Tauri 2 + SvelteKit。

### 核心特性

#### 账号档案 & 热切换
- 快照式档案管理:`~/.reclaude/device.{json,key}` + Claude 桌面 App 会话整套保存到 `~/.reclaude-profiles/<name>`
- 热切换流程基于 `reclaude stop` 优雅停止 + 端口探活,**切换窗口约 1.5s,正在 streaming 的 claude 请求不会被中断**
- 切换完成后下一条 claude 消息自动走新账号,无需 exit/重开
- 支持「只换凭证不开 App」模式,纯 CLI 用户友好
- daemon 端口动态读取自 `~/.reclaude/state.json`,兼容 reclaude 未来改端口

#### 拼车额度监控
- Hero 卡片:环形进度 + 剩余额度 + 重置倒计时 + 服务状态药丸(正常/抖动/故障/额度告急)
- 所有档案横向对比,**最多 3 并发拉取**避免触发限流
- 每 10s 自动跟随 `~/.reclaude/device.json` 变化(命令行切了 reclaude 账号 UI 立即同步)
- 托盘后台循环遇到失败时**指数退避到 5 分钟**,不会高频撞 API
- 失败显示「获取失败」chip,不再静默吞错

#### 用量统计
- 7 天 / 30 天 / 全部时间范围切换
- KPI:会话数、消息数、总用量、总 Tokens、活跃天数、连续天数、常用模型
- 按设备过滤、按模型分布展示成本/Tokens/占比
- 全部时间段显示年度热力图

#### 最小化形态
- **悬浮球**:圆形小窗 + 水波纹动画,鼠标穿透圆外四角,大小 30–600px 可调
- **菜单栏圆环**:tiny-skia 自绘的环 + 中央百分比数字,主窗口可全程隐藏(零电耗)
- 悬浮球刷新间隔自动跟随主面板设置(通过 ui.json 共享)
- 关闭按钮可配置「退出」或「后台运行」

### 安全 & 数据完整性
- 档案名校验:拒绝 `..` `/` `\` 及前导 `.`,杜绝路径穿越(save/use/remove 都加)
- 凭证文件 `chmod 600`(Unix):`device.key` / `monitor.json` / `.monitor-creds.json` 仅本人可读
- 保存档案前先停桌面 App + 等待 800ms,避免拷到不一致的 SQLite 状态(数据库损坏)
- `use_profile` mirror 传 CACHE_EXCLUDES,不再误删 logs/Crashpad/Cache
- `wait_port_down` 失败早返回,杜绝覆盖凭据后 daemon 端口冲突的半成功状态
- 切换账号后清理内存中的 cookie 缓存,旧账号凭据不长驻
- 后端固化 percent 字段语义(percent/pct = 0-100,share×100),前端只做钳制,避免 1.01 跳变到 1%

### 体验
- 主面板隐藏时 1Hz tickTimer 自动暂停(visibilitychange 监听),省电
- 切换账号后 UI 立即清理被删档案的 accountStatuses/Loading 缓存
- 托盘圆环字体用 Porter-Duff over 混合而非硬覆盖 alpha,数字若落到环上也优雅

### 安装

从本 Release 下载对应平台:

| 平台 | 文件 |
|---|---|
| macOS (Apple Silicon) | `*_aarch64.dmg` 或 `*.app.tar.gz` |
| macOS (Intel) | `*_x64.dmg` 或 `*.app.tar.gz` |
| Windows x64 | `*_x64-setup.exe` 或 `*_x64_en-US.msi` |

**首次打开提示**(本版本未做代码签名):
- macOS:右键 `.app` → 打开(绕过 Gatekeeper)
- Windows:SmartScreen 弹出 → 「更多信息」→「仍要运行」

### 依赖
- [reclaude CLI](https://reclaude.ai)(账号切换需要;未安装时监控仍可用)
- Claude 桌面 App(可选,只有想同步桌面会话快照才需要)

### 已知约束
- 同一账号**只 login 一次**就保存档案,之后只切换 —— 再次 `reclaude login` 会吊销旧 device key,使已存档案失效
- 当前包未做代码签名 / 公证,首次启动需要手动放行
- macOS dmg 本地打包时如终端无 Finder 自动化权限,需用 `hdiutil` 直接打基础 dmg(CI 不影响)

---

[1.1.0]: https://github.com/Voun8/ReClaude-Monitor-Desktop/releases/tag/v1.1.0
[1.0.0]: https://github.com/Voun8/ReClaude-Monitor-Desktop/releases/tag/v1.0.0

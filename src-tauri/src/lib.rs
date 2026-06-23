// 入口编排层：AppState、Tauri 命令薄包装与启动流程。
// 业务逻辑分布在各域模块：switcher（账号切换）、monitor（HTTP）、session（监控会话/重试）、
// ui_config（ui.json）、windows（窗口显隐）、tray（托盘与圆环循环）、forwarder（固定端口代理）。

mod autostart;
mod forwarder;
mod monitor;
mod session;
mod switcher;
mod tray;
mod tray_ring;
mod ui_config;
mod windows;

use monitor::{Allocation, MonitorSnapshot};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use switcher::{EnvInfo, MonitorCred, Paths, ProfileInfo};
use tauri::Manager;

const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

struct AppState {
    client: reqwest::Client,
    // 监控会话 cookie 缓存，key = api_base + "\n" + email（见 session 模块）
    cookies: Mutex<HashMap<String, String>>,
    // 菜单栏圆环：是否激活 + 刷新间隔（秒），由 Rust 后台循环读取
    tray_active: AtomicBool,
    tray_interval: AtomicU64,
    // 托盘面板点「设置」→ 打开主面板时由前端读取并自动弹设置弹窗
    pending_settings: AtomicBool,
    // 托盘面板上次隐藏时刻：toggle 时区分「失焦自动隐藏」与「用户主动再点关闭」
    panel_hidden_at: Mutex<std::time::Instant>,
}

fn normalize_proxy_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("direct://") {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }
    // reqwest 当前未启用 socks 特性，遇到 socks 代理时不强行套用，避免启动失败。
    if trimmed.starts_with("socks://")
        || trimmed.starts_with("socks5://")
        || trimmed.starts_with("socks4://")
    {
        return None;
    }
    Some(format!("http://{trimmed}"))
}

fn env_proxy_url() -> Option<String> {
    [
        "HTTPS_PROXY",
        "HTTP_PROXY",
        "ALL_PROXY",
        "https_proxy",
        "http_proxy",
        "all_proxy",
    ]
    .iter()
    .find_map(|key| {
        std::env::var(key)
            .ok()
            .and_then(|v| normalize_proxy_url(&v))
    })
}

#[cfg(windows)]
fn reg_query_value(name: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;

    let output = std::process::Command::new("reg")
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
            "/v",
            name,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with(name) {
            line.split_whitespace().last().map(|v| v.to_string())
        } else {
            None
        }
    })
}

#[cfg(windows)]
fn windows_system_proxy_url() -> Option<String> {
    let enabled = reg_query_value("ProxyEnable")?;
    let proxy_enabled = enabled == "0x1" || enabled == "1";
    if !proxy_enabled {
        return None;
    }

    let raw = reg_query_value("ProxyServer")?;
    if raw.contains('=') {
        let mut http_proxy: Option<String> = None;
        for part in raw.split(';') {
            let Some((kind, value)) = part.split_once('=') else {
                continue;
            };
            let kind = kind.trim().to_ascii_lowercase();
            let value = value.trim();
            if kind == "https" {
                return normalize_proxy_url(value);
            }
            if kind == "http" {
                http_proxy = normalize_proxy_url(value);
            }
        }
        return http_proxy;
    }
    normalize_proxy_url(&raw)
}

#[cfg(not(windows))]
fn windows_system_proxy_url() -> Option<String> {
    None
}

fn detected_proxy_url() -> Option<String> {
    env_proxy_url().or_else(windows_system_proxy_url)
}

fn build_http_client() -> reqwest::Client {
    let mut builder = reqwest::Client::builder().timeout(HTTP_TIMEOUT);
    if let Some(proxy_url) = detected_proxy_url() {
        match reqwest::Proxy::all(&proxy_url) {
            Ok(proxy) => {
                // Rust 后端请求需要显式套用系统代理，才能和浏览器 DevTools 里
                // 127.0.0.1:7890 的成功出口保持一致。
                eprintln!("HTTP client using proxy: {proxy_url}");
                builder = builder.proxy(proxy);
            }
            Err(e) => {
                eprintln!("忽略无效代理地址 {proxy_url}: {e}");
            }
        }
    }
    builder.build().expect("failed to build http client")
}

// ============ 环境 / 档案（rec-switch）============

/// 文件级阻塞操作放到 blocking 线程池，统一错误前缀。
async fn run_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("后台任务失败: {e}"))?
}

#[tauri::command]
async fn get_env() -> Result<EnvInfo, String> {
    run_blocking(|| {
        let paths = Paths::resolve()?;
        Ok(switcher::env_info(&paths))
    })
    .await
}

#[tauri::command]
async fn current_account() -> Result<Option<String>, String> {
    run_blocking(|| {
        let paths = Paths::resolve()?;
        Ok(switcher::current_email(&paths))
    })
    .await
}

#[tauri::command]
async fn list_profiles() -> Result<Vec<ProfileInfo>, String> {
    run_blocking(|| {
        let paths = Paths::resolve()?;
        Ok(switcher::list_profiles(&paths))
    })
    .await
}

#[tauri::command]
async fn get_monitor_cred(email: String) -> Result<Option<MonitorCred>, String> {
    run_blocking(move || {
        let paths = Paths::resolve()?;
        Ok(switcher::get_monitor_cred(&paths, &email))
    })
    .await
}

#[tauri::command]
async fn set_monitor_cred(cred: MonitorCred, profile_name: Option<String>) -> Result<(), String> {
    run_blocking(move || {
        let paths = Paths::resolve()?;
        switcher::set_monitor_cred(&paths, &cred, profile_name.as_deref())
    })
    .await
}

#[tauri::command]
async fn save_profile(name: String, monitor: Option<MonitorCred>) -> Result<String, String> {
    run_blocking(move || {
        let paths = Paths::resolve()?;
        switcher::save_profile(&paths, &name, monitor)
    })
    .await
}

#[tauri::command]
async fn use_profile(
    state: tauri::State<'_, AppState>,
    name: String,
    no_app: bool,
) -> Result<String, String> {
    let email = run_blocking(move || {
        let paths = Paths::resolve()?;
        switcher::use_profile(&paths, &name, no_app)
    })
    .await?;
    // 切换账号后清掉所有缓存 cookie，避免旧账号 cookie 长期驻留内存
    // 新账号下次刷新会按需重新登录写入
    state.cookies.lock().unwrap().clear();
    Ok(email)
}

#[tauri::command]
async fn remove_profile(name: String) -> Result<(), String> {
    run_blocking(move || {
        let paths = Paths::resolve()?;
        switcher::remove_profile(&paths, &name)
    })
    .await
}

// ============ 监控（会话编排在 session 模块）============

#[tauri::command]
async fn refresh_monitor(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
    org_id: String,
) -> Result<MonitorSnapshot, String> {
    let r = session::resolve_quota(&state, &email, &password, &org_id).await;
    // metrics 也走会话编排：cookie 过期时自动重登重试，而非旧实现 .ok() 吞掉 401 导致
    // 额度正常但指标空白且无法自愈。已知凭据错误(bad)时跳过，避免对错误凭据重复登录。
    let metrics = if r.bad {
        None
    } else {
        let client = &state.client;
        session::with_session(&state, &email, &password, |base, cookie| async move {
            monitor::fetch_metrics(client, &base, &cookie).await
        })
        .await
        .ok()
    };
    Ok(MonitorSnapshot {
        quota: r.quota,
        metrics,
        org_id: r.org_id,
        error: r.error,
        bad_credentials: r.bad,
    })
}

/// 单账号状态 = 快照去掉 metrics（前端 AccountStatus 类型对应）。
#[tauri::command]
async fn account_status(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
    org_id: String,
) -> Result<MonitorSnapshot, String> {
    let r = session::resolve_quota(&state, &email, &password, &org_id).await;
    Ok(MonitorSnapshot {
        quota: r.quota,
        metrics: None,
        org_id: r.org_id,
        error: r.error,
        bad_credentials: r.bad,
    })
}

#[tauri::command]
async fn list_allocations(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
) -> Result<Vec<Allocation>, String> {
    let client = &state.client;
    session::with_session(&state, &email, &password, |base, cookie| async move {
        monitor::list_allocations(client, &base, &cookie).await
    })
    .await
}

#[tauri::command]
async fn usage_devices(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
) -> Result<Vec<monitor::Device>, String> {
    let client = &state.client;
    session::with_session(&state, &email, &password, |base, cookie| async move {
        monitor::fetch_devices(client, &base, &cookie).await
    })
    .await
}

#[tauri::command]
async fn usage_sync(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
) -> Result<(), String> {
    let client = &state.client;
    session::with_session(&state, &email, &password, |base, cookie| async move {
        monitor::sync_usage(client, &base, &cookie).await
    })
    .await
}

#[tauri::command]
async fn usage_stats(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
    range: String,
    device_id: Option<String>,
    org_id: String,
) -> Result<monitor::UsageStats, String> {
    let client = &state.client;
    let did = device_id.as_deref().filter(|s| !s.is_empty());
    let (range, org_id) = (range.as_str(), org_id.as_str());
    session::with_session(&state, &email, &password, |base, cookie| async move {
        monitor::fetch_usage_stats(client, &base, &cookie, range, did, org_id).await
    })
    .await
}

// ============ 启动 ============

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let client = build_http_client();

    let mut builder = tauri::Builder::default();

    // 单实例守卫（仅桌面，必须最先注册）：再次启动时把已有窗口拉到前台，
    // 而不是再开一个进程——否则任务管理器里会堆出多个实例。
    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
                windows::focus_main(app);
            }))
            // 开机自启动：状态由系统持有，启用/禁用走 autostart 模块命令。
            // 静默启动是独立开关（存 ui.json），与「是否开机拉起」无关，故无需注入启动参数。
            .plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))
            // 托盘面板定位：缓存托盘图标位置，供面板按 TrayBottomCenter 弹在图标下方
            .plugin(tauri_plugin_positioner::init());
    }

    builder
        // 记住窗口位置（含悬浮球被拖到的位置），跨重启恢复
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(tauri_plugin_window_state::StateFlags::POSITION)
                .build(),
        )
        // 注意：.manage 必须在 .setup 之前，setup 里才能 app.state::<AppState>()
        .manage(AppState {
            client,
            cookies: Mutex::new(HashMap::new()),
            tray_active: AtomicBool::new(false),
            tray_interval: AtomicU64::new(30),
            pending_settings: AtomicBool::new(false),
            panel_hidden_at: Mutex::new(std::time::Instant::now()),
        })
        .setup(|app| {
            // 固定端口转发器：在 daemon 动态端口前架一个稳定代理端口，支撑账号热切换
            forwarder::spawn();
            // 悬浮球 WebView 背景透明（macOS 下圆外四角才真正透明）
            if let Some(f) = app.get_webview_window(windows::WIN_FLOAT) {
                let _ = f.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
            }
            tray::init(app)?;

            // 托盘面板：透明背景 + 失焦自动隐藏（仿菜单栏 popover：点击面板外即关闭）
            if let Some(panel) = app.get_webview_window(windows::WIN_PANEL) {
                let _ = panel.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
                let app_handle = app.handle().clone();
                let panel_ref = panel.clone();
                panel.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        windows::hide_panel(&app_handle, &panel_ref);
                    }
                });
            }

            // 启动形态由「最小化方式 + 静默开关」共同决定：
            //   圆环模式 → 开启后台圆环绘制；并立即把托盘画成占位环（避免冷启动时首拉数据未就绪、
            //   菜单栏停在默认 App 图标、与设置里「圆环」不一致——拉到真实数据后自动替换成带百分比的环）。
            //   静默关 / 首次启动 → 显示主窗口（首次启动让新用户看到界面，不会只剩屏幕中央一个悬浮球）。
            //   静默开 + 悬浮球 → 仅显示悬浮球；静默开 + 圆环 → 什么都不弹，仅菜单栏圆环。
            let first_run = ui_config::path().map(|p| !p.exists()).unwrap_or(true);
            let (mode, size) = ui_config::startup_mode();
            let silent = ui_config::silent_start();
            if mode == "tray" {
                app.state::<AppState>()
                    .tray_active
                    .store(true, Ordering::Relaxed);
                tray::show_loading_ring(app.handle());
            }
            if first_run || !silent {
                windows::focus_main(app.handle());
            } else if mode == "ball" {
                windows::show_float_window(app.handle(), size);
            }

            tray::spawn_ring_loop(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_env,
            current_account,
            list_profiles,
            get_monitor_cred,
            set_monitor_cred,
            save_profile,
            use_profile,
            remove_profile,
            refresh_monitor,
            account_status,
            list_allocations,
            usage_devices,
            usage_stats,
            usage_sync,
            windows::hide_float,
            windows::minimize_to_float,
            windows::restore_from_float,
            windows::resize_float,
            windows::hide_main,
            windows::quit_app,
            windows::open_main,
            windows::take_pending_settings,
            ui_config::save_ui_config,
            ui_config::get_refresh_sec,
            ui_config::get_api_base,
            ui_config::get_api_key,
            tray::set_tray_mode,
            autostart::get_autostart,
            autostart::set_autostart,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod monitor;
mod switcher;
mod tray_ring;

use monitor::{Allocation, MonErr, MonitorSnapshot};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use switcher::{EnvInfo, MonitorCred, Paths, ProfileInfo};
use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

struct AppState {
    client: reqwest::Client,
    cookies: Mutex<HashMap<String, String>>,
    // 菜单栏圆环：是否激活 + 刷新间隔（秒），由 Rust 后台循环读取
    tray_active: AtomicBool,
    tray_interval: AtomicU64,
}

// ============ 环境 / 档案（rec-switch）============

#[tauri::command]
fn get_env() -> Result<EnvInfo, String> {
    let paths = Paths::resolve()?;
    Ok(switcher::env_info(&paths))
}

#[tauri::command]
fn current_account() -> Result<Option<String>, String> {
    let paths = Paths::resolve()?;
    Ok(switcher::current_email(&paths))
}

#[tauri::command]
fn list_profiles() -> Result<Vec<ProfileInfo>, String> {
    let paths = Paths::resolve()?;
    Ok(switcher::list_profiles(&paths))
}

#[tauri::command]
fn get_monitor_cred(email: String) -> Result<Option<MonitorCred>, String> {
    let paths = Paths::resolve()?;
    Ok(switcher::get_monitor_cred(&paths, &email))
}

#[tauri::command]
fn set_monitor_cred(cred: MonitorCred, profile_name: Option<String>) -> Result<(), String> {
    let paths = Paths::resolve()?;
    switcher::set_monitor_cred(&paths, &cred, profile_name.as_deref())
}

#[tauri::command]
async fn save_profile(name: String, monitor: Option<MonitorCred>) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let paths = Paths::resolve()?;
        switcher::save_profile(&paths, &name, monitor)
    })
    .await
    .map_err(|e| format!("后台任务失败: {e}"))?
}

#[tauri::command]
async fn use_profile(
    state: tauri::State<'_, AppState>,
    name: String,
    no_app: bool,
) -> Result<String, String> {
    let email = tauri::async_runtime::spawn_blocking(move || {
        let paths = Paths::resolve()?;
        switcher::use_profile(&paths, &name, no_app)
    })
    .await
    .map_err(|e| format!("后台任务失败: {e}"))??;
    // 切换账号后清掉所有缓存 cookie，避免旧账号 cookie 长期驻留内存（极小内存泄漏）
    // 新账号下次刷新会按需重新登录写入
    state.cookies.lock().unwrap().clear();
    Ok(email)
}

#[tauri::command]
async fn remove_profile(name: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let paths = Paths::resolve()?;
        switcher::remove_profile(&paths, &name)
    })
    .await
    .map_err(|e| format!("后台任务失败: {e}"))?
}

// ============ 监控（reclaude.ai）============

fn snapshot_err(org: String, msg: String, bad: bool) -> MonitorSnapshot {
    MonitorSnapshot {
        quota: None,
        metrics: None,
        org_id: org,
        error: Some(msg),
        bad_credentials: bad,
    }
}

async fn ensure_cookie(
    state: &AppState,
    email: &str,
    password: &str,
    force: bool,
) -> Result<String, MonErr> {
    if !force {
        let cached = {
            let map = state.cookies.lock().unwrap();
            map.get(email).cloned()
        };
        if let Some(c) = cached {
            return Ok(c);
        }
    }
    let c = monitor::login(&state.client, email, password).await?;
    state
        .cookies
        .lock()
        .unwrap()
        .insert(email.to_string(), c.clone());
    Ok(c)
}

struct Resolved {
    quota: Option<monitor::QuotaOut>,
    org_id: String,
    error: Option<String>,
    bad: bool,
    cookie: Option<String>,
}

/// 登录（缓存 Cookie）→ 必要时自动探测 org_id → 拉取额度，带一次鉴权重登重试。
/// 不含 metrics，供单账号卡片复用。
async fn resolve_quota(
    state: &AppState,
    email: &str,
    password: &str,
    org_id: &str,
) -> Resolved {
    let mut cookie = match ensure_cookie(state, email, password, false).await {
        Ok(c) => c,
        Err(MonErr::BadCredentials(m)) => {
            return Resolved { quota: None, org_id: org_id.to_string(), error: Some(m), bad: true, cookie: None }
        }
        Err(e) => {
            return Resolved { quota: None, org_id: org_id.to_string(), error: Some(e.message()), bad: false, cookie: None }
        }
    };

    let mut org = org_id.trim().to_string();
    if org.is_empty() {
        if let Ok(list) = monitor::list_allocations(&state.client, &cookie).await {
            if let Some(first) = list.first() {
                org = first.id.clone();
            }
        }
    }

    if org.is_empty() {
        return Resolved { quota: None, org_id: org, error: None, bad: false, cookie: Some(cookie) };
    }

    let mut attempt = 0;
    loop {
        attempt += 1;
        match monitor::fetch_quota(&state.client, &cookie, &org).await {
            Ok(quota) => {
                return Resolved { quota, org_id: org, error: None, bad: false, cookie: Some(cookie) }
            }
            Err(MonErr::Auth(_)) if attempt < 2 => {
                match ensure_cookie(state, email, password, true).await {
                    Ok(c) => {
                        cookie = c;
                        continue;
                    }
                    Err(MonErr::BadCredentials(m)) => {
                        return Resolved { quota: None, org_id: org, error: Some(m), bad: true, cookie: None }
                    }
                    Err(e) => {
                        return Resolved { quota: None, org_id: org, error: Some(e.message()), bad: false, cookie: None }
                    }
                }
            }
            Err(e) => {
                return Resolved { quota: None, org_id: org, error: Some(e.message()), bad: false, cookie: Some(cookie) }
            }
        }
    }
}

#[tauri::command]
async fn refresh_monitor(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
    org_id: String,
) -> Result<MonitorSnapshot, String> {
    let r = resolve_quota(&state, &email, &password, &org_id).await;
    if r.bad {
        return Ok(snapshot_err(r.org_id, r.error.unwrap_or_default(), true));
    }
    let metrics = match &r.cookie {
        Some(c) => monitor::fetch_metrics(&state.client, c).await.ok(),
        None => None,
    };
    Ok(MonitorSnapshot {
        quota: r.quota,
        metrics,
        org_id: r.org_id,
        error: r.error,
        bad_credentials: false,
    })
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountStatus {
    quota: Option<monitor::QuotaOut>,
    org_id: String,
    error: Option<String>,
    bad_credentials: bool,
}

#[tauri::command]
async fn account_status(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
    org_id: String,
) -> Result<AccountStatus, String> {
    let r = resolve_quota(&state, &email, &password, &org_id).await;
    Ok(AccountStatus {
        quota: r.quota,
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
    let client = state.client.clone();
    let cookie = ensure_cookie(&state, &email, &password, false)
        .await
        .map_err(|e| e.message())?;
    match monitor::list_allocations(&client, &cookie).await {
        Ok(list) => Ok(list),
        Err(MonErr::Auth(_)) => {
            let c = ensure_cookie(&state, &email, &password, true)
                .await
                .map_err(|e| e.message())?;
            monitor::list_allocations(&client, &c)
                .await
                .map_err(|e| e.message())
        }
        Err(e) => Err(e.message()),
    }
}

#[tauri::command]
async fn usage_devices(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
) -> Result<Vec<monitor::Device>, String> {
    let cookie = ensure_cookie(&state, &email, &password, false)
        .await
        .map_err(|e| e.message())?;
    match monitor::fetch_devices(&state.client, &cookie).await {
        Ok(d) => Ok(d),
        Err(MonErr::Auth(_)) => {
            let c = ensure_cookie(&state, &email, &password, true)
                .await
                .map_err(|e| e.message())?;
            monitor::fetch_devices(&state.client, &c)
                .await
                .map_err(|e| e.message())
        }
        Err(e) => Err(e.message()),
    }
}

#[tauri::command]
async fn usage_sync(
    state: tauri::State<'_, AppState>,
    email: String,
    password: String,
) -> Result<(), String> {
    let cookie = ensure_cookie(&state, &email, &password, false)
        .await
        .map_err(|e| e.message())?;
    match monitor::sync_usage(&state.client, &cookie).await {
        Ok(()) => Ok(()),
        Err(MonErr::Auth(_)) => {
            let c = ensure_cookie(&state, &email, &password, true)
                .await
                .map_err(|e| e.message())?;
            monitor::sync_usage(&state.client, &c)
                .await
                .map_err(|e| e.message())
        }
        Err(e) => Err(e.message()),
    }
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
    let did = device_id.as_deref().filter(|s| !s.is_empty());
    let cookie = ensure_cookie(&state, &email, &password, false)
        .await
        .map_err(|e| e.message())?;
    match monitor::fetch_usage_stats(&state.client, &cookie, &range, did, &org_id).await {
        Ok(s) => Ok(s),
        Err(MonErr::Auth(_)) => {
            let c = ensure_cookie(&state, &email, &password, true)
                .await
                .map_err(|e| e.message())?;
            monitor::fetch_usage_stats(&state.client, &c, &range, did, &org_id)
                .await
                .map_err(|e| e.message())
        }
        Err(e) => Err(e.message()),
    }
}

// ============ 悬浮球窗口 ============

#[tauri::command]
fn toggle_float(app: tauri::AppHandle) -> Result<bool, String> {
    let w = app
        .get_webview_window("float")
        .ok_or_else(|| "找不到悬浮球窗口".to_string())?;
    let visible = w.is_visible().map_err(|e| e.to_string())?;
    if visible {
        w.hide().map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        w.show().map_err(|e| e.to_string())?;
        let _ = w.set_always_on_top(true);
        let _ = w.set_focus();
        Ok(true)
    }
}

#[tauri::command]
fn hide_float(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("float") {
        w.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn show_main(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    Ok(())
}

/// 设置悬浮球为正方形尺寸。resizable=false 在部分平台会把窗口 min=max 锁死，
/// 导致 set_size 无效，所以临时放开再设。
fn set_float_size(f: &tauri::WebviewWindow, size: f64) {
    if size > 0.0 {
        let _ = f.set_resizable(true);
        let _ = f.set_size(tauri::LogicalSize::new(size, size));
        let _ = f.set_resizable(false);
    }
}

/// 最小化为悬浮球：按设置的尺寸调整悬浮球 + 显示（置顶）+ 最小化主窗口
/// （Windows 即收进任务栏，macOS 收进 Dock）。
#[tauri::command]
fn minimize_to_float(app: tauri::AppHandle, size: f64) -> Result<(), String> {
    if let Some(f) = app.get_webview_window("float") {
        // WebView 默认白底，需显式透明，圆外四角才不会是白色方块
        let _ = f.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
        set_float_size(&f, size);
        let _ = f.show();
        let _ = f.set_always_on_top(true);
        let _ = f.set_focus();
    }
    if let Some(m) = app.get_webview_window("main") {
        let _ = m.minimize();
    }
    Ok(())
}

/// 实时调整悬浮球尺寸（设置里改大小时即时生效，无需重新进悬浮球）。
#[tauri::command]
fn resize_float(app: tauri::AppHandle, size: f64) -> Result<(), String> {
    if let Some(f) = app.get_webview_window("float") {
        set_float_size(&f, size);
    }
    Ok(())
}

/// 仅显示悬浮球（不动主窗口）——启动时主窗口本就隐藏，用这个避免最小化已隐藏窗口。
#[tauri::command]
fn show_float(app: tauri::AppHandle, size: f64) -> Result<(), String> {
    if let Some(f) = app.get_webview_window("float") {
        let _ = f.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
        set_float_size(&f, size);
        let _ = f.show();
        let _ = f.set_always_on_top(true);
    }
    Ok(())
}

/// 从悬浮球打开主面板：还原并聚焦主窗口（保留悬浮球，不隐藏）。
#[tauri::command]
fn restore_from_float(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(m) = app.get_webview_window("main") {
        let _ = m.unminimize();
        let _ = m.show();
        let _ = m.set_focus();
    }
    Ok(())
}

// ============ 菜单栏圆环（系统托盘）============

/// 显示 / 隐藏托盘圆环图标。
#[tauri::command]
fn set_tray_visible(app: tauri::AppHandle, visible: bool) -> Result<(), String> {
    if let Some(tray) = app.tray_by_id("ring") {
        tray.set_visible(visible).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 用前端 canvas 渲染好的 RGBA 像素更新托盘图标（圆环 + 中间百分比）。
#[tauri::command]
fn update_tray_icon(app: tauri::AppHandle, rgba: Vec<u8>, size: u32) -> Result<(), String> {
    if size == 0 || rgba.len() != (size as usize) * (size as usize) * 4 {
        return Ok(()); // 尺寸不匹配则跳过，避免 panic
    }
    if let Some(tray) = app.tray_by_id("ring") {
        tray.set_icon(Some(Image::new_owned(rgba, size, size)))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 仅最小化主窗口（菜单栏圆环模式用：托盘常驻，最小化时不弹悬浮球）。
#[tauri::command]
fn minimize_main(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(m) = app.get_webview_window("main") {
        let _ = m.minimize();
    }
    Ok(())
}

/// 隐藏主窗口（启动即进入指示器用：主窗口 webview 必须 visible 才会运行，
/// 故启动时先可见、onMount 里尽早隐藏，只留悬浮球/圆环）。
#[tauri::command]
fn hide_main(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(m) = app.get_webview_window("main") {
        let _ = m.hide();
    }
    Ok(())
}

/// 退出整个程序（关闭按钮选择「退出程序」时调用；否则悬浮球窗口会让进程常驻）。
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// 切换菜单栏圆环模式：托盘显隐 + 后台循环开关。
/// interval 是刷新间隔（秒），由前端传入（最小 5）。
#[tauri::command]
fn set_tray_mode(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    active: bool,
    interval: u64,
) -> Result<(), String> {
    state.tray_active.store(active, Ordering::Relaxed);
    state.tray_interval.store(interval.max(5), Ordering::Relaxed);
    if let Some(tray) = app.tray_by_id("ring") {
        let _ = tray.set_visible(active);
    }
    Ok(())
}

// ============ UI 启动配置（供 Rust 启动时决定显示悬浮球还是托盘）============

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct UiConfig {
    mode: String,
    size: f64,
    /// 主面板设的刷新间隔（秒），供悬浮球启动时同步使用；None 表示沿用旧文件值
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh_sec: Option<u64>,
}

fn ui_config_path() -> Option<std::path::PathBuf> {
    switcher::Paths::resolve()
        .ok()
        .map(|p| p.reclaude_dir.join("ui.json"))
}

fn read_ui_config_raw() -> UiConfig {
    if let Some(p) = ui_config_path() {
        if let Ok(s) = std::fs::read_to_string(p) {
            if let Ok(c) = serde_json::from_str::<UiConfig>(&s) {
                return c;
            }
        }
    }
    UiConfig::default()
}

/// 读取上次保存的最小化模式 + 悬浮球尺寸（默认 ball / 160）。
fn read_ui_config() -> (String, f64) {
    let c = read_ui_config_raw();
    let mode = if c.mode == "tray" { "tray" } else { "ball" }.to_string();
    let size = if (30.0..=600.0).contains(&c.size) { c.size } else { 160.0 };
    (mode, size)
}

/// 前端在设置变化时调用，持久化供下次启动用。
/// `refresh_sec` 传 None 时保留文件里的旧值（避免不传字段就丢失）。
#[tauri::command]
fn save_ui_config(mode: String, size: f64, refresh_sec: Option<u64>) -> Result<(), String> {
    let p = ui_config_path().ok_or_else(|| "找不到主目录".to_string())?;
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let merged_sec = refresh_sec.or_else(|| read_ui_config_raw().refresh_sec);
    let json = serde_json::to_string(&UiConfig {
        mode,
        size,
        refresh_sec: merged_sec,
    })
    .map_err(|e| e.to_string())?;
    std::fs::write(p, json).map_err(|e| e.to_string())
}

/// 悬浮球启动时读取主面板设定的刷新间隔；缺失或不合法返回 None，调用方用自己的默认值。
#[tauri::command]
fn get_refresh_sec() -> Option<u64> {
    let s = read_ui_config_raw().refresh_sec?;
    if (5..=3600).contains(&s) { Some(s) } else { None }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("failed to build http client");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
        })
        .setup(|app| {
            // 悬浮球 WebView 背景透明（macOS 下圆外四角才真正透明）
            if let Some(f) = app.get_webview_window("float") {
                let _ = f.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
            }
            // 菜单栏圆环托盘：默认隐藏；点击图标 → 还原主窗口
            let handle = app.handle().clone();
            let tray = TrayIconBuilder::with_id("ring")
                .icon(
                    app.default_window_icon()
                        .cloned()
                        .expect("默认窗口图标缺失"),
                )
                .icon_as_template(false)
                .tooltip("Reclaude 余额")
                .on_tray_icon_event(move |_tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(m) = handle.get_webview_window("main") {
                            let _ = m.unminimize();
                            let _ = m.show();
                            let _ = m.set_focus();
                        }
                    }
                })
                .build(app)?;
            let _ = tray.set_visible(false);

            // 启动按上次模式：圆环 → Rust 自取数据 + tiny-skia 自绘（主窗口全程隐藏、零闪烁）；
            // 悬浮球 → Rust 显示球（球用自己 webview 渲染）。
            let (mode, size) = read_ui_config();
            {
                let s = app.state::<AppState>();
                if mode == "tray" {
                    s.tray_active.store(true, Ordering::Relaxed);
                    let _ = tray.set_visible(true);
                }
            }
            if mode != "tray" {
                if let Some(f) = app.get_webview_window("float") {
                    set_float_size(&f, size);
                    let _ = f.show();
                    let _ = f.set_always_on_top(true);
                }
            }

            // 后台循环：tray_active 时按 tray_interval 拉余额 → tiny-skia 画环 → set_icon
            // 拉取失败（无凭证 / 网络错 / 鉴权失败）时指数退避到 5 分钟，避免高频撞 reclaude.ai 触发限流或封号
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut fail_count: u32 = 0;
                loop {
                    let (active, interval) = {
                        let s = app_handle.state::<AppState>();
                        (
                            s.tray_active.load(Ordering::Relaxed),
                            s.tray_interval.load(Ordering::Relaxed).max(5),
                        )
                    };
                    if !active {
                        fail_count = 0;
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        continue;
                    }
                    let data: Option<(f64, (u8, u8, u8))> = async {
                        let paths = switcher::Paths::resolve().ok()?;
                        let email = switcher::current_email(&paths)?;
                        let cred = switcher::get_monitor_cred(&paths, &email)?;
                        let s = app_handle.state::<AppState>();
                        let r = resolve_quota(&s, &cred.email, &cred.password, &cred.org_id).await;
                        let q = r.quota?;
                        if q.total_usd <= 0.0 {
                            return None;
                        }
                        let avail = (q.remaining_usd / q.total_usd * 100.0).clamp(0.0, 100.0);
                        Some((avail, tray_ring::color_for_ratio(q.ratio)))
                    }
                    .await;
                    let sleep_secs = if let Some((avail, color)) = data {
                        fail_count = 0;
                        let rgba = tray_ring::render_ring(avail, color);
                        if let Some(tray) = app_handle.tray_by_id("ring") {
                            let _ = tray.set_icon(Some(tauri::image::Image::new_owned(
                                rgba,
                                tray_ring::SIZE,
                                tray_ring::SIZE,
                            )));
                        }
                        interval
                    } else {
                        fail_count = fail_count.saturating_add(1);
                        // 退避：interval × 2^min(fail,5)，上限 300s
                        let mult = 1u64 << fail_count.min(5);
                        interval.saturating_mul(mult).min(300).max(interval)
                    };
                    tokio::time::sleep(std::time::Duration::from_secs(sleep_secs)).await;
                }
            });
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
            toggle_float,
            hide_float,
            show_main,
            minimize_to_float,
            restore_from_float,
            resize_float,
            show_float,
            set_tray_visible,
            update_tray_icon,
            minimize_main,
            hide_main,
            quit_app,
            save_ui_config,
            get_refresh_sec,
            set_tray_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

mod monitor;
mod switcher;

use monitor::{Allocation, MonErr, MonitorSnapshot};
use std::collections::HashMap;
use std::sync::Mutex;
use switcher::{EnvInfo, MonitorCred, Paths, ProfileInfo};
use tauri::Manager;

struct AppState {
    client: reqwest::Client,
    cookies: Mutex<HashMap<String, String>>,
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
async fn use_profile(name: String, no_app: bool) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let paths = Paths::resolve()?;
        switcher::use_profile(&paths, &name, no_app)
    })
    .await
    .map_err(|e| format!("后台任务失败: {e}"))?
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("failed to build http client");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            client,
            cookies: Mutex::new(HashMap::new()),
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

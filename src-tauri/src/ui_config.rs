// UI 启动配置（~/.reclaude/ui.json）：最小化模式、悬浮球尺寸、刷新间隔、API 根地址。
// Rust 启动时据此决定显示形态；悬浮球窗口读取刷新间隔；监控会话读取 api_base 决定域名策略。

use crate::switcher;

// 取值范围与前端 src/lib/settings.svelte.ts 的常量保持一致
const REFRESH_SEC_MIN: u64 = 5;
const REFRESH_SEC_MAX: u64 = 3600;
const FLOAT_SIZE_MIN: f64 = 30.0;
const FLOAT_SIZE_MAX: f64 = 600.0;
const FLOAT_SIZE_DEFAULT: f64 = 160.0;

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct UiConfig {
    pub mode: String,
    pub size: f64,
    /// 主面板设的刷新间隔（秒），供悬浮球启动时同步使用；None 表示沿用旧文件值
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_sec: Option<u64>,
    /// 监控 API 根地址；None/空字符串表示使用默认地址并允许自动切备用域名。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,
    /// 静默启动：true=启动直接进后台指示器（圆环/悬浮球）不弹主窗口。
    /// None（旧文件无此字段）按 true 处理，沿用既有「非首次启动不弹主窗口」的行为。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub silent: Option<bool>,
}

pub fn path() -> Option<std::path::PathBuf> {
    switcher::Paths::resolve()
        .ok()
        .map(|p| p.reclaude_dir.join("ui.json"))
}

pub fn read_raw() -> UiConfig {
    if let Some(p) = path() {
        if let Ok(s) = std::fs::read_to_string(p) {
            if let Ok(c) = serde_json::from_str::<UiConfig>(&s) {
                return c;
            }
        }
    }
    UiConfig::default()
}

/// 读取上次保存的最小化模式 + 悬浮球尺寸，非法值回退默认（ball / 160）。
pub fn startup_mode() -> (String, f64) {
    let c = read_raw();
    let mode = if c.mode == "tray" { "tray" } else { "ball" }.to_string();
    let size = if (FLOAT_SIZE_MIN..=FLOAT_SIZE_MAX).contains(&c.size) {
        c.size
    } else {
        FLOAT_SIZE_DEFAULT
    };
    (mode, size)
}

/// 读取静默启动开关；缺省（旧文件无字段）回退 true，保持既有「非首次启动不弹主窗口」的行为。
pub fn silent_start() -> bool {
    read_raw().silent.unwrap_or(true)
}

pub fn normalize_api_base(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{trimmed}")
    };
    Some(with_scheme.trim_end_matches('/').to_string())
}

/// 用户配置的 API 根地址；None 表示使用默认地址 + 自动备用域名。
pub fn configured_api_base() -> Option<String> {
    read_raw().api_base.as_deref().and_then(normalize_api_base)
}

/// 前端在设置变化时调用，持久化供下次启动用。
/// `refresh_sec` / `silent` 传 None 时保留文件里的旧值（避免不传字段就丢失）。
#[tauri::command]
pub fn save_ui_config(
    mode: String,
    size: f64,
    refresh_sec: Option<u64>,
    api_base: Option<String>,
    silent: Option<bool>,
) -> Result<(), String> {
    let p = path().ok_or_else(|| "找不到主目录".to_string())?;
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let old = read_raw();
    let merged_sec = refresh_sec.or(old.refresh_sec);
    let merged_silent = silent.or(old.silent);
    let merged_api_base = match api_base {
        Some(v) => normalize_api_base(&v),
        None => old.api_base.and_then(|v| normalize_api_base(&v)),
    };
    let json = serde_json::to_string(&UiConfig {
        mode,
        size,
        refresh_sec: merged_sec,
        api_base: merged_api_base,
        silent: merged_silent,
    })
    .map_err(|e| e.to_string())?;
    std::fs::write(p, json).map_err(|e| e.to_string())
}

/// 悬浮球启动时读取主面板设定的刷新间隔；缺失或不合法返回 None，调用方用自己的默认值。
#[tauri::command]
pub fn get_refresh_sec() -> Option<u64> {
    let s = read_raw().refresh_sec?;
    if (REFRESH_SEC_MIN..=REFRESH_SEC_MAX).contains(&s) {
        Some(s)
    } else {
        None
    }
}

/// 主面板设置弹窗读取上次保存的 API 地址；空值表示默认 + 自动备用。
#[tauri::command]
pub fn get_api_base() -> String {
    configured_api_base().unwrap_or_default()
}

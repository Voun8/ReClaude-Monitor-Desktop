// 主窗口 / 悬浮球 / 托盘面板窗口的显隐控制。窗口 label 与 tauri.conf.json 的 windows 定义对应。

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::Manager;

use crate::AppState;

pub const WIN_MAIN: &str = "main";
pub const WIN_FLOAT: &str = "float";
pub const WIN_PANEL: &str = "panel";

/// 还原并聚焦主窗口——托盘左键/菜单、二次启动、悬浮球点击共用。
pub fn focus_main(app: &tauri::AppHandle) {
    if let Some(m) = app.get_webview_window(WIN_MAIN) {
        let _ = m.unminimize();
        let _ = m.show();
        let _ = m.set_focus();
    }
}

/// 设置悬浮球为正方形尺寸。resizable=false 在部分平台会把窗口 min=max 锁死，
/// 导致 set_size 无效，所以临时放开再设。
pub fn set_float_size(f: &tauri::WebviewWindow, size: f64) {
    if size > 0.0 {
        let _ = f.set_resizable(true);
        let _ = f.set_size(tauri::LogicalSize::new(size, size));
        let _ = f.set_resizable(false);
    }
}

/// 显示悬浮球：透明背景 + 按尺寸调整 + 置顶。
/// WebView 默认白底，需显式透明，圆外四角才不会是白色方块。
pub fn show_float_window(app: &tauri::AppHandle, size: f64) -> Option<tauri::WebviewWindow> {
    let f = app.get_webview_window(WIN_FLOAT)?;
    let _ = f.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
    set_float_size(&f, size);
    let _ = f.show();
    let _ = f.set_always_on_top(true);
    Some(f)
}

#[tauri::command]
pub fn hide_float(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(WIN_FLOAT) {
        w.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// 最小化为悬浮球：显示球（置顶、聚焦）+ 隐藏主窗口
/// （hide 而非 minimize——minimize 在 Windows 会残留任务栏按钮，hide 才能彻底移出任务栏，只留悬浮球）。
#[tauri::command]
pub fn minimize_to_float(app: tauri::AppHandle, size: f64) -> Result<(), String> {
    if let Some(f) = show_float_window(&app, size) {
        let _ = f.set_focus();
    }
    if let Some(m) = app.get_webview_window(WIN_MAIN) {
        let _ = m.hide();
    }
    Ok(())
}

/// 实时调整悬浮球尺寸（设置里改大小时即时生效，无需重新进悬浮球）。
#[tauri::command]
pub fn resize_float(app: tauri::AppHandle, size: f64) -> Result<(), String> {
    if let Some(f) = app.get_webview_window(WIN_FLOAT) {
        set_float_size(&f, size);
    }
    Ok(())
}

/// 从悬浮球打开主面板：还原并聚焦主窗口（保留悬浮球，不隐藏）。
#[tauri::command]
pub fn restore_from_float(app: tauri::AppHandle) -> Result<(), String> {
    focus_main(&app);
    Ok(())
}

/// 隐藏主窗口（启动即进入指示器用：主窗口 webview 必须 visible 才会运行，
/// 故启动时先可见、onMount 里尽早隐藏，只留悬浮球/圆环）。
#[tauri::command]
pub fn hide_main(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(m) = app.get_webview_window(WIN_MAIN) {
        let _ = m.hide();
    }
    Ok(())
}

/// 退出整个程序（关闭按钮选择「退出程序」时调用；否则悬浮球窗口会让进程常驻）。
#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

// ============ 托盘面板（菜单栏 popover）============

/// 托盘面板：在托盘图标下方弹出的紧凑信息面板（仿菜单栏 popover）。
/// 失焦自动隐藏；再次点击托盘时若刚因失焦隐藏（<250ms）则视为「关闭」，实现稳定 toggle，
/// 避免点击托盘先触发失焦隐藏、随后又被重新弹出的闪烁。
pub fn toggle_panel(app: &tauri::AppHandle) {
    let Some(p) = app.get_webview_window(WIN_PANEL) else {
        return;
    };
    if p.is_visible().unwrap_or(false) {
        hide_panel(app, &p);
        return;
    }
    let recently_hidden = app
        .state::<AppState>()
        .panel_hidden_at
        .lock()
        .unwrap()
        .elapsed()
        < Duration::from_millis(250);
    if recently_hidden {
        return;
    }
    // 透明面板显示前的两步（与悬浮球 show_float_window 对齐，缺一不可）：
    // 1) 重设透明背景色；
    // 2) 强制一次真实 resize 触发重绘——macOS 上透明窗口必须经一次重绘透明才生效（tauri#10306）。
    //    悬浮球每次显示都 set_size 故始终透明；面板尺寸固定、显示时从不 resize，透明背景没被「冲」
    //    出来，露出不透明黑底，正是面板黑色底色的根因。在 show 前（窗口仍隐藏）微调 1px 再复位，
    //    无可见跳动；resizable=false 会把 min=max 锁死，需临时放开（同 set_float_size）。
    let _ = p.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
    if let Ok(sz) = p.inner_size() {
        let _ = p.set_resizable(true);
        let _ = p.set_size(tauri::PhysicalSize::new(sz.width, sz.height + 1));
        let _ = p.set_size(sz);
        let _ = p.set_resizable(false);
    }
    // 依据托盘事件缓存的图标位置定位面板（tray-icon 特性，跨平台）。
    // 用 TrayCenter 而非 TrayBottomCenter：后者把面板顶边钉在图标处往下展开——
    // macOS 菜单栏在顶部尚可，但 Windows 任务栏在底部，会让面板整块跌出屏幕底部/被任务栏遮住（即「显示不正确」）。
    // TrayCenter 默认把面板放到图标上方（y = tray_y − 高度）：Windows 正好落在任务栏上方；
    // macOS 上 y<0 触发插件回退 y = tray_y，与原 TrayBottomCenter 行为一致，无回归。
    use tauri_plugin_positioner::{Position, WindowExt};
    let _ = p.move_window(Position::TrayCenter);
    // Windows：托盘图标贴近屏幕右缘，居中后面板右侧可能溢出——夹回所在显示器宽度内。
    // （positioner 2.3.2 无 move_window_constrained；仅在会溢出时纠正一次，正常居中不受影响。）
    #[cfg(target_os = "windows")]
    if let (Ok(Some(mon)), Ok(pos), Ok(sz)) =
        (p.current_monitor(), p.outer_position(), p.outer_size())
    {
        let left = mon.position().x;
        let max_x = (left + mon.size().width as i32 - sz.width as i32).max(left);
        let x = pos.x.clamp(left, max_x);
        if x != pos.x {
            let _ = p.set_position(tauri::PhysicalPosition::new(x, pos.y));
        }
    }
    let _ = p.show();
    let _ = p.set_focus();
}

/// 隐藏托盘面板并记录隐藏时刻（供 toggle 的失焦竞态防抖）。
pub fn hide_panel(app: &tauri::AppHandle, p: &tauri::WebviewWindow) {
    *app.state::<AppState>().panel_hidden_at.lock().unwrap() = Instant::now();
    let _ = p.hide();
}

/// 从托盘面板打开完整主面板；settings=true 时附带打开设置弹窗
/// （经 pending 标志，主窗口挂载 / 聚焦时读取——主窗口此前可能从未挂载、无法直接收事件）。
#[tauri::command]
pub fn open_main(app: tauri::AppHandle, settings: bool) -> Result<(), String> {
    if let Some(p) = app.get_webview_window(WIN_PANEL) {
        hide_panel(&app, &p);
    }
    if settings {
        app.state::<AppState>()
            .pending_settings
            .store(true, Ordering::Relaxed);
    }
    focus_main(&app);
    Ok(())
}

/// 主窗口读取并清除「待打开设置」标志（面板点设置 → 打开主面板时自动弹设置）。
#[tauri::command]
pub fn take_pending_settings(state: tauri::State<'_, AppState>) -> bool {
    state.pending_settings.swap(false, Ordering::Relaxed)
}

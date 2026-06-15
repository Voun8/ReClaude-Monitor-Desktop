// 主窗口 / 悬浮球窗口的显隐控制。窗口 label 与 tauri.conf.json 的 windows 定义对应。

use tauri::Manager;

pub const WIN_MAIN: &str = "main";
pub const WIN_FLOAT: &str = "float";

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

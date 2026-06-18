// 开机自启动：薄封装 tauri-plugin-autostart，前端经 api.ts 调用本模块命令。
// 自启状态由系统持有（Windows 注册表 Run 键 / macOS LaunchAgent plist），不进 ui.json——
// 故只读写 OS、不做本地镜像，避免与系统真值漂移。

#[cfg(desktop)]
use tauri_plugin_autostart::ManagerExt;

/// 查询当前是否已登记开机自启；查询失败按「未启用」处理。
#[tauri::command]
pub fn get_autostart(app: tauri::AppHandle) -> bool {
    #[cfg(desktop)]
    {
        app.autolaunch().is_enabled().unwrap_or(false)
    }
    #[cfg(not(desktop))]
    {
        let _ = app;
        false
    }
}

/// 启用/禁用开机自启；直接写系统、失败把原始错误回传前端提示。
#[tauri::command]
pub fn set_autostart(app: tauri::AppHandle, enable: bool) -> Result<(), String> {
    #[cfg(desktop)]
    {
        let m = app.autolaunch();
        if enable { m.enable() } else { m.disable() }.map_err(|e| e.to_string())
    }
    #[cfg(not(desktop))]
    {
        let _ = (app, enable);
        Ok(())
    }
}

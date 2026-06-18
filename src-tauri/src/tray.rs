// 系统托盘：常驻图标 + 菜单，以及「菜单栏圆环」模式的后台绘制循环
// （Rust 自取当前账号凭证 → session::resolve_quota 拉余额 → tray_ring 自绘 → set_icon）。

use std::sync::atomic::Ordering;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

use crate::windows::focus_main;
use crate::{session, switcher, tray_ring, AppState};

pub const TRAY_ID: &str = "ring";
/// 刷新间隔下限（秒）——set_tray_mode 与后台循环共用，改一处即可
const MIN_INTERVAL_SECS: u64 = 5;
/// 拉取失败时的指数退避上限
const BACKOFF_MAX_SECS: u64 = 300;
/// 冷启动（还没成功渲染过真实圆环）时的快速重试间隔（秒）
const COLD_START_RETRY_SECS: u64 = 5;
/// 冷启动快速重试的最多次数（约 COLD_START_RETRY_SECS × 次数 ≈ 2 分钟），之后回落指数退避
const COLD_START_MAX_TRIES: u32 = 24;

/// 构建常驻托盘图标：任何模式（悬浮球 / 圆环 / 主窗口）都能从这里退出。
/// 左键 → 还原主窗口；右键 → 菜单「打开主面板 / 退出程序」。
pub fn init(app: &tauri::App) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, "tray_open_main", "打开主面板", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "tray_quit", "退出程序", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let tray_menu = Menu::with_items(app, &[&open_item, &sep, &quit_item])?;
    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(
            app.default_window_icon()
                .cloned()
                .expect("默认窗口图标缺失"),
        )
        .icon_as_template(false)
        .tooltip("Reclaude 控制台")
        .menu(&tray_menu)
        // 左键不弹菜单（macOS 默认会弹）→ 保留左键还原主窗口、右键才弹菜单
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "tray_open_main" => focus_main(app),
            "tray_quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            // 缓存托盘图标位置，供面板 TrayBottomCenter 定位（点击 / 移动都会刷新）
            tauri_plugin_positioner::on_tray_event(app, &event);
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // 左键：在图标下方弹出 / 收起紧凑面板（右键菜单仍可「打开主面板 / 退出」）
                crate::windows::toggle_panel(app);
            }
        })
        .build(app)?;
    let _ = tray.set_visible(true);
    Ok(())
}

/// 托盘图标目标像素尺寸。
/// Windows：按当前 DPI 渲染成通知区实际显示大小（16 逻辑像素 × scale），
/// 系统零缩放 → 无马赛克/毛边。macOS/Linux：沿用原固定 44px。
fn tray_icon_size(app: &tauri::AppHandle) -> u32 {
    #[cfg(target_os = "windows")]
    {
        let scale = app
            .primary_monitor()
            .ok()
            .flatten()
            .map(|m| m.scale_factor())
            .unwrap_or(1.0);
        ((16.0 * scale).round() as u32).clamp(16, 64)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = app;
        44
    }
}

/// 立即把托盘图标渲染成占位「加载中」环（同步、极轻量：仅画一条轨道圆）。
/// 进入圆环模式时调用——避免菜单栏停在默认 App 图标、与设置里「圆环」不一致，
/// 直到后台循环拉到真实数据后替换成带百分比的环。
pub fn show_loading_ring(app: &tauri::AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let size = tray_icon_size(app);
        let rgba = tray_ring::render_loading(size);
        let _ = tray.set_icon(Some(Image::new_owned(rgba, size, size)));
    }
}

/// 拉一次当前账号余额并把圆环画进托盘图标；成功渲染返回 true，无凭证/额度或失败返回 false。
/// 后台循环与「切到圆环模式」共用它——切换时立即渲染，不必等下一次循环 tick 或退避结束。
async fn refresh_tray_icon(app: &tauri::AppHandle) -> bool {
    // 凭证读取是同步磁盘 IO，放到 blocking 线程池，避免阻塞 async 运行时线程
    let cred = tauri::async_runtime::spawn_blocking(|| {
        let paths = switcher::Paths::resolve().ok()?;
        let email = switcher::current_email(&paths)?;
        switcher::get_monitor_cred(&paths, &email)
    })
    .await
    .ok()
    .flatten();
    let data: Option<(f64, (u8, u8, u8))> = async {
        let cred = cred?;
        let s = app.state::<AppState>();
        let r = session::resolve_quota(&s, &cred.email, &cred.password, &cred.org_id).await;
        let q = r.quota?;
        if q.total_usd <= 0.0 {
            return None;
        }
        let avail = (q.remaining_usd / q.total_usd * 100.0).clamp(0.0, 100.0);
        Some((avail, tray_ring::color_for_ratio(q.ratio)))
    }
    .await;
    let Some((avail, color)) = data else {
        return false;
    };
    // 按当前 DPI 渲染成托盘真实显示尺寸 → 系统零缩放，无马赛克/毛边
    let size = tray_icon_size(app);
    let rgba = tray_ring::render_ring(avail, color, size);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_icon(Some(Image::new_owned(rgba, size, size)));
        // Windows：精确百分比放 tooltip，悬停可见（macOS 保持原静态 tooltip）
        #[cfg(target_os = "windows")]
        {
            let _ = tray.set_tooltip(Some(format!("Reclaude 余额 {}%", avail.round() as u32)));
        }
    }
    true
}

/// 切换菜单栏圆环模式：后台循环开关 + 立即渲染/恢复默认图标。
/// interval 是刷新间隔（秒），由前端传入。
#[tauri::command]
pub fn set_tray_mode(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    active: bool,
    interval: u64,
) -> Result<(), String> {
    let was_active = state.tray_active.swap(active, Ordering::Relaxed);
    state
        .tray_interval
        .store(interval.max(MIN_INTERVAL_SECS), Ordering::Relaxed);
    if active {
        // 刚从非圆环切进圆环：先上占位环，避免短暂停留在默认图标；已在圆环模式则不动，避免覆盖真实环造成闪烁。
        if !was_active {
            show_loading_ring(&app);
        }
        // 切到圆环模式时立刻渲染一次圆环，避免「切了但图标半天不变成圆环」：
        // 之前只靠后台循环 tick + 首次登录，无缓存 cookie 时要等数秒，首登失败还会退避到 5 分钟。
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            refresh_tray_icon(&app).await;
        });
    } else if let (Some(icon), Some(tray)) =
        (app.default_window_icon().cloned(), app.tray_by_id(TRAY_ID))
    {
        // 退出圆环模式：托盘常驻不隐藏，恢复成默认 reclaude 图标
        let _ = tray.set_icon(Some(icon));
        let _ = tray.set_tooltip(Some("Reclaude 控制台"));
    }
    Ok(())
}

/// 后台循环：tray_active 时按 tray_interval 拉余额 → tiny-skia 画环 → set_icon。
/// 拉取失败（无凭证 / 网络错 / 鉴权失败）时指数退避，避免高频撞 API 触发限流或封号。
pub fn spawn_ring_loop(app_handle: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut fail_count: u32 = 0;
        // 是否成功渲染过真实圆环：冷启动首拉常因网络/daemon 未就绪而失败，
        // 在首次成功前用快速重试（而非指数退避），让真实百分比环尽快替换占位环。
        let mut ever_ok = false;
        loop {
            let (active, interval) = {
                let s = app_handle.state::<AppState>();
                (
                    s.tray_active.load(Ordering::Relaxed),
                    s.tray_interval
                        .load(Ordering::Relaxed)
                        .max(MIN_INTERVAL_SECS),
                )
            };
            if !active {
                fail_count = 0;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                continue;
            }
            let sleep_secs = if refresh_tray_icon(&app_handle).await {
                fail_count = 0;
                ever_ok = true;
                interval
            } else {
                fail_count = fail_count.saturating_add(1);
                if !ever_ok && fail_count <= COLD_START_MAX_TRIES {
                    // 冷启动还没拉到过数据：短间隔快速重试（占位环已先上屏，这里只为尽快出真实百分比）
                    COLD_START_RETRY_SECS
                } else {
                    // 稳态偶发失败：指数退避 interval × 2^min(fail,5)，上限 BACKOFF_MAX_SECS，避免高频撞 API
                    let mult = 1u64 << fail_count.min(5);
                    interval
                        .saturating_mul(mult)
                        .min(BACKOFF_MAX_SECS)
                        .max(interval)
                }
            };
            tokio::time::sleep(std::time::Duration::from_secs(sleep_secs)).await;
        }
    });
}

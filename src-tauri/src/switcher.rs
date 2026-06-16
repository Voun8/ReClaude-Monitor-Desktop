// Port of rec-switch.ps1 — snapshot-based reclaude account switching.
//
// 账号身份 = ~/.reclaude/device.json + device.key 里的 sk；桌面 App 登录态在
// %APPDATA%\Claude。每个账号整套快照到 ~/.reclaude-profiles/<name>，切换时只换
// 文件 + 重启 daemon/App，不再走浏览器授权。
//
// 与原脚本相比新增：每个 profile 可选附带 monitor.json（邮箱+密码+orgId），用于额度监控。

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System, UpdateKind};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;

// 快照 App 会话时跳过的纯缓存目录（不含登录态，省空间）
const CACHE_EXCLUDES: &[&str] = &[
    "Cache",
    "GPUCache",
    "Code Cache",
    "DawnGraphiteCache",
    "DawnWebGPUCache",
    "GrShaderCache",
    "ShaderCache",
    "blob_storage",
    "Crashpad",
    "logs",
];

#[derive(Clone)]
pub struct Paths {
    pub reclaude_dir: PathBuf,
    pub profiles_dir: PathBuf,
    pub appdata_dir: PathBuf,
    pub device_json: PathBuf,
    pub device_key: PathBuf,
    pub claude_app_prefix: PathBuf,
}

/// 用户主目录：Windows 用 USERPROFILE，Unix（macOS / Linux）用 HOME。
fn home_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let var = "USERPROFILE";
    #[cfg(not(windows))]
    let var = "HOME";
    std::env::var(var)
        .map(PathBuf::from)
        .map_err(|_| format!("找不到 {var} 环境变量"))
}

/// Claude 桌面 App 的数据目录（各平台不同）。
fn claude_app_data_dir(home: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join("AppData").join("Roaming"))
            .join("Claude")
    }
    #[cfg(target_os = "macos")]
    {
        home.join("Library")
            .join("Application Support")
            .join("Claude")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".config"))
            .join("Claude")
    }
}

impl Paths {
    pub fn resolve() -> Result<Paths, String> {
        let home = home_dir()?;
        let reclaude_dir = home.join(".reclaude");
        Ok(Paths {
            profiles_dir: home.join(".reclaude-profiles"),
            appdata_dir: claude_app_data_dir(&home),
            device_json: reclaude_dir.join("device.json"),
            device_key: reclaude_dir.join("device.key"),
            claude_app_prefix: reclaude_dir.join("claude-app"),
            reclaude_dir,
        })
    }

    fn root_creds_file(&self) -> PathBuf {
        self.profiles_dir.join(".monitor-creds.json")
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MonitorCred {
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub org_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    pub name: String,
    pub email: String,
    pub has_app_session: bool,
    pub has_monitor: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvInfo {
    pub reclaude_found: bool,
    pub reclaude_path: String,
    pub profiles_dir: String,
    pub appdata_dir: String,
    pub current_email: Option<String>,
}

/// 构造一个不弹控制台窗口的命令（Windows）。
fn quiet_cmd(program: &Path) -> Command {
    let mut c = Command::new(program);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

#[cfg(windows)]
fn quiet_cmd_str(program: &str) -> Command {
    let mut c = Command::new(program);
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

/// 定位 reclaude 可执行文件：默认安装路径 → PATH 各目录。
fn find_reclaude() -> Option<PathBuf> {
    #[cfg(windows)]
    let bin_name = "reclaude.exe";
    #[cfg(not(windows))]
    let bin_name = "reclaude";

    // 1) 常见安装目录（GUI 启动时 PATH 往往很精简，必须显式探测）
    #[cfg(windows)]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            let p = PathBuf::from(local)
                .join("Programs")
                .join("reclaude")
                .join("bin")
                .join(bin_name);
            if p.exists() {
                return Some(p);
            }
        }
    }
    #[cfg(not(windows))]
    {
        // 官方安装脚本默认装到 ~/.local/bin，可用 $RECLAUDE_INSTALL_DIR 覆盖。
        if let Ok(custom) = std::env::var("RECLAUDE_INSTALL_DIR") {
            let p = PathBuf::from(custom).join(bin_name);
            if p.exists() {
                return Some(p);
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            for sub in [".local/bin", "bin"] {
                let p = PathBuf::from(&home).join(sub).join(bin_name);
                if p.exists() {
                    return Some(p);
                }
            }
        }
        for dir in ["/usr/local/bin", "/opt/homebrew/bin", "/usr/bin"] {
            let p = Path::new(dir).join(bin_name);
            if p.exists() {
                return Some(p);
            }
        }
    }

    // 2) PATH 搜索
    #[cfg(windows)]
    let sep = ';';
    #[cfg(not(windows))]
    let sep = ':';
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(sep) {
            if dir.trim().is_empty() {
                continue;
            }
            let p = Path::new(dir).join(bin_name);
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

/// 从 device.json 读取账号邮箱。
fn read_email(json_path: &Path) -> String {
    if !json_path.exists() {
        return "(none)".to_string();
    }
    match fs::read_to_string(json_path) {
        Ok(raw) => match serde_json::from_str::<serde_json::Value>(&raw) {
            Ok(v) => v
                .get("user_email")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "(none)".to_string()),
            Err(_) => "(parse error)".to_string(),
        },
        Err(_) => "(parse error)".to_string(),
    }
}

pub fn current_email(paths: &Paths) -> Option<String> {
    if !paths.device_json.exists() {
        return None;
    }
    let e = read_email(&paths.device_json);
    if e == "(none)" || e == "(parse error)" {
        None
    } else {
        Some(e)
    }
}

pub fn env_info(paths: &Paths) -> EnvInfo {
    let rec = find_reclaude();
    EnvInfo {
        reclaude_found: rec.is_some(),
        reclaude_path: rec
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
        profiles_dir: paths.profiles_dir.to_string_lossy().to_string(),
        appdata_dir: paths.appdata_dir.to_string_lossy().to_string(),
        current_email: current_email(paths),
    }
}

/// 镜像目录（删除目标里多出来的文件）；可选按名排除子目录。
/// Windows 用 robocopy /MIR，Unix（macOS / Linux）用 rsync -a --delete。
fn mirror_dir(src: &Path, dst: &Path, exclude_dirs: &[&str]) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("创建目录失败 {}: {e}", dst.display()))?;
    #[cfg(windows)]
    {
        let mut cmd = quiet_cmd_str("robocopy");
        cmd.arg(src).arg(dst).args([
            "/MIR", "/NFL", "/NDL", "/NJH", "/NJS", "/NP", "/R:1", "/W:1",
        ]);
        if !exclude_dirs.is_empty() {
            cmd.arg("/XD");
            for d in exclude_dirs {
                cmd.arg(d);
            }
        }
        let status = cmd
            .status()
            .map_err(|e| format!("无法运行 robocopy: {e}"))?;
        // robocopy: 0-7 = OK, >=8 = error
        let code = status.code().unwrap_or(16);
        if code >= 8 {
            return Err(format!(
                "robocopy 失败 (code={code}): {} -> {}",
                src.display(),
                dst.display()
            ));
        }
    }
    #[cfg(not(windows))]
    {
        // 源路径加结尾斜杠 = 复制“目录内容”而非目录本身（对齐 robocopy 语义）。
        let mut cmd = Command::new("rsync");
        cmd.arg("-a").arg("--delete");
        for d in exclude_dirs {
            cmd.arg(format!("--exclude={d}"));
        }
        let mut src_arg = src.as_os_str().to_os_string();
        src_arg.push("/");
        cmd.arg(src_arg).arg(dst);
        let status = cmd.status().map_err(|e| format!("无法运行 rsync: {e}"))?;
        if !status.success() {
            return Err(format!(
                "rsync 失败 (code={:?}): {} -> {}",
                status.code(),
                src.display(),
                dst.display()
            ));
        }
    }
    Ok(())
}

// reclaude state.json 缺失/坏掉时的兜底端口
const DAEMON_PORT_FALLBACK: u16 = 49154;

// 快照前等桌面 App 退出的上限
const APP_EXIT_WAIT_MS: u64 = 3000;

/// 从 ~/.reclaude/state.json 读 daemon 端口。
/// 优先 daemon.port（运行中），其次 last_port（已停时残留的上次值），兜底 49154。
pub fn read_daemon_port(paths: &Paths) -> u16 {
    let raw = match fs::read_to_string(paths.reclaude_dir.join("state.json")) {
        Ok(s) => s,
        Err(_) => return DAEMON_PORT_FALLBACK,
    };
    let v: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return DAEMON_PORT_FALLBACK,
    };
    v.get("daemon")
        .and_then(|d| d.get("port"))
        .and_then(|p| p.as_u64())
        .or_else(|| v.get("last_port").and_then(|p| p.as_u64()))
        .and_then(|p| u16::try_from(p).ok())
        .unwrap_or(DAEMON_PORT_FALLBACK)
}

/// 本地端口是否在 LISTEN。
fn port_listening(port: u16) -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(150)).is_ok()
}

/// 等到指定端口不再 LISTEN，最长 max_ms。
fn wait_port_down(port: u16, max_ms: u64) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
    while std::time::Instant::now() < deadline {
        if !port_listening(port) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    !port_listening(port)
}

/// 等 daemon 启动完成（端口 LISTEN）。每次循环重读 state.json，
/// 这样即便 daemon 重启换了端口也能跟上。返回最终 port，失败返回 None。
fn wait_daemon_up(paths: &Paths, max_ms: u64) -> Option<u16> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
    loop {
        let port = read_daemon_port(paths);
        if port_listening(port) {
            return Some(port);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(80));
    }
}

/// 校验档案名：拒绝路径分隔符、`..`、前导 `.` —— 防止穿越到 profiles_dir 之外。
fn validate_profile_name(name: &str) -> Result<&str, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("档案名不能为空".to_string());
    }
    if n.contains('/') || n.contains('\\') || n.contains("..") || n.starts_with('.') {
        return Err("档案名不能包含 / \\ .. 或以 . 开头".to_string());
    }
    Ok(n)
}

/// 把含敏感数据的文件权限设为 0o600（仅本人可读写）。Windows 无 unix mode，跳过。
#[cfg(unix)]
fn set_secret_mode(p: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(p, fs::Permissions::from_mode(0o600));
}
#[cfg(not(unix))]
fn set_secret_mode(_p: &Path) {}

// ===== macOS 设备签名私钥（Keychain）=====
// reclaude 把设备 Ed25519 签名 seed 存在 macOS Keychain（service "Claude Code-device-key"，
// account = 登录用户名），不在 device.json。换账号必须连这把 seed 一起换，否则 daemon 用旧
// 账号的私钥去签新账号的请求，签名指纹不匹配 → 网关 502。这里在 save/use 时一并快照/还原。
#[cfg(target_os = "macos")]
const KEYCHAIN_SERVICE: &str = "Claude Code-device-key";

#[cfg(target_os = "macos")]
fn keychain_account() -> String {
    std::env::var("USER").unwrap_or_default()
}

/// 从 Keychain 读出当前设备签名 seed（失败/被拒/不存在返回 None）。
#[cfg(target_os = "macos")]
fn read_device_seed() -> Option<String> {
    let acct = keychain_account();
    let out = Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            &acct,
            "-w",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 把 seed 写回 Keychain 的那个槽（-U：存在则更新）。必须在重启 daemon 之前调用。
#[cfg(target_os = "macos")]
fn write_device_seed(seed: &str) -> Result<(), String> {
    let acct = keychain_account();
    let status = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            &acct,
            "-w",
            seed,
        ])
        .status()
        .map_err(|e| format!("写 Keychain seed 失败: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("security add-generic-password 写 seed 失败（Keychain 授权被拒？）".to_string())
    }
}

/// exe 路径是否在 .reclaude\claude-app 前缀下（即桌面 App 进程，绝不匹配 CLI claude）。
fn is_claude_app_proc(prefix: &str, process: &sysinfo::Process) -> bool {
    process
        .exe()
        .map(|e| e.to_string_lossy().to_lowercase().starts_with(prefix))
        .unwrap_or(false)
}

/// 进程扫描只按 exe / cmd 匹配，故只采集这两项，跳过 new_all() 的 CPU/内存/磁盘/用户全量采集。
fn proc_refresh_kind() -> ProcessRefreshKind {
    ProcessRefreshKind::nothing()
        .with_exe(UpdateKind::Always)
        .with_cmd(UpdateKind::Always)
}

fn scan_processes() -> System {
    System::new_with_specifics(RefreshKind::nothing().with_processes(proc_refresh_kind()))
}

/// 只杀"桌面 App"（路径在 .reclaude\claude-app 下），绝不动正在运行的 CLI claude。
fn stop_claude_app(paths: &Paths) {
    let prefix = paths.claude_app_prefix.to_string_lossy().to_lowercase();
    let sys = scan_processes();
    for process in sys.processes().values() {
        if is_claude_app_proc(&prefix, process) {
            process.kill();
        }
    }
}

/// 等桌面 App 进程全部退出（kill 是异步的），最长 max_ms；超时返回 false。
fn wait_claude_app_down(paths: &Paths, max_ms: u64) -> bool {
    let prefix = paths.claude_app_prefix.to_string_lossy().to_lowercase();
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(max_ms);
    // 复用同一 System 实例,循环内只重采集进程,避免每轮重建整个 System
    let mut sys = scan_processes();
    loop {
        if !sys
            .processes()
            .values()
            .any(|p| is_claude_app_proc(&prefix, p))
        {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, proc_refresh_kind());
    }
}

/// 强制回收所有残留的 reclaude daemon/_daemon 进程，避免每次切换越积越多。
/// 仅匹配命令行里带 "daemon" 的 reclaude 进程：交互式 reclaude/claude 会话、
/// 以及本监控 App 自身（命令行均无 daemon）都不会被误杀。
fn stop_reclaude_daemons() {
    let sys = scan_processes();
    for process in sys.processes().values() {
        let Some(exe) = process.exe() else {
            continue;
        };
        if !exe.to_string_lossy().to_lowercase().contains("reclaude") {
            continue;
        }
        let is_daemon = process
            .cmd()
            .iter()
            .any(|a| a.to_string_lossy().to_lowercase().contains("daemon"));
        if is_daemon {
            process.kill();
        }
    }
}

// ============ 档案监控凭证（monitor.json + 根映射）============

/// 读根映射。文件不存在视为空表；存在但解析失败必须报错——
/// 绝不能拿空表继续走「insert 后整文件覆盖写」，否则一次损坏就静默丢光全部凭证。
fn read_root_creds(paths: &Paths) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let p = paths.root_creds_file();
    if !p.exists() {
        return Ok(serde_json::Map::new());
    }
    let raw = fs::read_to_string(&p).map_err(|e| format!("读取 {} 失败: {e}", p.display()))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("{} 解析失败（文件可能损坏，已停止写入）: {e}", p.display()))
}

fn write_root_creds(
    paths: &Paths,
    map: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    fs::create_dir_all(&paths.profiles_dir).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    let p = paths.root_creds_file();
    fs::write(&p, raw).map_err(|e| e.to_string())?;
    set_secret_mode(&p);
    Ok(())
}

fn read_profile_monitor(dir: &Path) -> Option<MonitorCred> {
    let p = dir.join("monitor.json");
    let raw = fs::read_to_string(p).ok()?;
    serde_json::from_str::<MonitorCred>(&raw).ok()
}

fn write_profile_monitor(dir: &Path, cred: &MonitorCred) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(cred).map_err(|e| e.to_string())?;
    let p = dir.join("monitor.json");
    fs::write(&p, raw).map_err(|e| e.to_string())?;
    set_secret_mode(&p);
    Ok(())
}

/// 档案根目录下的全部子目录，按名排序——fs::read_dir 顺序非确定，排序保证结果稳定。
fn sorted_profile_dirs(paths: &Paths) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(&paths.profiles_dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn root_monitor_cred_for(
    map: &serde_json::Map<String, serde_json::Value>,
    email: &str,
) -> Option<MonitorCred> {
    let value = map.get(email).or_else(|| {
        map.iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(email))
            .map(|(_, value)| value)
    })?;
    Some(MonitorCred {
        email: email.to_string(),
        password: value.get("password")?.as_str()?.to_string(),
        org_id: value
            .get("orgId")
            .or_else(|| value.get("org_id"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

/// 查询某邮箱的监控凭证：先扫各档案 monitor.json（邮箱匹配），再回退根映射。
pub fn get_monitor_cred(paths: &Paths, email: &str) -> Option<MonitorCred> {
    let root_cred = match read_root_creds(paths) {
        Ok(map) => root_monitor_cred_for(&map, email),
        Err(e) => {
            eprintln!("[switcher] {e}");
            None
        }
    };
    for path in sorted_profile_dirs(paths) {
        if let Some(mut c) = read_profile_monitor(&path) {
            if c.email.eq_ignore_ascii_case(email) {
                if let Some(root) = &root_cred {
                    if c.password.trim().is_empty() {
                        c.password = root.password.clone();
                    }
                    if c.org_id.trim().is_empty() {
                        c.org_id = root.org_id.clone();
                    }
                }
                return Some(c);
            }
        }
    }
    root_cred
}

/// 写入监控凭证：profile_name 给定则写该档案 monitor.json（凭证随快照走），
/// 否则写根映射（供「当前账号还没有档案」的场景）。两处互斥，不做镜像双写；
/// 读取侧（get/has）先查档案再回退根映射，兼容历史双写产生的旧数据。
pub fn set_monitor_cred(
    paths: &Paths,
    cred: &MonitorCred,
    profile_name: Option<&str>,
) -> Result<(), String> {
    if let Some(name) = profile_name {
        let safe = validate_profile_name(name)?;
        let dir = paths.profiles_dir.join(safe);
        return write_profile_monitor(&dir, cred);
    }
    let mut map = read_root_creds(paths)?;
    map.insert(
        cred.email.clone(),
        serde_json::json!({ "password": cred.password, "orgId": cred.org_id }),
    );
    write_root_creds(paths, &map)
}

// ============ list / save / use / remove ============

pub fn list_profiles(paths: &Paths) -> Vec<ProfileInfo> {
    // 根映射读一次：避免每个档案都各读一遍整表（原 has_monitor_for 每次回退都重读 root_creds）。
    // 严格 contains_key（区分大小写）语义与原实现一致。
    let root_map = read_root_creds(paths).unwrap_or_else(|e| {
        eprintln!("[switcher] {e}");
        serde_json::Map::new()
    });
    let mut out = Vec::new();
    for dir in sorted_profile_dirs(paths) {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let device = dir.join("device.json");
        if !device.exists() {
            continue;
        }
        let email = read_email(&device);
        let has_app_session = dir.join("claude-app-data").exists();
        let has_monitor = read_profile_monitor(&dir).is_some() || root_map.contains_key(&email);
        out.push(ProfileInfo {
            name,
            email,
            has_app_session,
            has_monitor,
        });
    }
    out
}

pub fn save_profile(
    paths: &Paths,
    name: &str,
    monitor: Option<MonitorCred>,
) -> Result<String, String> {
    let safe = validate_profile_name(name)?;
    if !paths.device_json.exists() {
        return Err(
            "当前没有登录（找不到 device.json）。请先用 reclaude 登录目标账号再保存。".to_string(),
        );
    }
    let dest = paths.profiles_dir.join(safe);
    fs::create_dir_all(&dest).map_err(|e| format!("创建档案目录失败: {e}"))?;
    let dev_json_dst = dest.join("device.json");
    fs::copy(&paths.device_json, &dev_json_dst)
        .map_err(|e| format!("复制 device.json 失败: {e}"))?;
    set_secret_mode(&dev_json_dst);
    if paths.device_key.exists() {
        let dev_key_dst = dest.join("device.key");
        fs::copy(&paths.device_key, &dev_key_dst)
            .map_err(|e| format!("复制 device.key 失败: {e}"))?;
        set_secret_mode(&dev_key_dst);
    }
    // macOS：device.key 不存在，签名私钥在 Keychain。把它一并快照进档案，
    // 否则切回此档案时签名指纹不匹配 → 502。读不到（被拒/无项）则跳过。
    #[cfg(target_os = "macos")]
    {
        if let Some(seed) = read_device_seed() {
            let seed_dst = dest.join("device.seed");
            fs::write(&seed_dst, seed).map_err(|e| format!("写 device.seed 失败: {e}"))?;
            set_secret_mode(&seed_dst);
        }
    }
    if paths.appdata_dir.exists() {
        // 镜像前先停桌面 App 并确认进程真正退出，避免拷到不一致的 SQLite/leveldb
        stop_claude_app(paths);
        if !wait_claude_app_down(paths, APP_EXIT_WAIT_MS) {
            eprintln!("[switcher] 桌面 App 未在 {APP_EXIT_WAIT_MS}ms 内退出，快照可能不一致");
        }
        mirror_dir(
            &paths.appdata_dir,
            &dest.join("claude-app-data"),
            CACHE_EXCLUDES,
        )?;
    }
    if let Some(cred) = monitor {
        if !cred.password.is_empty() {
            set_monitor_cred(paths, &cred, Some(safe))?;
        }
    }
    Ok(read_email(&dev_json_dst))
}

pub fn use_profile(paths: &Paths, name: &str, no_app: bool) -> Result<String, String> {
    let safe = validate_profile_name(name)?;
    let src = paths.profiles_dir.join(safe);
    if !src.join("device.json").exists() {
        return Err(format!("档案 '{safe}' 不存在。"));
    }
    let rec = find_reclaude().ok_or_else(|| "未找到 reclaude，请先安装。".to_string())?;
    let email = read_email(&src.join("device.json"));

    // 记下切换前 daemon 在用的端口，用于等它真正退出
    let port_before = read_daemon_port(paths);

    // 1) 停桌面 App
    stop_claude_app(paths);
    // 2) 停 daemon（优雅停止：正在 streaming 的请求会继续完成）
    let _ = quiet_cmd(&rec).arg("stop").output();
    // 3) 等端口释放再覆盖凭据；否则新 daemon bind 会失败。
    //    失败必须早返回 —— 否则会在 daemon 仍占端口时改写 device.key，半成功状态。
    if !wait_port_down(port_before, 2000) {
        return Err("daemon 未在 2s 内停止，请稍后重试切换。".to_string());
    }
    // 3.5) 兜底回收历次切换残留的 daemon 进程：reclaude stop 只停"当前"那一个，
    //      旧的 launcher / 孤儿进程从不被回收，会越积越多并占住端口/TUN 路由，
    //      最终连新开的 claude 也连不上一个干净 daemon。
    stop_reclaude_daemons();

    // 4) 写凭证
    fs::create_dir_all(&paths.reclaude_dir).map_err(|e| e.to_string())?;
    fs::copy(src.join("device.json"), &paths.device_json)
        .map_err(|e| format!("写入 device.json 失败: {e}"))?;
    set_secret_mode(&paths.device_json);
    let k = src.join("device.key");
    if k.exists() {
        fs::copy(k, &paths.device_key).map_err(|e| format!("写入 device.key 失败: {e}"))?;
        set_secret_mode(&paths.device_key);
    }
    // macOS：把档案里的签名 seed 写回 Keychain（必须在重启 daemon 之前），
    // daemon 重启后才会加载到与新 device.json 匹配的私钥，避免签名指纹不匹配 502。
    // 档案没存 seed（老档案未重存）则跳过——这种情况切过去仍会 502，需重新保存该档案。
    #[cfg(target_os = "macos")]
    {
        if let Ok(seed) = fs::read_to_string(src.join("device.seed")) {
            let seed = seed.trim();
            if !seed.is_empty() {
                write_device_seed(seed)?;
            }
        }
    }

    // 5) 恢复 App 会话（与 save 对称排除 CACHE/logs/Crashpad，否则 --delete 会把这些清空）
    let app_src = src.join("claude-app-data");
    if app_src.exists() {
        mirror_dir(&app_src, &paths.appdata_dir, CACHE_EXCLUDES)?;
    }

    // 6) 拉起 daemon / App，并等端口 LISTEN 才算切换完成
    if no_app {
        let mut c = quiet_cmd(&rec);
        c.arg("daemon");
        #[cfg(windows)]
        c.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
        c.spawn().map_err(|e| format!("启动 daemon 失败: {e}"))?;
        if wait_daemon_up(paths, 3000).is_none() {
            return Err("daemon 启动后端口未监听，可能启动失败".to_string());
        }
    } else {
        quiet_cmd(&rec)
            .arg("app")
            .spawn()
            .map_err(|e| format!("启动桌面 App 失败: {e}"))?;
        // App 会顺带起 daemon，但 App 启动较慢，给 5s；
        // 超时如实上报（与 no_app 分支对称），凭证已写入、App 已拉起，稍后可能自行就绪
        if wait_daemon_up(paths, 5000).is_none() {
            return Err(
                "已写入凭证并启动桌面 App，但 daemon 5s 内未就绪，可能仍在启动中；若未生效请稍后重试".to_string(),
            );
        }
    }
    Ok(email)
}

pub fn remove_profile(paths: &Paths, name: &str) -> Result<(), String> {
    let safe = validate_profile_name(name)?;
    let dir = paths.profiles_dir.join(safe);
    if !dir.exists() {
        return Err(format!("档案 '{safe}' 不存在。"));
    }
    // 同时清理根映射里该档案的监控凭证（含历史双写产生的镜像条目）
    if let Some(cred) = read_profile_monitor(&dir) {
        match read_root_creds(paths) {
            Ok(mut map) => {
                if map.remove(&cred.email).is_some() {
                    let _ = write_root_creds(paths, &map);
                }
            }
            Err(e) => eprintln!("[switcher] {e}"),
        }
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("删除档案失败: {e}"))
}

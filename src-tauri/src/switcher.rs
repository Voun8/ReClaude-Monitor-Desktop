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

impl Paths {
    pub fn resolve() -> Result<Paths, String> {
        let home = std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .map_err(|_| "找不到 USERPROFILE 环境变量".to_string())?;
        let appdata = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join("AppData").join("Roaming"));
        let reclaude_dir = home.join(".reclaude");
        Ok(Paths {
            profiles_dir: home.join(".reclaude-profiles"),
            appdata_dir: appdata.join("Claude"),
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

fn quiet_cmd_str(program: &str) -> Command {
    let mut c = Command::new(program);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

/// 定位 reclaude.exe：默认安装路径 → PATH 各目录。
pub fn find_reclaude() -> Option<PathBuf> {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(local)
            .join("Programs")
            .join("reclaude")
            .join("bin")
            .join("reclaude.exe");
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(';') {
            if dir.trim().is_empty() {
                continue;
            }
            let p = Path::new(dir).join("reclaude.exe");
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

/// 从 device.json 读取账号邮箱。
pub fn read_email(json_path: &Path) -> String {
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

/// robocopy 镜像目录；可选排除子目录。
fn mirror_dir(src: &Path, dst: &Path, exclude_dirs: &[&str]) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("创建目录失败 {}: {e}", dst.display()))?;
    let mut cmd = quiet_cmd_str("robocopy");
    cmd.arg(src)
        .arg(dst)
        .args(["/MIR", "/NFL", "/NDL", "/NJH", "/NJS", "/NP", "/R:1", "/W:1"]);
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
    Ok(())
}

/// 只杀"桌面 App"（路径在 .reclaude\claude-app 下），绝不动正在运行的 CLI claude。
fn stop_claude_app(paths: &Paths) {
    use sysinfo::System;
    let prefix = paths
        .claude_app_prefix
        .to_string_lossy()
        .to_lowercase();
    let mut sys = System::new_all();
    sys.refresh_all();
    for (_pid, process) in sys.processes() {
        if let Some(exe) = process.exe() {
            let exe_lower = exe.to_string_lossy().to_lowercase();
            if exe_lower.starts_with(&prefix) {
                process.kill();
            }
        }
    }
}

// ============ 档案监控凭证（monitor.json + 根映射）============

fn read_root_creds(paths: &Paths) -> serde_json::Map<String, serde_json::Value> {
    match fs::read_to_string(paths.root_creds_file()) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
        Err(_) => serde_json::Map::new(),
    }
}

fn write_root_creds(
    paths: &Paths,
    map: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    fs::create_dir_all(&paths.profiles_dir).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    fs::write(paths.root_creds_file(), raw).map_err(|e| e.to_string())
}

fn read_profile_monitor(dir: &Path) -> Option<MonitorCred> {
    let p = dir.join("monitor.json");
    let raw = fs::read_to_string(p).ok()?;
    serde_json::from_str::<MonitorCred>(&raw).ok()
}

fn write_profile_monitor(dir: &Path, cred: &MonitorCred) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(cred).map_err(|e| e.to_string())?;
    fs::write(dir.join("monitor.json"), raw).map_err(|e| e.to_string())
}

/// 查询某邮箱的监控凭证：先扫各档案 monitor.json（邮箱匹配），再回退根映射。
pub fn get_monitor_cred(paths: &Paths, email: &str) -> Option<MonitorCred> {
    if paths.profiles_dir.exists() {
        if let Ok(entries) = fs::read_dir(&paths.profiles_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if let Some(c) = read_profile_monitor(&path) {
                    if c.email.eq_ignore_ascii_case(email) {
                        return Some(c);
                    }
                }
            }
        }
    }
    let map = read_root_creds(paths);
    map.get(email).and_then(|v| {
        Some(MonitorCred {
            email: email.to_string(),
            password: v.get("password")?.as_str()?.to_string(),
            org_id: v
                .get("orgId")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
        })
    })
}

/// 写入监控凭证：profile_name 给定则写该档案 monitor.json；总是同步根映射。
pub fn set_monitor_cred(
    paths: &Paths,
    cred: &MonitorCred,
    profile_name: Option<&str>,
) -> Result<(), String> {
    if let Some(name) = profile_name {
        let dir = paths.profiles_dir.join(name);
        write_profile_monitor(&dir, cred)?;
    }
    let mut map = read_root_creds(paths);
    map.insert(
        cred.email.clone(),
        serde_json::json!({ "password": cred.password, "orgId": cred.org_id }),
    );
    write_root_creds(paths, &map)
}

fn has_monitor_for(paths: &Paths, dir: &Path, email: &str) -> bool {
    if read_profile_monitor(dir).is_some() {
        return true;
    }
    read_root_creds(paths).contains_key(email)
}

// ============ list / save / use / remove ============

pub fn list_profiles(paths: &Paths) -> Vec<ProfileInfo> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&paths.profiles_dir) else {
        return out;
    };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
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
        let has_monitor = has_monitor_for(paths, &dir, &email);
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
    if name.trim().is_empty() {
        return Err("档案名不能为空".to_string());
    }
    if !paths.device_json.exists() {
        return Err("当前没有登录（找不到 device.json）。请先用 reclaude 登录目标账号再保存。".to_string());
    }
    let dest = paths.profiles_dir.join(name);
    fs::create_dir_all(&dest).map_err(|e| format!("创建档案目录失败: {e}"))?;
    fs::copy(&paths.device_json, dest.join("device.json"))
        .map_err(|e| format!("复制 device.json 失败: {e}"))?;
    if paths.device_key.exists() {
        fs::copy(&paths.device_key, dest.join("device.key"))
            .map_err(|e| format!("复制 device.key 失败: {e}"))?;
    }
    if paths.appdata_dir.exists() {
        mirror_dir(
            &paths.appdata_dir,
            &dest.join("claude-app-data"),
            CACHE_EXCLUDES,
        )?;
    }
    if let Some(cred) = monitor {
        if !cred.password.is_empty() {
            set_monitor_cred(paths, &cred, Some(name))?;
        }
    }
    Ok(read_email(&dest.join("device.json")))
}

pub fn use_profile(paths: &Paths, name: &str, no_app: bool) -> Result<String, String> {
    let src = paths.profiles_dir.join(name);
    if !src.join("device.json").exists() {
        return Err(format!("档案 '{name}' 不存在。"));
    }
    let rec = find_reclaude().ok_or_else(|| "未找到 reclaude.exe，请先安装。".to_string())?;
    let email = read_email(&src.join("device.json"));

    // 1) 停桌面 App
    stop_claude_app(paths);
    // 2) 停 daemon
    let _ = quiet_cmd(&rec).arg("stop").output();
    // 3) 等文件锁释放
    std::thread::sleep(std::time::Duration::from_millis(1500));

    // 4) 写凭证
    fs::create_dir_all(&paths.reclaude_dir).map_err(|e| e.to_string())?;
    fs::copy(src.join("device.json"), &paths.device_json)
        .map_err(|e| format!("写入 device.json 失败: {e}"))?;
    let k = src.join("device.key");
    if k.exists() {
        fs::copy(k, &paths.device_key).map_err(|e| format!("写入 device.key 失败: {e}"))?;
    }

    // 5) 恢复 App 会话
    let app_src = src.join("claude-app-data");
    if app_src.exists() {
        mirror_dir(&app_src, &paths.appdata_dir, &[])?;
    }

    // 6) 拉起 daemon / App
    if no_app {
        let mut c = quiet_cmd(&rec);
        c.arg("daemon");
        #[cfg(windows)]
        c.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
        c.spawn().map_err(|e| format!("启动 daemon 失败: {e}"))?;
    } else {
        quiet_cmd(&rec)
            .arg("app")
            .spawn()
            .map_err(|e| format!("启动桌面 App 失败: {e}"))?;
    }
    Ok(email)
}

pub fn remove_profile(paths: &Paths, name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("档案名不能为空".to_string());
    }
    let dir = paths.profiles_dir.join(name);
    if !dir.exists() {
        return Err(format!("档案 '{name}' 不存在。"));
    }
    // 同时清理根映射里该档案的监控凭证
    if let Some(cred) = read_profile_monitor(&dir) {
        let mut map = read_root_creds(paths);
        map.remove(&cred.email);
        let _ = write_root_creds(paths, &map);
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("删除档案失败: {e}"))
}

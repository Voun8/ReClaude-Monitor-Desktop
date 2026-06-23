// 监控 API 的会话编排：cookie 缓存登录、鉴权失败重登一次。
// 上游 failover 已收敛到中转（reclaude-proxy），客户端只走单一地址，不再做域名回退。
// HTTP 细节在 monitor 模块；本模块只负责「带会话地调用 + 鉴权重试」。

use crate::monitor::{self, MonErr};
use crate::ui_config;
use crate::AppState;

fn cookie_key(api_base: &str, email: &str) -> String {
    format!("{api_base}\n{email}")
}

// 单一中转地址：自定义则用自定义，否则用默认中转。
fn monitor_api_base() -> String {
    ui_config::configured_api_base().unwrap_or_else(|| monitor::DEFAULT_API_BASE.to_string())
}

async fn ensure_cookie_at(
    state: &AppState,
    api_base: &str,
    email: &str,
    password: &str,
    force: bool,
) -> Result<String, MonErr> {
    if !force {
        let cached = {
            let map = state.cookies.lock().unwrap();
            map.get(&cookie_key(api_base, email)).cloned()
        };
        if let Some(c) = cached {
            return Ok(c);
        }
    }
    let c = monitor::login(&state.client, api_base, email, password).await?;
    state
        .cookies
        .lock()
        .unwrap()
        .insert(cookie_key(api_base, email), c.clone());
    Ok(c)
}

async fn ensure_cookie(
    state: &AppState,
    email: &str,
    password: &str,
    force: bool,
) -> Result<(String, String), MonErr> {
    let api_base = monitor_api_base();
    let cookie = ensure_cookie_at(state, &api_base, email, password, force).await?;
    Ok((api_base, cookie))
}

/// 会话调用原语（返回原始 MonErr 供需要区分错误类型的调用方复用）：
/// 登录（缓存 cookie）→ 调用 → 鉴权失败重登一次重试。
async fn with_session_raw<T, F, Fut>(
    state: &AppState,
    email: &str,
    password: &str,
    call: F,
) -> Result<T, MonErr>
where
    F: Fn(String, String) -> Fut,
    Fut: std::future::Future<Output = Result<T, MonErr>>,
{
    let (api_base, cookie) = ensure_cookie(state, email, password, false).await?;
    match call(api_base, cookie).await {
        Ok(v) => Ok(v),
        Err(MonErr::Auth(_)) => {
            let (base, c) = ensure_cookie(state, email, password, true).await?;
            call(base, c).await
        }
        Err(e) => Err(e),
    }
}

/// with_session_raw 的 String 错误薄封装，供各命令直接返回给前端。
pub async fn with_session<T, F, Fut>(
    state: &AppState,
    email: &str,
    password: &str,
    call: F,
) -> Result<T, String>
where
    F: Fn(String, String) -> Fut,
    Fut: std::future::Future<Output = Result<T, MonErr>>,
{
    with_session_raw(state, email, password, call)
        .await
        .map_err(|e| e.message())
}

pub struct Resolved {
    pub quota: Option<monitor::QuotaOut>,
    pub org_id: String,
    pub error: Option<String>,
    pub bad: bool,
}

impl Resolved {
    fn from_err(org_id: String, e: MonErr) -> Self {
        Resolved {
            quota: None,
            org_id,
            bad: matches!(e, MonErr::BadCredentials(_)),
            error: Some(e.message()),
        }
    }
}

/// 登录（缓存 Cookie）→ 必要时自动探测 org_id → 拉取额度，带一次鉴权重登重试。
/// 不含 metrics，供单账号卡片与托盘圆环复用。
pub async fn resolve_quota(
    state: &AppState,
    email: &str,
    password: &str,
    org_id: &str,
) -> Resolved {
    let (api_base, cookie) = match ensure_cookie(state, email, password, false).await {
        Ok(session) => session,
        Err(e) => return Resolved::from_err(org_id.to_string(), e),
    };

    let mut org = org_id.trim().to_string();
    if org.is_empty() {
        if let Ok(list) = monitor::list_allocations(&state.client, &api_base, &cookie).await {
            if let Some(first) = list.first() {
                org = first.id.clone();
            }
        }
    }

    if org.is_empty() {
        return Resolved {
            quota: None,
            org_id: org,
            error: None,
            bad: false,
        };
    }

    // 额度拉取复用统一会话原语（鉴权重登），与其它监控 API 行为一致
    let client = &state.client;
    let org_for_fetch = org.clone();
    match with_session_raw(state, email, password, move |base, cookie| {
        let org = org_for_fetch.clone();
        async move { monitor::fetch_quota(client, &base, &cookie, &org).await }
    })
    .await
    {
        Ok(quota) => Resolved {
            quota,
            org_id: org,
            error: None,
            bad: false,
        },
        Err(e) => Resolved {
            bad: matches!(e, MonErr::BadCredentials(_)),
            quota: None,
            error: Some(e.message()),
            org_id: org,
        },
    }
}

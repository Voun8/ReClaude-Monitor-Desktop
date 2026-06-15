// 监控 API 的会话编排：cookie 缓存登录、鉴权失败重登一次、默认域名失败时按候选域名链降级。
// HTTP 细节在 monitor 模块；本模块只负责「带会话地调用 + 重试策略」。

use crate::monitor::{self, MonErr};
use crate::ui_config;
use crate::AppState;

fn cookie_key(api_base: &str, email: &str) -> String {
    format!("{api_base}\n{email}")
}

fn monitor_api_bases() -> Vec<String> {
    ui_config::configured_api_base()
        .map(|base| vec![base])
        .unwrap_or_else(|| {
            vec![
                monitor::DEFAULT_API_BASE.to_string(),
                monitor::FALLBACK_API_BASE.to_string(),
                monitor::LEGACY_API_BASE.to_string(),
            ]
        })
}

fn should_fallback(api_base: &str, err: &MonErr) -> bool {
    let bases = monitor_api_bases();
    let can_try_next = match bases.iter().position(|base| base == api_base) {
        Some(idx) => idx + 1 < bases.len(),
        None => false,
    };

    ui_config::configured_api_base().is_none()
        && can_try_next
        && !matches!(
            err,
            MonErr::AccessDenied(_)
                | MonErr::BadCredentials(_)
                | MonErr::Auth(_)
                | MonErr::RateLimited(_)
        )
}

fn next_api_bases(api_base: &str) -> Vec<String> {
    let bases = monitor_api_bases();
    let start = bases
        .iter()
        .position(|base| base == api_base)
        .map(|idx| idx + 1)
        .unwrap_or(0);
    bases.into_iter().skip(start).collect()
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
    let bases = monitor_api_bases();
    let mut last_err: Option<MonErr> = None;
    for api_base in bases {
        match ensure_cookie_at(state, &api_base, email, password, force).await {
            Ok(cookie) => return Ok((api_base, cookie)),
            Err(e) if should_fallback(&api_base, &e) => {
                last_err = Some(e);
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err.unwrap_or_else(|| MonErr::Other("没有可用 API 地址".to_string())))
}

async fn ensure_next_cookie(
    state: &AppState,
    email: &str,
    password: &str,
    api_base: &str,
) -> Result<(String, String), MonErr> {
    if ui_config::configured_api_base().is_some() {
        return Err(MonErr::Other("自定义 API 地址不可自动切换".to_string()));
    }
    let mut last_err: Option<MonErr> = None;
    for next_base in next_api_bases(api_base) {
        match ensure_cookie_at(state, &next_base, email, password, true).await {
            Ok(cookie) => return Ok((next_base, cookie)),
            Err(e) if should_fallback(&next_base, &e) => {
                last_err = Some(e);
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Err(last_err.unwrap_or_else(|| MonErr::Other("没有可用 API 地址".to_string())))
}

/// 带会话调用一个监控 API：登录（缓存 cookie）→ 调用 → 鉴权失败重登一次重试
/// → 默认域失败沿候选域名链重试一次。各命令只需提供业务调用闭包。
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
    let (mut api_base, mut cookie) = ensure_cookie(state, email, password, false)
        .await
        .map_err(|e| e.message())?;

    loop {
        match call(api_base.clone(), cookie.clone()).await {
            Ok(v) => return Ok(v),
            Err(MonErr::Auth(_)) => {
                let (base, c) = ensure_cookie(state, email, password, true)
                    .await
                    .map_err(|e| e.message())?;
                return call(base, c).await.map_err(|e| e.message());
            }
            Err(e) if should_fallback(&api_base, &e) => {
                match ensure_next_cookie(state, email, password, &api_base).await {
                    Ok((base, c)) => {
                        api_base = base;
                        cookie = c;
                    }
                    Err(e) => return Err(e.message()),
                }
            }
            Err(e) => return Err(e.message()),
        }
    }
}

pub struct Resolved {
    pub quota: Option<monitor::QuotaOut>,
    pub org_id: String,
    pub error: Option<String>,
    pub bad: bool,
    pub cookie: Option<String>,
    pub api_base: Option<String>,
}

impl Resolved {
    fn from_err(org_id: String, e: MonErr) -> Self {
        Resolved {
            quota: None,
            org_id,
            bad: matches!(e, MonErr::BadCredentials(_)),
            error: Some(e.message()),
            cookie: None,
            api_base: None,
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
    let (mut api_base, mut cookie) = match ensure_cookie(state, email, password, false).await {
        Ok(session) => session,
        Err(e) => return Resolved::from_err(org_id.to_string(), e),
    };

    let mut org = org_id.trim().to_string();
    if org.is_empty() {
        loop {
            match monitor::list_allocations(&state.client, &api_base, &cookie).await {
                Ok(list) => {
                    if let Some(first) = list.first() {
                        org = first.id.clone();
                    }
                    break;
                }
                Err(e) if should_fallback(&api_base, &e) => {
                    match ensure_next_cookie(state, email, password, &api_base).await {
                        Ok((base, c)) => {
                            api_base = base;
                            cookie = c;
                        }
                        Err(_) => break,
                    }
                }
                Err(_) => break,
            }
        }
    }

    if org.is_empty() {
        return Resolved {
            quota: None,
            org_id: org,
            error: None,
            bad: false,
            cookie: Some(cookie),
            api_base: Some(api_base),
        };
    }

    let mut attempt = 0;
    loop {
        attempt += 1;
        match monitor::fetch_quota(&state.client, &api_base, &cookie, &org).await {
            Ok(quota) => {
                return Resolved {
                    quota,
                    org_id: org,
                    error: None,
                    bad: false,
                    cookie: Some(cookie),
                    api_base: Some(api_base),
                }
            }
            Err(MonErr::Auth(_)) if attempt < 2 => {
                match ensure_cookie(state, email, password, true).await {
                    Ok((base, c)) => {
                        api_base = base;
                        cookie = c;
                    }
                    Err(e) => return Resolved::from_err(org, e),
                }
            }
            Err(e) if should_fallback(&api_base, &e) => {
                match ensure_next_cookie(state, email, password, &api_base).await {
                    Ok((base, c)) => {
                        api_base = base;
                        cookie = c;
                    }
                    Err(e) => return Resolved::from_err(org, e),
                }
            }
            Err(e) => {
                return Resolved {
                    quota: None,
                    org_id: org,
                    error: Some(e.message()),
                    bad: false,
                    cookie: Some(cookie),
                    api_base: Some(api_base),
                }
            }
        }
    }
}

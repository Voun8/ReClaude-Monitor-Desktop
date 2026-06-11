// Port of reclaude-monitor 的 HTTP 部分：邮箱密码登录可配置 API，拉取拼车额度与服务指标。

use serde::Serialize;
use serde_json::Value;

pub const DEFAULT_API_BASE: &str = "https://reclaude.ai";
pub const FALLBACK_API_BASE: &str = "https://www.recode.cat";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";

fn api_url(api_base: &str, path: &str) -> String {
    format!("{api_base}{path}")
}

fn is_rbac_access_denied(body: &str) -> bool {
    body.to_ascii_lowercase().contains("rbac: access denied")
}

#[derive(Debug)]
pub enum MonErr {
    BadCredentials(String),
    Auth(String),
    Network(String),
    Other(String),
}

impl MonErr {
    pub fn message(&self) -> String {
        match self {
            MonErr::BadCredentials(m) => m.clone(),
            MonErr::Auth(m) => m.clone(),
            MonErr::Network(m) => m.clone(),
            MonErr::Other(m) => m.clone(),
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct QuotaOut {
    pub used_usd: f64,
    pub total_usd: f64,
    pub remaining_usd: f64,
    pub pct: f64,
    pub ratio: f64,
    pub reset_at_ms: f64,
    pub enabled: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MetricsOut {
    pub error_rate_pct: f64,
    pub error_count: u64,
    pub req_count: u64,
    pub avg_latency_ms: f64,
    pub rpm: f64,
    pub tpm: f64,
    pub state_level: String,
    pub state_text: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Allocation {
    pub id: String,
    pub label: String,
    pub capacity: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSnapshot {
    pub quota: Option<QuotaOut>,
    pub metrics: Option<MetricsOut>,
    pub org_id: String,
    pub error: Option<String>,
    pub bad_credentials: bool,
}

/// 容忍字符串或数字的取值。
fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn num_at(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(num)
}

pub async fn login(
    client: &reqwest::Client,
    api_base: &str,
    email: &str,
    password: &str,
) -> Result<String, MonErr> {
    let res = client
        .post(api_url(api_base, "/api/auth/login"))
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .header("origin", api_base)
        .header("referer", format!("{api_base}/login"))
        .header("user-agent", UA)
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .map_err(|e| MonErr::Network(format!("网络错误：{e}")))?;

    let status = res.status();
    if !status.is_success() {
        let code = status.as_u16();
        let body = res.text().await.unwrap_or_default();
        if code == 403 && is_rbac_access_denied(&body) {
            let snippet: String = body.chars().take(200).collect();
            return Err(MonErr::Other(format!(
                "API 访问被拒绝（HTTP 403）：{snippet}"
            )));
        }
        if matches!(code, 400 | 401 | 403 | 422) {
            return Err(MonErr::BadCredentials(format!(
                "账号或密码错误（HTTP {code}）"
            )));
        }
        let snippet: String = body.chars().take(200).collect();
        return Err(MonErr::Other(format!("登录失败 HTTP {code}: {snippet}")));
    }

    let mut parts: Vec<String> = Vec::new();
    for hv in res.headers().get_all(reqwest::header::SET_COOKIE).iter() {
        if let Ok(s) = hv.to_str() {
            if let Some(first) = s.split(';').next() {
                let t = first.trim();
                if !t.is_empty() {
                    parts.push(t.to_string());
                }
            }
        }
    }
    if parts.is_empty() {
        return Err(MonErr::Other("登录响应缺少 Set-Cookie".to_string()));
    }
    Ok(parts.join("; "))
}

async fn api_get(
    client: &reqwest::Client,
    api_base: &str,
    url: &str,
    cookie: &str,
) -> Result<Value, MonErr> {
    let res = client
        .get(url)
        .header("accept", "*/*")
        .header("accept-language", "zh-CN,zh;q=0.9")
        .header("cookie", cookie)
        .header("referer", format!("{api_base}/app"))
        .header("user-agent", UA)
        .header("x-lang", "zh")
        .send()
        .await
        .map_err(|e| MonErr::Network(format!("网络错误：{e}")))?;
    let status = res.status();
    if !status.is_success() {
        let code = status.as_u16();
        let body = res.text().await.unwrap_or_default();
        if code == 403 && is_rbac_access_denied(&body) {
            let snippet: String = body.chars().take(200).collect();
            return Err(MonErr::Other(format!(
                "API 访问被拒绝（HTTP 403）：{snippet}"
            )));
        }
        if code == 401 || code == 403 {
            return Err(MonErr::Auth(format!("auth-{code}")));
        }
        let snippet: String = body.chars().take(200).collect();
        return Err(MonErr::Other(format!("HTTP {code}: {snippet}")));
    }
    res.json::<Value>()
        .await
        .map_err(|e| MonErr::Other(format!("解析响应失败：{e}")))
}

pub async fn fetch_metrics(
    client: &reqwest::Client,
    api_base: &str,
    cookie: &str,
) -> Result<MetricsOut, MonErr> {
    let v = api_get(
        client,
        api_base,
        &api_url(api_base, "/api/app/ops/metrics"),
        cookie,
    )
    .await?;
    let error_rate = num_at(&v, "error_rate").unwrap_or(0.0);
    let error_rate_pct = error_rate * 100.0;
    let state_level = if error_rate_pct < 1.0 {
        "ok"
    } else if error_rate_pct < 5.0 {
        "warn"
    } else {
        "err"
    };
    let state_text = match state_level {
        "ok" => "正常",
        "warn" => "抖动",
        _ => "故障",
    };
    Ok(MetricsOut {
        error_rate_pct,
        error_count: num_at(&v, "error_count").unwrap_or(0.0) as u64,
        req_count: num_at(&v, "req_count").unwrap_or(0.0) as u64,
        avg_latency_ms: num_at(&v, "avg_latency_ms").unwrap_or(0.0),
        rpm: num_at(&v, "rps").unwrap_or(0.0) * 60.0,
        tpm: num_at(&v, "tpm").unwrap_or(0.0),
        state_level: state_level.to_string(),
        state_text: state_text.to_string(),
    })
}

pub async fn fetch_quota(
    client: &reqwest::Client,
    api_base: &str,
    cookie: &str,
    org_id: &str,
) -> Result<Option<QuotaOut>, MonErr> {
    let url = format!(
        "{}?org_id={}",
        api_url(api_base, "/api/app/billing/carpool-quota"),
        urlencode(org_id)
    );
    let v = api_get(client, api_base, &url, cookie).await?;
    let total = match num_at(&v, "quota_usd") {
        Some(x) => x,
        None => return Ok(None),
    };
    let used = num_at(&v, "used_usd").unwrap_or(0.0);
    let ratio = if total > 0.0 { used / total } else { 0.0 };
    let enabled = v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true);
    Ok(Some(QuotaOut {
        used_usd: used,
        total_usd: total,
        remaining_usd: total - used,
        pct: ratio * 100.0,
        ratio,
        reset_at_ms: num_at(&v, "resets_at_ms").unwrap_or(0.0),
        enabled,
    }))
}

pub async fn list_allocations(
    client: &reqwest::Client,
    api_base: &str,
    cookie: &str,
) -> Result<Vec<Allocation>, MonErr> {
    let v = api_get(
        client,
        api_base,
        &api_url(api_base, "/api/app/billing/carpool-allocations"),
        cookie,
    )
    .await?;
    let list: Vec<Value> = if v.is_array() {
        v.as_array().cloned().unwrap_or_default()
    } else {
        for key in ["allocations", "items", "data"] {
            if let Some(arr) = v.get(key).and_then(|x| x.as_array()) {
                return Ok(arr.iter().map(alloc_from).collect());
            }
        }
        Vec::new()
    };
    Ok(list.iter().map(alloc_from).collect())
}

fn alloc_from(a: &Value) -> Allocation {
    let id = a
        .get("org_id")
        .or_else(|| a.get("allocation_id"))
        .or_else(|| a.get("id"))
        .map(|x| match x {
            Value::Number(n) => n.to_string(),
            Value::String(s) => s.clone(),
            _ => String::new(),
        })
        .unwrap_or_default();
    let sku = a
        .get("sku")
        .or_else(|| a.get("plan"))
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    let capacity = num_at(a, "capacity").map(|x| x as u64);
    let label = if sku.is_empty() {
        format!("org_id={id}")
    } else {
        format!("{sku} (org_id={id})")
    };
    Allocation {
        id,
        label,
        capacity,
    }
}

/// 按 RFC 3986 对 query 值做 percent-encoding（unreserved 集合外全部转义）。
/// 与 `percent_encoding::utf8_percent_encode(s, NON_ALPHANUMERIC ∪ {-,_,.,~})` 等价；
/// 此处自实现以省一个依赖。
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// ============ 用量统计（/api/app/usage/stats、/api/app/devices）============

fn pick_num(v: &Value, keys: &[&str]) -> Option<f64> {
    for k in keys {
        if let Some(x) = v.get(k).and_then(num) {
            return Some(x);
        }
    }
    None
}

fn pick_str(v: &Value, keys: &[&str]) -> String {
    for k in keys {
        if let Some(s) = v.get(k).and_then(|x| x.as_str()) {
            if !s.is_empty() {
                return s.to_string();
            }
        }
    }
    String::new()
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsageOverview {
    pub sessions: f64,
    pub messages: f64,
    pub total_usd: f64,
    pub total_tokens: f64,
    pub active_days: f64,
    pub current_streak: f64,
    pub longest_streak: f64,
    pub favorite_model: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HeatCell {
    pub date: String,
    pub count: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub model: String,
    pub total_usd: f64,
    pub total_tokens: f64,
    pub percent: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsageStats {
    pub overview: UsageOverview,
    pub heatmap: Vec<HeatCell>,
    pub models: Vec<ModelUsage>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
}

fn value_to_id(x: &Value) -> String {
    match x {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        _ => String::new(),
    }
}

fn parse_usage(v: &Value) -> UsageStats {
    // 总览字段可能在顶层，也可能在 overview 子对象里
    let o = v.get("overview").filter(|x| x.is_object()).unwrap_or(v);

    let overview = UsageOverview {
        sessions: pick_num(o, &["sessions", "total_sessions", "session_count"]).unwrap_or(0.0),
        messages: pick_num(o, &["messages", "total_messages", "message_count"]).unwrap_or(0.0),
        total_usd: pick_num(o, &["total_usd", "total_cost", "total_spend", "spend_usd"])
            .unwrap_or(0.0),
        total_tokens: pick_num(o, &["total_tokens", "tokens"]).unwrap_or(0.0),
        active_days: pick_num(o, &["active_days"]).unwrap_or(0.0),
        current_streak: pick_num(o, &["current_streak"]).unwrap_or(0.0),
        longest_streak: pick_num(o, &["longest_streak"]).unwrap_or(0.0),
        favorite_model: pick_str(o, &["favorite_model", "top_model", "most_used_model"]),
    };

    let mut heatmap = Vec::new();
    let hm_src = v
        .get("heatmap")
        .or_else(|| o.get("heatmap"))
        .or_else(|| v.get("calendar"))
        .or_else(|| v.get("activity"));
    if let Some(arr) = hm_src.and_then(|x| x.as_array()) {
        for it in arr {
            let date = {
                let s = pick_str(it, &["date", "day"]);
                if !s.is_empty() {
                    s
                } else if let Some(ts) = it.get("ts").and_then(num) {
                    (ts as i64).to_string()
                } else {
                    String::new()
                }
            };
            let count = pick_num(
                it,
                &[
                    "count",
                    "value",
                    "messages",
                    "sessions",
                    "total_usd",
                    "total",
                    "level",
                ],
            )
            .unwrap_or(0.0);
            if !date.is_empty() {
                heatmap.push(HeatCell { date, count });
            }
        }
    }

    let mut models = Vec::new();
    let m_src = v
        .get("models")
        .or_else(|| v.get("breakdown"))
        .or_else(|| o.get("models"))
        .or_else(|| o.get("breakdown"));
    if let Some(arr) = m_src.and_then(|x| x.as_array()) {
        for it in arr {
            // percent 字段语义按 key 名固化，避免单值启发（如 1.01 被误认为 0-1 范围）
            //   percent / pct → 视为 0-100
            //   share         → 视为 0-1，乘 100
            let percent = if let Some(v) = num_at(it, "percent") {
                v
            } else if let Some(v) = num_at(it, "pct") {
                v
            } else if let Some(v) = num_at(it, "share") {
                v * 100.0
            } else {
                0.0
            };
            models.push(ModelUsage {
                model: pick_str(it, &["model", "name"]),
                total_usd: pick_num(it, &["total_usd", "total_cost"]).unwrap_or(0.0),
                total_tokens: pick_num(it, &["total_tokens", "tokens", "value"]).unwrap_or(0.0),
                percent,
            });
        }
    }

    UsageStats {
        overview,
        heatmap,
        models,
    }
}

pub async fn fetch_usage_stats(
    client: &reqwest::Client,
    api_base: &str,
    cookie: &str,
    range: &str,
    device_id: Option<&str>,
    org_id: &str,
) -> Result<UsageStats, MonErr> {
    let mut url = format!(
        "{}?range={}",
        api_url(api_base, "/api/app/usage/stats"),
        urlencode(range)
    );
    if let Some(d) = device_id {
        if !d.is_empty() {
            url.push_str(&format!("&device_id={}", urlencode(d)));
        }
    }
    if !org_id.is_empty() {
        url.push_str(&format!("&org_id={}", urlencode(org_id)));
    }
    let v = api_get(client, api_base, &url, cookie).await?;
    Ok(parse_usage(&v))
}

/// 触发服务端重算用量：POST /api/app/usage/stats（空 body）。
pub async fn sync_usage(
    client: &reqwest::Client,
    api_base: &str,
    cookie: &str,
) -> Result<(), MonErr> {
    let res = client
        .post(api_url(api_base, "/api/app/usage/stats"))
        .header("accept", "*/*")
        .header("accept-language", "zh-CN,zh;q=0.9")
        .header("cookie", cookie)
        .header("referer", format!("{api_base}/app"))
        .header("user-agent", UA)
        .header("x-lang", "zh")
        .send()
        .await
        .map_err(|e| MonErr::Network(format!("网络错误：{e}")))?;
    let s = res.status();
    if !s.is_success() {
        let code = s.as_u16();
        let body = res.text().await.unwrap_or_default();
        if code == 403 && is_rbac_access_denied(&body) {
            let snippet: String = body.chars().take(200).collect();
            return Err(MonErr::Other(format!(
                "API 访问被拒绝（HTTP 403）：{snippet}"
            )));
        }
        if code == 401 || code == 403 {
            return Err(MonErr::Auth(format!("auth-{code}")));
        }
        let snippet: String = body.chars().take(200).collect();
        return Err(MonErr::Other(format!("HTTP {code}: {snippet}")));
    }
    Ok(())
}

pub async fn fetch_devices(
    client: &reqwest::Client,
    api_base: &str,
    cookie: &str,
) -> Result<Vec<Device>, MonErr> {
    let v = api_get(
        client,
        api_base,
        &api_url(api_base, "/api/app/devices"),
        cookie,
    )
    .await?;
    let arr = v
        .get("data")
        .and_then(|d| d.get("items"))
        .and_then(|x| x.as_array())
        .or_else(|| v.get("items").and_then(|x| x.as_array()))
        .or_else(|| v.as_array());
    let mut out = Vec::new();
    if let Some(a) = arr {
        for it in a {
            let id = it.get("id").map(value_to_id).unwrap_or_default();
            if id.is_empty() {
                continue;
            }
            let mut name = pick_str(it, &["name", "alias", "device_name", "hostname", "label"]);
            if name.is_empty() {
                name = id.clone();
            }
            out.push(Device { id, name });
        }
    }
    Ok(out)
}

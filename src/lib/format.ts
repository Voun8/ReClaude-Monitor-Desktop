// 额度告警阈值——heroBadge(MonitorView)、quotaColor 与 Rust 端托盘配色
// (src-tauri/src/tray_ring.rs color_for_ratio)共用同一组业务阈值,两端需保持一致。
export const QUOTA_ERR_RATIO = 0.95;
export const QUOTA_WARN_RATIO = 0.8;

export function fmtMs(v: number | null | undefined): string {
  if (v === null || v === undefined) return "-";
  if (v >= 1000) return (v / 1000).toFixed(2) + "s";
  return Math.round(v) + "ms";
}

export function fmtUsd(v: number | null | undefined): string {
  if (v === null || v === undefined) return "$--";
  return "$" + v.toFixed(2);
}

// 大额按货币惯例 2 位小数（$141.22）；小额（<$1）保留更高精度（最多 4 位，如 $0.0234）
export function fmtUsd4(v: number | null | undefined): string {
  if (v === null || v === undefined) return "$--";
  const maxFrac = Math.abs(v) >= 1 ? 2 : 4;
  return (
    "$" +
    v.toLocaleString("en-US", {
      minimumFractionDigits: 2,
      maximumFractionDigits: maxFrac,
    })
  );
}

// 悬浮球等极小空间用的紧凑美元：$9.99 / $123 / $1.2k
export function fmtUsdCompact(v: number): string {
  if (v >= 1000) return "$" + (v / 1000).toFixed(v >= 10000 ? 0 : 1) + "k";
  if (v >= 100) return "$" + Math.round(v);
  return "$" + v.toFixed(2);
}

export function fmtCountdown(ms: number): string {
  if (ms <= 0) return "已重置";
  const min = Math.floor(ms / 60000);
  const h = Math.floor(min / 60);
  const m = min % 60;
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

export function quotaColor(ratio: number): string {
  if (ratio >= QUOTA_ERR_RATIO) return "var(--err)";
  if (ratio >= QUOTA_WARN_RATIO) return "var(--warn)";
  return "var(--accent)";
}

export function fmtInt(v: number | null | undefined): string {
  if (v === null || v === undefined) return "-";
  return Math.round(v).toLocaleString("en-US");
}

// 753.07M / 1.20B 这种风格（k=千 小写，M=百万 / B=十亿 大写，符合 SI 惯例）。
// decimals 控制小数位：坐标轴用 0 位（141M），tooltip 用 2 位（140.78M）。
export function fmtTokens(v: number | null | undefined, decimals = 2): string {
  if (v === null || v === undefined) return "-";
  const n = Math.abs(v);
  if (n >= 1e9) return (v / 1e9).toFixed(decimals) + "B";
  if (n >= 1e6) return (v / 1e6).toFixed(decimals) + "M";
  if (n >= 1e3) return (v / 1e3).toFixed(decimals) + "k";
  return String(Math.round(v));
}

// 后端日期字段的弹性解析：YYYY-MM-DD（可带时间）、秒/毫秒时间戳、Date 可解析字符串。
// 无法解析返回 null。
export function flexDate(raw: string): Date | null {
  if (/^\d{4}-\d{2}-\d{2}/.test(raw)) return new Date(raw.slice(0, 19));
  const n = Number(raw);
  if (raw.trim() !== "" && !Number.isNaN(n)) return new Date(n < 1e12 ? n * 1000 : n);
  const d = new Date(raw);
  return Number.isNaN(d.getTime()) ? null : d;
}

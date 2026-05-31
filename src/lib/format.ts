export function fmtNum(v: number | null | undefined): string {
  if (v === null || v === undefined) return "-";
  if (v >= 1e6) return (v / 1e6).toFixed(1) + "M";
  if (v >= 1e3) return (v / 1e3).toFixed(1) + "k";
  return String(Math.round(v));
}

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

export function fmtCountdown(ms: number): string {
  if (ms <= 0) return "已重置";
  const min = Math.floor(ms / 60000);
  const h = Math.floor(min / 60);
  const m = min % 60;
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

export function latencyLabel(ms: number): string {
  if (ms < 1000) return "流畅";
  if (ms < 3000) return "正常";
  return "偏慢";
}

export function quotaColor(ratio: number): string {
  if (ratio >= 0.95) return "var(--err)";
  if (ratio >= 0.8) return "var(--warn)";
  return "var(--accent)";
}

export function fmtInt(v: number | null | undefined): string {
  if (v === null || v === undefined) return "-";
  return Math.round(v).toLocaleString("en-US");
}

// 753.07M / 1.20B 这种风格（k=千 小写，M=百万 / B=十亿 大写，符合 SI 惯例）
export function fmtTokens(v: number | null | undefined): string {
  if (v === null || v === undefined) return "-";
  if (v >= 1e9) return (v / 1e9).toFixed(2) + "B";
  if (v >= 1e6) return (v / 1e6).toFixed(2) + "M";
  if (v >= 1e3) return (v / 1e3).toFixed(2) + "k";
  return String(Math.round(v));
}

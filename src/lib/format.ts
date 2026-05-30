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

// 总用量这类需要更高精度：2~4 位小数（与 reclaude 面板一致）
export function fmtUsd4(v: number | null | undefined): string {
  if (v === null || v === undefined) return "$--";
  return (
    "$" +
    v.toLocaleString("en-US", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 4,
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

// 753.07m / 1.20b 这种风格（小写后缀，跟 reclaude 面板一致）
export function fmtTokens(v: number | null | undefined): string {
  if (v === null || v === undefined) return "-";
  if (v >= 1e9) return (v / 1e9).toFixed(2) + "b";
  if (v >= 1e6) return (v / 1e6).toFixed(2) + "m";
  if (v >= 1e3) return (v / 1e3).toFixed(2) + "k";
  return String(Math.round(v));
}

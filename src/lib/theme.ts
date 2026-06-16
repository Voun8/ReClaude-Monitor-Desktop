// G2 图表共用：图表库不认 CSS 变量，需在 JS 侧取实际色值与主题。

// 返回绑定到单次 getComputedStyle 的读取器：build 内多次取色复用同一份计算，避免重复 reflow。
export function cssVarReader(): (name: string, fallback: string) => string {
  const cs = getComputedStyle(document.documentElement);
  return (name, fallback) => cs.getPropertyValue(name).trim() || fallback;
}

export function isDarkTheme(): boolean {
  return !window.matchMedia("(prefers-color-scheme: light)").matches;
}

// G2 图表共用：图表库不认 CSS 变量，需在 JS 侧取实际色值与主题。

export function cssVar(name: string, fallback: string): string {
  const v = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim();
  return v || fallback;
}

export function isDarkTheme(): boolean {
  return !window.matchMedia("(prefers-color-scheme: light)").matches;
}

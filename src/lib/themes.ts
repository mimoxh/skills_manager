export type Palette =
  | "m3-cobalt"
  | "m3-berry"
  | "m3-lime"
  | "m3-teal"
  | "linear"
  | "modern"
  | "parchment";

/** 组件语言：决定按钮、卡片、导航等组件形态；配色由 Palette 的 CSS 变量决定。 */
export type ThemeStyle = "m3" | "linear" | "classic";

export type ThemeGroup = "Material 3 Expressive" | "Linear / Raycast" | "经典";

export interface ThemeOption {
  value: Palette;
  label: string;
  desc: string;
  group: ThemeGroup;
  style: ThemeStyle;
  /** 预览色：[背景, 主色, 主色容器, 辅助色] */
  swatch: { light: [string, string, string, string]; dark: [string, string, string, string] };
}

export const DEFAULT_PALETTE: Palette = "m3-cobalt";

export const THEME_OPTIONS: ThemeOption[] = [
  {
    value: "m3-cobalt",
    label: "钴蓝 · 杏橙",
    desc: "冷静专业，适合长时间使用的工具界面",
    group: "Material 3 Expressive",
    style: "m3",
    swatch: { light: ["#f9f9ff", "#3654d1", "#dbe1ff", "#ffdcc0"], dark: ["#11131a", "#b6c4ff", "#1e3bb5", "#6d3a00"] },
  },
  {
    value: "m3-berry",
    label: "莓果 · 湖青",
    desc: "饱和莓红配湖青对比色，表现力最强",
    group: "Material 3 Expressive",
    style: "m3",
    swatch: { light: ["#fff8f9", "#9a247a", "#ffd8ee", "#b2ecff"], dark: ["#191115", "#ffaee0", "#7d0a61", "#004e5c"] },
  },
  {
    value: "m3-lime",
    label: "青柠 · 鸢尾",
    desc: "高亮青柠容器配鸢尾紫，深色为石墨底",
    group: "Material 3 Expressive",
    style: "m3",
    swatch: { light: ["#fafaf3", "#4c6800", "#c8f169", "#ebddff"], dark: ["#121410", "#b9e05a", "#384e00", "#52379a"] },
  },
  {
    value: "m3-teal",
    label: "深青 · 珊瑚",
    desc: "清爽的深青绿配珊瑚色强调",
    group: "Material 3 Expressive",
    style: "m3",
    swatch: { light: ["#f5fbf8", "#006a60", "#9ef2e2", "#ffdbd0"], dark: ["#0e1513", "#82d5c6", "#005048", "#7d2c15"] },
  },
  {
    value: "linear",
    label: "Linear / Raycast",
    desc: "紧凑精密、发丝描边、电光靛蓝",
    group: "Linear / Raycast",
    style: "linear",
    swatch: { light: ["#fcfcfd", "#5b5bd6", "#e9e9f6", "#f2f2f5"], dark: ["#0b0c0e", "#7c7cf0", "#25263a", "#1b1c20"] },
  },
  {
    value: "modern",
    label: "清爽极简",
    desc: "纯白画布 + 蓝色点缀，现代干净",
    group: "经典",
    style: "classic",
    swatch: { light: ["#fafafa", "#3b82f6", "#eff6ff", "#f4f4f5"], dark: ["#0a0a0a", "#60a5fa", "#1e3a5f", "#1e1e22"] },
  },
  {
    value: "parchment",
    label: "经典暖色",
    desc: "象牙奶油 + 琥珀暖金，温润通透质感",
    group: "经典",
    style: "classic",
    swatch: { light: ["#faf7f2", "#d97706", "#fef3c7", "#f5efe6"], dark: ["#141210", "#f59e0b", "#3d2c12", "#27221e"] },
  },
];

export const THEME_GROUPS: ThemeGroup[] = ["Material 3 Expressive", "Linear / Raycast", "经典"];

export function isPalette(value: string | null): value is Palette {
  return THEME_OPTIONS.some((option) => option.value === value);
}

export function themeOption(palette: Palette): ThemeOption {
  return THEME_OPTIONS.find((option) => option.value === palette) ?? THEME_OPTIONS[0];
}

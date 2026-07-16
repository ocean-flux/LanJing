/**
 * L2 Appearance pack 色表：仅重绑 L1 角色 hex。
 * 默认墨砚精密；冷银朱为第二内置包；纸灯 id 仅作迁移别名。
 */

export type AppearancePackId = 'inkstone-precision' | 'cold-cinnabar';

/** 历史默认包 id → 现行默认 */
export const LEGACY_APPEARANCE_PACK_MAP = {
  'paper-lantern-precision': 'inkstone-precision',
} as const satisfies Record<string, AppearancePackId>;

export const DEFAULT_APPEARANCE_PACK_ID: AppearancePackId = 'inkstone-precision';

export const BUILTIN_APPEARANCE_PACK_IDS: readonly AppearancePackId[] = [
  'inkstone-precision',
  'cold-cinnabar',
] as const;

/** 写入 documentElement 的 L1 变量（与 index.css 角色对齐） */
export type AppearanceRoleVar =
  | '--canvas'
  | '--canvas-elevated'
  | '--ink'
  | '--ink-muted'
  | '--ink-subtle'
  | '--hairline'
  | '--hairline-strong'
  | '--surface-1'
  | '--surface-2'
  | '--surface-3'
  | '--lantern'
  | '--lantern-strong'
  | '--lantern-hover'
  | '--lantern-soft'
  | '--lantern-tint'
  | '--on-lantern'
  | '--media-void'
  | '--reader-canvas'
  | '--reader-ink'
  | '--ring'
  | '--focus-ring';

export type AppearanceTokenMap = Readonly<Record<AppearanceRoleVar, string>>;

const inkstoneLight = {
  '--canvas': '#f4f5f5',
  '--canvas-elevated': '#ffffff',
  '--ink': '#171a1b',
  '--ink-muted': '#5c6568',
  '--ink-subtle': '#7a8488',
  '--hairline': 'rgb(23 26 27 / 0.1)',
  '--hairline-strong': 'rgb(23 26 27 / 0.16)',
  '--surface-1': '#ffffff',
  '--surface-2': '#f7f8f8',
  '--surface-3': '#eef0f0',
  '--lantern': '#2a6f7a',
  '--lantern-strong': '#1d5560',
  '--lantern-hover': '#174850',
  '--lantern-soft': 'rgb(42 111 122 / 0.14)',
  '--lantern-tint': '#dce8ea',
  '--on-lantern': '#f4fbfc',
  '--media-void': '#e2e6e7',
  '--reader-canvas': '#f3efe6',
  '--reader-ink': '#211e1a',
  '--ring': 'rgb(29 85 96 / 0.34)',
  '--focus-ring': '0 0 0 2px rgb(29 85 96 / 0.24)',
} as const satisfies AppearanceTokenMap;

const inkstoneDark = {
  '--canvas': '#0e1214',
  '--canvas-elevated': '#151a1c',
  '--ink': '#e6eceb',
  '--ink-muted': '#9aa8a9',
  '--ink-subtle': '#7a8788',
  '--hairline': 'rgb(230 236 235 / 0.1)',
  '--hairline-strong': 'rgb(230 236 235 / 0.16)',
  '--surface-1': '#151a1c',
  '--surface-2': '#1b2224',
  '--surface-3': '#242c2e',
  '--lantern': '#5fa8b4',
  '--lantern-strong': '#4a96a2',
  '--lantern-hover': '#3d8793',
  '--lantern-soft': 'rgb(95 168 180 / 0.18)',
  '--lantern-tint': '#1a2c30',
  '--on-lantern': '#071416',
  '--media-void': '#1a2224',
  '--reader-canvas': '#1a1714',
  '--reader-ink': '#d8d2c4',
  '--ring': 'rgb(95 168 180 / 0.34)',
  '--focus-ring': '0 0 0 2px rgb(95 168 180 / 0.28)',
} as const satisfies AppearanceTokenMap;

const cinnabarLight = {
  '--canvas': '#f2f2f0',
  '--canvas-elevated': '#ffffff',
  '--ink': '#1a1b1d',
  '--ink-muted': '#63656a',
  '--ink-subtle': '#81848a',
  '--hairline': 'rgb(26 27 29 / 0.1)',
  '--hairline-strong': 'rgb(26 27 29 / 0.16)',
  '--surface-1': '#ffffff',
  '--surface-2': '#f6f6f4',
  '--surface-3': '#ecece8',
  '--lantern': '#c45a3c',
  '--lantern-strong': '#9a3f2a',
  '--lantern-hover': '#853625',
  '--lantern-soft': 'rgb(196 90 60 / 0.14)',
  '--lantern-tint': '#f0e4df',
  '--on-lantern': '#fff8f2',
  '--media-void': '#e4e4e1',
  '--reader-canvas': '#f3efe6',
  '--reader-ink': '#211e1a',
  '--ring': 'rgb(154 63 42 / 0.34)',
  '--focus-ring': '0 0 0 2px rgb(154 63 42 / 0.24)',
} as const satisfies AppearanceTokenMap;

const cinnabarDark = {
  '--canvas': '#121316',
  '--canvas-elevated': '#1a1b1f',
  '--ink': '#eceae6',
  '--ink-muted': '#a3a19c',
  '--ink-subtle': '#85837e',
  '--hairline': 'rgb(236 234 230 / 0.1)',
  '--hairline-strong': 'rgb(236 234 230 / 0.16)',
  '--surface-1': '#1a1b1f',
  '--surface-2': '#212227',
  '--surface-3': '#2a2b31',
  '--lantern': '#d4785a',
  '--lantern-strong': '#c45a3c',
  '--lantern-hover': '#b04e33',
  '--lantern-soft': 'rgb(212 120 90 / 0.18)',
  '--lantern-tint': '#2a1c18',
  '--on-lantern': '#1a0c08',
  '--media-void': '#1e1f24',
  '--reader-canvas': '#1b1714',
  '--reader-ink': '#d8d2c4',
  '--ring': 'rgb(212 120 90 / 0.34)',
  '--focus-ring': '0 0 0 2px rgb(212 120 90 / 0.28)',
} as const satisfies AppearanceTokenMap;

export const APPEARANCE_PACK_TOKENS: Record<
  AppearancePackId,
  { light: AppearanceTokenMap; dark: AppearanceTokenMap }
> = {
  'inkstone-precision': { light: inkstoneLight, dark: inkstoneDark },
  'cold-cinnabar': { light: cinnabarLight, dark: cinnabarDark },
};

export function normalizeAppearancePackId(id: string): AppearancePackId {
  if (id in LEGACY_APPEARANCE_PACK_MAP) {
    return LEGACY_APPEARANCE_PACK_MAP[id as keyof typeof LEGACY_APPEARANCE_PACK_MAP];
  }
  if ((BUILTIN_APPEARANCE_PACK_IDS as readonly string[]).includes(id)) {
    return id as AppearancePackId;
  }
  return DEFAULT_APPEARANCE_PACK_ID;
}

export function isBuiltinAppearancePackId(id: string): id is AppearancePackId {
  return (BUILTIN_APPEARANCE_PACK_IDS as readonly string[]).includes(id);
}

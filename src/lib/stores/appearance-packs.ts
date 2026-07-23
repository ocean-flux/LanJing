/**
 * 单 face 主题注册表（VS Code 分轨）。
 * 每个 AppearancePackId 只含一套 token；亮/暗轨分别选 id，禁止镜像反相。
 * 色相禁令：无黄/橙/琥珀/朱砂/紫/靛紫主色。
 */

export type ThemeFace = 'light' | 'dark';

export type AppearancePackId =
  'obsidian-void' | 'graphite-atelier' | 'porcelain-day' | 'mist-studio';

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

export type BuiltinThemeDefinition = {
  id: AppearancePackId;
  face: ThemeFace;
  labelKey: string;
  stone: string;
  tokens: AppearanceTokenMap;
};

export const DEFAULT_DARK_THEME_ID: AppearancePackId = 'obsidian-void';
export const DEFAULT_LIGHT_THEME_ID: AppearancePackId = 'porcelain-day';

/** 兼容旧「单默认 pack」导出：产品默认暗轨 */
export const DEFAULT_APPEARANCE_PACK_ID: AppearancePackId = DEFAULT_DARK_THEME_ID;

export const BUILTIN_APPEARANCE_PACK_IDS: readonly AppearancePackId[] = [
  'obsidian-void',
  'graphite-atelier',
  'porcelain-day',
  'mist-studio',
] as const;

const obsidianVoid: AppearanceTokenMap = {
  '--canvas': '#0b0e12',
  '--canvas-elevated': '#12171d',
  '--ink': '#e8eaed',
  '--ink-muted': '#9aa3ad',
  '--ink-subtle': '#6f7882',
  '--hairline': 'rgb(232 234 237 / 0.12)',
  '--hairline-strong': 'rgb(232 234 237 / 0.2)',
  '--surface-1': '#151a20',
  '--surface-2': '#1b2229',
  '--surface-3': '#252d36',
  '--lantern': '#6ec8d4',
  '--lantern-strong': '#3aa9b8',
  '--lantern-hover': '#2f96a4',
  '--lantern-soft': 'rgb(110 200 212 / 0.18)',
  '--lantern-tint': '#143038',
  '--on-lantern': '#061016',
  '--media-void': '#12171d',
  '--reader-canvas': '#1a1714',
  '--reader-ink': '#d8d2c4',
  '--ring': 'rgb(58 169 184 / 0.34)',
  '--focus-ring': '0 0 0 2px rgb(58 169 184 / 0.28)',
};

const graphiteAtelier: AppearanceTokenMap = {
  '--canvas': '#111416',
  '--canvas-elevated': '#181c1e',
  '--ink': '#eceff0',
  '--ink-muted': '#a4abad',
  '--ink-subtle': '#7d8588',
  '--hairline': 'rgb(236 239 240 / 0.1)',
  '--hairline-strong': 'rgb(236 239 240 / 0.16)',
  '--surface-1': '#1c2022',
  '--surface-2': '#252a2c',
  '--surface-3': '#303638',
  '--lantern': '#79a6ab',
  '--lantern-strong': '#4d777d',
  '--lantern-hover': '#456b71',
  '--lantern-soft': 'rgb(121 166 171 / 0.16)',
  '--lantern-tint': '#1b2b2e',
  '--on-lantern': '#f2f8f8',
  '--media-void': '#171d1f',
  '--reader-canvas': '#1a1714',
  '--reader-ink': '#d8d2c4',
  '--ring': 'rgb(77 119 125 / 0.34)',
  '--focus-ring': '0 0 0 2px rgb(77 119 125 / 0.28)',
};

const porcelainDay: AppearanceTokenMap = {
  '--canvas': '#f3f4f6',
  '--canvas-elevated': '#ffffff',
  '--ink': '#1a1b1e',
  '--ink-muted': '#5c616a',
  '--ink-subtle': '#7a808a',
  '--hairline': 'rgb(26 27 30 / 0.1)',
  '--hairline-strong': 'rgb(26 27 30 / 0.16)',
  '--surface-1': '#ffffff',
  '--surface-2': '#f6f7f9',
  '--surface-3': '#e8eaee',
  '--lantern': '#0f6e7a',
  '--lantern-strong': '#0b5a64',
  '--lantern-hover': '#094c55',
  '--lantern-soft': 'rgb(15 110 122 / 0.12)',
  '--lantern-tint': '#d9e8ea',
  '--on-lantern': '#f4fcfd',
  '--media-void': '#e4e6ea',
  '--reader-canvas': '#f3efe6',
  '--reader-ink': '#211e1a',
  '--ring': 'rgb(11 90 100 / 0.34)',
  '--focus-ring': '0 0 0 2px rgb(11 90 100 / 0.24)',
};

const mistStudio: AppearanceTokenMap = {
  '--canvas': '#eef1f5',
  '--canvas-elevated': '#f7f9fc',
  '--ink': '#171a1f',
  '--ink-muted': '#5a6370',
  '--ink-subtle': '#76808f',
  '--hairline': 'rgb(23 26 31 / 0.1)',
  '--hairline-strong': 'rgb(23 26 31 / 0.16)',
  '--surface-1': '#ffffff',
  '--surface-2': '#f1f4f8',
  '--surface-3': '#e4e9f0',
  '--lantern': '#0e7490',
  '--lantern-strong': '#0b5f75',
  '--lantern-hover': '#0a5568',
  '--lantern-soft': 'rgb(14 116 144 / 0.12)',
  '--lantern-tint': '#d9eef3',
  '--on-lantern': '#f3fbfd',
  '--media-void': '#e2e7ee',
  '--reader-canvas': '#f3efe6',
  '--reader-ink': '#211e1a',
  '--ring': 'rgb(11 95 117 / 0.34)',
  '--focus-ring': '0 0 0 2px rgb(11 95 117 / 0.24)',
};

export const THEME_REGISTRY: Record<AppearancePackId, BuiltinThemeDefinition> = {
  'obsidian-void': {
    id: 'obsidian-void',
    face: 'dark',
    labelKey: 'settings_theme_obsidian_void',
    stone: '#6ec8d4',
    tokens: obsidianVoid,
  },
  'graphite-atelier': {
    id: 'graphite-atelier',
    face: 'dark',
    labelKey: 'settings_theme_graphite_atelier',
    stone: '#79a6ab',
    tokens: graphiteAtelier,
  },
  'porcelain-day': {
    id: 'porcelain-day',
    face: 'light',
    labelKey: 'settings_theme_porcelain_day',
    stone: '#0f6e7a',
    tokens: porcelainDay,
  },
  'mist-studio': {
    id: 'mist-studio',
    face: 'light',
    labelKey: 'settings_theme_mist_studio',
    stone: '#0e7490',
    tokens: mistStudio,
  },
};

/**
 * 历史 id → 按目标 face 映射到新主题（丢弃橙/紫/靛色相）。
 */
const LEGACY_TO_FACE: Record<string, { light: AppearancePackId; dark: AppearancePackId }> = {
  'inkstone-precision': { light: 'porcelain-day', dark: 'obsidian-void' },
  'paper-lantern-precision': { light: 'porcelain-day', dark: 'obsidian-void' },
  'cold-cinnabar': { light: 'mist-studio', dark: 'graphite-atelier' },
};

/** 无 face 上下文时的默认映射（偏暗默认） */
export const LEGACY_APPEARANCE_PACK_MAP = {
  'paper-lantern-precision': DEFAULT_DARK_THEME_ID,
  'inkstone-precision': DEFAULT_DARK_THEME_ID,
  'cold-cinnabar': 'graphite-atelier',
} as const satisfies Record<string, AppearancePackId>;

export function isBuiltinAppearancePackId(id: string): id is AppearancePackId {
  return (BUILTIN_APPEARANCE_PACK_IDS as readonly string[]).includes(id);
}

export function listThemesForFace(face: ThemeFace): BuiltinThemeDefinition[] {
  return BUILTIN_APPEARANCE_PACK_IDS.map((id) => THEME_REGISTRY[id]).filter((t) => t.face === face);
}

/**
 * 将任意历史/现行 id 规范到给定 face 的合法主题。
 */
export function normalizeAppearancePackId(id: string, face: ThemeFace = 'dark'): AppearancePackId {
  const legacy =
    LEGACY_TO_FACE[id] ??
    (id.startsWith('paper-lantern-') ? LEGACY_TO_FACE['paper-lantern-precision'] : undefined);
  if (legacy) return legacy[face];

  if (isBuiltinAppearancePackId(id)) {
    if (THEME_REGISTRY[id].face === face) return id;
    return face === 'dark' ? DEFAULT_DARK_THEME_ID : DEFAULT_LIGHT_THEME_ID;
  }

  return face === 'dark' ? DEFAULT_DARK_THEME_ID : DEFAULT_LIGHT_THEME_ID;
}

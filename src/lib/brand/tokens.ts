export const lanjingPalette = {
  canvas: '#141417',
  canvasElevated: '#1b1b1f',
  ink: '#e8e6e1',
  inkMuted: '#a8a39c',
  inkSubtle: '#85807a',
  hairline: 'rgb(232 230 225 / 0.10)',
  hairlineStrong: 'rgb(232 230 225 / 0.16)',
  surface1: '#1b1b1f',
  surface2: '#222327',
  surface3: '#2b2c31',
  lantern: '#c2683a',
  lanternStrong: '#9a4e2c',
  lanternHover: '#8a4624',
  lanternSoft: 'rgb(194 104 58 / 0.14)',
  lanternTint: '#2c201c',
  onLantern: '#fdf8f1',
  mediaVoid: '#23262d',
  readerCanvas: '#1b1714',
  readerInk: '#d8d2c4',
  positive: '#7ea882',
  warning: '#d4a24a',
  danger: '#db5a6a',
} as const;

export const lanjingLightPalette = {
  canvas: '#f5f4f1',
  canvasElevated: '#ffffff',
  ink: '#1f1e1c',
  inkMuted: '#706b64',
  inkSubtle: '#8a847c',
  hairline: 'rgb(31 30 28 / 0.10)',
  hairlineStrong: 'rgb(31 30 28 / 0.16)',
  surface1: '#ffffff',
  surface2: '#faf9f6',
  surface3: '#f0efea',
  lantern: lanjingPalette.lantern,
  lanternStrong: lanjingPalette.lanternStrong,
  lanternHover: lanjingPalette.lanternHover,
  lanternSoft: lanjingPalette.lanternSoft,
  lanternTint: '#eee0d7',
  onLantern: lanjingPalette.onLantern,
  mediaVoid: '#e3e1dc',
  readerCanvas: '#f4efe4',
  readerInk: '#211e1a',
  positive: '#557d59',
  warning: '#9f6d1e',
  danger: '#b83f4e',
} as const;

export const lanjingRadii = {
  sm: '6px',
  md: '10px',
  lg: '14px',
  xl: '20px',
  full: '9999px',
} as const;

export const lanjingSpacing = {
  unit: '8px',
  xs: '4px',
  sm: '8px',
  md: '16px',
  lg: '24px',
  xl: '40px',
  gutter: '16px',
  cardPad: '20px',
  sectionGap: '32px',
  contentMax: '1280px',
  readingMax: '680px',
} as const;

export const lanjingMotion = {
  fast: '160ms cubic-bezier(0.32, 0.72, 0, 1)',
  standard: '260ms cubic-bezier(0.32, 0.72, 0, 1)',
  slow: '420ms cubic-bezier(0.2, 0.9, 0.1, 1)',
} as const;

export type LanjingPalette = typeof lanjingPalette;

export type MediaAppKey =
  | 'novel'
  | 'comic'
  | 'music'
  | 'video'
  | 'images'
  | 'podcast'
  | 'article'
  | 'local';

export const mediaApps: Array<{ key: MediaAppKey }> = [
  { key: 'novel' },
  { key: 'comic' },
  { key: 'music' },
  { key: 'video' },
  { key: 'images' },
  { key: 'podcast' },
  { key: 'article' },
  { key: 'local' },
];

export type CapabilityKey = 'search' | 'discover' | 'detail' | 'units' | 'asset';

export const capabilities: Array<{ key: CapabilityKey }> = [
  { key: 'search' },
  { key: 'discover' },
  { key: 'detail' },
  { key: 'units' },
  { key: 'asset' },
];

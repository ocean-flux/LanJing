export const lanjingPalette = {
  canvas: '#0b0e12',
  canvasElevated: '#12171d',
  ink: '#e8eaed',
  inkMuted: '#9aa3ad',
  inkSubtle: '#6f7882',
  hairline: 'rgb(232 234 237 / 0.12)',
  hairlineStrong: 'rgb(232 234 237 / 0.2)',
  surface1: '#151a20',
  surface2: '#1b2229',
  surface3: '#252d36',
  lantern: '#6ec8d4',
  lanternStrong: '#3aa9b8',
  lanternHover: '#2f96a4',
  lanternSoft: 'rgb(110 200 212 / 0.18)',
  lanternTint: '#143038',
  onLantern: '#061016',
  mediaVoid: '#12171d',
  readerCanvas: '#1a1714',
  readerInk: '#d8d2c4',
  positive: '#7ea882',
  warning: '#9aa3ad',
  danger: '#db5a6a',
} as const;

export const lanjingLightPalette = {
  canvas: '#f3f4f6',
  canvasElevated: '#ffffff',
  ink: '#1a1b1e',
  inkMuted: '#5c616a',
  inkSubtle: '#7a808a',
  hairline: 'rgb(26 27 30 / 0.10)',
  hairlineStrong: 'rgb(26 27 30 / 0.16)',
  surface1: '#ffffff',
  surface2: '#f6f7f9',
  surface3: '#e8eaee',
  lantern: '#0f6e7a',
  lanternStrong: '#0b5a64',
  lanternHover: '#094c55',
  lanternSoft: 'rgb(15 110 122 / 0.12)',
  lanternTint: '#d9e8ea',
  onLantern: '#f4fcfd',
  mediaVoid: '#e4e6ea',
  readerCanvas: '#f3efe6',
  readerInk: '#211e1a',
  positive: '#557d59',
  warning: '#5c616a',
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
  'novel' | 'comic' | 'music' | 'video' | 'images' | 'podcast' | 'article' | 'local';

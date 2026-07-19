/** 可丢弃 UI 原型稿数据 — 灯芯与帧 (Wick & Frame) */

export type BoardId =
  | 'discover'
  | 'library'
  | 'reader'
  | 'player'
  | 'empty'
  | 'settings'
  | 'install'
  | 'mobile-discover';

export type PackId = 'inkstone' | 'cinnabar';

export type BoardMeta = {
  id: BoardId;
  label: string;
  title: string;
  blurb: string;
  form: 'desktop' | 'mobile';
};

export const boards: BoardMeta[] = [
  {
    id: 'discover',
    label: '01 境',
    title: '发现 · 非对称镶嵌',
    blurb: '封面主导叙事，拒绝同质卡片栅格',
    form: 'desktop',
  },
  {
    id: 'library',
    label: '02 库',
    title: '资料库 · 窄脊帧',
    blurb: '48px 灯芯脊 + 全宽媒体物，继续阅读文学片段',
    form: 'desktop',
  },
  {
    id: 'reader',
    label: '03 读',
    title: '阅读 · 纸面沉浸',
    blurb: '壳层隐退，Source Serif 长文，暖纸独立 L3',
    form: 'desktop',
  },
  {
    id: 'player',
    label: '04 播',
    title: '播放 · 媒体舞台',
    blurb: 'cool void 舞台，单一前景，环境音不抢戏',
    form: 'desktop',
  },
  {
    id: 'empty',
    label: '05 空',
    title: '诚实空态',
    blurb: '无假货架；点名真实下一步',
    form: 'desktop',
  },
  {
    id: 'settings',
    label: '06 设',
    title: '双轨主题',
    blurb: '墨砚精密 / 冷银朱 分轨绑定 L1',
    form: 'desktop',
  },
  {
    id: 'install',
    label: '07 装',
    title: '安装来源',
    blurb: 'opaque candidate + 能力授予，不泄 Plan JSON',
    form: 'desktop',
  },
  {
    id: 'mobile-discover',
    label: '08 机',
    title: '移动发现',
    blurb: '底栏四境 + 安全区，封面流而非网站缩放',
    form: 'mobile',
  },
];

export const continueExcerpt =
  '夜航船靠岸时，江面只剩灯影。他把书签夹在第三页，听雨声把远处的市声压成一层薄纱。';

export const discoverTiles = [
  { id: 'd1', kind: 'novel', title: '夜航船', meta: '继续 · 第 3 章', featured: true },
  { id: 'd2', kind: 'music', title: '雨巷习作', meta: '本地 · 专辑', featured: false },
  { id: 'd3', kind: 'video', title: '岸边短片', meta: '来源已验证', featured: false },
  { id: 'd4', kind: 'novel', title: '灯下书', meta: '最近添加', featured: false },
  { id: 'd5', kind: 'music', title: '潮音', meta: '环境可随行', featured: false },
] as const;

export const libraryItems = [
  { id: 'l1', title: '夜航船', type: '小说', progress: '62%' },
  { id: 'l2', title: '雨巷习作', type: '音乐', progress: '—' },
  { id: 'l3', title: '岸边短片', type: '视频', progress: '11:20' },
  { id: 'l4', title: '灯下书', type: '小说', progress: '未读' },
  { id: 'l5', title: '潮音', type: '音乐', progress: '收藏' },
  { id: 'l6', title: '旧港纪事', type: '小说', progress: '8%' },
  { id: 'l7', title: '银灯夜谈', type: '播客', progress: 'Ep.12' },
  { id: 'l8', title: '冷山行', type: '视频', progress: '未开始' },
] as const;

export const queueUnits = [
  { id: 'u1', title: '第一曲 · 入港', duration: '3:12' },
  { id: 'u2', title: '第二曲 · 灯影', duration: '4:01' },
  { id: 'u3', title: '第三曲 · 雨停', duration: '2:48' },
] as const;

export const installCaps = [
  { id: 'http', label: '网络请求', risk: '中', granted: true },
  { id: 'js', label: '脚本执行', risk: '高', granted: false },
  { id: 'cookie', label: 'Cookie 命名空间', risk: '中', granted: true },
] as const;

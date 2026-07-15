// PROTOTYPE ONLY — 原创本地 fixture；不得接入真实来源、播放或持久化。

export type PrototypeVariant = 'A' | 'B' | 'C';
export type PrototypeSpace = 'realm' | 'novel' | 'music' | 'reader';
export type PrototypeTheme = 'light' | 'dark';
export type PrototypePalette = 'signal' | 'tide' | 'volt';
export type PrototypeAudioState = 'playing' | 'paused' | 'none';
export type CoverMotif = 'horizon' | 'orbit' | 'type' | 'petal' | 'signal' | 'paper';

export type SpaceOption = {
  id: PrototypeSpace;
  label: string;
  shortLabel: string;
};

export type PrototypeShellProps = {
  space: PrototypeSpace;
  theme: PrototypeTheme;
  audioState: PrototypeAudioState;
  onspacechange: (space: PrototypeSpace) => void;
  onthemechange: (theme: PrototypeTheme) => void;
  onaudiochange: (state: PrototypeAudioState) => void;
  onqueueopen: () => void;
};

export type CoverFixture = {
  id: string;
  title: string;
  creator: string;
  kicker: string;
  art: string;
  motif: CoverMotif;
  foreground?: 'light' | 'dark';
};

export const spaceOptions: SpaceOption[] = [
  { id: 'realm', label: '境场', shortLabel: '境' },
  { id: 'novel', label: '小说', shortLabel: '书' },
  { id: 'music', label: '音乐', shortLabel: '声' },
  { id: 'reader', label: '阅读器', shortLabel: '读' },
];

export const paletteOptions: Array<{
  id: PrototypePalette;
  label: string;
  swatch: string;
}> = [
  { id: 'signal', label: '冷银朱红', swatch: '#cf5a3b' },
  { id: 'tide', label: '墨青冰蓝', swatch: '#4f9fad' },
  { id: 'volt', label: '黑白电光黄', swatch: '#d5e548' },
];

export const realmStories: Array<
  CoverFixture & {
    kind: string;
    note: string;
    progress?: number;
    format: 'wide' | 'tall' | 'square';
  }
> = [
  {
    id: 'realm-dawn',
    title: '在潮汐线醒来',
    creator: '林未央',
    kicker: '继续阅读',
    kind: '小说',
    note: '第 18 章 · 风暴将至',
    progress: 62,
    format: 'wide',
    motif: 'horizon',
    foreground: 'light',
    art: 'radial-gradient(circle at 72% 24%, rgba(247,224,194,.88) 0 7%, transparent 8%), linear-gradient(152deg, #0e2631 0 38%, #386b75 39% 58%, #d29a72 59% 63%, #1d1a21 64%)',
  },
  {
    id: 'realm-signal',
    title: '北纬四十度',
    creator: '缓慢电台',
    kicker: '正在播放',
    kind: '音乐',
    note: '离岸信号 · 04:12',
    progress: 42,
    format: 'square',
    motif: 'signal',
    foreground: 'light',
    art: 'radial-gradient(circle at 50% 48%, transparent 0 22%, rgba(237,231,218,.82) 23% 24%, transparent 25% 34%, rgba(237,231,218,.48) 35% 36%, transparent 37%), linear-gradient(135deg, #1b222b, #8d4635)',
  },
  {
    id: 'realm-garden',
    title: '白昼花园',
    creator: '伊澄',
    kicker: '昨日收藏',
    kind: '图集',
    note: '24 幅 · 本地缓存',
    format: 'tall',
    motif: 'petal',
    foreground: 'dark',
    art: 'radial-gradient(ellipse at 32% 28%, #e2b6a5 0 13%, transparent 14%), radial-gradient(ellipse at 62% 46%, #8ea99a 0 18%, transparent 19%), linear-gradient(155deg, #ebe5dc, #aeb8b2 56%, #475255)',
  },
  {
    id: 'realm-night',
    title: '夜航备忘录',
    creator: '陈砚',
    kicker: '最近加入',
    kind: '文章',
    note: '约 12 分钟',
    format: 'wide',
    motif: 'type',
    foreground: 'light',
    art: 'linear-gradient(90deg, transparent 0 48%, rgba(225,84,56,.9) 49% 51%, transparent 52%), linear-gradient(165deg, #17171c, #374253 58%, #9fa7ad)',
  },
];

export const novelShelf: Array<
  CoverFixture & { progress: number; chapter: string; status: string }
> = [
  {
    id: 'book-tide',
    title: '在潮汐线醒来',
    creator: '林未央',
    kicker: '长篇小说',
    chapter: '第 18 章 · 风暴将至',
    status: '昨晚阅读',
    progress: 62,
    motif: 'horizon',
    foreground: 'light',
    art: 'linear-gradient(176deg, transparent 0 56%, rgba(231,225,210,.82) 57% 59%, transparent 60%), radial-gradient(circle at 68% 22%, #e6c7a4 0 7%, transparent 8%), linear-gradient(145deg, #16303b, #517684 62%, #1b2028)',
  },
  {
    id: 'book-archive',
    title: '无声档案馆',
    creator: '周昼',
    kicker: '悬疑',
    chapter: '第 7 卷 · 缺失的索引',
    status: '3 天前',
    progress: 34,
    motif: 'type',
    foreground: 'light',
    art: 'linear-gradient(90deg, rgba(232,227,215,.8) 0 2%, transparent 2% 16%, rgba(232,227,215,.34) 16% 17%, transparent 17%), linear-gradient(160deg, #14161a, #454340)',
  },
  {
    id: 'book-snow',
    title: '雪原以南',
    creator: '沈砚秋',
    kicker: '短篇集',
    chapter: '五 · 候鸟',
    status: '本周加入',
    progress: 18,
    motif: 'paper',
    foreground: 'dark',
    art: 'linear-gradient(170deg, transparent 0 68%, rgba(48,61,72,.28) 69% 72%, transparent 73%), linear-gradient(145deg, #f1ede4, #aeb9c2 54%, #67727a)',
  },
  {
    id: 'book-orbit',
    title: '微光轨道',
    creator: '唐屿',
    kicker: '科幻',
    chapter: '第 31 节 · 近地点',
    status: '收藏',
    progress: 0,
    motif: 'orbit',
    foreground: 'light',
    art: 'radial-gradient(circle at 54% 42%, transparent 0 17%, rgba(223,214,198,.72) 18% 19%, transparent 20% 30%, rgba(223,214,198,.32) 31% 32%, transparent 33%), linear-gradient(140deg, #161820, #44384c 58%, #9a5a4e)',
  },
];

export const albums: Array<CoverFixture & { year: string; tracks: number; mood: string }> = [
  {
    id: 'album-offshore',
    title: '离岸信号',
    creator: '缓慢电台',
    kicker: '正在播放',
    year: '2026',
    tracks: 9,
    mood: '夜航 / 氛围',
    motif: 'signal',
    foreground: 'light',
    art: 'radial-gradient(circle at 50% 48%, transparent 0 20%, rgba(239,232,219,.8) 21% 22%, transparent 23% 31%, rgba(239,232,219,.42) 32% 33%, transparent 34%), linear-gradient(135deg, #19212a, #9b4d38)',
  },
  {
    id: 'album-rain',
    title: '雨停之前',
    creator: '南窗',
    kicker: '最近播放',
    year: '2025',
    tracks: 7,
    mood: '器乐 / 室内',
    motif: 'horizon',
    foreground: 'light',
    art: 'linear-gradient(120deg, transparent 0 52%, rgba(231,224,209,.65) 53% 54%, transparent 55%), linear-gradient(150deg, #29424b, #687c7e 55%, #c2a58a)',
  },
  {
    id: 'album-ember',
    title: '余烬采样',
    creator: 'Mica',
    kicker: '固定歌单',
    year: '2024',
    tracks: 12,
    mood: '电子 / 低速',
    motif: 'orbit',
    foreground: 'light',
    art: 'radial-gradient(circle at 44% 40%, rgba(226,93,60,.9) 0 7%, transparent 8%), radial-gradient(circle at 45% 40%, transparent 0 26%, rgba(226,209,191,.55) 27% 28%, transparent 29%), linear-gradient(145deg, #1c1718, #4b3031)',
  },
];

export const tracks = [
  { id: 'track-north', title: '北纬四十度', artist: '缓慢电台', duration: '04:12' },
  { id: 'track-lighthouse', title: '灯塔以西', artist: '缓慢电台', duration: '03:47' },
  { id: 'track-current', title: '逆流层', artist: '缓慢电台', duration: '05:08' },
  { id: 'track-fog', title: '雾中短波', artist: '缓慢电台', duration: '02:56' },
];

export const ambientTrack = {
  title: '北纬四十度',
  artist: '缓慢电台 · 离岸信号',
  elapsed: '01:46',
  duration: '04:12',
  progress: 42,
  art: albums[0].art,
};

export const readerParagraphs = [
  '风从海面向陆地移动，经过废弃灯塔时短暂停顿，像有人在那里翻动一页很薄的纸。',
  '岑遥把窗推开一条缝。潮气沿木框渗入室内，桌上的收音机仍停在昨夜的频率。没有人说话，只有远处浮标以固定间隔发出微弱的红光。',
  '她想起地图背面的那句话：当岸线开始后退，不要追随第一束光。',
  '天亮前，整座镇子都听见了来自北方的长鸣。',
];

export const variantNames: Record<PrototypeVariant, string> = {
  A: '稳定画框',
  B: '空间接管',
  C: '边缘门户',
};

export type MediaKind = 'text' | 'image' | 'audio' | 'video' | 'mixed';
export interface MediaItem {
  id: string;
  title: string;
  creator: string;
  kind: MediaKind;
  progress?: number;
  source?: string;
}

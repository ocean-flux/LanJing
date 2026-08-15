import { Icon, type IconName } from '@/components/Icon';
import { Badge } from '@/components/ui/badge';
import { Item, ItemContent, ItemMedia, ItemTitle } from '@/components/ui/item';
import { useMessages } from '@/shared/i18n/messages';
import type { MediaKind } from '@/shared/types/media';

type AppSurface = {
  id: string;
  kind: MediaKind;
  icon: IconName;
  /** 该媒体面是否已经接线；当前全部尚未实现，不伪装成可用。 */
  enabled: boolean;
};

const SURFACES: readonly AppSurface[] = [
  { id: 'reading', kind: 'text', icon: 'book-open', enabled: false },
  { id: 'gallery', kind: 'image', icon: 'image', enabled: false },
  { id: 'podcast', kind: 'audio', icon: 'microphone-stage', enabled: false },
  { id: 'video', kind: 'video', icon: 'film-slate', enabled: false },
  { id: 'music', kind: 'mixed', icon: 'music-notes', enabled: false },
];

export function AppsHome() {
  const m = useMessages();
  const titles: Record<string, string> = {
    reading: m.apps_reading_title(),
    gallery: m.apps_gallery_title(),
    podcast: m.apps_podcast_title(),
    video: m.apps_video_title(),
    music: m.apps_music_title(),
  };

  return (
    <div className="px-(--page-gutter) py-(--density-section-gap)">
      <ul className="divide-y divide-hairline border-y border-hairline">
        {SURFACES.map((surface) => (
          <li key={surface.id}>
            <Item size="xs" aria-disabled={!surface.enabled} className="aria-disabled:opacity-60">
              <ItemMedia variant="icon">
                <Icon name={surface.icon} className="text-base text-ink-subtle" />
              </ItemMedia>
              <ItemContent>
                <ItemTitle>{titles[surface.id]}</ItemTitle>
              </ItemContent>
              <span className="font-mono text-ui-sm text-ink-subtle">{surface.kind}</span>
              {surface.enabled ? null : (
                <Badge variant="secondary" className="shrink-0">
                  {m.apps_unavailable()}
                </Badge>
              )}
            </Item>
          </li>
        ))}
      </ul>
    </div>
  );
}

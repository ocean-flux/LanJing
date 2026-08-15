import { BookOpenText, Compass, Film, Headphones, Image, Music2, Search } from 'lucide-react';
import { useState } from 'react';
import { Card, CardContent, Input } from '@/components/ui';
import { useMessages } from '@/shared/i18n/messages';

export function AppsHome() {
  const m = useMessages();
  const [query, setQuery] = useState('');
  const apps = [
    {
      title: m.apps_reading_title(),
      description: m.apps_reading_description(),
      icon: BookOpenText,
      accent: 'text-(--accent-strong) bg-(--lantern-tint)',
    },
    {
      title: m.apps_podcast_title(),
      description: m.apps_podcast_description(),
      icon: Headphones,
      accent: 'text-(--accent-strong) bg-(--lantern-tint)',
    },
    {
      title: m.apps_video_title(),
      description: m.apps_video_description(),
      icon: Film,
      accent: 'text-(--accent-strong) bg-(--lantern-tint)',
    },
    {
      title: m.apps_gallery_title(),
      description: m.apps_gallery_description(),
      icon: Image,
      accent: 'text-(--accent-strong) bg-(--lantern-tint)',
    },
    {
      title: m.apps_music_title(),
      description: m.apps_music_description(),
      icon: Music2,
      accent: 'text-(--accent-strong) bg-(--lantern-tint)',
    },
  ];
  const filtered = apps.filter(
    (app) => app.title.includes(query) || app.description.includes(query),
  );

  return (
    <div className="mx-auto max-w-7xl px-5 py-8 sm:px-8 lg:px-12 lg:py-12">
      <div className="flex flex-col justify-between gap-5 border-b border-(--border) pb-8 sm:flex-row sm:items-end">
        <div>
          <p className="eyebrow">{m.apps_eyebrow()}</p>
          <h2 className="font-display mt-2 text-4xl font-semibold">{m.apps_heading()}</h2>
          <p className="mt-3 max-w-lg text-(--muted-text)">{m.apps_description()}</p>
        </div>
      </div>
      <div className="mt-7 flex max-w-md items-center gap-2">
        <Search className="ml-3 text-(--muted-text)" size={17} />
        <Input
          aria-label={m.apps_search_label()}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={m.apps_search_placeholder()}
          className="-ml-10 pl-10"
        />
      </div>
      <div className="mt-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {filtered.map(({ title, description, icon: Icon, accent }) => (
          <Card
            key={title}
            className="group transition-[transform,box-shadow] hover:-translate-y-0.5 hover:shadow-md"
          >
            <CardContent className="flex min-h-44 flex-col justify-between pt-5">
              <div className={`grid h-11 w-11 place-items-center rounded-md ${accent}`}>
                <Icon size={21} />
              </div>
              <div>
                <h3 className="font-display mt-8 text-xl font-semibold">{title}</h3>
                <p className="mt-1 text-sm leading-6 text-(--muted-text)">{description}</p>
              </div>
            </CardContent>
          </Card>
        ))}
        <Card className="border-dashed bg-transparent">
          <CardContent className="flex min-h-44 flex-col items-center justify-center text-center">
            <Compass className="text-(--muted-text)" size={25} />
            <p className="mt-3 text-sm font-medium">{m.apps_more_title()}</p>
            <p className="mt-1 text-xs text-(--muted-text)">{m.apps_more_description()}</p>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

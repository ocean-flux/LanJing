import { ArrowRight, BookOpen, Headphones, Image, Play, Sparkles } from 'lucide-react';
import { Link } from 'react-router-dom';
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui';
import { useMessages } from '@/shared/i18n/messages';

export function RealmHome() {
  const m = useMessages();
  const recent = [
    {
      title: m.realm_recent_tide_title(),
      detail: m.realm_recent_tide_detail(),
      icon: BookOpen,
      tone: 'bg-(--lantern-tint)',
    },
    {
      title: m.realm_recent_night_title(),
      detail: m.realm_recent_night_detail(),
      icon: Headphones,
      tone: 'bg-(--lantern-tint)',
    },
    {
      title: m.realm_recent_city_title(),
      detail: m.realm_recent_city_detail(),
      icon: Image,
      tone: 'bg-(--lantern-tint)',
    },
  ];

  return (
    <div className="mx-auto max-w-7xl px-5 py-8 sm:px-8 lg:px-12 lg:py-12">
      <section className="relative overflow-hidden border-b border-(--border) pt-3 pb-12 lg:pb-16">
        <div className="max-w-3xl">
          <Badge className="border-(--accent)/30 bg-(--accent-soft) text-(--accent-strong)">
            <Sparkles size={13} className="mr-1" />
            {m.realm_badge()}
          </Badge>
          <h2 className="font-display mt-5 max-w-2xl text-4xl leading-[1.08] font-semibold tracking-normal sm:text-6xl">
            {m.realm_headline_before()}
            <br />
            <span className="text-(--accent-strong)">{m.realm_headline_after()}</span>
          </h2>
          <p className="mt-6 max-w-xl text-base leading-7 text-(--muted-text)">
            {m.realm_description()}
          </p>
          <div className="mt-8 flex flex-wrap gap-3">
            <Button asChild size="lg">
              <Link to="/apps">
                {m.realm_explore()} <ArrowRight size={17} />
              </Link>
            </Button>
            <Button asChild size="lg" variant="outline">
              <Link to="/sources">{m.action_manage_sources()}</Link>
            </Button>
          </div>
        </div>
        <div className="pointer-events-none absolute right-8 -bottom-16 hidden h-56 w-56 rotate-12 border border-(--accent)/25 md:block">
          <div className="absolute inset-5 border border-(--accent)/25" />
          <div className="absolute top-1/2 -left-8 h-px w-72 bg-(--accent)/30" />
        </div>
      </section>
      <section className="grid gap-10 py-10 lg:grid-cols-[1.15fr_0.85fr] lg:gap-16">
        <div>
          <div className="flex items-end justify-between">
            <div>
              <p className="eyebrow">{m.realm_continue()}</p>
              <h3 className="font-display mt-2 text-2xl font-semibold">{m.realm_recent()}</h3>
            </div>
            <Link className="text-sm text-(--accent-strong) hover:underline" to="/library">
              {m.realm_open_library()}
            </Link>
          </div>
          <div className="mt-5 space-y-3">
            {recent.map(({ title, detail, icon: Icon, tone }) => (
              <Link
                key={title}
                to="/library"
                className="group flex items-center gap-4 border-b border-(--border) py-4 transition-colors hover:border-(--accent)"
              >
                <span
                  className={`grid h-12 w-12 shrink-0 place-items-center rounded-md ${tone} text-(--accent-strong)`}
                >
                  <Icon size={21} />
                </span>
                <span className="min-w-0 flex-1">
                  <strong className="font-display block text-lg font-semibold">{title}</strong>
                  <span className="text-sm text-(--muted-text)">{detail}</span>
                </span>
                <ArrowRight
                  className="text-(--muted-text) transition-transform group-hover:translate-x-1"
                  size={18}
                />
              </Link>
            ))}
          </div>
        </div>
        <Card className="self-start border-(--accent)/20 bg-(--accent-soft)">
          <CardHeader>
            <p className="eyebrow">{m.realm_workspace_status()}</p>
            <CardTitle className="text-2xl">{m.realm_status_title()}</CardTitle>
            <CardDescription>{m.realm_status_description()}</CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex items-center justify-between border-t border-(--accent)/20 pt-4 text-sm">
              <span className="text-(--muted-text)">{m.realm_indexed_content()}</span>
              <strong>{m.realm_indexed_count({ count: 12 })}</strong>
            </div>
            <div className="mt-3 flex items-center justify-between text-sm">
              <span className="text-(--muted-text)">{m.realm_storage_location()}</span>
              <strong>{m.realm_this_device()}</strong>
            </div>
            <Link
              to="/settings"
              className="mt-5 inline-flex items-center gap-2 text-sm font-medium text-(--accent-strong)"
            >
              {m.realm_privacy_settings()} <ArrowRight size={15} />
            </Link>
          </CardContent>
        </Card>
      </section>
      <div className="flex items-center gap-2 border-t border-(--border) pt-5 text-xs text-(--muted-text)">
        <Play size={13} className="text-(--accent)" />
        {m.realm_footer()}
      </div>
    </div>
  );
}

/**
 * 壳层演示数据：境场状态、来源卡、媒体应用卡、迷你播放器槽。
 * 仅 UI 占位，非真实库/规则数据。
 */
import { capabilities, type MediaAppKey } from '$lib/brand';
import { m } from '$lib/i18n';
import type {
  MediaAppCardState,
  MiniPlayerSlotState,
  RealmState,
  SourceCardState,
} from './shell-types';

const fact = (label: string, value: string) => ({ label, value });

const mediaLabels: Record<MediaAppKey, string> = {
  novel: m.media_label_novel(),
  comic: m.media_label_comic(),
  music: m.media_label_music(),
  video: m.media_label_video(),
  images: m.media_label_images(),
  podcast: m.media_label_podcast(),
  article: m.media_label_article(),
  local: m.media_label_local(),
};

const mediaActions: Record<MediaAppKey, string> = {
  novel: m.media_novel_action(),
  comic: m.media_comic_action(),
  music: m.media_music_action(),
  video: m.media_video_action(),
  images: m.media_images_action(),
  podcast: m.media_podcast_action(),
  article: m.media_article_action(),
  local: m.media_local_action(),
};

/** 境场：尚无来源。 */
export const noSourceRealmState: RealmState = {
  kind: 'no-source',
  title: m.realm_title_no_source(),
  description: m.realm_desc_no_source(),
  primaryAction: m.action_add_source(),
  secondaryAction: m.action_import_local(),
};

/** 境场：有来源但无资源。 */
export const sourceNoResourceRealmState: RealmState = {
  kind: 'source-no-resource',
  title: m.realm_title_no_resource(),
  description: m.realm_desc_no_resource(),
  primaryAction: m.action_search_content(),
  secondaryAction: m.action_import_local(),
  sourceSummary: m.realm_summary_one_source(),
};

/** 境场：来源告警。 */
export const sourceWarningRealmState: RealmState = {
  kind: 'source-warning',
  title: m.realm_title_warning(),
  description: m.realm_desc_warning(),
  primaryAction: m.action_view_source_status(),
  secondaryAction: m.action_continue_available(),
  sourceSummary: m.realm_summary_warning(),
};

/** 演示用来源卡列表。 */
export const demoSources: SourceCardState[] = [
  {
    id: 'demo-legado',
    name: m.source_demo_novel(),
    kind: m.source_kind_online_media(),
    status: 'ready',
    summary: m.source_demo_novel_summary(),
    capabilities: Object.fromEntries(capabilities.map((capability) => [capability.key, true])),
    trustFacts: [
      fact(m.trust_origin(), m.trust_local_package()),
      fact(m.trust_network(), m.trust_access_target()),
      fact(m.trust_remote_parse(), m.trust_none()),
      fact(m.trust_isolation(), m.trust_source_only()),
    ],
    actions: [m.search(), m.action_open_discover(), m.action_view_detail()],
    checkedAt: m.time_now(),
  },
  {
    id: 'demo-warning',
    name: m.source_demo_music_failed(),
    kind: m.source_kind_music(),
    status: 'failed',
    summary: m.source_demo_music_summary(),
    capabilities: { search: true, discover: false, detail: true, units: false, asset: false },
    trustFacts: [
      fact(m.trust_origin(), m.trust_subscription_url()),
      fact(m.trust_network(), m.trust_access_target()),
      fact(m.trust_remote_parse(), m.trust_possible()),
      fact(m.trust_isolation(), m.trust_no_others()),
    ],
    actions: [m.action_retry(), m.action_view_reason(), m.action_disable()],
    checkedAt: m.time_12_min(),
  },
  {
    id: 'demo-partial',
    name: m.source_demo_partial(),
    kind: m.source_kind_image(),
    status: 'partial',
    summary: m.source_demo_partial_summary(),
    capabilities: { search: true, discover: false, detail: true, units: false, asset: false },
    trustFacts: [
      fact(m.trust_origin(), m.trust_subscription_url()),
      fact(m.trust_network(), m.trust_access_target()),
      fact(m.trust_remote_parse(), m.trust_none()),
      fact(m.trust_isolation(), m.trust_source_only()),
    ],
    actions: [m.search(), m.action_view_capability(), m.action_view_detail()],
    checkedAt: m.time_20_min(),
  },
  {
    id: 'demo-disabled',
    name: m.source_demo_disabled(),
    kind: m.source_kind_video(),
    status: 'disabled',
    summary: m.source_demo_disabled_summary(),
    capabilities: {},
    trustFacts: [
      fact(m.trust_origin(), m.trust_local_package()),
      fact(m.trust_network(), m.trust_disabled()),
      fact(m.trust_remote_parse(), m.trust_none()),
      fact(m.trust_isolation(), m.trust_not_run()),
    ],
    actions: [m.action_enable(), m.action_view_detail()],
    checkedAt: m.time_yesterday(),
  },
  {
    id: 'demo-unchecked',
    name: m.source_demo_unchecked(),
    kind: m.source_kind_article(),
    status: 'unchecked',
    summary: m.source_demo_unchecked_summary(),
    capabilities: {},
    trustFacts: [
      fact(m.trust_origin(), m.trust_remote_url()),
      fact(m.trust_network(), m.trust_pending()),
      fact(m.trust_remote_parse(), m.trust_pending()),
      fact(m.trust_isolation(), m.trust_after_check()),
    ],
    actions: [m.action_start_check(), m.action_remove()],
  },
];

/** 媒体应用宫格演示卡。 */
export const mediaAppCards: MediaAppCardState[] = Object.keys(mediaLabels).map((key) => {
  const appKey = key as MediaAppKey;
  return {
    key: appKey,
    label: mediaLabels[appKey],
    description:
      appKey === 'novel'
        ? m.media_novel_desc()
        : appKey === 'music'
          ? m.media_music_desc()
          : appKey === 'video'
            ? m.media_video_desc()
            : m.media_generic_desc(),
    status: appKey === 'novel' ? 'explorable' : 'unconnected',
    statusLabel: appKey === 'novel' ? m.app_status_explorable() : m.app_status_unconnected(),
    primaryAction: mediaActions[appKey],
    href: appKey === 'novel' ? '/apps/novel' : undefined,
  };
});

/** 迷你播放器槽演示态（占位、无真实播放）。 */
export const miniPlayerSlot: MiniPlayerSlotState = {
  reserved: true,
  visible: false,
  label: m.mini_player_empty(),
};

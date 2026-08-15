import { getVersion } from '@tauri-apps/api/app';
import { isTauri } from '@tauri-apps/api/core';
import { useEffect, useState } from 'react';
import { Icon } from '@/components/Icon';
import { Field, FieldLabel, FieldSet, FieldLegend, FieldDescription } from '@/components/ui/field';
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group';
import { useLocale, useMessages, type AppLocale } from '@/shared/i18n/messages';
import { useTheme } from '@/shared/theme/use-theme';
import {
  DARK_PACK_IDS,
  LIGHT_PACK_IDS,
  type AppearancePackId,
  type Theme,
} from '@/shared/theme/theme';

/** 显式映射，避免用模板字符串拼 message key 而绕过类型检查。 */
function packName(id: AppearancePackId, m: ReturnType<typeof useMessages>): string {
  switch (id) {
    case 'obsidian-void': {
      return m.settings_theme_obsidian_void();
    }
    case 'graphite-atelier': {
      return m.settings_theme_graphite_atelier();
    }
    case 'porcelain-day': {
      return m.settings_theme_porcelain_day();
    }
    case 'mist-studio': {
      return m.settings_theme_mist_studio();
    }
  }
}

/**
 * 色板直接把 data-appearance-pack 挂在自身上，让 index.css 的色值块生效，
 * 因此预览就是该主题的真实配色，而不是另抄一份色值。
 */
function PackSwatch({ id, label }: { id: AppearancePackId; label: string }) {
  return (
    <span
      data-appearance-pack={id}
      aria-label={label}
      role="img"
      className="flex h-4 shrink-0 overflow-hidden border border-hairline-strong"
    >
      <span className="w-3 bg-canvas" />
      <span className="w-3 bg-surface-2" />
      <span className="w-3 bg-lantern" />
    </span>
  );
}

function PackChoice({
  ids,
  value,
  onChange,
  label,
}: {
  ids: readonly AppearancePackId[];
  value: AppearancePackId;
  onChange: (id: AppearancePackId) => void;
  label: string;
}) {
  const m = useMessages();
  return (
    <Field orientation="horizontal">
      <FieldLabel>{label}</FieldLabel>
      <ToggleGroup
        value={[value]}
        onValueChange={(next) => {
          const [selected] = next;
          if (selected && ids.includes(selected as AppearancePackId)) {
            onChange(selected as AppearancePackId);
          }
        }}
      >
        {ids.map((id) => {
          const name = packName(id, m);
          return (
            <ToggleGroupItem key={id} value={id} className="gap-2">
              <PackSwatch id={id} label={m.settings_theme_swatch({ name })} />
              {name}
            </ToggleGroupItem>
          );
        })}
      </ToggleGroup>
    </Field>
  );
}

export function SettingsHome() {
  const m = useMessages();
  const { locale, setLocale } = useLocale();
  const { theme, setTheme, lightThemeId, darkThemeId, chooseAppearancePack } = useTheme();
  const [version, setVersion] = useState('');

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    void getVersion()
      .then((value) => {
        if (!cancelled) setVersion(value);
      })
      .catch(() => {
        if (!cancelled) setVersion('');
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const modes: { value: Theme; label: string; icon: 'sun' | 'moon' | 'monitor' }[] = [
    { value: 'light', label: m.theme_mode_light(), icon: 'sun' },
    { value: 'dark', label: m.theme_mode_dark(), icon: 'moon' },
    { value: 'system', label: m.theme_mode_system(), icon: 'monitor' },
  ];

  return (
    <div className="max-w-(--measure-detail) px-(--page-gutter) py-(--density-section-gap)">
      <FieldSet className="border-b border-hairline pb-(--density-section-gap)">
        <FieldLegend>{m.settings_appearance_group()}</FieldLegend>
        <Field orientation="horizontal">
          <FieldLabel>{m.settings_mode_label()}</FieldLabel>
          <ToggleGroup
            value={[theme]}
            onValueChange={(next) => {
              const [selected] = next;
              if (selected) setTheme(selected as Theme);
            }}
          >
            {modes.map((mode) => (
              <ToggleGroupItem key={mode.value} value={mode.value} className="gap-1.5">
                <Icon name={mode.icon} className="text-base" />
                {mode.label}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>
        </Field>
        <PackChoice
          ids={LIGHT_PACK_IDS}
          value={lightThemeId}
          onChange={chooseAppearancePack}
          label={m.settings_theme_light_label()}
        />
        <PackChoice
          ids={DARK_PACK_IDS}
          value={darkThemeId}
          onChange={chooseAppearancePack}
          label={m.settings_theme_dark_label()}
        />
      </FieldSet>

      <FieldSet className="border-b border-hairline py-(--density-section-gap)">
        <FieldLegend>{m.settings_language_group()}</FieldLegend>
        <Field orientation="horizontal">
          <FieldLabel>{m.settings_language()}</FieldLabel>
          <ToggleGroup
            value={[locale]}
            onValueChange={(next) => {
              const [selected] = next;
              if (selected === 'zh-CN' || selected === 'en') {
                void setLocale(selected as AppLocale);
              }
            }}
          >
            <ToggleGroupItem value="zh-CN">{m.settings_lang_zh()}</ToggleGroupItem>
            <ToggleGroupItem value="en">{m.settings_lang_en()}</ToggleGroupItem>
          </ToggleGroup>
        </Field>
      </FieldSet>

      <FieldSet className="border-b border-hairline py-(--density-section-gap)">
        <FieldLegend>{m.settings_privacy_group()}</FieldLegend>
        <FieldDescription>{m.settings_privacy_description()}</FieldDescription>
      </FieldSet>

      <FieldSet className="pt-(--density-section-gap)">
        <FieldLegend>{m.settings_about_group()}</FieldLegend>
        <Field orientation="horizontal">
          <FieldLabel>{m.settings_version()}</FieldLabel>
          <span className="font-mono tabular-nums">{version || m.settings_version_unknown()}</span>
        </Field>
      </FieldSet>
    </div>
  );
}

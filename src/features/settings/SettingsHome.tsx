import { Check, Monitor, Moon, ShieldCheck, Sun } from 'lucide-react';
import { Button, Card, CardContent, CardHeader, CardTitle } from '@/components/ui';
import { useLocale, useMessages } from '@/shared/i18n/messages';
import { useTheme } from '@/shared/theme/use-theme';
import type { AppearancePackId, Theme } from '@/shared/theme/theme';

export function SettingsHome() {
  const m = useMessages();
  const { locale, setLocale } = useLocale();
  const { theme, setTheme, lightThemeId, darkThemeId, chooseAppearancePack } = useTheme();
  const themes: { value: Theme; label: string; icon: typeof Sun }[] = [
    { value: 'light', label: m.theme_mode_light(), icon: Sun },
    { value: 'dark', label: m.theme_mode_dark(), icon: Moon },
    { value: 'system', label: m.theme_mode_system(), icon: Monitor },
  ];

  return (
    <div className="mx-auto max-w-5xl px-5 py-8 sm:px-8 lg:px-12 lg:py-12">
      <div className="border-b border-(--border) pb-8">
        <p className="eyebrow">{m.settings_title()}</p>
        <h2 className="font-display mt-2 text-4xl font-semibold">{m.settings_title()}</h2>
        <p className="mt-3 text-(--muted-text)">{m.settings_description()}</p>
      </div>
      <div className="mt-8 grid gap-5 lg:grid-cols-[1fr_0.8fr]">
        <Card>
          <CardHeader>
            <CardTitle>{m.settings_appearance_group()}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-(--muted-text)">{m.settings_appearance_description()}</p>
            <div className="mt-5 grid grid-cols-3 gap-2">
              {themes.map(({ value, label, icon: Icon }) => (
                <Button
                  key={value}
                  variant={theme === value ? 'secondary' : 'outline'}
                  className="h-auto flex-col gap-2 py-3"
                  onClick={() => setTheme(value)}
                  aria-pressed={theme === value}
                >
                  <Icon size={18} />
                  <span>{label}</span>
                  {theme === value && <Check size={13} className="text-(--accent-strong)" />}
                </Button>
              ))}
            </div>
            <div className="mt-6 space-y-4 border-t border-(--border) pt-6">
              <div>
                <p className="text-sm font-medium">{m.settings_theme_light_label()}</p>
                <div className="mt-2 grid grid-cols-2 gap-2">
                  {(['porcelain-day', 'mist-studio'] as AppearancePackId[]).map((id) => (
                    <Button
                      key={id}
                      variant={lightThemeId === id ? 'secondary' : 'outline'}
                      className="h-auto justify-start py-2"
                      onClick={() => chooseAppearancePack(id)}
                      aria-pressed={lightThemeId === id}
                    >
                      {m[`settings_theme_${id.replace('-', '_')}`]()}
                    </Button>
                  ))}
                </div>
              </div>
              <div>
                <p className="text-sm font-medium">{m.settings_theme_dark_label()}</p>
                <div className="mt-2 grid grid-cols-2 gap-2">
                  {(['obsidian-void', 'graphite-atelier'] as AppearancePackId[]).map((id) => (
                    <Button
                      key={id}
                      variant={darkThemeId === id ? 'secondary' : 'outline'}
                      className="h-auto justify-start py-2"
                      onClick={() => chooseAppearancePack(id)}
                      aria-pressed={darkThemeId === id}
                    >
                      {m[`settings_theme_${id.replace('-', '_')}`]()}
                    </Button>
                  ))}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{m.settings_language_group()}</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-(--muted-text)">{m.settings_language()}</p>
            <div className="mt-5 grid grid-cols-2 gap-2">
              <Button
                variant={locale === 'zh-CN' ? 'secondary' : 'outline'}
                onClick={() => void setLocale('zh-CN')}
                aria-pressed={locale === 'zh-CN'}
              >
                {m.settings_lang_zh()}
              </Button>
              <Button
                variant={locale === 'en' ? 'secondary' : 'outline'}
                onClick={() => void setLocale('en')}
                aria-pressed={locale === 'en'}
              >
                {m.settings_lang_en()}
              </Button>
            </div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <ShieldCheck className="text-(--accent-strong)" size={19} />
              {m.settings_privacy_group()}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm leading-6 text-(--muted-text)">
              {m.settings_privacy_description()}
            </p>
            <div className="mt-6 border-t border-(--border) pt-4">
              <p className="text-xs text-(--muted-text)">{m.settings_architecture()}</p>
              <p className="mt-1 font-medium">React 19 · Vite 8 · Tauri 2</p>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

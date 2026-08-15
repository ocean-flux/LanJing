import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { platform } from '@tauri-apps/plugin-os';
import { Maximize2, Minus, Moon, Sun, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { toast } from '@/components/ui/toast';
import { appConfig } from '@/shared/config/app';
import { useMessages } from '@/shared/i18n/messages';
import { useTheme } from '@/shared/theme/use-theme';

function isDesktopTauriWindow() {
  if (!isTauri()) {
    return false;
  }
  try {
    return ['windows', 'macos', 'linux'].includes(platform());
  } catch {
    return false;
  }
}

function runWindowCommand(command: () => Promise<void>, label: string) {
  void command().catch(() => toast(`${label}失败`, '请重试或使用系统窗口控件。'));
}

export function Titlebar() {
  const m = useMessages();
  const { theme, toggleTheme } = useTheme();
  const showWindowControls = isDesktopTauriWindow();

  return (
    <header
      className="titlebar flex h-12 items-center justify-between border-b border-(--border) px-4"
      data-tauri-drag-region
    >
      <div className="flex items-center gap-3">
        <span className="lanjing-mark" aria-hidden="true">
          L
        </span>
        <span className="font-display text-sm font-semibold tracking-wide">{appConfig.name}</span>
        <span className="hidden text-xs text-(--muted-text) sm:inline">
          {appConfig.englishName}
        </span>
      </div>
      <div className="flex items-center gap-1">
        <Button
          aria-label={theme === 'dark' ? m.theme_mode_light() : m.theme_mode_dark()}
          title={m.settings_appearance_group()}
          variant="ghost"
          size="icon"
          onClick={toggleTheme}
        >
          {theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
        </Button>
        {showWindowControls && (
          <>
            <Button
              aria-label={m.window_minimize()}
              title={m.window_minimize()}
              variant="ghost"
              size="icon"
              onClick={() =>
                runWindowCommand(() => getCurrentWindow().minimize(), m.window_minimize())
              }
            >
              <Minus size={16} aria-hidden="true" />
            </Button>
            <Button
              aria-label={m.window_toggle_maximize()}
              title={m.window_toggle_maximize()}
              variant="ghost"
              size="icon"
              onClick={() =>
                runWindowCommand(
                  () => getCurrentWindow().toggleMaximize(),
                  m.window_toggle_maximize(),
                )
              }
            >
              <Maximize2 size={16} aria-hidden="true" />
            </Button>
            <Button
              aria-label={m.window_close()}
              title={m.window_close()}
              variant="ghost"
              size="icon"
              onClick={() => runWindowCommand(() => getCurrentWindow().close(), m.window_close())}
            >
              <X size={16} aria-hidden="true" />
            </Button>
          </>
        )}
      </div>
    </header>
  );
}

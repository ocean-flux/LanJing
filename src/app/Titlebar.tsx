import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { platform } from '@tauri-apps/plugin-os';
import { toast } from 'sonner';
import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import { SidebarTrigger } from '@/components/ui/sidebar';
import { useMessages } from '@/shared/i18n/messages';
import { useTheme } from '@/shared/theme/use-theme';

function isDesktopTauriWindow() {
  if (!isTauri()) return false;
  try {
    return ['windows', 'macos', 'linux'].includes(platform());
  } catch {
    return false;
  }
}

export function Titlebar({ title }: { title: string }) {
  const m = useMessages();
  const { resolvedTheme, setTheme } = useTheme();
  const showWindowControls = isDesktopTauriWindow();

  const runWindowCommand = (command: () => Promise<void>, label: string) => {
    void command().catch(() => {
      toast.error(m.shell_window_command_failed({ action: label }), {
        description: m.shell_window_command_failed_hint(),
      });
    });
  };

  const nextTheme = resolvedTheme === 'dark' ? 'light' : 'dark';
  const themeLabel = resolvedTheme === 'dark' ? m.theme_mode_light() : m.theme_mode_dark();

  return (
    <header
      data-tauri-drag-region
      className="titlebar flex h-(--shell-titlebar-height) shrink-0 items-center gap-2 border-b border-hairline bg-surface-1 px-2"
      aria-label={m.titlebar_label({ context: title })}
    >
      <SidebarTrigger aria-label={m.nav_main()} title={m.nav_main()} />
      <span className="truncate font-medium">{title}</span>
      <div className="ml-auto flex items-center gap-0.5">
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={themeLabel}
          title={themeLabel}
          onClick={() => setTheme(nextTheme)}
        >
          <Icon name={resolvedTheme === 'dark' ? 'sun' : 'moon'} className="text-base" />
        </Button>
        {showWindowControls ? (
          <>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={m.window_minimize()}
              title={m.window_minimize()}
              onClick={() =>
                runWindowCommand(() => getCurrentWindow().minimize(), m.window_minimize())
              }
            >
              <Icon name="minus" className="text-base" />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={m.window_toggle_maximize()}
              title={m.window_toggle_maximize()}
              onClick={() =>
                runWindowCommand(
                  () => getCurrentWindow().toggleMaximize(),
                  m.window_toggle_maximize(),
                )
              }
            >
              <Icon name="square" className="text-base" />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={m.window_close()}
              title={m.window_close()}
              onClick={() => runWindowCommand(() => getCurrentWindow().close(), m.window_close())}
            >
              <Icon name="x" className="text-base" />
            </Button>
          </>
        ) : null}
      </div>
    </header>
  );
}

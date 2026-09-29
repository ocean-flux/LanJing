//! 画布快捷键速查。
//!
//! 快捷键本身挂在 RuleFlowCanvas 的 window keydown 上，这里只负责让它们
//! 可被发现 —— 改了那边的按键组合，这份清单也要跟着改。

import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import { Kbd, KbdGroup } from '@/components/ui/kbd';
import { Popover, PopoverContent, PopoverTitle, PopoverTrigger } from '@/components/ui/popover';
import { useMessages } from '@/shared/i18n/messages';

/** 当前平台为 macOS 时主修饰键显示为 ⌘；Tauri 桌面端按 UA 判一次就够。 */
const MOD = typeof navigator !== 'undefined' && /mac/iu.test(navigator.userAgent) ? '⌘' : 'Ctrl';

type Messages = ReturnType<typeof useMessages>;

const SHORTCUTS: readonly { keys: readonly string[]; label: (m: Messages) => string }[] = [
  { keys: [MOD, 'Z'], label: (m) => m.rules_shortcut_undo() },
  { keys: [MOD, 'Shift', 'Z'], label: (m) => m.rules_shortcut_redo() },
  { keys: [MOD, 'C'], label: (m) => m.rules_shortcut_copy() },
  { keys: [MOD, 'V'], label: (m) => m.rules_shortcut_paste() },
  { keys: [MOD, 'D'], label: (m) => m.rules_shortcut_duplicate() },
  { keys: ['Delete'], label: (m) => m.rules_shortcut_delete() },
  { keys: ['Shift'], label: (m) => m.rules_shortcut_marquee() },
];

export function ShortcutHints() {
  const m = useMessages();

  return (
    <Popover>
      <PopoverTrigger
        render={
          <Button
            variant="outline"
            size="icon-sm"
            className="shrink-0"
            aria-label={m.rules_shortcuts()}
            title={m.rules_shortcuts()}
          >
            <Icon name="keyboard" />
          </Button>
        }
      />
      <PopoverContent align="end" className="w-64">
        <PopoverTitle className="mb-2">{m.rules_shortcuts()}</PopoverTitle>
        <dl className="flex flex-col gap-1.5">
          {SHORTCUTS.map((shortcut) => (
            <div key={shortcut.keys.join('+')} className="flex items-center justify-between gap-3">
              <dt className="min-w-0 truncate text-ui-sm text-ink-muted">{shortcut.label(m)}</dt>
              <dd>
                <KbdGroup>
                  {shortcut.keys.map((key) => (
                    <Kbd key={key}>{key}</Kbd>
                  ))}
                </KbdGroup>
              </dd>
            </div>
          ))}
        </dl>
      </PopoverContent>
    </Popover>
  );
}

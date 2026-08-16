import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { toast } from 'sonner';
import { App } from './App';
import { m } from '@/shared/i18n/messages';
import { startPreferencesPersistence } from '@/shared/theme/theme';
import { LocaleProvider } from '@/shared/i18n/locale';
import './index.css';

const root = document.querySelector('#root');
if (!root) {
  throw new Error('找不到 React 根节点');
}

// 先渲染再接管持久化：Rust 侧读盘是异步的，等它会让首帧变慢。
createRoot(root).render(
  <StrictMode>
    <LocaleProvider>
      <App />
    </LocaleProvider>
  </StrictMode>,
);

// 顶层 await 放在 render 之后：渲染已同步冲刷，这里只是接管持久化。
const persistenceError = await startPreferencesPersistence();
if (persistenceError) {
  // 主题当次仍然可用，只是重启后会还原，所以是提示而非阻断。
  toast.error(m.shell_preferences_persist_failed(), {
    description: m.shell_preferences_persist_failed_hint(),
  });
}

import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './App';
import { LocaleProvider } from '@/shared/i18n/locale';
import './index.css';

const root = document.querySelector('#root');
if (!root) {
  throw new Error('找不到 React 根节点');
}

createRoot(root).render(
  <StrictMode>
    <LocaleProvider>
      <App />
    </LocaleProvider>
  </StrictMode>,
);

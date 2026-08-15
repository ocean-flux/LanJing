import { lazy, Suspense, useEffect } from 'react';
import { BrowserRouter, Navigate, Route, Routes, useNavigate, useParams } from 'react-router-dom';
import { AppShell } from '@/app/AppShell';
import { Toaster, toast } from '@/components/ui';
import {
  AppsHome,
  LibraryHome,
  LibraryItem,
  RealmHome,
  RulesMigration,
  SettingsHome,
  SourcesHome,
} from '@/features';
import { useMessages } from '@/shared/i18n/messages';
import { startDeepLinkRuntime } from '@/shared/tauri/deep-link';

const RuleWorkspace = lazy(() =>
  import('@/features/rules/RuleWorkspace').then((module) => ({ default: module.RuleWorkspace })),
);

function Startup() {
  const m = useMessages();
  const navigate = useNavigate();

  useEffect(() => {
    document.documentElement.dir = 'ltr';
    let cancelled = false;
    let cleanup: () => void = () => undefined;
    void startDeepLinkRuntime((intent) => {
      if (cancelled) return;
      switch (intent.kind) {
        case 'item': {
          navigate(`/library/item/${encodeURIComponent(intent.resourceId)}`);
          break;
        }
        case 'source': {
          navigate(`/sources?highlight=${encodeURIComponent(intent.sourceId)}`);
          break;
        }
        case 'install': {
          navigate(`/sources?import=${encodeURIComponent(intent.src)}`);
          break;
        }
        case 'reject': {
          toast(m.sources_deeplink_error_title(), intent.subject ?? intent.reason);
          break;
        }
      }
    }).then((unlisten) => {
      if (cancelled) unlisten();
      else cleanup = unlisten;
    });
    return () => {
      cancelled = true;
      cleanup();
    };
  }, [m, navigate]);
  return null;
}

function RuleWorkspaceRoute() {
  const m = useMessages();
  return (
    <Suspense
      fallback={
        <div className="grid min-h-[480px] place-items-center text-sm text-(--muted-text)">
          {m.rules_loading()}
        </div>
      }
    >
      <RuleWorkspace />
    </Suspense>
  );
}

export function App() {
  return (
    <BrowserRouter>
      <Startup />
      <Routes>
        <Route element={<AppShell />}>
          <Route index element={<RealmHome />} />
          <Route path="apps" element={<AppsHome />} />
          <Route path="sources" element={<SourcesHome />} />
          <Route path="sources/rules" element={<RuleWorkspaceRoute />} />
          <Route path="sources/rules/migration" element={<RulesMigration />} />
          <Route path="library" element={<LibraryHome />} />
          <Route path="library/item/:resourceId" element={<LibraryItemRoute />} />
          <Route path="settings" element={<SettingsHome />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
      <Toaster />
    </BrowserRouter>
  );
}

function LibraryItemRoute() {
  const { resourceId = '' } = useParams();
  return <LibraryItem resourceId={resourceId} />;
}

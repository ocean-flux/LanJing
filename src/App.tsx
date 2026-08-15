import { lazy, Suspense, useEffect } from 'react';
import { BrowserRouter, Route, Routes, useNavigate, useParams } from 'react-router-dom';
import { toast } from 'sonner';
import { AppShell } from '@/app/AppShell';
import { Spinner } from '@/components/ui/spinner';
import { Toaster } from '@/components/ui/sonner';
import { TooltipProvider } from '@/components/ui/tooltip';
import {
  AppsHome,
  LibraryHome,
  LibraryItem,
  NotFound,
  RealmHome,
  SettingsHome,
  SourcesHome,
} from '@/features';
import { useMessages } from '@/shared/i18n/messages';
import { startDeepLinkRuntime } from '@/shared/tauri/deep-link';

// 规则工作区带 xyflow / CodeMirror / elkjs，只在进入该路由时加载。
const RuleWorkspace = lazy(() =>
  import('@/features/rules/RuleWorkspace').then((module) => ({ default: module.RuleWorkspace })),
);

function Startup() {
  const m = useMessages();
  const navigate = useNavigate();

  useEffect(() => {
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
          toast.error(m.sources_deeplink_error_title(), {
            description: intent.subject ?? intent.reason,
          });
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
  const { documentId } = useParams();
  const m = useMessages();
  return (
    <Suspense
      fallback={
        <div className="flex h-full items-center justify-center gap-2 text-ink-muted">
          <Spinner className="animate-spin" />
          {m.rules_loading()}
        </div>
      }
    >
      <RuleWorkspace documentId={documentId} />
    </Suspense>
  );
}

function LibraryItemRoute() {
  const { resourceId = '' } = useParams();
  return <LibraryItem resourceId={resourceId} />;
}

export function App() {
  return (
    <BrowserRouter>
      <TooltipProvider>
        <Startup />
        <Routes>
          <Route element={<AppShell />}>
            <Route index element={<RealmHome />} />
            <Route path="apps" element={<AppsHome />} />
            <Route path="sources" element={<SourcesHome />} />
            <Route path="sources/rules" element={<RuleWorkspaceRoute />} />
            <Route path="sources/rules/:documentId" element={<RuleWorkspaceRoute />} />
            <Route path="library" element={<LibraryHome />} />
            <Route path="library/item/:resourceId" element={<LibraryItemRoute />} />
            <Route path="settings" element={<SettingsHome />} />
            <Route path="*" element={<NotFound />} />
          </Route>
        </Routes>
        <Toaster />
      </TooltipProvider>
    </BrowserRouter>
  );
}

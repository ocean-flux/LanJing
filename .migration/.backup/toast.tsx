import { useEffect, useState } from 'react';
import { cn } from '@/shared/utils';

interface ToastMessage {
  id: number;
  title: string;
  description?: string;
}
let nextId = 1;
const listeners = new Set<(toast: ToastMessage) => void>();

export function toast(title: string, description?: string) {
  const message = { id: nextId++, title, description };
  listeners.forEach((listener) => listener(message));
}

export function Toaster() {
  const [items, setItems] = useState<ToastMessage[]>([]);
  useEffect(() => {
    const listener = (item: ToastMessage) => {
      setItems((current) => [...current, item]);
      window.setTimeout(
        () => setItems((current) => current.filter((toastItem) => toastItem.id !== item.id)),
        4000,
      );
    };
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, []);
  return (
    <div
      aria-live="polite"
      className="fixed top-4 right-4 z-50 flex w-[min(360px,calc(100vw-2rem))] flex-col gap-2"
    >
      {items.map((item) => (
        <div
          key={item.id}
          className={cn('rounded-lg border border-(--border) bg-(--surface) p-4 shadow-lg')}
        >
          <p className="text-sm font-medium">{item.title}</p>
          {item.description && (
            <p className="mt-1 text-xs text-(--muted-text)">{item.description}</p>
          )}
        </div>
      ))}
    </div>
  );
}

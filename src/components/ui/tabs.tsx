import { Tabs as TabsPrimitive } from '@base-ui/react/tabs';
import { cn } from '@/shared/utils';

export function Tabs({ className, ...props }: TabsPrimitive.Root.Props) {
  return <TabsPrimitive.Root className={cn('flex flex-col gap-4', className)} {...props} />;
}

export function TabsList({ className, ...props }: TabsPrimitive.List.Props) {
  return (
    <TabsPrimitive.List
      className={cn(
        'inline-flex w-fit items-center gap-1 rounded-md bg-(--surface-2) p-1',
        className,
      )}
      {...props}
    />
  );
}

export function TabsTrigger({ className, ...props }: TabsPrimitive.Tab.Props) {
  return (
    <TabsPrimitive.Tab
      className={cn(
        'rounded px-3 py-1.5 text-sm text-(--muted-text) transition-colors hover:text-(--text) focus-visible:ring-2 focus-visible:ring-(--ring) focus-visible:outline-none data-active:bg-(--surface) data-active:text-(--text) data-active:shadow-sm',
        className,
      )}
      {...props}
    />
  );
}

export function TabsContent({ className, ...props }: TabsPrimitive.Panel.Props) {
  return (
    <TabsPrimitive.Panel
      className={cn(
        'focus-visible:ring-2 focus-visible:ring-(--ring) focus-visible:outline-none',
        className,
      )}
      {...props}
    />
  );
}

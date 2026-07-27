<script lang="ts">
  import { Toaster, type ToasterProps } from 'svelte-sonner';
  import { cn } from '$lib/utils.js';
  type ToastClasses = NonNullable<NonNullable<ToasterProps['toastOptions']>['classes']>;

  const safeOffset: NonNullable<ToasterProps['offset']> = {
    top: 'calc(var(--safe-area-top) + 12px)',
    right: 'calc(var(--safe-area-right) + 12px)',
    bottom: 'calc(var(--safe-area-bottom) + 12px)',
    left: 'calc(var(--safe-area-left) + 12px)',
  };

  const toastRootClass =
    'elevated-overlay relative flex w-(--width) items-start gap-2 rounded-[var(--radius-overlay)] border border-hairline p-3 text-sm text-ink transition-[transform,opacity,height,box-shadow]! duration-(--motion-base)! focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-lantern-strong';

  const toastClasses: ToastClasses = {
    title: 'font-semibold leading-5 text-ink',
    description: 'text-xs leading-5 text-ink-muted',
    content: 'flex min-w-0 flex-1 flex-col gap-0.5',
    icon: 'flex size-4 shrink-0 items-center justify-center',
    loader: 'size-4',
    closeButton:
      'glass-control absolute -start-2 -top-2 flex size-(--density-control-sm) items-center justify-center rounded-full border border-hairline text-ink-muted outline-none hover:bg-surface-2 hover:text-ink focus-visible:shadow-[var(--focus-ring)] [@media(pointer:coarse)]:size-(--density-touch-target)',
    actionButton:
      'ms-2 inline-flex h-(--density-control-sm) shrink-0 items-center justify-center rounded-md bg-lantern-strong px-2 text-xs font-semibold text-on-lantern outline-none hover:bg-lantern-hover focus-visible:shadow-[var(--focus-ring)] [@media(pointer:coarse)]:min-h-(--density-touch-target)',
    cancelButton:
      'glass-control ms-2 inline-flex h-(--density-control-sm) shrink-0 items-center justify-center rounded-md border border-hairline px-2 text-xs font-semibold text-ink outline-none hover:bg-surface-2 focus-visible:shadow-[var(--focus-ring)] [@media(pointer:coarse)]:min-h-(--density-touch-target)',
    success: 'border-s-2! border-s-positive!',
    info: 'border-s-2! border-s-lantern-strong!',
    warning: 'border-s-2! border-s-warning!',
    error: 'border-s-2! border-s-destructive!',
    loading: 'border-s-2! border-s-lantern-strong!',
  };

  let {
    class: className,
    theme = 'system',
    richColors = true,
    closeButton = true,
    offset = safeOffset,
    mobileOffset = safeOffset,
    toastOptions,
    ...restProps
  }: ToasterProps = $props();

  const resolvedToastOptions = $derived.by(() => ({
    ...toastOptions,
    unstyled: true,
    class: cn(toastRootClass, toastOptions?.class),
    classes: {
      ...toastClasses,
      ...toastOptions?.classes,
      title: cn(toastClasses.title, toastOptions?.classes?.title),
      description: cn(toastClasses.description, toastOptions?.classes?.description),
      content: cn(toastClasses.content, toastOptions?.classes?.content),
      icon: cn(toastClasses.icon, toastOptions?.classes?.icon),
      loader: cn(toastClasses.loader, toastOptions?.classes?.loader),
      closeButton: cn(toastClasses.closeButton, toastOptions?.classes?.closeButton),
      actionButton: cn(toastClasses.actionButton, toastOptions?.classes?.actionButton),
      cancelButton: cn(toastClasses.cancelButton, toastOptions?.classes?.cancelButton),
      success: cn(toastClasses.success, toastOptions?.classes?.success),
      info: cn(toastClasses.info, toastOptions?.classes?.info),
      warning: cn(toastClasses.warning, toastOptions?.classes?.warning),
      error: cn(toastClasses.error, toastOptions?.classes?.error),
      loading: cn(toastClasses.loading, toastOptions?.classes?.loading),
    },
  }));
</script>

<Toaster
  {...restProps}
  class={cn('z-(--layer-toast)!', className)}
  {theme}
  {richColors}
  {closeButton}
  {offset}
  {mobileOffset}
  toastOptions={resolvedToastOptions}
/>

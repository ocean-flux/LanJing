import { cn } from '@/shared/utils';
import { SpinnerIcon } from '@/components/ui/icon-glyphs';

function Spinner({ className, ...props }: React.ComponentProps<'span'>) {
  return (
    <SpinnerIcon
      data-slot="spinner"
      role="status"
      aria-label="Loading"
      className={cn('size-4 animate-spin', className)}
      {...props}
    />
  );
}

export { Spinner };

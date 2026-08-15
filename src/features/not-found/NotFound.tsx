import { Link } from 'react-router-dom';
import { Icon } from '@/components/Icon';
import { Button } from '@/components/ui/button';
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from '@/components/ui/empty';
import { useMessages } from '@/shared/i18n/messages';

export function NotFound() {
  const m = useMessages();
  return (
    <Empty className="h-full">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <Icon name="warning-circle" className="text-base" />
        </EmptyMedia>
        <EmptyTitle>{m.shell_not_found_title()}</EmptyTitle>
        <EmptyDescription>{m.shell_not_found_description()}</EmptyDescription>
      </EmptyHeader>
      <Button variant="outline" size="sm" render={<Link to="/" />}>
        {m.shell_not_found_return()}
      </Button>
    </Empty>
  );
}

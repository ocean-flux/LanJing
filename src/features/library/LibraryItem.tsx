import { useNavigate } from 'react-router-dom';
import { LibraryHome } from './LibraryHome';

export function LibraryItem({ resourceId }: { resourceId: string }) {
  const navigate = useNavigate();
  return (
    <LibraryHome initialResourceId={resourceId} onInspectorClose={() => navigate('/library')} />
  );
}

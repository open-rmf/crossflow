import { type RefObject, useEffect } from 'react';
import {
  type DraftWorkspaceContent,
  writeDraftWorkspace,
} from './draft-workspace';

export function useDraftPagehideFlush(
  latestWorkspace: RefObject<DraftWorkspaceContent | null>,
  enabled: boolean,
  setStorageFailed: (failed: boolean) => void,
) {
  useEffect(() => {
    const flushWorkspace = () => {
      if (!enabled || !latestWorkspace.current) {
        return;
      }
      try {
        writeDraftWorkspace(latestWorkspace.current);
        setStorageFailed(false);
      } catch {
        setStorageFailed(true);
      }
    };

    window.addEventListener('pagehide', flushWorkspace);
    return () => window.removeEventListener('pagehide', flushWorkspace);
  }, [enabled, latestWorkspace, setStorageFailed]);
}

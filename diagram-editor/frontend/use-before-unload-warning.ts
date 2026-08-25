import { useEffect } from 'react';

export function useBeforeUnloadWarning(enabled: boolean) {
  useEffect(() => {
    if (!enabled) {
      return;
    }

    const warnBeforeUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = '';
    };

    window.addEventListener('beforeunload', warnBeforeUnload);
    return () => window.removeEventListener('beforeunload', warnBeforeUnload);
  }, [enabled]);
}

import { render } from '@testing-library/react';
import { useBeforeUnloadWarning } from './use-before-unload-warning';

function Harness({ enabled }: { enabled: boolean }) {
  useBeforeUnloadWarning(enabled);
  return null;
}

test('warns before unload only while unsaved changes exist', () => {
  const { rerender } = render(<Harness enabled={false} />);

  const cleanEvent = new Event('beforeunload', { cancelable: true });
  window.dispatchEvent(cleanEvent);
  expect(cleanEvent.defaultPrevented).toBe(false);

  rerender(<Harness enabled={true} />);
  const dirtyEvent = new Event('beforeunload', { cancelable: true });
  window.dispatchEvent(dirtyEvent);
  expect(dirtyEvent.defaultPrevented).toBe(true);

  rerender(<Harness enabled={false} />);
  const cleanedEvent = new Event('beforeunload', { cancelable: true });
  window.dispatchEvent(cleanedEvent);
  expect(cleanedEvent.defaultPrevented).toBe(false);
});

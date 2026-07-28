import { render } from '@testing-library/react';
import { useBeforeUnloadWarning } from './use-before-unload-warning';

function Harness() {
  useBeforeUnloadWarning();
  return null;
}

test('always warns before refresh, tab close, or navigation', () => {
  render(<Harness />);
  const event = new Event('beforeunload', { cancelable: true });

  window.dispatchEvent(event);
  expect(event.defaultPrevented).toBe(true);
});

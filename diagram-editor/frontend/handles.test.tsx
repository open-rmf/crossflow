import { glowEdge } from './handles';

describe('edge activity', () => {
  beforeEach(() => {
    jest.useFakeTimers();
    document.body.innerHTML =
      '<svg><g class="react-flow__edge" data-id="edge-1"><path class="react-flow__edge-path" /></g></svg>';
  });

  afterEach(() => {
    jest.useRealTimers();
    document.body.replaceChildren();
  });

  test('keeps an edge glowing while messages remain frequent', () => {
    const edge = document.querySelector('[data-id="edge-1"]');

    glowEdge('edge-1');
    expect(edge).toHaveClass('message-active');

    jest.advanceTimersByTime(150);
    glowEdge('edge-1');
    jest.advanceTimersByTime(150);
    expect(edge).toHaveClass('message-active');

    jest.advanceTimersByTime(50);
    expect(edge).not.toHaveClass('message-active');
  });
});

import '@testing-library/jest-dom';

// jsdom has no layout engine and so no ResizeObserver.
if (!globalThis.ResizeObserver) {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}

const nodeCrypto = jest.requireActual('node:crypto') as { webcrypto: Crypto };

if (!globalThis.crypto?.subtle) {
  Object.defineProperty(globalThis, 'crypto', {
    configurable: true,
    value: nodeCrypto.webcrypto,
  });
}

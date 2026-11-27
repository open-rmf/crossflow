import '@testing-library/jest-dom';

// RJSF clones JSON values with the browser's structuredClone API.
if (!globalThis.structuredClone) {
  const { serialize, deserialize } = jest.requireActual('node:v8');
  globalThis.structuredClone = (value) => deserialize(serialize(value));
}

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

import '@testing-library/jest-dom';

const nodeCrypto = jest.requireActual('node:crypto') as { webcrypto: Crypto };

if (!globalThis.crypto?.subtle) {
  Object.defineProperty(globalThis, 'crypto', {
    configurable: true,
    value: nodeCrypto.webcrypto,
  });
}

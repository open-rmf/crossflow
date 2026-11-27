import { createDefaultPreset } from 'ts-jest';

const tsJestTransformCfg = createDefaultPreset().transform;

/** @type {import("jest").Config} **/
export default {
  testEnvironment: 'jsdom',
  transform: {
    ...tsJestTransformCfg,
    '^.+\\.js$': ['ts-jest', { tsconfig: { allowJs: true }, diagnostics: false }],
  },
  // Use the same MUI exports as the browser and compile RJSF's ESM dependencies.
  moduleNameMapper: { '^@rjsf/mui$': '<rootDir>/node_modules/@rjsf/mui/lib/index.js' },
  transformIgnorePatterns: ['node_modules/(?!\\.pnpm/|@x0k/json-schema-merge/|@rjsf/mui/)'],
  setupFilesAfterEnv: ['./jest-setup.ts'],
};

import { createEmptyDiagramProperties } from './diagram-properties-provider';

test('creates canonical empty diagram properties with independent collections', () => {
  const first = createEmptyDiagramProperties();
  const second = createEmptyDiagramProperties();

  expect(first).toEqual({
    description: '',
    input_examples: [],
    script_environments: {},
  });
  expect(first.input_examples).not.toBe(second.input_examples);
  expect(first.script_environments).not.toBe(second.script_environments);
});

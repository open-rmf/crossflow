/**
 * @jest-environment node
 */

import fs from 'node:fs';
import path from 'node:path';
import { NodeManager } from '../node-manager';
import { validateEdgeSimple } from './connection';
import { loadDiagramJson } from './load-diagram';

const testDataDir = path.join(__dirname, 'test-data');
const calculatorDiagramsDir = path.join(
  __dirname,
  '../../../examples/diagram/calculator/diagrams',
);

function getJsonDiagrams(dir: string): string[] {
  if (!fs.existsSync(dir)) {
    return [];
  }
  return fs
    .readdirSync(dir)
    .filter((file) => file.endsWith('.json'))
    .map((file) => path.join(dir, file));
}

describe('Diagram Connection Validation Integration Tests', () => {
  const diagramPaths = [
    ...getJsonDiagrams(testDataDir),
    ...getJsonDiagrams(calculatorDiagramsDir),
  ];

  if (diagramPaths.length === 0) {
    test('no diagrams found', () => {
      fail('No diagram JSON files found in test directories.');
    });
  }

  for (const diagramPath of diagramPaths) {
    const fileName = path.basename(diagramPath);
    test(`validate connection constraints for ${fileName}`, async () => {
      const jsonStr = fs.readFileSync(diagramPath, 'utf-8');
      const [_diagram, { graph }] = await loadDiagramJson(jsonStr);
      const nodeManager = new NodeManager(graph.nodes);

      for (const edge of graph.edges) {
        const result = validateEdgeSimple(edge, nodeManager, graph.edges);
        expect(result.valid).toBe(true);
      }
    });
  }
});

/**
 * @jest-environment node
 */

import { type ChildProcess, spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { firstValueFrom } from 'rxjs';
import type { Diagram } from '../types/api';
import { ApiClient } from './rest-client';

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
    .filter(
      (file) =>
        file.endsWith('.json') &&
        file !== 'test-diagram.json' &&
        file !== 'test-diagram-scope.json',
    )
    .map((file) => path.join(dir, file));
}

describe('REST API Executor Integration Tests', () => {
  let backendProcess: ChildProcess;
  const apiClient = new ApiClient();
  const originalFetch = global.fetch;

  beforeAll(async () => {
    // Setup fetch interceptor for relative REST API requests
    global.fetch = (input: RequestInfo | URL, init?: RequestInit) => {
      if (typeof input === 'string' && input.startsWith('/api/')) {
        input = `http://localhost:3001${input}`;
      }
      return originalFetch(input, init);
    };

    // Start the calculator executor server in the background on port 3001
    const calculatorCwd = path.join(calculatorDiagramsDir, '..');

    backendProcess = spawn(
      'cargo',
      ['run', '--features', 'python', '--', 'serve', '--port', '3001'],
      {
        cwd: calculatorCwd,
        env: {
          ...process.env,
          BUILD_FRONTEND: '1', // Prevent build-script blocking
        },
        stdio: 'ignore', // Ignore server output
      },
    );

    // Wait until the server is online and ready
    await waitForServer('http://localhost:3001/api/registry');
  }, 35000); // Allow up to 35s for compile & launch

  afterAll(() => {
    // Restore global fetch
    global.fetch = originalFetch;

    // Cleanup spawned backend server process
    if (backendProcess) {
      backendProcess.kill('SIGTERM');
    }
  });

  const diagramPaths = getJsonDiagrams(calculatorDiagramsDir);

  for (const diagramPath of diagramPaths) {
    const fileName = path.basename(diagramPath);

    const diagram: Diagram = JSON.parse(fs.readFileSync(diagramPath, 'utf-8'));
    const inputExamples = diagram.input_examples;
    if (inputExamples && inputExamples.length > 0) {
      describe(`Diagram: ${fileName}`, () => {
        for (const example of inputExamples) {
          test(`postRunWorkflow with example: "${example.description}"`, async () => {
            let requestPayload: unknown = example.value;
            if (typeof example.value === 'string') {
              try {
                // If it parses as JSON (like arrays or objects), we use the parsed value
                requestPayload = JSON.parse(example.value);
              } catch (_e) {
                // Otherwise check if it's a number representation
                const num = Number(example.value);
                if (!Number.isNaN(num)) {
                  requestPayload = num;
                }
              }
            }

            const response = await firstValueFrom(
              apiClient.postRunWorkflow(diagram, requestPayload),
            );

            expect(response).toBeDefined();
            expect(response).not.toBeNull();
          }, 15000);
        }
      });
    }
  }
});

async function waitForServer(url: string, timeoutMs = 30000): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const res = await fetch(url);
      if (res.ok) {
        return;
      }
    } catch (_e) {
      // Ignored
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`Server at ${url} did not become ready in time`);
}

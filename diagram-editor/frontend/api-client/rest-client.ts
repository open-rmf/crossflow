import { from, type Observable } from 'rxjs';
import type {
  Diagram,
  DiagramElementMetadata,
  PostRunRequest,
} from '../types/api';
import { getSchema } from '../utils/ajv';
import type { BaseApiClient } from './base-api-client';
import { DebugSession } from './debug-session';

const validateRegistry = getSchema<DiagramElementMetadata>(
  'DiagramElementMetadata',
);

async function getErrorMessage(response: Response) {
  const text = await response.text();
  return text || `${response.status} ${response.statusText}`;
}

export class ApiClient implements BaseApiClient {
  getRegistry(): Observable<DiagramElementMetadata> {
    return from(
      (async () => {
        const response = await fetch('/api/registry');
        if (!response.ok) {
          throw new Error(await getErrorMessage(response));
        }
        const data = await response.json();
        if (!validateRegistry(data)) {
          throw validateRegistry.errors;
        }
        return data;
      })(),
    );
  }

  postRunWorkflow(diagram: Diagram, request: unknown): Observable<unknown> {
    return from(
      (async () => {
        const body: PostRunRequest = {
          diagram,
          request,
        };
        const response = await fetch('/api/executor/run', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify(body),
        });
        if (!response.ok) {
          throw new Error(await getErrorMessage(response));
        }
        return response.json();
      })(),
    );
  }

  async wsDebugWorkflow(
    diagram: Diagram,
    request: unknown,
  ): Promise<DebugSession> {
    const url = new URL('/api/executor/debug', window.location.href);
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(url);
    return new Promise((resolve, reject) => {
      ws.onopen = () => {
        const session = new DebugSession(ws);
        const body: PostRunRequest = {
          diagram,
          request,
        };
        ws.send(JSON.stringify(body));
        resolve(session);
      };
      ws.onerror = () => reject(new Error('debug websocket error'));
    });
  }
}

import { from, type Observable } from 'rxjs';
import type {
  CompatibilityRequest,
  CompatibilityResponse,
  Diagram,
  DiagramElementMetadata,
  PostRunRequest,
} from '../types/api';
import { getSchema } from '../utils/ajv';
import type { BaseApiClient } from './base-api-client';
import { InteractionSession } from './interaction-session';

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

  checkCompatibility(
    request: CompatibilityRequest,
  ): Observable<CompatibilityResponse> {
    return from(
      (async () => {
        const response = await fetch('/api/executor/compatibility', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify(request),
        });
        if (!response.ok) {
          throw new Error(await getErrorMessage(response));
        }
        return response.json();
      })(),
    );
  }

  async wsInteractWithWorkflow(
    diagram: Diagram,
    request: unknown,
  ): Promise<InteractionSession> {
    const url = new URL('/api/executor/interaction', window.location.href);
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    const ws = new WebSocket(url);
    return new Promise((resolve, reject) => {
      ws.onopen = () => {
        const session = new InteractionSession(ws);
        const body: PostRunRequest = {
          diagram,
          request,
        };
        ws.send(JSON.stringify(body));
        resolve(session);
      };
      ws.onerror = () => reject(new Error('interaction websocket error'));
    });
  }
}

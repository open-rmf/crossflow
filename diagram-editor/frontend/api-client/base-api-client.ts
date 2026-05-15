import type { Observable } from 'rxjs';
import type { Diagram, DiagramElementMetadata } from '../types/api';
import type { DebugSession } from './debug-session';

export interface BaseApiClient {
  getRegistry(): Observable<DiagramElementMetadata>;
  postRunWorkflow(diagram: Diagram, request: unknown): Observable<unknown>;
  wsDebugWorkflow?(
    diagram: Diagram,
    request: unknown,
  ): Promise<DebugSession>;
}

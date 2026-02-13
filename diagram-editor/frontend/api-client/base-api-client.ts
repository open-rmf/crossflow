import type { Observable } from 'rxjs';
import type { Diagram, DiagramElementMetadata } from '../types/api';

export interface BaseApiClient {
  getRegistry(): Observable<DiagramElementMetdata>;
  postRunWorkflow(diagram: Diagram, request: unknown): Observable<unknown>;
  // WIP
  // wsDebugWorkflow(diagram: Diagram, request: unknown): Promise<DebugSession>;
}

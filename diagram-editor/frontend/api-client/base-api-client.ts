import type { Observable } from 'rxjs';
import type { Diagram, DiagramElementMetadata } from '../types/api';
import type { InteractionSession } from './interaction-session';

export interface BaseApiClient {
  getRegistry(): Observable<DiagramElementMetadata>;
  postRunWorkflow(diagram: Diagram, request: unknown): Observable<unknown>;
  wsInteractWithWorkflow?(
    diagram: Diagram,
    request: unknown,
  ): Promise<InteractionSession>;
}

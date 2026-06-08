import type { Observable } from 'rxjs';
import type {
  CompatibilityRequest,
  CompatibilityResponse,
  Diagram,
  DiagramElementMetadata,
} from '../types/api';

export interface BaseApiClient {
  getRegistry(): Observable<DiagramElementMetadata>;
  postRunWorkflow(diagram: Diagram, request: unknown): Observable<unknown>;
  checkCompatibility(
    request: CompatibilityRequest,
  ): Observable<CompatibilityResponse>;
  // WIP
  // wsDebugWorkflow(diagram: Diagram, request: unknown): Promise<DebugSession>;
}

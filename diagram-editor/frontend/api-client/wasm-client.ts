import { from, type Observable, of } from 'rxjs';
import type { Diagram, DiagramElementMetadata } from '../types/api';
import { getSchema } from '../utils/ajv';
import type { BaseApiClient } from './base-api-client';
import * as wasmApi from './wasm-stub/stub.js';

const validateRegistry = getSchema<DiagramElementMetadata>(
  'DiagramElementMetadata',
);

export class ApiClient implements BaseApiClient {
  constructor() {
    wasmApi.init_wasm();
  }

  getRegistry(): Observable<DiagramElementMetadata> {
    const registry = wasmApi.get_registry();
    if (!validateRegistry(registry)) {
      throw validateRegistry.errors;
    }
    return of(registry);
  }

  postRunWorkflow(diagram: Diagram, request: unknown): Observable<unknown> {
    return from(
      wasmApi.post_run(new wasmApi.PostRunRequestWasm({ diagram, request })),
    );
  }
}

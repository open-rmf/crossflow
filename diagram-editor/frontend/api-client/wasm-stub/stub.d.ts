// biome-ignore-all lint: generated
/* tslint:disable */
/* eslint-disable */
export function init_wasm(): void;
export function post_run(request: PostRunRequestWasm): Promise<any>;
export function check_compatibility(
  request: CompatibilityRequestWasm,
): Promise<any>;
export function get_registry(): any;
type PostRunRequest = import('../../types/api').PostRunRequest;
type CompatibilityRequest = import('../../types/api').CompatibilityRequest;

export class PostRunRequestWasm {
  free(): void;
  constructor(js: PostRunRequest);
}

export class CompatibilityRequestWasm {
  free(): void;
  constructor(js: CompatibilityRequest);
}

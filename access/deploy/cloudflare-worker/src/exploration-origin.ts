/** Public schema constants and host/test packet types; Rust owns origin rules. */
export const REQUEST_V2 = 'tos_exploration_request_v2';
export const RESULT_V2 = 'tos_exploration_result_v2';
export type Origin = {kind: 'node' | 'relation'; id: string; content_revision: string};
type Endpoint = {node_id: string; entity_id: string; content_revision: string};
export type ResolvedOrigin = Origin & {endpoints?: {from: Endpoint; to: Endpoint}};

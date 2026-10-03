import {withSecurity} from './common.ts';
import {NATIVE_LENS_RESPONSE_BYTES, nativePacketJson, type NativePacket, type NativeRef} from './native-lens.ts';

/** Existing search/native packet responses retain their bounded first serialization. */
export function nativePacketResponse(packet: NativePacket | NativeRef, status = 200, method = 'GET'): Response {
  const body = nativePacketJson(packet, {maxBytes: NATIVE_LENS_RESPONSE_BYTES});
  return withSecurity(new Response(method === 'HEAD' ? null : body, {status,
    headers: {'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store'}}));
}

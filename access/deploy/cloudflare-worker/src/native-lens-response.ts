import {withSecurity} from './common.ts';
import {NATIVE_LENS_RESPONSE_BYTES, nativePacketJson, type NativeLensResult, type NativePacket, type NativeRef} from './native-lens.ts';

/** The public ABI is plain LensResult JSON, never the internal packet/preview. */
export function nativeLensResponse(result: NativeLensResult, status = 200, method = 'GET'): Response {
  return nativePacketResponse(result.packet, status, method);
}

/** Full inspection packets share the same first-serialization byte boundary. */
export function nativePacketResponse(packet: NativePacket | NativeRef, status = 200, method = 'GET'): Response {
  const body = nativePacketJson(packet, {maxBytes: NATIVE_LENS_RESPONSE_BYTES});
  return withSecurity(new Response(method === 'HEAD' ? null : body, {status,
    headers: {'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store'}}));
}

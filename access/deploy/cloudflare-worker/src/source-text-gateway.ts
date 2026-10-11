import { jsonResponse, withSecurity } from "./common.ts";

const SEARCH_PATH = "/api/source-text/search";
const MAX_REQUEST_BYTES = 64 * 1024;
const MAX_RESPONSE_BYTES = 1024 * 1024;
const MAX_ACCESS_JWT_BYTES = 16 * 1024;
const MAX_ACCESS_JWKS_BYTES = 16 * 1024;
const MAX_ACCESS_JWKS_KEYS = 8;
// One end-to-end cap covers caller auth/JWKS, request-body reading, and the
// native exchange; preflight time therefore reduces native time available.
const MAX_OPERATION_MS = 65_000;
const CAPABILITY_SCHEMA = "tos_source_text_gateway_capability_v1";

type GatewayBindings = Env & {
  TOS_SOURCE_TEXT_GATEWAY_URL?: string;
  TOS_SOURCE_TEXT_GATEWAY_ACCESS_CLIENT_ID?: string;
  TOS_SOURCE_TEXT_GATEWAY_ACCESS_CLIENT_SECRET?: string;
  TOS_SOURCE_TEXT_GATEWAY_CALLER_ACCESS_ISSUER?: string;
  TOS_SOURCE_TEXT_GATEWAY_CALLER_ACCESS_AUDIENCE?: string;
};

type GatewaySelection =
  | { status: "unsupported"; reason: "gateway-not-configured" }
  | {
      status: "misconfigured";
      reason:
        | "gateway-configuration-incomplete"
        | "gateway-configuration-invalid"
        | "gateway-url-invalid"
        | "gateway-origin-loops-to-worker"
        | "gateway-caller-access-configuration-incomplete"
        | "gateway-caller-access-configuration-invalid";
    }
  | {
      status: "selected";
      origin: string;
      clientId: string;
      clientSecret: string;
      callerAccessIssuer: string;
      callerAccessAudience: string;
    };

type GatewayFetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

class GatewayRequestError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

class GatewayAuthenticationError extends Error {
  readonly status: number;

  constructor(status: number, message = "source-text caller is not authorized") {
    super(message);
    this.status = status;
  }
}

type AbortKind = "client" | "deadline";

class GatewayAbortError extends Error {
  readonly kind: AbortKind;

  constructor(kind: AbortKind) {
    super(kind === "deadline" ? "source-text gateway deadline exceeded" : "source-text request was cancelled");
    this.kind = kind;
  }
}

type GatewayOperation = {
  signal: AbortSignal;
  dispose: () => void;
};

function gatewayOperation(requestSignal: AbortSignal, timeoutMs: number): GatewayOperation {
  const controller = new AbortController();
  const abort = (cause: AbortKind) => {
    if (controller.signal.aborted) return;
    controller.abort(new GatewayAbortError(cause));
  };
  const onClientAbort = () => abort("client");
  if (requestSignal.aborted) onClientAbort();
  else requestSignal.addEventListener("abort", onClientAbort, { once: true });
  const timer = setTimeout(() => abort("deadline"), timeoutMs);
  return {
    signal: controller.signal,
    dispose: () => {
      clearTimeout(timer);
      requestSignal.removeEventListener("abort", onClientAbort);
    },
  };
}

function operationAbortError(signal: AbortSignal): GatewayAbortError {
  if (signal.reason instanceof GatewayAbortError) return signal.reason;
  return new GatewayAbortError("client");
}

function operationAbortResponse(signal: AbortSignal): Response {
  const error = operationAbortError(signal);
  return error.kind === "deadline"
    ? gatewayError("source-text gateway deadline exceeded", 504)
    : gatewayError("source-text request was cancelled", 499);
}

function cancelReader(reader: ReadableStreamDefaultReader<Uint8Array>, reason?: unknown): void {
  try {
    void reader.cancel(reason).catch(() => undefined);
  } catch {
    // Cancellation is best-effort, but it must never hold the gateway open.
  }
}

function cancelStream(stream: ReadableStream<Uint8Array> | null, reason?: unknown): void {
  if (!stream) return;
  try {
    void stream.cancel(reason).catch(() => undefined);
  } catch {
    // The stream may already be locked or closed; do not delay error handling.
  }
}

function raceWithAbort<T>(promise: Promise<T>, signal: AbortSignal): Promise<T> {
  if (signal.aborted) return Promise.reject(operationAbortError(signal));
  let onAbort: (() => void) | undefined;
  const aborted = new Promise<never>((_, reject) => {
    onAbort = () => reject(operationAbortError(signal));
    signal.addEventListener("abort", onAbort, { once: true });
  });
  return Promise.race([promise, aborted]).finally(() => {
    if (onAbort) signal.removeEventListener("abort", onAbort);
  });
}

function selection(env: Env, requestOrigin?: string): GatewaySelection {
  const bindings = env as GatewayBindings;
  const values = [
    bindings.TOS_SOURCE_TEXT_GATEWAY_URL,
    bindings.TOS_SOURCE_TEXT_GATEWAY_ACCESS_CLIENT_ID,
    bindings.TOS_SOURCE_TEXT_GATEWAY_ACCESS_CLIENT_SECRET,
    bindings.TOS_SOURCE_TEXT_GATEWAY_CALLER_ACCESS_ISSUER,
    bindings.TOS_SOURCE_TEXT_GATEWAY_CALLER_ACCESS_AUDIENCE,
  ];
  if (values.every((value) => value === undefined)) {
    return { status: "unsupported", reason: "gateway-not-configured" };
  }
  if (values.slice(0, 3).some((value) => value === undefined || value.length === 0)) {
    return { status: "misconfigured", reason: "gateway-configuration-incomplete" };
  }
  if (values.slice(3).some((value) => value === undefined || value.length === 0)) {
    return { status: "misconfigured", reason: "gateway-caller-access-configuration-incomplete" };
  }

  const [rawUrl, clientId, clientSecret, rawIssuer, rawAudience] = values as [string, string, string, string, string];
  if (
    !/^https:\/\/[^/?#]+\/?$/i.test(rawUrl) ||
    rawUrl.trim() !== rawUrl ||
    rawUrl.includes("?") ||
    rawUrl.includes("#") ||
    rawUrl.length > 2048
  ) {
    return { status: "misconfigured", reason: "gateway-url-invalid" };
  }
  if (
    clientId.trim() !== clientId ||
    clientSecret.trim() !== clientSecret ||
    /[\u0000-\u0020\u007f]/.test(clientId) ||
    /[\u0000-\u0020\u007f]/.test(clientSecret) ||
    clientId.length > 4096 ||
    clientSecret.length > 4096
  ) {
    return { status: "misconfigured", reason: "gateway-configuration-invalid" };
  }

  const callerIssuer = parseCallerAccessIssuer(rawIssuer);
  if (!callerIssuer || !validCallerAccessAudience(rawAudience)) {
    return { status: "misconfigured", reason: "gateway-caller-access-configuration-invalid" };
  }

  let target: URL;
  try {
    target = new URL(rawUrl);
  } catch {
    return { status: "misconfigured", reason: "gateway-url-invalid" };
  }
  if (
    target.protocol !== "https:" ||
    !target.hostname ||
    target.username ||
    target.password ||
    target.port ||
    target.pathname !== "/" ||
    target.search ||
    target.hash
  ) {
    return { status: "misconfigured", reason: "gateway-url-invalid" };
  }
  if (requestOrigin && (
    target.origin === requestOrigin || target.hostname === new URL(requestOrigin).hostname
  )) {
    return { status: "misconfigured", reason: "gateway-origin-loops-to-worker" };
  }
  return {
    status: "selected",
    origin: target.origin,
    clientId,
    clientSecret,
    callerAccessIssuer: callerIssuer,
    callerAccessAudience: rawAudience,
  };
}

function parseCallerAccessIssuer(raw: string): string | null {
  if (
    raw.trim() !== raw || raw.length > 256 ||
    !/^https:\/\/[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.cloudflareaccess\.com\/?$/i.test(raw)
  ) return null;
  try {
    const issuer = new URL(raw);
    if (
      issuer.protocol !== "https:" || issuer.username || issuer.password || issuer.port ||
      issuer.pathname !== "/" || issuer.search || issuer.hash
    ) return null;
    return issuer.origin;
  } catch {
    return null;
  }
}

function validCallerAccessAudience(raw: string): boolean {
  return raw.length > 0 && raw.length <= 256 &&
    raw.trim() === raw && !/[\u0000-\u0020\u007f]/.test(raw);
}

function isJsonObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item): item is string => typeof item === "string");
}

function decodeBase64Url(value: string, maximumBytes: number): Uint8Array {
  if (!value || value.length > maximumBytes * 2 || value.length % 4 === 1 || !/^[A-Za-z0-9_-]+$/.test(value)) {
    throw new GatewayAuthenticationError(401);
  }
  let decoded: string;
  try {
    const normalized = value.replace(/-/g, "+").replace(/_/g, "/");
    decoded = atob(normalized + "=".repeat((4 - normalized.length % 4) % 4));
  } catch {
    throw new GatewayAuthenticationError(401);
  }
  if (decoded.length > maximumBytes) throw new GatewayAuthenticationError(401);
  return Uint8Array.from(decoded, (character) => character.charCodeAt(0));
}

function decodeJsonSegment(value: string, maximumBytes: number): Record<string, unknown> {
  let decoded: string;
  try {
    decoded = new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(decodeBase64Url(value, maximumBytes));
    const parsed: unknown = JSON.parse(decoded);
    if (!isJsonObject(parsed)) throw new Error("JWT segment must be an object");
    return parsed;
  } catch {
    throw new GatewayAuthenticationError(401);
  }
}

function validIdentifier(value: unknown, maximum = 256): value is string {
  return typeof value === "string" && value.length > 0 && value.length <= maximum &&
    !/[\u0000-\u0020\u007f]/.test(value);
}

function validateCallerClaims(claims: Record<string, unknown>, issuer: string, audience: string): void {
  const audiences = typeof claims.aud === "string"
    ? [claims.aud]
    : Array.isArray(claims.aud) && claims.aud.length > 0 && claims.aud.length <= 16 &&
      claims.aud.every((candidate) => typeof candidate === "string")
      ? claims.aud as string[]
      : [];
  const now = Date.now() / 1000;
  const expiration = claims.exp;
  const issuedAt = claims.iat;
  if (
    claims.iss !== issuer || !audiences.includes(audience) || claims.type !== "app" ||
    typeof expiration !== "number" || !Number.isFinite(expiration) || expiration <= now ||
    typeof issuedAt !== "number" || !Number.isFinite(issuedAt) || issuedAt > now + 60 || issuedAt > expiration
  ) throw new GatewayAuthenticationError(401);
  if (claims.nbf !== undefined && (
    typeof claims.nbf !== "number" || !Number.isFinite(claims.nbf) || claims.nbf > now
  )) throw new GatewayAuthenticationError(401);
}

type AccessJwk = JsonWebKey & { kid: string; kty: "RSA"; alg?: string; use?: string; key_ops?: string[] };

function parseAccessJwk(value: unknown): AccessJwk | null {
  if (!isJsonObject(value)) return null;
  const { kid, kty, n, e, alg, use, key_ops } = value;
  if (
    !validIdentifier(kid) || kty !== "RSA" || !validIdentifier(n, 2048) || !validIdentifier(e, 32) ||
    (alg !== undefined && alg !== "RS256") ||
    (use !== undefined && use !== "sig")
  ) return null;
  if (key_ops !== undefined && (
    !isStringArray(key_ops) ||
    !key_ops.includes("verify")
  )) return null;
  return {
    kid,
    kty: "RSA",
    n,
    e,
    ...(alg === undefined ? {} : { alg }),
    ...(use === undefined ? {} : { use }),
    ...(key_ops === undefined ? {} : { key_ops }),
  };
}

async function loadCallerAccessJwks(
  issuer: string,
  fetcher: GatewayFetcher,
  signal: AbortSignal,
): Promise<AccessJwk[]> {
  const url = new URL("/cdn-cgi/access/certs", issuer);
  let response: Response;
  try {
    const pending = fetcher(url, {
      method: "GET",
      headers: { Accept: "application/json" },
      cache: "no-store",
      redirect: "manual",
      signal,
    });
    void pending.then((lateResponse) => {
      if (signal.aborted) cancelStream(lateResponse.body, signal.reason);
    }, () => undefined);
    response = await raceWithAbort(pending, signal);
  } catch {
    if (signal.aborted) throw operationAbortError(signal);
    throw new GatewayAuthenticationError(503, "caller authentication service is unavailable");
  }

  const contentType = (response.headers.get("content-type")?.split(";")[0] ?? "").trim().toLowerCase();
  if (response.status !== 200 || contentType !== "application/json") {
    cancelStream(response.body);
    throw new GatewayAuthenticationError(503, "caller authentication service returned an invalid response");
  }

  let jwksBytes: Uint8Array;
  try {
    jwksBytes = await readBoundedStream(
      response.body,
      response.headers.get("content-length"),
      MAX_ACCESS_JWKS_BYTES,
      "caller authentication key set is empty",
      "caller authentication key set has invalid Content-Length",
      `caller authentication key set exceeds ${MAX_ACCESS_JWKS_BYTES} bytes`,
      signal,
    );
  } catch {
    if (signal.aborted) throw operationAbortError(signal);
    throw new GatewayAuthenticationError(503, "caller authentication key set is invalid");
  }

  let jwks: unknown;
  try {
    jwks = JSON.parse(new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(jwksBytes));
  } catch {
    throw new GatewayAuthenticationError(503, "caller authentication key set is invalid");
  }
  if (!isJsonObject(jwks) || !Array.isArray(jwks.keys) || jwks.keys.length === 0 || jwks.keys.length > MAX_ACCESS_JWKS_KEYS) {
    throw new GatewayAuthenticationError(503, "caller authentication key set is invalid");
  }
  const seenKids = new Set<string>();
  const keys: AccessJwk[] = [];
  for (const candidate of jwks.keys) {
    const key = parseAccessJwk(candidate);
    if (!key || seenKids.has(key.kid)) {
      throw new GatewayAuthenticationError(503, "caller authentication key set is invalid");
    }
    seenKids.add(key.kid);
    keys.push(key);
  }
  return keys;
}

async function authenticateCaller(
  request: Request,
  issuer: string,
  audience: string,
  fetcher: GatewayFetcher,
  signal: AbortSignal,
): Promise<void> {
  if (signal.aborted) throw operationAbortError(signal);
  const token = request.headers.get("cf-access-jwt-assertion");
  if (!token || token.length > MAX_ACCESS_JWT_BYTES) throw new GatewayAuthenticationError(401);
  const parts = token.split(".");
  if (parts.length !== 3) throw new GatewayAuthenticationError(401);
  const [encodedHeader, encodedClaims, encodedSignature] = parts;
  if (!encodedHeader || !encodedClaims || !encodedSignature) throw new GatewayAuthenticationError(401);
  const header = decodeJsonSegment(encodedHeader, 2048);
  const claims = decodeJsonSegment(encodedClaims, MAX_ACCESS_JWT_BYTES);
  if (
    header.alg !== "RS256" || !validIdentifier(header.kid) ||
    header.crit !== undefined || header.jku !== undefined || header.x5u !== undefined
  ) throw new GatewayAuthenticationError(401);
  validateCallerClaims(claims, issuer, audience);
  const signature = decodeBase64Url(encodedSignature, 1024);
  const keys = await loadCallerAccessJwks(issuer, fetcher, signal);
  const jwk = keys.find((candidate) => candidate.kid === header.kid);
  if (!jwk) throw new GatewayAuthenticationError(401);

  let key: CryptoKey;
  try {
    key = await raceWithAbort(crypto.subtle.importKey(
      "jwk",
      jwk,
      { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" },
      false,
      ["verify"],
    ), signal);
  } catch {
    if (signal.aborted) throw operationAbortError(signal);
    throw new GatewayAuthenticationError(503, "caller authentication key is invalid");
  }

  let valid: boolean;
  try {
    const signingInput = new TextEncoder().encode(`${encodedHeader}.${encodedClaims}`);
    valid = await raceWithAbort(crypto.subtle.verify(
      { name: "RSASSA-PKCS1-v1_5" },
      key,
      signature as BufferSource,
      signingInput as BufferSource,
    ), signal);
  } catch {
    if (signal.aborted) throw operationAbortError(signal);
    throw new GatewayAuthenticationError(401);
  }
  if (!valid) throw new GatewayAuthenticationError(401);
}

export function sourceTextGatewayCapability(env: Env, requestUrl: string) {
  const configured = selection(env, new URL(requestUrl).origin);
  return {
    schema_version: CAPABILITY_SCHEMA,
    operation_id: "tos.source-text.search",
    operation_profile: "tos_source_text_search_v1",
    request_path: SEARCH_PATH,
    status: configured.status,
    selected: configured.status === "selected",
    reason: configured.status === "selected" ? null : configured.reason,
    runtime_readiness: "not-probed",
    endpoint_origin_exposed: false,
    authentication: "cloudflare-access-jwt-and-service-token-required",
    caller_access_audience_exposed: false,
    authority: {
      d1_reads_source_text: false,
      worker_selects_source_consumer: false,
      native_selected_consumer_owns_currentness_and_disclosure: true,
      grants_rights_or_publication: false,
    },
  } as const;
}

async function readBoundedStream(
  stream: ReadableStream<Uint8Array> | null,
  declaredLength: string | null,
  limit: number,
  emptyMessage: string,
  invalidLengthMessage: string,
  oversizedMessage: string,
  signal: AbortSignal,
): Promise<Uint8Array> {
  if (signal.aborted) throw operationAbortError(signal);
  if (declaredLength !== null) {
    const normalizedLength = declaredLength.trim();
    if (!/^[0-9]+$/.test(normalizedLength)) {
      cancelStream(stream);
      throw new GatewayRequestError(400, invalidLengthMessage);
    }
    if (BigInt(normalizedLength) > BigInt(limit)) {
      cancelStream(stream);
      throw new GatewayRequestError(413, oversizedMessage);
    }
  }
  if (!stream) throw new GatewayRequestError(400, emptyMessage);
  const reader = stream.getReader();
  const cancelActiveRead = () => cancelReader(reader, operationAbortError(signal));
  signal.addEventListener("abort", cancelActiveRead, { once: true });
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    while (true) {
      const { done, value } = await raceWithAbort(reader.read(), signal);
      if (done) break;
      size += value.byteLength;
      if (size > limit) {
        cancelReader(reader);
        throw new GatewayRequestError(413, oversizedMessage);
      }
      chunks.push(value);
    }
    if (size === 0) throw new GatewayRequestError(400, emptyMessage);
  } finally {
    signal.removeEventListener("abort", cancelActiveRead);
    try {
      reader.releaseLock();
    } catch {
      // A non-conforming stream may keep a read pending after cancel; the
      // operation itself still exits at its deadline and the reader was canceled.
    }
  }
  const body = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    body.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return body;
}

function gatewayError(message: string, status: number): Response {
  return jsonResponse({ error: message }, status, "POST");
}

export async function sourceTextSearchGatewayResponse(
  request: Request,
  env: Env,
  fetcher: GatewayFetcher = fetch,
  requestedTimeoutMs = MAX_OPERATION_MS,
): Promise<Response> {
  if (request.method !== "POST") return gatewayError("method not allowed", 405);
  const contentType = (request.headers.get("content-type")?.split(";")[0] ?? "").trim().toLowerCase();
  if (contentType !== "application/json") {
    return gatewayError("source-text search request must use application/json", 415);
  }

  const configured = selection(env, new URL(request.url).origin);
  if (configured.status === "unsupported") {
    return gatewayError(
      "source-text search unavailable: an explicitly selected durable PostgreSQL consumer is required",
      501,
    );
  }
  if (configured.status !== "selected") {
    return gatewayError("source-text gateway is misconfigured", 503);
  }

  const operation = gatewayOperation(
    request.signal,
    Number.isFinite(requestedTimeoutMs)
      ? Math.max(1, Math.min(MAX_OPERATION_MS, Math.floor(requestedTimeoutMs)))
      : MAX_OPERATION_MS,
  );
  try {
    try {
      await authenticateCaller(
        request,
        configured.callerAccessIssuer,
        configured.callerAccessAudience,
        fetcher,
        operation.signal,
      );
    } catch (error) {
      if (operation.signal.aborted) return operationAbortResponse(operation.signal);
      if (error instanceof GatewayAuthenticationError) return gatewayError(error.message, error.status);
      return gatewayError("source-text caller is not authorized", 401);
    }

    let body: Uint8Array;
    try {
      body = await readBoundedStream(
        request.body,
        request.headers.get("content-length"),
        MAX_REQUEST_BYTES,
        "source-text request body is required",
        "invalid source-text request Content-Length",
        `source-text request must not exceed ${MAX_REQUEST_BYTES} bytes`,
        operation.signal,
      );
    } catch (error) {
      if (operation.signal.aborted) return operationAbortResponse(operation.signal);
      if (error instanceof GatewayRequestError) {
        return gatewayError(error.message, error.status);
      }
      return gatewayError("source-text request could not be read", 400);
    }

    const target = new URL(SEARCH_PATH, configured.origin);
    let upstream: Response;
    try {
      const pendingUpstream = fetcher(target, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "Accept": "application/json",
          "CF-Access-Client-Id": configured.clientId,
          "CF-Access-Client-Secret": configured.clientSecret,
        },
        body: body as unknown as BodyInit,
        redirect: "manual",
        signal: operation.signal,
      });
      void pendingUpstream.then((lateResponse) => {
        if (operation.signal.aborted) cancelStream(lateResponse.body, operation.signal.reason);
      }, () => undefined);
      upstream = await raceWithAbort(pendingUpstream, operation.signal);
    } catch {
      if (operation.signal.aborted) return operationAbortResponse(operation.signal);
      return gatewayError("source-text gateway is unavailable", 502);
    }

    const upstreamContentType = (upstream.headers.get("content-type")?.split(";")[0] ?? "").trim().toLowerCase();
    if (
      upstream.status < 200 ||
      upstream.status > 599 ||
      (upstream.status >= 300 && upstream.status < 400) ||
      upstream.status === 204 ||
      upstream.status === 205 ||
      upstreamContentType !== "application/json"
    ) {
      cancelStream(upstream.body);
      return gatewayError("source-text gateway returned an invalid response", 502);
    }

    let responseBytes: Uint8Array;
    try {
      responseBytes = await readBoundedStream(
        upstream.body,
        upstream.headers.get("content-length"),
        MAX_RESPONSE_BYTES,
        "source-text gateway returned an empty response",
        "source-text gateway returned an invalid Content-Length",
        `source-text gateway response exceeds ${MAX_RESPONSE_BYTES} bytes`,
        operation.signal,
      );
      const decoded = new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(responseBytes);
      const parsed: unknown = JSON.parse(decoded);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        return gatewayError("source-text gateway returned an invalid response", 502);
      }
    } catch {
      if (operation.signal.aborted) return operationAbortResponse(operation.signal);
      return gatewayError("source-text gateway returned an invalid response", 502);
    }

    if (operation.signal.aborted) return operationAbortResponse(operation.signal);
    const headers = new Headers({
      "Content-Type": "application/json; charset=utf-8",
      "Cache-Control": "no-store",
    });
    return withSecurity(new Response(responseBytes as unknown as BodyInit, { status: upstream.status, headers }));
  } finally {
    operation.dispose();
  }
}

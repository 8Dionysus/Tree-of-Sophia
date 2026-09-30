export type Item = Record<string, unknown>;

export class HttpError extends Error {
  readonly status: number;

  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

export const SECURITY_HEADERS: Readonly<Record<string, string>> = {
  "Content-Security-Policy":
    "default-src 'self'; base-uri 'none'; connect-src 'self'; font-src 'self'; form-action 'self'; frame-ancestors 'none'; img-src 'self' data:; object-src 'none'; script-src 'self'; style-src 'self'; worker-src 'self'",
  "Permissions-Policy":
    "tools=(self), accelerometer=(), camera=(), geolocation=(), gyroscope=(), microphone=(), payment=(), usb=()",
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
  "Cross-Origin-Resource-Policy": "same-origin",
  "Origin-Agent-Cluster": "?1",
  "Referrer-Policy": "no-referrer",
  "X-Content-Type-Options": "nosniff",
  "X-Frame-Options": "DENY",
};

export function boundedInt(
  value: string | null,
  fallback: number,
  minimum: number,
  maximum: number,
): number {
  const parsed = Number.parseInt(value ?? "", 10);
  return Number.isFinite(parsed) ? Math.max(minimum, Math.min(maximum, parsed)) : fallback;
}

export function listParam(search: URLSearchParams, key: string): string[] {
  return (search.get(key) ?? "")
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean);
}

export function stringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string" && item.length > 0)
    : [];
}

export function objectArray(value: unknown): Item[] {
  return Array.isArray(value)
    ? value.filter((item): item is Item => Boolean(item) && typeof item === "object" && !Array.isArray(item))
    : [];
}

export function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

export function itemId(item: Item): string {
  return stringValue(item.node_id || item.edge_id || item.id);
}

export function parseItem(value: string): Item {
  const parsed: unknown = JSON.parse(value);
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("edge read-model row is not a JSON object");
  }
  return parsed as Item;
}

export function withSecurity(response: Response): Response {
  const headers = new Headers(response.headers);
  for (const [name, value] of Object.entries(SECURITY_HEADERS)) headers.set(name, value);
  return new Response(response.body, { status: response.status, statusText: response.statusText, headers });
}

export function jsonResponse(payload: unknown, status = 200, method = "GET"): Response {
  const headers = new Headers(SECURITY_HEADERS);
  headers.set("Content-Type", "application/json; charset=utf-8");
  headers.set("Cache-Control", "no-store");
  return new Response(method === "HEAD" ? null : JSON.stringify(payload), { status, headers });
}

export function matchesMask(rowMask: number, filterMask: number | null): boolean {
  return filterMask === null || (rowMask & filterMask) !== 0;
}

export function allowedPredicate(predicate: string, filters: string[]): boolean {
  return filters.length === 0 || filters.includes(predicate);
}

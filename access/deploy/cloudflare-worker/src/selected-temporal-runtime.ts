/** Request-local transport over the Rust temporal continuation. This module
 * selects no publication and issues no authority. Published snapshot delivery
 * checks its epoch/revision; stronger private native capture separately needs
 * its selected owner and current lease. The two APIs keep those scopes apart.
 */
export interface TemporalReplayStep {
  need(): string | undefined;
  bytes(): Uint8Array;
  error_code(): string | undefined;
  free(): void;
}
export interface TemporalReplaySession {
  advance(): TemporalReplayStep;
  provide(id: string, bytes: Uint8Array, absent: boolean): void;
  free(): void;
}
export interface TemporalReplayModule {
  TemporalReplaySession: new (revision: string, profile: string, request: Uint8Array,
    admission: Uint8Array, publishedOutput?: boolean) => TemporalReplaySession;
}
export interface TemporalPublishedModule extends TemporalReplayModule {
  validate_temporal_request_wasm_v1(request: Uint8Array, admission: Uint8Array): void;
}
export interface TemporalReadAccess {
  /** All values belong to one selected source revision and Claim profile. */
  readonly sourceRevision: string;
  readonly claimSourceGraph: string;
  readonly admission: Uint8Array;
  checkSelected(): Promise<void>;
  /** Return full verified retained bytes, or verified exact absence. The owner
   * meters physical I/O and transfer, verifies digest/membership and visibility,
   * and checks cancellation around each platform operation. */
  readExactNode(id: string, signal?: AbortSignal): Promise<Uint8Array | null>;
}

export interface SelectedTemporalAccess extends TemporalReadAccess {
  /** The owner binds tos.knowledge.temporal.compare and its intended use,
   * selected model receipt and every consulted carrier, holds its current
   * disclosure lease through private capture, rechecks selection/current policy
   * and releases the lease when the callback settles. This does not bind later
   * Worker body consumption/enqueue. Delivery under this stronger native profile
   * would need its owner to cover that lifecycle; the published snapshot API
   * below instead checks the publication epoch/revision. */
  withCurrentDisclosure<T>(capture: () => Promise<T>): Promise<T>;
}

export class SelectedTemporalError extends Error {
  readonly code: string;
  constructor(code: string) {
    super(`selected temporal continuation: ${code}`);
    this.code = code;
  }
}

function checkAbort(signal?: AbortSignal): void { signal?.throwIfAborted(); }

async function computeSelectedTemporal(runtime: TemporalReplayModule, selected: TemporalReadAccess,
  request: Uint8Array, signal?: AbortSignal, publishedOutput = false): Promise<Uint8Array> {
  checkAbort(signal);
  await selected.checkSelected();
  checkAbort(signal);
  let session: TemporalReplaySession;
  try { session = new runtime.TemporalReplaySession(selected.sourceRevision, selected.claimSourceGraph,
    request, selected.admission, publishedOutput); }
  catch (error) {
    if (typeof error === 'string') throw new SelectedTemporalError(error);
    throw error;
  }
  try {
    while (true) {
      checkAbort(signal);
      await selected.checkSelected();
      checkAbort(signal);
      const step = session.advance();
      let need: string | undefined, bytes: Uint8Array;
      try {
        const error = step.error_code();
        if (error !== undefined) throw new SelectedTemporalError(error);
        need = step.need();
        bytes = step.bytes();
      } finally { step.free(); }
      if (need !== undefined) {
        const carrier = await selected.readExactNode(need, signal);
        checkAbort(signal);
        await selected.checkSelected();
        checkAbort(signal);
        session.provide(need, carrier ?? new Uint8Array(), carrier === null);
        continue;
      }
      return bytes;
    }
  } catch (error) {
    // wasm-bindgen Result errors from provide() are string domain codes, like
    // constructor errors; preserve their transport class instead of Worker 500.
    if (typeof error === 'string') throw new SelectedTemporalError(error);
    throw error;
  } finally { session.free(); }
}

/** Private capture occurs while the current lease is held. Captured bytes or
 * values are not accepted for public delivery. A Response is refused because
 * its construction does not await platform body consumption. */
export async function captureSelectedTemporal<T>(runtime: TemporalReplayModule, selected: SelectedTemporalAccess,
  request: Uint8Array, capture: (bytes: Uint8Array) => Promise<T>, signal?: AbortSignal): Promise<T> {
  const bytes = await computeSelectedTemporal(runtime, selected, request, signal);
  return await selected.withCurrentDisclosure(async () => {
    checkAbort(signal);
    await selected.checkSelected();
    checkAbort(signal);
    const captured = await capture(bytes);
    if (captured instanceof Response) throw new SelectedTemporalError('response_delivery_lifecycle_unavailable');
    return captured;
  });
}

/** Demand-driven delivery from one verified published snapshot. Selection is
 * checked again immediately before the whole packet is enqueued and closed.
 * This is an optimistic publication check, not a policy grant or a database
 * transaction spanning remote network flush. No bytes enqueue on construction.
 */
export async function respondTemporalSnapshot(runtime: TemporalReplayModule, selected: TemporalReadAccess,
  request: Uint8Array, signal?: AbortSignal): Promise<Response> {
  let bytes: Uint8Array | undefined = await computeSelectedTemporal(runtime, selected, request, signal, true);
  let terminal = false;
  let controller: ReadableStreamDefaultController<Uint8Array> | undefined;
  const finish = (): void => {
    terminal = true;
    bytes = undefined;
    signal?.removeEventListener('abort', onAbort);
  };
  const onAbort = (): void => {
    if (terminal) return;
    controller?.error(signal?.reason ?? new DOMException('request aborted', 'AbortError'));
    finish();
  };
  try {
    checkAbort(signal);
    await selected.checkSelected();
    checkAbort(signal);
    const body = new ReadableStream<Uint8Array>({
      start(value) { controller = value; },
      async pull(value) {
        if (terminal) return;
        try {
          await selected.checkSelected();
          if (terminal) return;
          checkAbort(signal);
          // No await between the snapshot check and final whole-body handoff.
          value.enqueue(bytes!);
          value.close();
        } catch (error) {
          if (!terminal) value.error(error);
        } finally { finish(); }
      },
      cancel() { finish(); },
    }, {highWaterMark: 0});
    signal?.addEventListener('abort', onAbort, {once: true});
    if (signal?.aborted) { onAbort(); checkAbort(signal); }
    return new Response(body, {headers: {
      'Content-Type': 'application/json; charset=utf-8', 'Cache-Control': 'no-store',
    }});
  } catch (error) { finish(); throw error; }
}

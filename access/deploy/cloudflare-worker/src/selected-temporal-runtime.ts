/** Request-local transport over the Rust temporal continuation. This module
 * selects no publication and issues no authority. It is deliberately unbound
 * until a source owner supplies verified selected reads and a current lease.
 * It supports private byte capture only, not Worker response-body delivery.
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
    admission: Uint8Array) => TemporalReplaySession;
}
export interface SelectedTemporalAccess {
  /** All values belong to one genuinely selected model/policy/profile scope. */
  readonly sourceRevision: string;
  readonly claimSourceGraph: string;
  readonly admission: Uint8Array;
  checkSelected(): Promise<void>;
  /** Return full verified retained bytes, or verified exact absence. The owner
   * meters physical I/O and transfer, verifies digest/membership and visibility,
   * and checks cancellation around each platform operation. */
  readExactNode(id: string, signal?: AbortSignal): Promise<Uint8Array | null>;
  /** The owner binds tos.knowledge.temporal.compare and its intended use,
   * selected model receipt and every consulted carrier, holds its current
   * disclosure lease through private capture, rechecks selection/current policy
   * and releases the lease when the callback settles. This does not bind later
   * Worker body consumption/enqueue; public delivery needs an owner primitive
   * covering that actual lifecycle. */
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

/** Private capture occurs while the current lease is held. Captured bytes or
 * values are not accepted for public delivery. A Response is refused because
 * its construction does not await platform body consumption. Worker response
 * lifetime/final enqueue and lease release remain an unimplemented owner gate.
 */
export async function captureSelectedTemporal<T>(runtime: TemporalReplayModule, selected: SelectedTemporalAccess,
  request: Uint8Array, capture: (bytes: Uint8Array) => Promise<T>, signal?: AbortSignal): Promise<T> {
  checkAbort(signal);
  await selected.checkSelected();
  checkAbort(signal);
  const session = new runtime.TemporalReplaySession(selected.sourceRevision, selected.claimSourceGraph,
    request, selected.admission);
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
      return await selected.withCurrentDisclosure(async () => {
        checkAbort(signal);
        await selected.checkSelected();
        checkAbort(signal);
        const captured = await capture(bytes);
        if (captured instanceof Response) throw new SelectedTemporalError('response_delivery_lifecycle_unavailable');
        return captured;
      });
    }
  } finally { session.free(); }
}

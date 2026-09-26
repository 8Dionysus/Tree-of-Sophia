/** Request-local transport over the Rust temporal continuation. This module
 * selects no publication and issues no authority. It is deliberately unbound
 * until a source owner supplies verified selected reads and a current lease.
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
   * disclosure lease through delivery, rechecks selection/current policy and
   * releases the lease when the callback settles. */
  withCurrentDisclosure<T>(deliver: () => Promise<T>): Promise<T>;
}

export class SelectedTemporalError extends Error {
  constructor(readonly code: string) { super(`selected temporal continuation: ${code}`); }
}

function checkAbort(signal?: AbortSignal): void { signal?.throwIfAborted(); }

/** The callback consumes complete packet bytes while the disclosure lease is
 * held. Returning a Response here buffers those exact bytes; a streaming host
 * must finish its delivery within the callback instead of returning a stream.
 */
export async function deliverSelectedTemporal<T>(runtime: TemporalReplayModule, selected: SelectedTemporalAccess,
  request: Uint8Array, deliver: (bytes: Uint8Array) => Promise<T>, signal?: AbortSignal): Promise<T> {
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
        return await deliver(bytes);
      });
    }
  } finally { session.free(); }
}

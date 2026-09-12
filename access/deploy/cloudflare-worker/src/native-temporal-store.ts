/** Bounded exact-ID temporal operands in the caller's guarded D1 snapshot. */
import {HttpError} from './common.ts';
import {NativeBudgetExceeded} from '../../../shared/native-semantics.ts';
import {NativeD1Read,NativeD1Rows,nativeD1Limits,nativeUnavailable} from './native-d1-read.ts';
import {readNativeInspectionPublication} from './native-inspection-store.ts';
import {nativeField,type NativePacket,type NativeRef} from './native-lens.ts';
import {compareTemporalOperands,normalizeTemporalComparisonRequest} from './temporal-comparison.ts';
import {KnowledgeRevisionConflict} from './lens-pagination.ts';

export async function compareNativeTemporalD1(db:D1Database, request:unknown, expectedRevision:string):Promise<NativePacket> {
  const normalized = normalizeTemporalComparisonRequest(request);
  try {
    const read = new NativeD1Read(db,nativeD1Limits,true);
    const top = await readNativeInspectionPublication(read,expectedRevision);
    const rows = new NativeD1Rows(read,nativeD1Limits,false);
    const cache = new Map<string,NativeRef[]>();
    let calls = 0;
    return await compareTemporalOperands(nativeField(top.ref,'source_revision').value,normalized,async id => {
      if (++calls > 6) throw new NativeBudgetExceeded('temporal exact lookup budget');
      if (!id.isWellFormed()) return nativeUnavailable('prepared response contains invalid JSON values');
      const cached = cache.get(id); if (cached) return cached;
      const found = await read.textRows<{id:string}>(['id'],['id'],
        'SELECT id FROM knowledge_nodes WHERE id=? ORDER BY id LIMIT 2',id);
      if (found.length > 1) throw new NativeBudgetExceeded('prepared temporal has too many exact identity matches');
      const result = found.length ? [await rows.get('node',id)] : [];
      cache.set(id,result); return result;
    });
  } catch (error) {
    if (error instanceof HttpError || error instanceof NativeBudgetExceeded || error instanceof KnowledgeRevisionConflict) throw error;
    return nativeUnavailable('prepared temporal publication unavailable or invalid');
  }
}

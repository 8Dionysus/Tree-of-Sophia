/** Optional, independently versioned lens stores bound to the exact v9 epoch. */
import {HttpError} from './common.ts';
import {derived, nativeChild, nativePacketJson, type NativeRef} from './native-lens.ts';
import {NativeD1Read, nativeSha256, nativeUnavailable} from './native-d1-read.ts';

const STATES = {compact: ['knowledge_compact_lens_state','tos_compact_lens_carrier_v1'],
  membership: ['knowledge_lens_membership_state','tos_lens_membership_index_v1']} as const;

export async function readLensPublicationBinding(read: NativeD1Read, top: {raw: string; ref: NativeRef}, selectedEpoch?: number) {
  // The actual snapshot driver already admits this epoch before and after
  // metadata selection. Reuse that value; auxiliary admission still reads
  // their own current clock. Neither value is a policy or rights grant.
  let epoch=selectedEpoch;
  if(epoch===undefined) {
    const clock=await read.query<{epoch:number; kind:string}>('SELECT epoch,typeof(epoch) AS kind FROM knowledge_exploration_clock WHERE singleton=1 LIMIT 2');
    if(clock.length!==1||clock[0]!.kind!=='integer')nativeUnavailable('lens publication epoch invalid');
    epoch=clock[0]!.epoch;
  }
  if(!Number.isSafeInteger(epoch)||epoch!<0)nativeUnavailable('lens publication epoch invalid');
  return derived({schema:'tos_published_knowledge_snapshot_v1',publication_epoch:epoch!,
    metadata_sha256:await nativeSha256(top.raw),...Object.fromEntries(['read_model_schema','source_revision','data_revision','graph_schema','normalization_binding'].map(key=>[key,nativeChild(top.ref,key)]))});
}

export async function admitAuxiliary(read: NativeD1Read, top: {raw: string; ref: NativeRef}, requested: (keyof typeof STATES)[]) {
  if (!requested.length) return {installed:requested,verify:async()=>{}};
  const names=requested.map(key=>STATES[key][0]);
  const found=await read.textRows<{name:string}>(['name'],['name'],
    "SELECT name FROM sqlite_master WHERE type='table' AND name IN (SELECT value FROM json_each(?))",JSON.stringify(names));
  const installed=requested.filter(key=>found.some(row=>row.name===STATES[key][0]));
  if (!installed.length) return {installed, verify:async()=>{}};
  const expected=nativePacketJson(await readLensPublicationBinding(read,top));
  const verify=async (initial=false)=>{
    for (const key of installed) {
      const [table,schema]=STATES[key];
      const rows=await read.query<{schema:unknown; binding:unknown; valid:unknown}>(
        `SELECT CASE WHEN typeof(schema)='text' AND length(schema)<=128 THEN schema END AS schema,
        CASE WHEN typeof(binding)='text' AND length(CAST(binding AS BLOB))<=65536 THEN binding END AS binding,
        CASE WHEN typeof(valid)='integer' THEN valid END AS valid FROM ${table} WHERE singleton=1 LIMIT 2`);
      if (rows.length!==1 || rows[0]!.schema!==schema || rows[0]!.binding!==expected || rows[0]!.valid!==1) {
        if (initial) nativeUnavailable('native lens auxiliary store stale or incompatible: '+key);
        throw new HttpError(409,'native lens auxiliary store changed during query');
      }
    }
  };
  await verify(true);
  return {installed,verify:()=>verify(false)};
}

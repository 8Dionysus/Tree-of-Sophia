/** Search selection is ID-only; source-valued packets keep native JSON. */
import {HttpError, type Item} from './common.ts';
import {NativeBudgetExceeded, nativeLower, codePointCompare, pythonStr} from '../../../shared/native-semantics.ts';
import {NativeD1Read, NativeD1Rows, nativeD1Limits, nativeUnavailable, nativeSha256} from './native-d1-read.ts';
import {readNativeInspectionPublication} from './native-inspection-store.ts';
import {derived, nativeField, nativeKeys, nativeChild, nativePacketArray, nativePacketObject, type NativePacket, type NativeRef} from './native-lens.ts';

function sourceString(value:string):string {
  if(!value.isWellFormed())nativeUnavailable('search source contains an invalid Unicode string');
  return value;
}

/** Python's existing ensure_ascii=False, sort_keys=True default JSON/lower. */
function searchable(ref:NativeRef):string {
  let remaining=4*1024*1024;
  const emit=(text:string)=>{remaining-=text.length;if(remaining<0)throw new NativeBudgetExceeded('selected search document byte budget');return text;};
  const walk=(item:NativeRef):string=>{
    const value=item.value;
    if(value===null)return emit('null');
    if(typeof value==='boolean')return emit(String(value));
    if(typeof value==='string')return emit(JSON.stringify(sourceString(value)));
    if(typeof value==='number')return emit(pythonStr(item));
    const array=Array.isArray(value),keys=array?nativeKeys(item):[...nativeKeys(item)].sort(codePointCompare);
    return emit(array?'[':'{')+keys.map((key,index)=>(index?emit(', '):'')+(array?'':emit(JSON.stringify(sourceString(key))+': '))+walk(nativeChild(item,key))).join('')+emit(array?']':'}');
  };
  return nativeLower(walk(ref));
}

function rankValues(ref:NativeRef,fields:string[]):string[] {
  const result:string[]=[];
  for(const field of fields){const item=nativeField(ref,'display.'+field),value=item.value;
    if(typeof value==='string')result.push(nativeLower(value));
    else if(value&&typeof value==='object'&&!Array.isArray(value))for(const key of nativeKeys(item)){
      const candidate=nativeChild(item,key).value;if(typeof candidate==='string')result.push(nativeLower(candidate));
    }
  }
  return result;
}

export class NativeSearchDelivery {
  readonly read: NativeD1Read;
  readonly rows: NativeD1Rows;
  readonly top: NativeRef;
  private verifiedChars=0;
  private constructor(read: NativeD1Read, top: NativeRef) {
    this.read=read; this.rows=new NativeD1Rows(read,nativeD1Limits,false); this.top=top;
  }
  static async open(db: D1Database, revision: string): Promise<NativeSearchDelivery> {
    const read=new NativeD1Read(db,nativeD1Limits,true);
    const top=await readNativeInspectionPublication(read,revision);
    // Common publication admission validates all header strings/keys. Selected
    // source/searchable strings retain their independent sourceString check.
    // Both v8/v9 producers emit the rank carrier. Presence is readiness, not
    // a global completeness proof; selected payloads still verify row digests.
    await read.query('SELECT kind,position,id,id_lower,native_id_lower,identity_values,visible_values,document_chars FROM knowledge_search_documents LIMIT 0');
    return new NativeSearchDelivery(read,top.ref);
  }
  async select(sql:string,args:unknown[]):Promise<D1Result<{id:string;id_lower:string;position:number;search_rank:number}>> {
    const allowance=this.read.limits.maxDecodedBytes-this.read.deliveredBytes;
    // Callers serialize the two selected-kind deliveries. Mask both individual
    // and aggregate ID text in SQL before it can cross the D1 boundary. Global
    // selection work is intentionally not charged as native payload rows-read.
    const result=await this.read.db.prepare(`WITH selected AS (${sql}),framed AS (
      SELECT *,sum(coalesce(length(CAST(id AS BLOB)),0)+coalesce(length(CAST(id_lower AS BLOB)),0))
       OVER (ORDER BY search_rank,id_lower,position ROWS UNBOUNDED PRECEDING) AS _native_bytes,
       CASE WHEN typeof(id)='text' AND typeof(id_lower)='text' AND length(CAST(id AS BLOB))<=1048576
        AND length(CAST(id_lower AS BLOB))<=1048576 AND typeof(position)='integer' AND position>=0
        AND typeof(search_rank)='integer' AND search_rank BETWEEN 0 AND 3 THEN 1 ELSE 0 END AS _native_valid FROM selected)
      SELECT CASE WHEN _native_valid=1 AND _native_bytes<=? THEN id ELSE NULL END AS id,
       CASE WHEN _native_valid=1 AND _native_bytes<=? THEN id_lower ELSE NULL END AS id_lower,
       position,search_rank,_native_bytes,_native_valid FROM framed ORDER BY search_rank,id_lower,position`).bind(...args,allowance,allowance)
      .all<{id:string;id_lower:string;position:number;search_rank:number;_native_bytes:number;_native_valid:number}>();
    for(const row of result.results){if(row._native_valid!==1)nativeUnavailable('selected search identity/rank is invalid');
      if(!Number.isSafeInteger(row._native_bytes)||row._native_bytes>allowance)throw new NativeBudgetExceeded('selected search identity delivery-byte budget');
    }
    this.read.deliveredBytes+=result.results.at(-1)?._native_bytes??0;
    this.read.returned+=result.results.length;
    if(this.read.returned>this.read.limits.maxRows)throw new NativeBudgetExceeded('selected search returned-row budget');
    return result;
  }
  async items(kind: 'nodes'|'relations', selected: {id:string;position:number}[]): Promise<NativeRef[]> {
    const ids=selected.map(row=>row.id);
    if(ids.some(id=>typeof id!=='string'||!id)||new Set(ids).size!==ids.length) nativeUnavailable('search selected identity carrier is invalid');
    const rows=await this.rows.load(kind==='nodes'?'node':'relation',ids);
    if(!ids.length)return [];
    const columns=['id','source_graph','kind_id','predicate_id','id_lower','native_id_lower','identity_values','visible_values','document_digest','document_chars'] as const;
    const carriers=await this.read.textRows<Record<typeof columns[number],string>>(columns,['id'],
      `SELECT s.id,s.source_graph,s.kind_id,s.predicate_id,s.id_lower,s.native_id_lower,s.identity_values,s.visible_values,s.document_digest,
       CASE WHEN typeof(s.document_chars)='integer' AND s.document_chars>=0 THEN CAST(s.document_chars AS TEXT) ELSE NULL END AS document_chars
       FROM json_each(?) wanted JOIN knowledge_search_documents s ON s.kind=? AND s.position=json_extract(wanted.value,'$.position')
       WHERE s.id=json_extract(wanted.value,'$.id') ORDER BY s.id LIMIT ?`,JSON.stringify(selected),kind,ids.length+1);
    if(carriers.length!==ids.length||new Set(carriers.map(row=>row.id)).size!==ids.length)nativeUnavailable('selected search carrier closure is incomplete');
    for(const carrier of carriers){const ref=rows.get(carrier.id);if(!ref)nativeUnavailable('selected search carrier identity differs');
      const text=searchable(ref);this.verifiedChars+=[...text].length;
      if(this.verifiedChars>16_000_000)throw new NativeBudgetExceeded('selected search verification character budget');
      const relation=kind==='relations',primary=relation?'label':'title',visible=relation?['label','inverse_label','statement','explanation']:['title','kind_label','summary'];
      if(carrier.id_lower!==nativeLower(carrier.id)||carrier.native_id_lower!==nativeLower(nativeField(ref,'native_id').value as string)
        ||carrier.source_graph!==nativeField(ref,'source_graph').value||carrier.kind_id!==(nativeField(ref,'kind_id').value??'')
        ||carrier.predicate_id!==(nativeField(ref,'predicate_id').value??'')||carrier.identity_values!==JSON.stringify(rankValues(ref,[primary]))
        ||carrier.visible_values!==JSON.stringify(rankValues(ref,visible))||carrier.document_chars!==String([...text].length)
        ||carrier.document_digest!==await nativeSha256(text))nativeUnavailable('selected search carrier differs from source payload');
    }
    return ids.map(id=>rows.get(id)!);
  }
  packet(fields: Item, nodes: NativeRef[], relations: NativeRef[]): NativePacket {
    return nativePacketObject(Object.entries(fields).map(([key,value])=>[key,
      key==='nodes'?nativePacketArray(nodes):key==='relations'?nativePacketArray(relations):
      key==='source_revision'?nativeField(this.top,'source_revision'):
      key==='authority_boundary'?nativeField(this.top,'authority_boundary'):derived(value)]));
  }
}

export async function nativeSearchFailure<T>(run:()=>Promise<T>):Promise<T> {
  try {return await run();}
  catch(error) {if(error instanceof HttpError||error instanceof NativeBudgetExceeded)throw error;
    return nativeUnavailable('prepared search publication unavailable or invalid');}
}

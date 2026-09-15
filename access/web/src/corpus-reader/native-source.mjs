import {readExactSource,SOURCE_READ_DEADLINE_MS} from '../observatory/exact-source-read.mjs';
import {ContractError,RequestError} from '../observatory/knowledge-client.mjs';
import {withAbort} from '../observatory/bounded-response.mjs';
import {validateNativeReference,matchNativeReference} from './native-reference.mjs';

export class NativeReferenceError extends Error{
  constructor(status){super(status==='stale'?'The source text or its exact binding has changed. The saved reference was retained.':'This exact source text is currently unavailable.');this.name='NativeReferenceError';this.status=status;}
}

const active=new WeakMap();
async function boundedRead(client,work){
  const count=active.get(client)??0;
  if(count>=2)throw new RequestError(429,'Two source reads are already in progress. Retry after one completes.');
  active.set(client,count+1);
  try{return await work();}finally{active.set(client,(active.get(client)??1)-1);}
}

export function readNativeSelection(client,selection,{signal,representation='native_public_unit',read=readExactSource}={}){
  if(!['native_public_unit','native_local_unit'].includes(representation))throw new ContractError('Unsupported native text representation.');
  return boundedRead(client,()=>read(client,selection,{signal,representation}));
}

export function readNativeReference(client,reference,{signal,read=readExactSource,timeoutMs=SOURCE_READ_DEADLINE_MS}={}){
  const checked=validateNativeReference(reference);
  if(!Number.isSafeInteger(timeoutMs)||timeoutMs<1||timeoutMs>SOURCE_READ_DEADLINE_MS)throw new ContractError('Invalid source reading deadline.');
  return boundedRead(client,async()=>{
    const controller=new AbortController();let timedOut=false;
    const abort=()=>controller.abort(signal?.reason);
    if(signal?.aborted)abort();else signal?.addEventListener('abort',abort,{once:true});
    const timer=setTimeout(()=>{timedOut=true;controller.abort();},timeoutMs);
    try{
    // Rediscover the origin at the selected current snapshot. Do not carry an
    // expired graph revision into a durable passage address. The owner-issued
    // target is read normally, then every saved text binding must still match.
    controller.signal.throwIfAborted();
    const {packet,match}=await withAbort(client.inspect(checked.origin.kind,checked.origin.id,controller.signal),controller.signal);
    const selection={...checked.origin,source_revision:packet.source_revision,content_revision:match.content_revision};
    const result=await withAbort(read(client,selection,{signal:controller.signal,representation:checked.representation}),controller.signal);
    controller.signal.throwIfAborted();
    const resolution=matchNativeReference(checked,result);
    if(resolution.status!=='exact')throw new NativeReferenceError(resolution.status);
    return {result,reference:checked,spanIndex:resolution.spanIndex};
    }catch(error){if(timedOut)throw new RequestError(504,'Source reading exceeded its deadline.');throw error;}
    finally{clearTimeout(timer);signal?.removeEventListener('abort',abort);}
  });
}

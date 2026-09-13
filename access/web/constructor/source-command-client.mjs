/** Explicit source-owner transport. Never uses the read-only /api adapter.
 * Credentials live only in this object; no URL, browser storage or diagnostics.
 * Failed delivery never causes an automatic retry of a potentially applied command.
 */
export const COMMAND_REQUEST_BYTES=1048576;
export const COMMAND_RESPONSE_BYTES=4194304;
const hex=bytes=>Array.from(new Uint8Array(bytes),value=>value.toString(16).padStart(2,'0')).join('');
const utf8=value=>new TextEncoder().encode(value);
const digest=async value=>hex(await crypto.subtle.digest('SHA-256',utf8(value)));
const signature=async(key,fields)=>hex(await crypto.subtle.sign('HMAC',key,utf8(JSON.stringify(fields))));

export class SourceCommandTransportError extends Error {
  constructor(code,{outcome='unconfirmed',status=null}={}){
    super(code);this.name='SourceCommandTransportError';this.code=code;this.outcome=outcome;this.status=status;
  }
}

function endpoint(value){
  const parsed=new URL(value);
  if(parsed.protocol!=='http:'||parsed.hostname!=='127.0.0.1'||!parsed.port||
    parsed.username||parsed.password||parsed.pathname!=='/'||parsed.search||parsed.hash||
    value!==parsed.origin)throw new TypeError('Select one explicit loopback source-owner origin.');
  return parsed.origin;
}

async function readBounded(response){
  if(!response.body?.getReader)throw new SourceCommandTransportError('owner-response-stream-unavailable');
  const declared=response.headers.get('content-length');
  if(declared!==null&&(!/^\d+$/.test(declared)||Number(declared)>COMMAND_RESPONSE_BYTES)){
    await response.body.cancel();throw new SourceCommandTransportError('owner-response-budget');
  }
  if(!/^application\/json(?:;|$)/i.test(response.headers.get('content-type')??'')){
    await response.body.cancel();throw new SourceCommandTransportError('owner-response-not-json');
  }
  const reader=response.body.getReader(),decoder=new TextDecoder('utf-8',{fatal:true});
  let bytes=0,text='';
  try{
    for(;;){const {done,value}=await reader.read();if(done)break;
      bytes+=value.byteLength;if(bytes>COMMAND_RESPONSE_BYTES)throw new SourceCommandTransportError('owner-response-budget');
      text+=decoder.decode(value,{stream:true});}
    text+=decoder.decode();const value=JSON.parse(text);
    if(value===null||Array.isArray(value)||typeof value!=='object')throw new Error('invalid envelope');
    return {value,text};
  }catch(error){await reader.cancel().catch(()=>{});throw error;}
  finally{reader.releaseLock();}
}

export function createSourceCommandClient({origin,token,fetchImpl=globalThis.fetch,timeoutMs=30_000}={}){
  const base=endpoint(origin);
  if(typeof token!=='string'||! /^[a-f0-9]{64}$/.test(token))throw new TypeError('A private owner transport credential is required.');
  if(typeof fetchImpl!=='function')throw new TypeError('A fetch transport is required.');
  if(!Number.isInteger(timeoutMs)||timeoutMs<1||timeoutMs>120_000)throw new TypeError('Select a bounded transport deadline.');
  let credential=token,closed=false,busy=false,active=null;
  async function send(path,request,{signal}={}){
    if(closed)throw new SourceCommandTransportError('owner-connection-closed',{outcome:'not-dispatched'});
    if(busy)throw new SourceCommandTransportError('owner-command-in-flight',{outcome:'not-dispatched'});
    let body;
    if(request!==undefined){
      if(!request||typeof request!=='object'||Array.isArray(request))throw new TypeError('An exact owner command object is required.');
      body=JSON.stringify(request);
      if(new TextEncoder().encode(body).byteLength>COMMAND_REQUEST_BYTES)
        throw new SourceCommandTransportError('owner-request-budget',{outcome:'not-dispatched'});
    }
    busy=true;
    const controller=new AbortController();active=controller;
    const abort=()=>controller.abort();
    if(signal?.aborted)abort();else signal?.addEventListener('abort',abort,{once:true});
    const timer=setTimeout(abort,timeoutMs);
    try{
      const key=await crypto.subtle.importKey('raw',Uint8Array.from(credential.match(/../g),value=>parseInt(value,16)),
        {name:'HMAC',hash:'SHA-256'},false,['sign','verify']);
      const method=body===undefined?'GET':'POST',timestamp=String(Math.floor(Date.now()/1000));
      const nonce=hex(crypto.getRandomValues(new Uint8Array(32))),bodyDigest=await digest(body??'');
      const signed=await signature(key,['tos-request-v1',method,path,timestamp,nonce,bodyDigest]);
      const response=await fetchImpl(base+path,{method,body,signal:controller.signal,
        headers:{Authorization:`ToS-HMAC-SHA256 ${timestamp}:${nonce}:${bodyDigest}:${signed}`,'Content-Type':'application/json'},
        cache:'no-store',credentials:'omit',redirect:'error'});
      const {value,text}=await readBounded(response);
      const proof=response.headers.get('x-tos-response-signature');
      if(!/^[a-f0-9]{64}$/.test(proof??'')||!await crypto.subtle.verify('HMAC',key,
        Uint8Array.from(proof.match(/../g),value=>parseInt(value,16)),
        utf8(JSON.stringify(['tos-response-v1',nonce,response.status,await digest(text)]))))
        throw new SourceCommandTransportError('owner-response-unverified');
      if(!response.ok)throw new SourceCommandTransportError(
        typeof value.code==='string'&&/^[a-z-]{1,80}$/.test(value.code)?value.code:'owner-command-failed',
        {outcome:value.outcome==='not-dispatched'?'not-dispatched':'unconfirmed',status:response.status});
      return value;
    }catch(error){
      if(error instanceof SourceCommandTransportError)throw error;
      // Network errors may follow an owner commit. Do not echo tokens/URLs or
      // claim cancellation/rollback. Reconciliation uses the original command.
      throw new SourceCommandTransportError('owner-delivery-unconfirmed');
    }finally{clearTimeout(timer);signal?.removeEventListener('abort',abort);active=null;busy=false;}
  }
  return Object.freeze({catalog:options=>send('/commands/catalog',undefined,options),
    execute:(request,options)=>send('/commands',request,options),
    close(){closed=true;credential='';active?.abort();},isClosed:()=>closed});
}

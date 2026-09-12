// Browser transport admission, not the backend's larger response allowance.
export const DEFAULT_RESPONSE_BYTES=4*1024*1024;
export const MAX_RESPONSE_BYTES=8*1024*1024;
export class ResponseLimitError extends Error {}
export function validateResponseLimit(limit){
  if(!Number.isSafeInteger(limit)||limit<1||limit>MAX_RESPONSE_BYTES)throw new RangeError('maxResponseBytes must be an integer between 1 and 8388608');
}

// Consume eventual rejections even when a custom transport ignores abort.
// Cancellation must finish the request without waiting for that transport.
export function withAbort(promise,signal){
  return new Promise((resolve,reject)=>{
    const abort=()=>{signal.removeEventListener('abort',abort);reject(signal.reason);};
    signal.addEventListener('abort',abort,{once:true});
    if(signal.aborted)abort();
    Promise.resolve(promise).then(value=>{
      signal.removeEventListener('abort',abort);
      if(signal.aborted)reject(signal.reason);else resolve(value);
    },error=>{signal.removeEventListener('abort',abort);reject(error);});
  });
}

// Cancellation is best effort and must not obscure the original error or wait
// forever for an uncooperative custom source's cancellation promise.
export function cancelResponseBody(response,reason){
  try{Promise.resolve(response?.body?.cancel?.(reason)).catch(()=>{});}catch{}
}
function cancelReader(reader,reason){
  try{Promise.resolve(reader.cancel(reason)).catch(()=>{});}catch{}
}
function exceedsDeclaredLength(response,limit){
  const header=response.headers?.get?.('Content-Length');
  if(typeof header!=='string')return false;
  const value=header.replace(/^[\t ]+|[\t ]+$/g,'');
  // HTTP's decimal grammar accepts leading zeros, not signs, exponents,
  // duplicate/comma values or a trailing newline. Huge valid integers exceed
  // this small ceiling even when Number represents them as Infinity.
  return /^[0-9]+$(?![\s\S])/.test(value)&&Number(value)>limit;
}

export async function readBoundedJSON(response,limit,signal){
  validateResponseLimit(limit);
  signal.throwIfAborted();
  if(exceedsDeclaredLength(response,limit)){
    const error=new ResponseLimitError();cancelResponseBody(response,error);throw error;
  }
  const body=response.body;
  if(body!==null&&body!==undefined){
    if(typeof body.getReader!=='function'){
      const error=new SyntaxError('response body has no byte reader');cancelResponseBody(response,error);throw error;
    }
    let reader;
    try{reader=body.getReader();}catch(error){cancelResponseBody(response,error);throw error;}
    try{
      // A bounded growing buffer avoids retaining one allocation per tiny
      // chunk. Count the stream's actual bytes before copying or parsing.
      let bytes=new Uint8Array(Math.min(limit,65536)),used=0;
      for(;;){
        const {done,value}=await withAbort(reader.read(),signal);
        signal.throwIfAborted();
        if(done)break;
        if(!(value instanceof Uint8Array))throw new SyntaxError('response body contains a non-byte chunk');
        if(value.byteLength>limit-used)throw new ResponseLimitError();
        if(used+value.byteLength>bytes.length){
          const grown=new Uint8Array(Math.min(limit,Math.max(used+value.byteLength,bytes.length*2)));
          grown.set(bytes.subarray(0,used));bytes=grown;
        }
        bytes.set(value,used);used+=value.byteLength;
      }
      let text;
      try{text=new TextDecoder('utf-8',{fatal:true}).decode(bytes.subarray(0,used));}
      catch{throw new SyntaxError('response body contains invalid UTF-8');}
      signal.throwIfAborted();
      const packet=JSON.parse(text);
      signal.throwIfAborted();return packet;
    }catch(error){cancelReader(reader,error);throw error;}
    finally{try{reader.releaseLock();}catch{}}
  }
  // Compatibility for existing in-memory json-only test/custom transports.
  // This is a post-parse size check, not a network-memory bound. Real bodies
  // always take the stream path above and never fall back to response.json().
  const packet=await withAbort(response.json(),signal);
  signal.throwIfAborted();
  let serialized;
  try{serialized=JSON.stringify(packet);}catch{throw new SyntaxError('response is not JSON data');}
  if(typeof serialized!=='string')throw new SyntaxError('response is not JSON data');
  if(new TextEncoder().encode(serialized).byteLength>limit)throw new ResponseLimitError();
  signal.throwIfAborted();return packet;
}

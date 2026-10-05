/** JSON transport for source-owned constructor reading mechanics. */
import {PRIMARY_SOURCES} from './source-references.mjs';
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
let execute;
export function installConstructorReadingRules(runtime){
 if(typeof runtime?.constructor_reading_wasm_v1!=='function')throw Error('Constructor reading rules are unavailable');
 execute=runtime.constructor_reading_wasm_v1;
}
export function readingRule(operation,input={}){
 if(!execute)throw Error('Constructor reading rules are unavailable');
 try{return JSON.parse(decoder.decode(execute(encoder.encode(JSON.stringify({operation,references:PRIMARY_SOURCES,...input})))));}
 catch(cause){throw Error(cause instanceof Error?cause.message:String(cause));}
}

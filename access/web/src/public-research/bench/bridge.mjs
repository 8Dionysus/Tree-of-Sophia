// A passage read elsewhere (the Library reader, the stand's quotations and parallel reading) can be laid on the
// workbench. The bench, where a stand mounts one, receives it; a reader asks first whether anyone is listening.

let receiver=null;

/** The mounted bench receives passages; the returned function stops it. */
export function receiveOnBench(receive){receiver=receive;return ()=>{if(receiver===receive)receiver=null;};}

/** Whether a bench is mounted to receive a passage. */
export const benchListens=()=>receiver!==null;

/**
 * Lays a passage of a witness on the bench: {language, chapter, unitId, start, end} with offsets in the unit's
 * own text, or in its display text with `display: true`; or a quotation cited in the original, {cite, language}
 * with the witness it is read in. Resolves to {id, label} or null.
 */
export function layOnBench(passage){const receive=receiver;return receive?Promise.resolve().then(()=>receive(passage)).then(laid=>laid??null,()=>null):Promise.resolve(null);}

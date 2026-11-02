import {canonicalJSON} from './research-index.mjs';
const snapshots=import.meta.glob('../../../../bonsai/zarathustra/history/*.json',{import:'default'});
export const retainedResearchVersions=()=>Object.keys(snapshots).map(path=>path.split('/').at(-1).slice(0,-5));
export async function loadResearchVersion(revision){
  if(!/^[a-f0-9]{64}$/.test(revision??''))throw new TypeError('Invalid research snapshot revision.');
  const loader=snapshots[`../../../../bonsai/zarathustra/history/${revision}.json`];
  if(!loader)return {status:'version-unavailable',revision};
  const snapshot=await loader();
  const actual=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(canonicalJSON(snapshot.core)))),byte=>byte.toString(16).padStart(2,'0')).join('');
  if(snapshot.schema!=='tos_bonsai_research_snapshot_v1'||snapshot.revision!==revision||actual!==revision)throw new Error('Research snapshot digest does not match.');
  return {status:'available',...snapshot};
}

/** A local demo reader manifest. Its checks verify bindings, not legal conclusions. */
const isRecord=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const require=(ok,message)=>{if(!ok)throw Error(message);};
const bi=(value,name)=>require(isRecord(value)&&['ru','en'].every(code=>typeof value[code]==='string'&&value[code].trim()),`${name} must have Russian and English wording`);
const web=value=>{try{return ['https:','http:'].includes(new URL(value).protocol);}catch{return false;}};
export const fragmentText=version=>version.paragraphs.join('\n\n');
export async function textDigest(text){const raw=await globalThis.crypto.subtle.digest('SHA-256',new TextEncoder().encode(text));return [...new Uint8Array(raw)].map(n=>n.toString(16).padStart(2,'0')).join('');}

export async function bindFragmentCatalog(data,materialIds){
 require(isRecord(data)&&data.schema==='tos_demo_fragments_v1','Unsupported fragment catalog');
 require(data.audience==='local-reading-and-recorded-video','Fragment catalog audience is not the recorded demo');
 require(Array.isArray(data.passages)&&data.passages.length<=100,'Invalid fragment collection');
 require(Array.isArray(data.bindings)&&data.bindings.length<=300,'Invalid fragment bindings');
 const passages=new Map(),bindings=new Map(),known=new Set(materialIds);
 for(const passage of data.passages){
  require(isRecord(passage)&&typeof passage.id==='string'&&passage.id.length<150&&!passages.has(passage.id),'Duplicate or invalid passage');
  for(const field of ['title','work','author','locator'])bi(passage[field],`Passage ${field}`);
  require(['available','link-only'].includes(passage.status),'Unknown passage status');
  if(passage.status==='available'){
   require(passage.complete===true,'Only a complete declared source unit can be displayed');
   bi(passage.boundary,'Source unit boundary');
   require(isRecord(passage.versions),'Missing bilingual versions');
   for(const code of ['ru','en']){
    const version=passage.versions[code];require(isRecord(version),`Missing ${code} version`);
    require(Array.isArray(version.paragraphs)&&version.paragraphs.length>0&&version.paragraphs.length<=1000&&version.paragraphs.every(p=>typeof p==='string'&&p.trim()),'A fragment must contain complete nonempty paragraphs');
    const text=fragmentText(version);require(text.length<=150_000,'Fragment is too large');
    require(await textDigest(text)===version.textSha256,'Fragment text digest mismatch');
    bi(version.translator,'Translator');bi(version.edition,'Edition');bi(version.editorialNote,'Text preparation');
    require(web(version.sourceUrl),'Missing source URL');
    require(typeof version.sourceRevision==='string'&&version.sourceRevision.trim(),'Missing exact source revision');
    const rights=version.rights;
    require(isRecord(rights)&&rights.uses?.includes('local-reading')&&rights.uses?.includes('video-display'),'Version does not carry a reviewed video-display basis');
    bi(rights.basis,'Rights basis');bi(rights.credit,'Attribution');
    require(typeof rights.label==='string'&&rights.label.trim()&&web(rights.url),'Missing rights reference');
    require(Array.isArray(rights.jurisdictions)&&rights.jurisdictions.length>0,'Missing reviewed rights scope');
   }
  }else{
   require(!passage.versions,'A link-only passage cannot carry hidden protected text');
   bi(passage.reason,'Unavailable passage explanation');
   require(Array.isArray(passage.links)&&passage.links.length>0&&passage.links.every(link=>typeof link.label==='string'&&web(link.url)),'Missing lawful source links');
  }
  passages.set(passage.id,passage);
 }
 for(const binding of data.bindings){
  require(isRecord(binding)&&known.has(binding.nodeId)&&!bindings.has(binding.nodeId),'Unknown or duplicate material binding');
  require(Array.isArray(binding.passageIds)&&binding.passageIds.length>0&&new Set(binding.passageIds).size===binding.passageIds.length&&binding.passageIds.every(id=>passages.has(id)),'Unresolved passage binding');
  bi(binding.context,'Passage context');bindings.set(binding.nodeId,binding);
 }
 return {data,passages,bindings,forMaterial:id=>{const binding=bindings.get(id);return binding?{...binding,passages:binding.passageIds.map(id=>passages.get(id))}:null;}};
}

// Alignment offsets belong to one exact text layer. A bounded read either
// reaches both requested endpoints in that layer or returns no usable fragment.
export async function readCorpusFragmentRange(provider,work,version,first,last=first){
  const revision=version?.revision,versionId=version?.versionId;
  if(typeof revision!=='string'||!revision||!versionId||!first||!last)throw new Error('A corpus fragment requires an exact version and endpoints');
  const found=[],seenUnits=new Set(),seenCursors=new Set();
  let cursor=null;
  for(let round=0;round<4;round++){
    const args={documentId:work.documentId,versionId,revision,limit:8,...(cursor?{cursor}:{unitId:first})};
    const page=await provider.window(args);
    if(page.revision!==revision||page.documentId!==work.documentId||page.versionId!==versionId)throw new Error('The corpus fragment text layer changed');
    if(!Array.isArray(page.units)||!page.units.length||page.units.length>8)throw new Error('Incomplete corpus fragment window');
    for(const unit of page.units){
      if(!found.length&&unit.id!==first)throw new Error('The corpus fragment start was not returned');
      if(seenUnits.has(unit.id))throw new Error('Repeated corpus fragment unit');
      seenUnits.add(unit.id);found.push(unit);
      if(unit.id===last)return found;
    }
    cursor=page.nextCursor;
    if(!cursor||seenCursors.has(cursor))throw new Error('Incomplete corpus fragment range');
    seenCursors.add(cursor);
  }
  throw new Error('Corpus fragment range exceeds the bounded window budget');
}

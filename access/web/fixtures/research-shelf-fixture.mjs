import {createResearchShelfStore} from '../src/research-shelf/storage.mjs';

const output=document.querySelector('#research-shelf-proof');
const revision='a'.repeat(64),content='b'.repeat(64);
const record=(id,title)=>({id,title,type:'material',target:{kind:'node',id:`fixture:node:${id}`,sourceRevision:revision,contentRevision:content},collectionIds:[]});

async function run(){
  const suffix=`${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
  const dbName=`tos-research-shelf-browser-proof-${suffix}`;
  const first=createResearchShelfStore({dbName,indexedDB:window.indexedDB});
  const second=createResearchShelfStore({dbName,indexedDB:window.indexedDB});
  let reopened;
  try{
    await Promise.all([first.ready(),second.ready()]);
    const created=await first.save(record('cas','CAS initial'),{expectedRevision:null});
    const secondView=await second.get('cas');
    const changed=await second.save({...secondView,title:'CAS second edit'},{expectedRevision:secondView.revision});
    let casConflict=null;
    try{await first.save({...created.item,title:'CAS stale edit'},{expectedRevision:created.item.revision});}
    catch(error){casConflict=error?.code||'unknown';}

    for(const id of ['page-a','page-b','page-c'])await first.save(record(id,`Page ${id}`),{expectedRevision:null});
    const page=await second.list({limit:1});
    if(!page.nextCursor)throw new Error('The synthetic page did not produce a cursor.');
    await first.save(record('cursor-new','Cursor invalidation'),{expectedRevision:null});
    let cursorConflict=null;
    try{await second.list({limit:1,cursor:page.nextCursor});}
    catch(error){cursorConflict=error?.code||'unknown';}

    const incoming=createResearchShelfStore({adapter:'memory'});
    await incoming.save(record('atomic-new','Atomic new'),{expectedRevision:null});
    const packet=await incoming.export();
    packet.records.push({...packet.records[0],id:'cas',title:'Atomic conflict'});
    let importConflict=null;
    try{await first.import(packet);}catch(error){importConflict=error?.code||'unknown';}
    const afterAtomic=await first.get('atomic-new');
    await incoming.close();
    await first.close();await second.close();
    reopened=createResearchShelfStore({dbName,indexedDB:window.indexedDB});
    const reopenedStatus=await reopened.ready();
    const reopenedCas=await reopened.get('cas');
    const reopenedPage=await reopened.list({limit:100});
    const proof={schema:'tos.research_shelf.browser_proof.v1',tests:{
      twoConnectionCas:{ok:casConflict==='conflict'&&changed.item.revision===2,error:casConflict},
      cursorInvalidation:{ok:cursorConflict==='invalid-cursor',error:cursorConflict},
      atomicImport:{ok:importConflict==='conflict'&&afterAtomic===null,error:importConflict},
      reopen:{ok:reopenedStatus.adapter==='indexeddb'&&reopenedCas?.title==='CAS second edit',adapter:reopenedStatus.adapter,recordCount:reopenedPage.items.length},
    }};
    if(Object.values(proof.tests).some(test=>!test.ok))throw Object.assign(new Error('A browser proof assertion failed.'),{proof});
    return proof;
  }finally{
    await reopened?.close().catch(()=>{});
    await first.close().catch(()=>{});await second.close().catch(()=>{});
  }
}

run().then(proof=>{
  window.researchShelfProof=proof;output.dataset.state='passed';output.textContent=JSON.stringify(proof);
}).catch(error=>{
  const proof=error?.proof||{schema:'tos.research_shelf.browser_proof.v1',tests:{ok:false},error:error?.message||String(error)};
  window.researchShelfProof=proof;output.dataset.state='failed';output.textContent=JSON.stringify(proof);
});

// Browser checks for the shared reader and public research components.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {readFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {describe,test} from 'node:test';

const HERE=path.dirname(fileURLToPath(import.meta.url));
const ROOT=path.resolve(HERE,'../..');
const WEB=path.join(ROOT,'access/web');
const playwrightRoot=process.env.TOS_E2E_NODE_MODULES?path.dirname(process.env.TOS_E2E_NODE_MODULES):ROOT;
const globalRequire=createRequire(path.join(playwrightRoot,'package.json'));
const {chromium}=globalRequire('playwright');
const {createServer}=createRequire(path.join(WEB,'package.json'))('vite');
// Poll from the Node host: waitForFunction evaluates its predicate inside the
// page timer and conflicts with the installed host's strict CSP.
async function waitFor(page,expression,timeout=30000,arg=null){
  const until=performance.now()+timeout;
  do{
    let timer;
    const matched=await Promise.race([
      page.evaluate(expression,arg),
      new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error(`Browser condition did not settle within ${timeout}ms`)),Math.max(1,until-performance.now()));}),
    ]).finally(()=>clearTimeout(timer));
    if(matched)return;
    await new Promise(resolve=>setTimeout(resolve,100));
  }while(performance.now()<until);
  throw new Error(`Browser condition did not settle within ${timeout}ms`);
}
async function startVite(){
  const server=await createServer({configFile:path.join(WEB,'vite.config.ts'),root:WEB,logLevel:'silent',server:{host:'127.0.0.1',port:0,strictPort:false}});
  await server.listen();const address=server.httpServer.address();
  return {base:`http://127.0.0.1:${address.port}`,close:()=>server.close()};
}
async function withVite(fn){const host=await startVite();try{await fn(host.base);}finally{await host.close();}}
async function idbReadings(page,dbName,store='readings'){
  return page.evaluate(async([name,storeName])=>{const db=await new Promise((resolve,reject)=>{const req=indexedDB.open(name);req.onsuccess=()=>resolve(req.result);req.onerror=()=>reject(req.error);});try{return await new Promise((resolve,reject)=>{const req=db.transaction(storeName,'readonly').objectStore(storeName).getAll();req.onsuccess=()=>resolve(req.result);req.onerror=()=>reject(req.error);});}finally{db.close();}},[dbName,store]);
}
async function waitForReading(page,dbName,predicate,store='readings',timeout=15000){const until=Date.now()+timeout;while(Date.now()<until){const items=await idbReadings(page,dbName,store);if(items.some(predicate))return items;await page.waitForTimeout(100);}throw new Error(`IndexedDB reading did not settle for ${dbName}`);}
async function downloadJson(page,button){const [download]=await Promise.all([page.waitForEvent('download'),button.click()]);return JSON.parse(await readFile(await download.path(),'utf8'));}

// Each case owns its browser context and Vite host.
describe('static reader browser behavior',()=>{

// 15. Pagehide and interrupted IndexedDB writes recover exactly one draft.
test('test_corpus_note_exit_recovers_exact_draft',async()=>withVite(async base=>{
  const url=base+'/__research/fixtures/corpus-reader.html';const browser=await chromium.launch({headless:true,args:['--no-sandbox'],...(process.env.TOS_E2E_CHROMIUM?{executablePath:process.env.TOS_E2E_CHROMIUM}:{})});
  try{for(const interrupted of [false,true]){const context=await browser.newContext({locale:'ru-RU'});const page=await context.newPage();page.on('dialog',d=>d.accept());await page.goto(url,{waitUntil:'domcontentloaded'});await page.waitForSelector('#tree[data-fixture-ready="true"]');await page.locator('[data-corpus-open]').click();await page.locator('.cr-unit:not(.cr-heading) .cr-unit-marker').first().click();await page.locator('.cr-note-editor').waitFor();const unitId=await page.locator('.cr-unit:not(.cr-heading)').first().getAttribute('data-unit-id');const value='Незавершённая заметка 😀 α — '+interrupted;
    if(interrupted){const nav=page.waitForNavigation({waitUntil:'domcontentloaded'});await page.evaluate(text=>{const transaction=IDBDatabase.prototype.transaction;IDBDatabase.prototype.transaction=function(names,mode,...rest){if(mode==='readwrite')throw new DOMException('Interrupted test write','AbortError');return transaction.call(this,names,mode,...rest);};const editor=document.querySelector('.cr-note-editor');editor.value=text;editor.dispatchEvent(new Event('input',{bubbles:true}));location.reload();},value);await nav;}
    else await page.evaluate(text=>{const editor=document.querySelector('.cr-note-editor');editor.value=text;editor.dispatchEvent(new Event('input',{bubbles:true}));window.dispatchEvent(new PageTransitionEvent('pagehide',{persisted:false}));},value);
    await waitFor(page,async text=>{const db=await new Promise((resolve,reject)=>{const r=indexedDB.open('tos-corpus-reader-fixture-v1');r.onsuccess=()=>resolve(r.result);r.onerror=()=>reject(r.error);});try{return await new Promise((resolve,reject)=>{const r=db.transaction('notes','readonly').objectStore('notes').getAll();r.onsuccess=()=>resolve(r.result.some(n=>n.text===text));r.onerror=()=>reject(r.error);});}finally{db.close();}},15000,value);
    const notes=await idbReadings(page,'tos-corpus-reader-fixture-v1','notes');assert.equal(notes.length,1);assert.equal(notes[0].reference.unitId,unitId);if(interrupted){assert.ok(notes[0].id.startsWith('recovered-'));await page.reload({waitUntil:'domcontentloaded'});await page.waitForSelector('#tree[data-fixture-ready="true"]');assert.equal((await idbReadings(page,'tos-corpus-reader-fixture-v1','notes')).length,1);assert.equal(await page.evaluate("sessionStorage.getItem('tos.corpus.note-draft.v1:tos-corpus-reader-fixture-v1')"),null);}await context.close();}
  }finally{await browser.close();}
}));

// 16. Two-connection collection deletion remains atomic.
test('test_research_shelf_collection_delete_is_atomic',async()=>withVite(async base=>{
  const browser=await chromium.launch({headless:true,args:['--no-sandbox'],...(process.env.TOS_E2E_CHROMIUM?{executablePath:process.env.TOS_E2E_CHROMIUM}:{})});try{const page=await browser.newPage({locale:'ru-RU'});await page.goto(base+'/__research/fixtures/research-shelf.html',{waitUntil:'domcontentloaded'});const proof=page.locator('#research-shelf-proof[data-state]');await proof.waitFor({state:'visible'});const value=JSON.parse(await proof.innerText());assert.equal(await proof.getAttribute('data-state'),'passed',JSON.stringify(value));assert.ok(value.tests.collectionDetach.ok);assert.ok(value.tests.collectionDeleteRollback.ok);assert.ok(Object.values(value.tests).every(item=>item.ok));}finally{await browser.close();}
}));

// 18. Native delivery export preserves identity and expires with its owner.
test('test_native_delivery_export_preserves_identity_and_respects_expiry',async()=>withVite(async base=>{
  const browser=await chromium.launch({headless:true,args:['--no-sandbox'],...(process.env.TOS_E2E_CHROMIUM?{executablePath:process.env.TOS_E2E_CHROMIUM}:{})});try{const page=await browser.newPage({locale:'ru-RU'});await page.goto(base+'/__research/fixtures/native-reader.html');await page.getByRole('button',{name:'Открыть текст',exact:true}).click();let reader=page.locator('.native-reader[data-native-state="available"]');await reader.waitFor();await reader.getByText('Об источнике',{exact:true}).click();const exported=await downloadJson(page,reader.getByRole('button',{name:'Скачать данные',exact:true}));assert.equal(Object.hasOwn(exported,'handle'),false);assert.equal(exported.record_ref.id,'tos.text-unit.fixture');assert.equal(exported.native_unit.packet.version,1);assert.ok(exported.native_unit.spans[0].text.startsWith('A😀é'));assert.equal(exported.text_access.scope,'public-native-unit');assert.equal(exported.selection.id,'source-claims:identity:tos.text-unit.fixture');
  await page.evaluate(()=>{window.oldExport=document.querySelector('.nr-exact .sc-data-download');window.exportAttempts=0;URL.createObjectURL=()=>{window.exportAttempts++;return 'blob:blocked';};});await reader.getByRole('button',{name:'К Древу',exact:true}).click();await page.locator('.native-reader').waitFor({state:'hidden'});assert.equal(await page.evaluate('oldExport.click();exportAttempts'),0);await page.getByRole('button',{name:'Локальные условия',exact:true}).click();await page.clock.install();await page.getByRole('button',{name:'Открыть текст',exact:true}).click();reader=page.locator('.native-reader[data-native-state="available"]');await reader.waitFor();assert.equal(await page.evaluate('oldExport.click();exportAttempts'),0);await page.evaluate(()=>window.localExport=document.querySelector('.nr-exact .sc-data-download'));const now=await page.evaluate('Date.now()');await page.clock.setFixedTime(now+91000);assert.equal(await page.evaluate('localExport.click();exportAttempts'),0);await page.clock.fastForward(91000);await page.locator('.native-reader[data-native-state="expired"]').waitFor();assert.equal(await page.locator('.nr-exact .sc-data-download').count(),0);assert.equal(await page.evaluate('localExport.click();exportAttempts'),0);
  }finally{await browser.close();}
}));

// 19. pagehide flush preserves corpus and native reader position before debounce.
// Corpus positions refer to the first row below the measured sticky location
// header, whose height follows reader typography, rather than a fixed pixel band.
test('test_reader_positions_survive_pagehide_before_debounce',async()=>withVite(async base=>{
  const corpusUrl=base+'/__research/fixtures/corpus-reader.html',nativeUrl=base+'/__research/fixtures/native-reader.html';const browser=await chromium.launch({headless:true,args:['--no-sandbox'],...(process.env.TOS_E2E_CHROMIUM?{executablePath:process.env.TOS_E2E_CHROMIUM}:{})});try{const context=await browser.newContext({locale:'ru-RU'});const corpus=await context.newPage();await corpus.goto(corpusUrl,{waitUntil:'domcontentloaded'});await corpus.waitForSelector('#tree[data-fixture-ready="true"]');await corpus.locator('[data-corpus-open]').click();await corpus.locator('.cr-pane').waitFor();const pos=await corpus.evaluate(()=>{const pane=document.querySelector('.cr-pane'),maximum=pane.scrollHeight-pane.clientHeight,requested=Math.min(maximum,Math.max(640,Math.floor(maximum*.45)));pane.scrollTop=requested;pane.dispatchEvent(new Event('scroll'));const rect=pane.getBoundingClientRect(),boundary=Math.max(rect.top,pane.querySelector('.cr-current-location')?.getBoundingClientRect().bottom??rect.top)+2,row=[...pane.querySelectorAll('.cr-unit')].find(item=>item.getBoundingClientRect().bottom>boundary);window.dispatchEvent(new PageTransitionEvent('pagehide',{persisted:false}));history.replaceState(history.state,'',location.pathname+location.search);return {top:pane.scrollTop,unitId:row?.dataset.unitId,versionId:pane.dataset.versionId};});assert.ok(pos.top>0);assert.ok(pos.unitId);const readings=await waitForReading(corpus,'tos-corpus-reader-fixture-v1',item=>item.reference?.unitId===pos.unitId);const saved=readings.find(item=>item.reference?.unitId===pos.unitId);assert.equal(saved.versionId,pos.versionId);assert.ok(saved.reference.target.workId);
  await corpus.reload({waitUntil:'domcontentloaded'});await corpus.waitForSelector('#tree[data-fixture-ready="true"]');await corpus.locator('[data-corpus-open]').click();await corpus.locator('.cr-pane').waitFor();await waitFor(corpus,unitId=>{const pane=document.querySelector('.cr-pane'),row=pane?.querySelector(`.cr-unit[data-unit-id="${CSS.escape(unitId)}"]`);if(!row||!pane)return false;const pr=pane.getBoundingClientRect(),rr=row.getBoundingClientRect(),boundary=Math.max(pr.top,pane.querySelector('.cr-current-location')?.getBoundingClientRect().bottom??pr.top)+2;return rr.bottom>boundary&&rr.top>=boundary-1&&rr.top<=boundary+2;},30000,pos.unitId);
  const native=await context.newPage();await native.goto(nativeUrl,{waitUntil:'domcontentloaded'});await native.getByRole('button',{name:'Открыть текст',exact:true}).click();await native.locator('.native-reader:not([hidden])').waitFor();await native.locator('.native-reader[data-native-state="available"]').waitFor();await native.locator('.nr-article').waitFor();const nativePos=await native.evaluate(()=>{const article=document.querySelector('.nr-article');article.scrollTop=Math.min(420,article.scrollHeight-article.clientHeight);article.dispatchEvent(new Event('scroll'));window.dispatchEvent(new PageTransitionEvent('pagehide',{persisted:false}));return {top:article.scrollTop};});assert.ok(nativePos.top>0);const nativeReads=await waitForReading(native,'tos-native-reader-validation-v1',item=>item.offset===nativePos.top);const nativeSaved=nativeReads.find(item=>item.offset===nativePos.top);assert.equal(nativeSaved.reference.schemaVersion,'tos.corpus.reader.native-reference.v1');await native.evaluate("history.replaceState(history.state,'',location.pathname+location.search)");await native.reload({waitUntil:'domcontentloaded'});await native.getByRole('button',{name:'Открыть текст',exact:true}).click();await native.locator('.native-reader[data-native-state="available"]').waitFor();await waitFor(native,expected=>Math.abs(document.querySelector('.nr-article').scrollTop-expected)<1,30000,nativePos.top);await context.close();
  }finally{await browser.close();}
}));

// 20. Real chapter imports cannot reopen a closed reader or replace newer navigation.
test('test_fragment_reader_close_navigation_and_disposal', {timeout:90000}, async()=>withVite(async base=>{
  const browser=await chromium.launch({headless:true,args:['--no-sandbox'],...(process.env.TOS_E2E_CHROMIUM?{executablePath:process.env.TOS_E2E_CHROMIUM}:{})});
  const releases=[],diagnostics=[];
  const record=value=>{if(diagnostics.length<20)diagnostics.push(String(value).slice(0,1000));};
  try{
    const page=await browser.newPage({locale:'en-US'});
    page.on('pageerror',error=>record(error.message));
    page.on('console',message=>{if(message.type()==='error')record(message.text());});
    page.on('response',response=>{if(response.status()>=400)record(`${response.status()} ${new URL(response.url()).pathname}`);});
    await page.route('**/fragment-reader-check',route=>route.fulfill({status:200,contentType:'text/html',body:'<!doctype html><html lang="en"><head><link rel="stylesheet" href="/__research/src/public-research/public-space.css"></head><body></body></html>'}));
    await page.goto(base+'/fragment-reader-check',{waitUntil:'domcontentloaded'});
    await page.evaluate(async()=>{
      const {createFragments}=await import('/__research/src/public-research/fragments.mjs');
      window.fragmentReader=createFragments({locale:()=> 'en',t:key=>key});
      await window.fragmentReader.showChapter('I.01');
    });
    const dialog=page.locator('.ps-parallel');await dialog.waitFor({state:'visible'});
    await page.keyboard.press('Escape');await dialog.waitFor({state:'hidden'});
    assert.equal(await dialog.evaluate(node=>getComputedStyle(node).display),'none');
    await page.evaluate(()=>window.fragmentReader.showChapter('I.01'));
    const hold=async key=>{
      let release;const gate=new Promise(resolve=>{release=resolve;});releases.push(release);
      const pattern=new RegExp('/witnesses/'+key.replaceAll('.','\\.')+'\\.json(?:\\?|$)');
      await page.route(pattern,async route=>{await gate;await route.continue();});
      const requested=page.waitForRequest(pattern);
      await page.evaluate(value=>{window.fragmentPending=window.fragmentReader.showChapter(value);},key);
      await requested;return release;
    };
    const releaseClosed=await hold('I.02');
    await dialog.getByRole('button',{name:'close',exact:true}).click();await dialog.waitFor({state:'hidden'});
    releaseClosed();await page.evaluate(()=>window.fragmentPending);
    assert.equal(await dialog.isVisible(),false);assert.equal(await dialog.evaluate(node=>node.open),false);
    const releaseOld=await hold('I.03');
    await page.evaluate(()=>window.fragmentReader.showChapter('I.04'));await dialog.waitFor({state:'visible'});
    releaseOld();await page.evaluate(()=>window.fragmentPending);
    assert.equal(await page.evaluate(()=>window.fragmentReader.reader.current.key),'I.04');
    const releaseDisposed=await hold('I.05');
    await page.evaluate(()=>window.fragmentReader.dispose());releaseDisposed();await page.evaluate(()=>window.fragmentPending);
    assert.equal(await page.locator('.ps-parallel').count(),0);
    await page.evaluate(async()=>{
      const {createFragments}=await import('/__research/src/public-research/fragments.mjs');
      const host=document.createElement('section');host.id='source-question';document.body.append(host);
      // Keep the citation-only question case even when the stand's authored
      // question changes: this is the original witness location used here.
      const question={cite:[{w:'also-sprach-zarathustra',c:'I.02',u:['tos.text-unit.sid-f9c4ff1773a49108cc92e6a5d92d12e9','tos.text-unit.sid-f9c4ff1773a49108cc92e6a5d92d12e9'],s:86}]};
      createFragments({locale:()=> 'en',t:key=>key}).render(host,question);
    });
    await page.locator('#source-question .ps-fragment:not([aria-busy]) .ps-fragment-text').waitFor({state:'visible'});
    assert.ok((await page.locator('#source-question .ps-fragment-text').innerText()).trim());
  }catch(error){error.message+=`\nFragment browser diagnostics: ${JSON.stringify(diagnostics)}`;throw error;}
  finally{for(const release of releases)release();await browser.close();}
}));

});

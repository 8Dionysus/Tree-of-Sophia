import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect, it} from 'vitest';
import {createBrowserResearchWorkspace} from '../research-workspace-rust.ts';
import {createCorpusNotebook} from '../corpus-reader/notebook.mjs';
import {createReference} from '../corpus-reader/model.mjs';
import {createResearchShelfStore} from '../research-shelf/storage.mjs';
import {PUBLIC_WORKSPACE_SCHEMA, PUBLIC_WORKSPACE_VERSION, changeResearchHistory, importAvailability, planNotebookImport, savePublicThought, validateAggregate, workspaceEmpty} from './workspace-view.mjs';

it('does not treat a workspace with only undo history as empty during import', () => {
  const workspace=createBrowserResearchWorkspace({persistence:false});
  try {
    expect(workspaceEmpty(JSON.parse(workspace.exportPacket()))).toBe(true);
    workspace.addHypothesis({id:'history-only',title:'Question',body:'A synthetic question'});
    expect(workspace.undo()).toBe(true);
    const packet=JSON.parse(workspace.exportPacket());
    expect(packet.hypotheses).toHaveLength(0);
    expect(packet.journal.length).toBeGreaterThan(0);
    expect(workspaceEmpty(packet)).toBe(false);
  } finally {workspace.dispose();}
});

it('does not offer an import for equal and conflicting records when there are no additions', () => {
  const workspace={hypotheses:[],proposals:[],notes:[],journal:[]}, shelf={records:[{id:'same'}],collections:[]};
  const current={workspace,shelf}, incoming={workspace,shelf};
  const counts={notes:{added:0},readings:{added:0},preferences:{added:0}};
  const assessment={current,incoming,conflicts:{workspace:[],shelf:[]},notebookPlan:{counts},graph:{addable:0}};
  expect(importAvailability(assessment).total).toBe(0);
  expect(importAvailability({...assessment,notebookPlan:{counts:{...counts,notes:{added:1}}}}).notebook).toBe(1);
  expect(importAvailability({...assessment,incoming:{...incoming,shelf:{records:[{id:'same'},{id:'new'}],collections:[]}}}).shelf).toBe(1);
  expect(importAvailability({...assessment,conflicts:{workspace:[],shelf:['conflict']},incoming:{...incoming,shelf:{records:[{id:'new'}],collections:[]}}}).shelf).toBe(0);
});

it('still validates the original v1 complete-research packet without graphSpace', async () => {
  const workspace = createBrowserResearchWorkspace({persistence: false});
  const notebook = createCorpusNotebook({adapter: 'memory'});
  const shelf = createResearchShelfStore({adapter: 'memory'});
  try {
    const legacy = {schema: PUBLIC_WORKSPACE_SCHEMA, version: PUBLIC_WORKSPACE_VERSION, exportedAt: new Date().toISOString(),
      workspace: JSON.parse(workspace.exportPacket()), notebook: await notebook.exportPacket(), shelf: await shelf.exportPacket()};
    const checked = await validateAggregate(legacy);
    expect(checked).not.toHaveProperty('graphSpace');
    expect(checked.schema).toBe(PUBLIC_WORKSPACE_SCHEMA);
  } finally {
    workspace.dispose(); await notebook.close(); await shelf.close();
  }
});

it('keeps conflicts while importing independent notebook records', async () => {
  const current = createCorpusNotebook({adapter: 'memory'}), incoming = createCorpusNotebook({adapter: 'memory'});
  const reference = createReference({source: {workId:'work-1',expressionId:'expression-1',editionId:'edition-1',itemId:'item-1',
    fileId:'file-1',fileSha256:'a'.repeat(64),textLayerRef:'fixture/source.txt',textLayerSha256:'b'.repeat(64)},
    versionId:'version-1',unitId:'unit-1',start:0,end:4});
  try {
    await current.putNote({id:'same',kind:'note',reference,text:'keep current'});
    await incoming.putNote({id:'same',kind:'note',reference,text:'different incoming'});
    await incoming.putNote({id:'new',kind:'note',reference,text:'independent incoming'});
    await current.setPreference('question','current question');
    await incoming.setPreference('question','incoming question');
    await incoming.setPreference('separate','incoming setting');
    const plan = planNotebookImport(await current.exportPacket(), await incoming.exportPacket());
    expect(plan.counts.notes).toMatchObject({added:1,skipped:1});
    expect(plan.counts.preferences).toMatchObject({added:1,skipped:1});
    await current.importPacket(plan.packet);
    expect((await current.getNote('same')).text).toBe('keep current');
    expect((await current.getNote('new')).text).toBe('independent incoming');
    expect(await current.getPreference('question')).toBe('current question');
    expect(await current.getPreference('separate')).toBe('incoming setting');
  } finally { await current.close(); await incoming.close(); }
});

it('validates a complete packet both with and without an owner-valid lens', async () => {
  const workspace = createBrowserResearchWorkspace({persistence:false});
  const notebook = createCorpusNotebook({adapter:'memory'}), shelf = createResearchShelfStore({adapter:'memory'});
  try {
    const base = {schema:PUBLIC_WORKSPACE_SCHEMA,version:PUBLIC_WORKSPACE_VERSION,exportedAt:new Date().toISOString(),
      workspace:JSON.parse(workspace.exportPacket()),notebook:await notebook.exportPacket(),shelf:await shelf.exportPacket()};
    expect((await validateAggregate(base)).shelf.records).toHaveLength(0);
    await shelf.save({id:'lens-control',title:'Synthetic lens',type:'lens',target:{draft:{v:2,name:'Synthetic lens',scope:'all',
      sources:['knowledge'],nodeIds:[],focusId:null,query:'',kinds:[],predicates:[],depth:0,direction:'either',profile:'overview',
      limit:1,relations:false,conditions:{nodes:[],relations:[]}}},collectionIds:[]});
    const withLens = {...base,shelf:await shelf.exportPacket()};
    const checked = await validateAggregate(withLens);
    expect(checked.shelf.records).toHaveLength(1);
    expect(checked.shelf.records[0].target.draft).toEqual(withLens.shelf.records[0].target.draft);
  } finally { workspace.dispose(); await notebook.close(); await shelf.close(); }
});

if (process.env.TOS_LEGACY_RESEARCH_FILE) it('keeps historical note identities, question, graph and shelf on aggregate validation', async () => {
  const {readFile}=await import('node:fs/promises');
  const original=JSON.parse(await readFile(process.env.TOS_LEGACY_RESEARCH_FILE,'utf8'));
  const checked=await validateAggregate(original);
  const byId=items=>new Map(items.map(item=>[item.id,item]));
  const notes=byId(checked.notebook.notes), shelf=byId(checked.shelf.records);
  for (const item of original.notebook.notes) expect(notes.get(item.id)).toEqual(item);
  for (const item of original.shelf.records) expect(shelf.get(item.id)).toEqual(item);
  const question=packet=>packet.notebook.preferences.find(item=>item.key==='tos.public-research.question.v1')?.value;
  expect(question(checked)).toEqual(question(original));
  expect(checked.graphSpace).toEqual(original.graphSpace);
  expect(checked.workspace.hypotheses).toEqual(original.workspace.hypotheses);
  expect(checked.workspace.proposals).toEqual(original.workspace.proposals);
});

if (process.env.TOS_LENS_WITH && process.env.TOS_LENS_WITHOUT) it('validates the historical owner-valid lens pair without changing the lens record', async () => {
  const {readFile}=await import('node:fs/promises');
  const withLens=JSON.parse(await readFile(process.env.TOS_LENS_WITH,'utf8'));
  const withoutLens=JSON.parse(await readFile(process.env.TOS_LENS_WITHOUT,'utf8'));
  const checkedWith=await validateAggregate(withLens), checkedWithout=await validateAggregate(withoutLens);
  expect(checkedWith.shelf.records).toEqual(withLens.shelf.records);
  expect(checkedWithout.shelf.records).toEqual(withoutLens.shelf.records);
});

it('undoes and redoes a newly saved thought together while keeping source notes', async () => {
  const workspace=createBrowserResearchWorkspace({persistence:false}),notebook=createCorpusNotebook({adapter:'memory'}),shelf=createResearchShelfStore({adapter:'memory'});
  const reference = unit => createReference({source:{workId:'work-1',expressionId:'expression-1',editionId:'edition-1',itemId:'item-1',
    fileId:'file-1',fileSha256:'a'.repeat(64),textLayerRef:'fixture/source.txt',textLayerSha256:'b'.repeat(64)},versionId:'version-1',unitId:unit,start:0,end:4});
  const grounds=[{reference:reference('unit-1'),title:'First source',quote:'one'}, {reference:reference('unit-2'),title:'Second source',quote:'two'}];
  try {
    await savePublicThought({thesis:'A thesis',grounds,objection:'A counterargument',objectionSources:[grounds[0]],revision:'A reply',
      api:{getQuestion:()=> 'A question',provider:{resolve:async ({reference})=>({status:'exact',reference})}},workspace,shelf,notebook,
      labels:()=>({pick:'Choose grounds',open:'Open',thesis:'Thesis',objection:'Objection',revision:'Reply'})});
    const saved=workspace.getState(),notes=(await notebook.exportPacket()).notes;
    expect(saved.hypotheses).toHaveLength(3); expect(saved.proposals).toHaveLength(3);
    expect(saved.hypotheses.map(item=>item.kind)).toEqual(['interpretation','interpretation','interpretation']);
    // Existing API callers that omit revisionSources retain the old fallback.
    expect(saved.proposals[2].sourceRefs).toEqual(saved.proposals[1].sourceRefs);
    const undone=changeResearchHistory(workspace,'undo',{state:'applied',revision:workspace.summary().revision,actions:6});
    expect(undone.grouped).toBe(true); expect(workspace.getState().hypotheses).toHaveLength(0); expect(workspace.getState().proposals).toHaveLength(0);
    expect((await notebook.exportPacket()).notes).toEqual(notes);
    const redone=changeResearchHistory(workspace,'redo',undone.group);
    expect(redone.grouped).toBe(true); expect(workspace.getState().hypotheses).toEqual(saved.hypotheses); expect(workspace.getState().proposals).toEqual(saved.proposals);
    expect((await notebook.exportPacket()).notes).toEqual(notes);
  } finally {workspace.dispose();await notebook.close();await shelf.close();}
});

it('requires an explicit reply source in the new form path and stores a distinct third source', async () => {
  const workspace=createBrowserResearchWorkspace({persistence:false}),notebook=createCorpusNotebook({adapter:'memory'}),shelf=createResearchShelfStore({adapter:'memory'});
  const reference=unit=>createReference({source:{workId:'work-1',expressionId:'expression-1',editionId:'edition-1',itemId:'item-1',
    fileId:'file-1',fileSha256:'a'.repeat(64),textLayerRef:'fixture/source.txt',textLayerSha256:'b'.repeat(64)},versionId:'version-1',unitId:unit,start:0,end:4});
  const sources=['unit-1','unit-2','unit-3'].map(unit=>({reference:reference(unit),title:unit,quote:'text'}));
  const api={getQuestion:()=> 'A question',provider:{resolve:async ({reference})=>({status:'exact',reference})}};
  const labels=()=>({pick:'Choose grounds',open:'Open',thesis:'Thesis',objection:'Objection',revision:'Reply'});
  const args={thesis:'A thesis',grounds:sources.slice(0,2),objection:'A counterargument',objectionSources:[sources[1]],
    revision:'A reply',api,workspace,shelf,notebook,labels};
  try {
    await expect(savePublicThought({...args,revisionSources:[]})).rejects.toMatchObject({code:'missing-reply-source'});
    expect(workspace.getState().hypotheses).toHaveLength(0);expect((await shelf.exportPacket()).records).toHaveLength(0);expect((await notebook.exportPacket()).notes).toHaveLength(0);
    const changedApi={...api,provider:{resolve:async ({reference})=>({status:reference.unitId==='unit-3'?'changed':'exact',reference})}};
    await expect(savePublicThought({...args,api:changedApi,revisionSources:[sources[2]]})).rejects.toThrow('stale source references');
    expect(workspace.getState().hypotheses).toHaveLength(0);expect((await shelf.exportPacket()).records).toHaveLength(0);
    await savePublicThought({...args,revisionSources:[sources[2]]});
    const proposals=workspace.getState().proposals,objection=proposals[1],reply=proposals[2];
    expect(reply.sourceRefs).toEqual(reply.evidenceRefs);expect(reply.sourceRefs).toHaveLength(1);
    expect(reply.sourceRefs).not.toEqual(objection.sourceRefs);
    const replyRecord=await shelf.get(reply.sourceRefs[0].slice('research-shelf:'.length));
    const objectionRecord=await shelf.get(objection.sourceRefs[0].slice('research-shelf:'.length));
    expect(replyRecord.target.reference).toEqual(sources[2].reference);
    expect(objectionRecord.target.reference).toEqual(sources[1].reference);
    const notes=(await notebook.exportPacket()).notes;
    expect(notes).toHaveLength(2);
    for (const note of notes) {
      expect(note.text).toContain('Research summary');
      expect(note.text.split('Objection:')[0]).toContain('unit-1');
      expect(note.text.split('Objection:')[0]).toContain('unit-2');
      expect(note.text.split('Objection:')[1].split('Reply:')[0]).toContain('unit-2');
      expect(note.text.split('Objection:')[1].split('Reply:')[0]).not.toContain('unit-3');
      expect(note.text.split('Reply:')[1]).toContain('unit-3');
    }
  } finally {workspace.dispose();await notebook.close();await shelf.close();}
});

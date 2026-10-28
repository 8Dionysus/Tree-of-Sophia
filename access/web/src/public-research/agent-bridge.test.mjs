import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect, it} from 'vitest';
import {createReference} from '../corpus-reader/model.mjs';
import {selectedSourceContext} from '../corpus-reader/source-context.mjs';
import {AGENT_RESPONSE_SCHEMA, AGENT_TASK_SCHEMA, AGENT_TASK_SCHEMA_V2, mergeSourceBasket, parseSavedSourceBasket, responsePacket, selectAgentProposals, taskPacket} from './agent-bridge.mjs';
import {THOUGHT_DRAFT_SCHEMA, validateThoughtDraft} from './workspace-view.mjs';

const instructions = 'Read the exact excerpts and references. Return only the response JSON contract below. Keep claims and objections separate, cite only the supplied source ids, and state uncertainty. Do not claim canon or review.';
const reference = createReference({source:{workId:'w',expressionId:'x',editionId:'e',itemId:'i',fileId:'f',fileSha256:'a'.repeat(64),textLayerRef:'fixture/text.txt',textLayerSha256:'b'.repeat(64)},versionId:'v',unitId:'u',start:1,end:3});
const source = {id:'s1',reference,title:'Fixture',quote:'😀β',context:'α😀βγ',contextTruncated:false,language:'grc',versionLabel:'Edition'};
const base = {taskId:'task-legacy',createdAt:'2026-10-27T00:00:00Z',profile:'antiquity',question:'Question?',workspaceRevision:0,
  sources:[source],instructions,responseContract:{schema:AGENT_RESPONSE_SCHEMA,version:1,fields:[]}};
const v2source = {...source,contextRange:{start:0,end:4,positionUnit:'unicode_code_point',interval:'half_open'},editorialAnnotations:[{kind:'addition',start:1,end:2}]};

it('keeps a v1 task readable and validates v2 context and editorial ranges without changing the exact reference', () => {
  const legacy = taskPacket({...base,schema:AGENT_TASK_SCHEMA,version:1});
  expect(legacy.sources[0].reference).toEqual(reference);
  expect(legacy.sources[0]).not.toHaveProperty('editorialAnnotations');
  const value = {...base,schema:AGENT_TASK_SCHEMA_V2,version:2,sources:[v2source],
    instructions:`${instructions} Context and editorial annotation ranges use Unicode code-point offsets in the referenced unit; annotations are not a complete apparatus.`};
  const checked = taskPacket(value);
  expect(checked.sources[0].quote).toBe('😀β');
  expect(checked.sources[0].reference).toEqual(reference);
  expect(checked.sources[0].editorialAnnotations).toEqual([{kind:'addition',start:1,end:2}]);
  expect(() => taskPacket({...value,sources:[{...v2source,editorialAnnotations:[{kind:'addition',start:1,end:5}]}]})).toThrow(/annotation/);
  expect(() => taskPacket({...value,sources:[{...v2source,quote:'different'}]})).toThrow(/quote/);
  expect(() => taskPacket({...base,schema:AGENT_TASK_SCHEMA,version:1,sources:[v2source]})).toThrow(/shape/);
});

it('can omit a nested objection without changing the received response', () => {
  const response = {proposals:[{id:'p',statement:'Claim',objection:{statement:'Counterargument',sourceRefs:['s1']}}]};
  const original = structuredClone(response);
  const without = selectAgentProposals(response,new Set(['p']),new Set());
  expect(without[0].objection).toBeUndefined();
  expect(selectAgentProposals(response,new Set(['p']),new Set(['p']))[0].objection).toEqual(original.proposals[0].objection);
  expect(response).toEqual(original);
});

it('restores a per-tab basket and merges a draft without changing existing exact sources', () => {
  const restored=parseSavedSourceBasket(JSON.stringify([v2source]));
  expect(restored.sequence).toBe(1);
  expect(restored.sources[0].reference).toEqual(reference);
  expect(restored.sources[0].editorialAnnotations).toEqual(v2source.editorialAnnotations);
  const other={...v2source,reference:{...reference,selector:{...reference.selector,start:2,end:3}},quote:'β'};
  const before=structuredClone(restored.sources);
  const merged=mergeSourceBasket(restored.sources,[other],restored.sequence);
  expect(merged.mapping.get('s1')).toBe('s2');
  expect(merged.sources.map(item=>item.id)).toEqual(['s1','s2']);
  expect(restored.sources).toEqual(before);
  expect(() => mergeSourceBasket(restored.sources,[{...v2source,title:'different'}],restored.sequence)).toThrow(/different context/);
  expect(restored.sources).toEqual(before);
  expect(() => parseSavedSourceBasket(JSON.stringify([v2source,v2source]))).toThrow(/Duplicate/);
});

it('carries a verified editorial addition in the neighboring source context as well as the quote', () => {
  const revision='48d5e7c0cbd364402d2e8d70a3bd1e30eaf1bb9e44c7dd2458d174da743a3652';
  const version={id:'retained-athenian-constitution-grc2-6e0c298a511e',revision,source:{textLayerSha256:revision}};
  const unit={id:'tos.text-unit.sid-6276729d215947f182b03878e4bd2193',text:'[…] κατηγοροῦντος Μύρωνος'};
  const makeSource=(id,start,end,quote) => {
    const ref=createReference({source:{workId:'w',expressionId:'x',editionId:'e',itemId:'i',fileId:'f',fileSha256:'a'.repeat(64),textLayerRef:'fixture/text.txt',textLayerSha256:revision},versionId:version.id,unitId:unit.id,start,end});
    return {id,reference:ref,title:'Ath.Pol. §1',quote,language:'grc',versionLabel:'Edition',...selectedSourceContext({reference:ref,version,unit})};
  };
  const sources=[makeSource('s1',4,17,'κατηγοροῦντος'),makeSource('s2',18,25,'Μύρωνος')];
  const task=taskPacket({...base,schema:AGENT_TASK_SCHEMA_V2,version:2,sources,
    instructions:`${instructions} Context and editorial annotation ranges use Unicode code-point offsets in the referenced unit; annotations are not a complete apparatus.`});
  expect(task.sources.map(item=>item.reference.selector)).toEqual(sources.map(item=>item.reference.selector));
  expect(task.sources[0].editorialAnnotations).toContainEqual({kind:'addition',start:4,end:17});
  expect(task.sources[1].editorialAnnotations).toContainEqual({kind:'addition',start:4,end:17});
  expect(task.sources[1].quote).toBe('Μύρωνος');
});

it('keeps a thought form draft separate and rejects lost or duplicated source selections', () => {
  const draft={schema:THOUGHT_DRAFT_SCHEMA,version:1,exportedAt:'2026-10-27T00:00:00Z',question:'Question?',fields:{thesis:'draft',objection:'',revision:''},
    sources:[v2source],selections:{grounds:['s1'],objection:[],revision:[]}};
  expect(validateThoughtDraft(draft)).toEqual(draft);
  expect(() => validateThoughtDraft({...draft,selections:{...draft.selections,grounds:['missing']}})).toThrow(/selection/);
  expect(() => validateThoughtDraft({...draft,sources:[v2source,v2source]})).toThrow(/source id/);
});

if (process.env.TOS_AGENT_LEGACY_TASK) it('accepts an operator-supplied historical v1 task unchanged', async () => {
  const {readFile} = await import('node:fs/promises');
  const original = JSON.parse(await readFile(process.env.TOS_AGENT_LEGACY_TASK, 'utf8'));
  const checked = taskPacket(original);
  expect(checked.schema).toBe(AGENT_TASK_SCHEMA);
  expect(checked.taskId).toBe(original.taskId);
  expect(checked.sources.map(item => item.reference)).toEqual(original.sources.map(item => item.reference));
  expect(checked.sources.map(item => item.quote)).toEqual(original.sources.map(item => item.quote));
  if (process.env.TOS_AGENT_LEGACY_RESPONSE) {
    const response = JSON.parse(await readFile(process.env.TOS_AGENT_LEGACY_RESPONSE, 'utf8'));
    const checkedResponse = responsePacket(response, checked);
    expect(checkedResponse.taskId).toBe(original.taskId);
    expect(checkedResponse.proposals.length).toBeGreaterThan(0);
  }
});

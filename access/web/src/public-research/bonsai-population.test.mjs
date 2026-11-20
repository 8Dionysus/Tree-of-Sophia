import {execFileSync} from 'node:child_process';
import {existsSync} from 'node:fs';
import {join} from 'node:path';
import {describe,expect,it} from 'vitest';
import {checkBonsai,loadBonsai} from '../../../../bonsai/lib/graft.mjs';
import {semanticRegistry} from '../../../../bonsai/lib/objects.mjs';
import {controlIssues,POPULATION_SCHEMA,populationIssues,populationReviewIssues} from '../../../../bonsai/lib/population.mjs';
import {fold,formHead,formKeys,slugify,strength} from '../../../../bonsai/review/population/keys.mjs';
import {compare} from '../../../../bonsai/review/population/resolve.mjs';

const root=new URL('../../../../',import.meta.url).pathname;
const review=join(root,'bonsai','review','population');
const {modules}=loadBonsai(root,'population'),{population,chronology,identities}=modules;
const registry=semanticRegistry(root);
const issues=module=>populationIssues(module,{chronology,identities,registry});
const changed=change=>{const copy=structuredClone(population);change(copy);return issues(copy);};
const layersFetched=existsSync(join(review,'layer','agents'));
const dated=Object.entries(population.members).find(([,member])=>member.phases.length)[0];
const plain=Object.keys(population.members).find(id=>id!==dated);

describe('the Population bonsai',()=>{
  it('keeps its contract against the Tree\'s registries, its chronology and its graft map',()=>{
    expect(population.schema).toBe(POPULATION_SCHEMA);
    expect(issues(population)).toEqual([]);
    expect(Object.keys(population.members).length).toBeGreaterThan(3000);
    for(const member of Object.values(population.members)){expect(member.core).toBe(true);expect(member.verdict).toBe('undecided');}
  });
  it('grew every member from the register rows keys.jsonl gives it, and keeps its control cases',()=>{
    expect(populationReviewIssues(root,{population})).toEqual([]);
  });
  it('refuses a broken member',()=>{
    expect(changed(copy=>{copy.members['m-NOT']=copy.members[plain];delete copy.members[plain];})).toContain('member m-NOT: id is not m- and base32');
    expect(changed(copy=>{copy.members[plain].kind='spirit';})).toContain(`member ${plain}: unknown kind spirit`);
    expect(changed(copy=>{copy.members[plain].name.en='';})).toContain(`member ${plain}: name needs ru and en`);
    expect(changed(copy=>{copy.members[plain].verdict='yes';}).some(issue=>issue.startsWith(`member ${plain}: verdict yes`))).toBe(true);
    const row=population.members[dated].grew_from[0];
    expect(changed(copy=>{copy.members[plain].grew_from.push(row);}).some(issue=>issue.includes(`${row} already grew`))).toBe(true);
    expect(changed(copy=>{copy.members[plain].phases.push('tos.biographical-phase.nobody.lifetime');})).toContain(`member ${plain}: phase tos.biographical-phase.nobody.lifetime is not in the chronology`);
    expect(changed(copy=>{copy.members[dated].phases=[];}).some(issue=>issue.endsWith('its member does not list it'))).toBe(true);
    expect(changed(copy=>{copy.redirects={'m-aaaaaaaaaa':'m-bbbbbbbbbb'};})).toContain('redirect m-aaaaaaaaaa: leads to m-bbbbbbbbbb, which is not a member');
    expect(changed(copy=>{copy.kinds.person.tree_type='tos.entity.work';})).toContain('kind person: tree_type tos.entity.work is not an agent or organization type');
    expect(populationIssues(population,{chronology,identities:{...identities,agents:{}},registry})).toContain(`member ${dated}: no Tree identity in the graft map's agents`);
  });
  it('reports a member the graft map lacks instead of stopping the export',()=>{
    const agents={...identities.agents};delete agents[dated];
    const result=checkBonsai(root,'population',{overrides:{identities:{...identities,agents}}});
    expect(result.problems).toContain(`export: No Tree identity for agents ${dated}`);
  });
  it('folds names by script and keeps the order of their words',()=>{
    expect(fold('Ḥunayn ibn Isḥāq')).toBe('hunayn ibn ishaq');
    expect(fold('Isḥāq ibn Ḥunayn')).not.toBe(fold('Ḥunayn ibn Isḥāq'));
    expect(fold('Ἀριστοτέλης')).toBe('αριστοτελησ');
    expect(fold('Йоганн Ёлкин')).toBe('йоганн елкин');
    expect(fold("ʿAbd al-Raḥmān")).toBe('abd al rahman');
    expect(strength(fold('Ibn Sīnā'),'Latn')).toBe('short');
    expect(strength(fold('Gottfried Wilhelm Leibniz'),'Latn')).toBe('strong');
    expect(strength('慧遠','Hani')).toBe('short');expect(strength('淨影慧遠','Hani')).toBe('strong');
    expect(formKeys('Leibniz, Gottfried Wilhelm')).toEqual(expect.arrayContaining([expect.objectContaining({key:'gottfried wilhelm leibniz',strength:'strong'}),expect.objectContaining({key:'leibniz',how:'base',strength:'short'})]));
    expect(formKeys('Nāgārjuna (Jaina)').find(item=>item.how==='base')).toMatchObject({key:'nagarjuna',strength:'short'});
    expect(formHead('Hierocles of Alexandria')).toBe('hierocles');expect(formHead('Хуэйюань из Лушаня')).toBe('хуэйюань');
    expect(formHead('Critias, son of Callaeschrus')).toBe(null);expect(formHead('Amenope son of Amenope')).toBe(null);expect(formHead('Hierocles')).toBe(null);
    expect(slugify('Søren Kierkegaard')).toBe('soren-kierkegaard');expect(slugify('Kurt Gödel')).toBe('kurt-goedel');expect(slugify('Хуэйюань из Лушаня')).toBe('khueiyuan-iz-lushanya');
  });
  it('joins strictly and leaves every other pair that shares a key to a reader',()=>{
    const row=(batch,{keys={},heads=[],ids={},birth=null,death=null,kind='person',status='verified'}={})=>({batch,kind,status,
      keys:new Map(Object.entries(keys)),heads:new Set(heads),idSets:new Map(Object.entries(ids).map(([scheme,value])=>[scheme,new Set([value])])),
      years:{birth:birth===null?null:{year:birth,certainty:'exact'},death:death===null?null:{year:death,certainty:'exact'}}});
    const exact=()=>0;
    const name={'gottfried wilhelm leibniz':'strong'};
    expect(compare(row('T3-06',{keys:name,birth:1646}),row('T3-06',{keys:name,birth:1646}),exact)).toEqual({decision:'apart',reason:'same-batch'});
    expect(compare(row('T3-06',{keys:name,birth:1646}),row('T3-15',{keys:name,birth:1646}),exact)).toEqual({decision:'auto',tier:'name+year'});
    expect(compare(row('T3-06',{keys:name}),row('T3-15',{keys:name}),exact)).toEqual({decision:'queue',reason:'strong-undated'});
    expect(compare(row('T3-06',{keys:name,birth:1646}),row('T3-15',{keys:name,birth:1700}),exact)).toEqual({decision:'queue',reason:'date-conflict'});
    expect(compare(row('A26',{keys:{zeno:'short'},birth:-333,death:-261}),row('A34',{keys:{zeno:'short'},birth:-333,death:-261}),exact)).toEqual({decision:'auto',tier:'short+years'});
    expect(compare(row('A26',{keys:{zeno:'short'}}),row('A34',{keys:{zeno:'short'}}),exact)).toEqual({decision:'queue',reason:'short-only'});
    expect(compare(row('A26',{keys:{zeno:'short'},birth:-333}),row('A24',{keys:{zeno:'short'},birth:-489}),exact)).toEqual({decision:'apart',reason:'short-conflict'});
    expect(compare(row('T3-10',{keys:{}},),row('T3-33',{keys:{},ids:{gnd:'118607979'}}),exact)).toEqual({decision:'apart',reason:'no-shared-key'});
    const hierocles=row('A34',{keys:{'hierocles of alexandria':'strong'},heads:['hierocles'],birth:410,death:460});
    expect(compare(hierocles,row('T2-02',{keys:{hierocles:'short'},birth:410,death:460}),exact)).toEqual({decision:'queue',reason:'contained'});
    expect(compare(hierocles,row('T2-02',{keys:{hierocles:'short'},birth:100}),exact)).toEqual({decision:'apart',reason:'contained-conflict'});
    expect(compare(row('T3-10',{ids:{gnd:'118607979'}}),row('T3-33',{ids:{gnd:'118607979'}}),exact)).toEqual({decision:'auto',tier:'id'});
    expect(compare(row('T3-10',{keys:name,ids:{gnd:'1'}}),row('T3-33',{keys:name,ids:{gnd:'2'}}),exact)).toEqual({decision:'queue',reason:'id-conflict'});
    expect(compare(row('A34',{keys:name,kind:'pseudonymous'}),row('T2-03',{keys:name}),exact)).toEqual({decision:'apart',reason:'pseudo-with-other'});
    expect(compare(row('A34',{keys:name,kind:'conventional'}),row('T2-03',{keys:name}),exact)).toEqual({decision:'queue',reason:'kind'});
  });
  it('holds a control case only as its requirement says',()=>{
    const keys=[{row:'A01:A01-A001',member:'m-aaaaaaaaaa',basis:'founding'},{row:'A02:A02-A001',member:'m-aaaaaaaaaa',basis:'name+year'},{row:'A03:A03-A001',member:'m-bbbbbbbbbb',basis:'founding'}];
    const control=must=>({cases:[{id:'c',must,rows:['A01:A01-A001','A02:A02-A001']}]});
    expect(controlIssues(control('same'),keys)).toEqual([]);
    expect(controlIssues(control('never-same'),keys)).toEqual(['c: A01:A01-A001 and A02:A02-A001 are one member']);
    expect(controlIssues(control('never-auto'),keys)).toEqual(['c: A01:A01-A001 and A02:A02-A001 were joined without a reader\'s decision']);
    expect(controlIssues({cases:[{id:'d',must:'same',rows:['A01:A01-A001','A03:A03-A001']}]},keys)).toEqual(['d: the rows are not one member']);
  });
  it.skipIf(!layersFetched)('is what the register rows, the decisions and the ledger give',()=>{
    const run=(script,...args)=>execFileSync(process.execPath,[join(review,script),...args,'--check'],{encoding:'utf8'});
    expect(run('resolve.mjs')).toMatch(/^resolve: matches what the rows and decisions give/);
    expect(run('plant.mjs')).toBe('population.json: matches what the records give\nchronology.json: matches what the records give\nidentities.json: matches what the records give\n');
    expect(run(join('line','kit.mjs'),'queues')).toBe(['kind','dates','latin','living'].map(name=>`queue/${name}.jsonl: matches what population.json and the rows give\n`).join(''));
  });
});

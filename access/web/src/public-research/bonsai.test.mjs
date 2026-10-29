import {readFileSync,readdirSync} from 'node:fs';
import {describe,expect,it} from 'vitest';
import {centuryOf,centuryStart,dateIssues,dateSpan,formatDating,formatYears as formatYearsNb,lexemeBounds,lexemeParts,yearLexeme} from '../../../../bonsai/lib/time.mjs';
import {chronologyIssues,readChronology} from '../../../../bonsai/lib/chronology.mjs';
import {placesIssues,readPlaces} from '../../../../bonsai/lib/places.mjs';
import {createValidator} from '../../../../bonsai/lib/schema.mjs';
import {bonsaiIds,checkBonsai,loadBonsai,treeContracts} from '../../../../bonsai/lib/graft.mjs';
import {bonsaiFor} from './bonsai.mjs';

const root=new URL('../../../../',import.meta.url).pathname;
const contracts=treeContracts(root);
// Words join numbers with no-break spaces; the tests read them as plain spaces.
const plain=text=>text.replaceAll('\u00a0',' ');
const formatYears=(...args)=>plain(formatYearsNb(...args));
const HISTORICAL_DATE='https://tree-of-sophia.local/ToS/contracts/historical-claim.schema.json#/$defs/historicalDate';

describe('the Tree date grammar',()=>{
  it('reads Tree lexemes in astronomical numbering and writes them back',()=>{
    expect(lexemeParts('-0386')).toEqual({year:-386,month:null,day:null,precision:'year'});
    expect(lexemeParts('1883-07')).toMatchObject({year:1883,month:7,precision:'month'});
    expect(lexemeParts('1886-02-29')).toBeNull();expect(lexemeParts('1888-02-29')).toMatchObject({day:29});
    expect(lexemeParts('387 BCE')).toBeNull();expect(lexemeParts('-386')).toBeNull();
    expect(yearLexeme(-386)).toBe('-0386');expect(yearLexeme(0)).toBe('0000');expect(yearLexeme(44)).toBe('0044');
    expect(lexemeBounds('-0043')).toEqual([-43,-42]);
    const [from,to]=lexemeBounds('1883-02');expect(from).toBeCloseTo(1883+1/12,9);expect(to).toBeCloseTo(1883+2/12,9);
  });
  it('speaks as historians write: no year zero, as firm as the dating',()=>{
    expect(formatYears({first:-398,last:-386,certainty:'approximate'},'ru')).toBe('ок. 399–387 до н. э.');
    expect(formatYears({first:-43,last:-43,certainty:'exact'},'en')).toBe('44 BCE');
    expect(formatYears({first:0,last:65,certainty:'approximate'},'en')).toBe('c. 1 BCE – 65 CE');
    expect(formatYears({first:253,last:270,certainty:'uncertain'},'ru')).toBe('253–270 н. э. (?)');
    expect(formatYears({},'ru')).toBe('');
    // A year of the common era from 1000 on needs no era.
    expect(formatYears({first:1883,last:1885,certainty:'exact'},'ru')).toBe('1883–1885');
    expect(formatYears({first:900,last:1100,certainty:'exact'},'en')).toBe('900 CE – 1100');
    expect(formatYearsNb({first:-386,last:-379,certainty:'approximate'},'ru')).not.toMatch(/ /);
    expect(centuryOf(-399)).toEqual({n:4,era:'BCE'});expect(centuryOf(-400)).toEqual({n:5,era:'BCE'});expect(centuryOf(100)).toEqual({n:1,era:'CE'});
    expect([centuryStart(-399),centuryStart(-400),centuryStart(1),centuryStart(100),centuryStart(101)]).toEqual([-399,-499,1,1,101]);
  });
  it('speaks each kind of dating in its own words: months, open intervals, a relative span',()=>{
    const base={role:'historical-time',calendar:null,year_numbering:'astronomical',source_wording:{text:'x',language:'en'}};
    const say=(value,locale,options)=>plain(formatDating({...base,...value},locale,options));
    expect(say({kind:'interval-assertion',certainty:'exact',interval:{start:'1883-01',end:'1883-02'}},'ru')).toBe('январь–февраль 1883');
    expect(say({kind:'interval-assertion',certainty:'exact',interval:{start:'1883-01',end:'1883-02'}},'en')).toBe('January–February 1883');
    expect(say({kind:'interval-assertion',certainty:'approximate',interval:{start:'1884-11',end:'1885-02'}},'ru')).toBe('ок. ноября 1884 – февраля 1885');
    expect(say({kind:'date-assertion',certainty:'approximate',value:'1884-01'},'ru')).toBe('ок. января 1884');
    expect(say({kind:'interval-assertion',certainty:'approximate',interval:{start:'1940',end:'1941-02'}},'en')).toBe('c. 1940 – February 1941');
    expect(say({kind:'date-assertion',certainty:'exact',value:'1881-08'},'ru')).toBe('август 1881');
    expect(say({kind:'date-assertion',certainty:'exact',value:'1843-10-16'},'ru')).toBe('16 октября 1843');
    expect(say({kind:'date-assertion',certainty:'approximate',value:'-0499'},'ru')).toBe('ок. 500 до н. э.');
    expect(say({kind:'interval-assertion',certainty:'exact',interval:{end:'1962'}},'ru')).toBe('до 1962');
    expect(say({kind:'interval-assertion',certainty:'exact',interval:{start:'1941-02'}},'ru')).toBe('с февраля 1941');
    expect(say({kind:'interval-assertion',certainty:'exact',interval:{end:'1962'}},'en')).toBe('by 1962');
    expect(say({kind:'relative-order',year_numbering:null,certainty:'approximate',relative:{relation:'during',anchor_ref:'tos.biographical-phase.a.lifetime'}},'ru',{span:{first:-386,last:-379}})).toBe('ок. 387–380 до н. э.');
    expect(say({kind:'unknown-date',certainty:'unknown'},'ru')).toBe('');
  });
  it('agrees with the Tree contract on what a dating may be',()=>{
    const base={role:'historical-time',calendar:null,year_numbering:'astronomical',source_wording:{text:'c. 387–380 BCE',language:'en'}};
    const samples=[
      {...base,kind:'interval-assertion',certainty:'approximate',interval:{start:'-0386',end:'-0379'}},
      {...base,kind:'date-assertion',certainty:'exact',value:'-0043'},
      {...base,kind:'relative-order',year_numbering:null,certainty:'exact',relative:{relation:'during',anchor_ref:'tos.biographical-phase.plato.lifetime'}},
      {...base,kind:'unknown-date',certainty:'unknown'},
      {...base,kind:'unknown-date',certainty:'approximate'},
      {...base,kind:'interval-assertion',certainty:'exact',interval:{start:'-0386'},value:'-0386'},
      {...base,kind:'relative-order',certainty:'exact',relative:{relation:'within',anchor_ref:'tos.biographical-phase.plato.lifetime'}},
      {...base,kind:'date-assertion',certainty:'exact',value:'44 BCE',extra:true},
    ];
    for(const sample of samples){
      const tree=contracts.validate(HISTORICAL_DATE,sample).length===0,own=dateIssues(sample).length===0;
      // The bonsai is stricter only where it reads lexemes: '44 BCE' is a valid Tree string but no astronomical lexeme.
      if(sample.value==='44 BCE')expect(own).toBe(false);else expect(own,JSON.stringify(sample)).toBe(tree);
    }
    expect(dateIssues({...base,kind:'interval-assertion',certainty:'exact',interval:{start:'-0300',end:'-0400'}})).toContain('interval: start after end');
  });
  it('resolves a dating "during" another situation to that situation\'s span, never through a cycle',()=>{
    const date=(kind,extra)=>({kind,role:'historical-time',calendar:null,year_numbering:kind==='relative-order'?null:'astronomical',certainty:'approximate',source_wording:{text:'x',language:'en'},...extra});
    const module={schema:'tos.bonsai.chronology.v1',sources:{s:{short:'S',title:'Source',url:'https://example.org/'}},situations:{
      'tos.biographical-phase.a.lifetime':{dating:date('interval-assertion',{interval:{start:'-0427',end:'-0346'}})},
      'tos.historical-process.a.w-composition':{dating:date('relative-order',{relative:{relation:'during',anchor_ref:'tos.biographical-phase.a.lifetime'}})},
      'tos.historical-process.a.x-composition':{dating:date('relative-order',{relative:{relation:'during',anchor_ref:'tos.historical-process.a.y-composition'}})},
      'tos.historical-process.a.y-composition':{dating:date('relative-order',{relative:{relation:'during',anchor_ref:'tos.historical-process.a.x-composition'}})},
      'tos.historical-process.a.z-composition':{dating:date('relative-order',{relative:{relation:'before',anchor_ref:'tos.biographical-phase.a.lifetime'}})},
    }};
    const reader=readChronology(module);
    expect(reader.span('tos.historical-process.a.w-composition')).toMatchObject({from:-427,to:-345,first:-427,last:-346,relation:'during',anchor:'tos.biographical-phase.a.lifetime'});
    expect(reader.span('tos.historical-process.a.x-composition')).toBeNull();
    expect(reader.span('tos.historical-process.a.z-composition')).toBeNull();
    expect(dateSpan(date('unknown-date',{certainty:'unknown'}))).toBeNull();
  });
});

describe('the schema validator the bonsai checks against',()=>{
  it('accepts a real Tree claim and refuses what its contract forbids',()=>{
    const temporal='https://tree-of-sophia.local/ToS/contracts/source-temporal-claim.schema.json';
    const claim={schema_version:'tos_source_temporal_claim_v1',claim_id:'tos.claim.example.dating',claim_type:'relation',assertion_layer:'scholarly_report',
      subject_ref:'tos.historical-process.example.writing',predicate:'historical_dating',
      object:{kind:'interval-assertion',role:'historical-time',calendar:null,year_numbering:null,certainty:'exact',interval:{start:'1886-06',end:'1886-07'},source_wording:{text:'Im Juni und Juli 1886',language:'de'}},
      evidence_refs:['https://example.org/'],maker:{maker_type:'model',agent_ref:'agent:example'},provenance_event_ref:'tos.event.example',epistemic_status:'reported',
      review_status:'unreviewed',claim_version:1,visibility:'public_metadata_only',qualifiers:{statement:'Пример.',statement_language:'ru',statement_script:'Cyrl'}};
    expect(contracts.validate(temporal,claim)).toEqual([]);
    expect(contracts.validate(temporal,{...claim,extra:1}).map(error=>error.path)).toContain('/extra');
    expect(contracts.validate(temporal,{...claim,review_status:'accepted'}).length).toBeGreaterThan(0);
    expect(contracts.validate(temporal,{...claim,qualifiers:{statement:'x'}}).length).toBeGreaterThan(0);
    const validator=createValidator([{$id:'https://example.org/a.json',type:'object',properties:{n:{type:'integer',minimum:1}},required:['n'],unevaluatedProperties:false}]);
    expect(validator.validate('https://example.org/a.json',{n:2})).toEqual([]);
    expect(validator.validate('https://example.org/a.json',{n:0})).toHaveLength(1);
    expect(()=>createValidator([{$id:'https://example.org/b.json',contains:{}}]).validate('https://example.org/b.json',[])).toThrow(/Unsupported/);
  });
});

describe('every bonsai',()=>{
  const ids=bonsaiIds(root);
  it('is found at the repository root, one per stand',()=>{
    expect(ids).toEqual(['antiquity','zarathustra']);
    for(const id of ids)expect(bonsaiFor(id)?.manifest?.id).toBe(id);
  });
  for(const id of ids)it(`${id}: its modules keep their contracts and its export fits the Tree's contracts`,()=>{
    const result=checkBonsai(root,id,{validator:contracts});
    expect(result.problems).toEqual([]);
    expect(result.packet.claims.length).toBeGreaterThan(0);
    for(const claim of result.packet.claims){expect(claim.review_status).toBe('unreviewed');expect(claim.visibility).toBe('public_metadata_only');}
    // Every identity a claim names is listed with its graft status.
    const listed=new Set(result.packet.identities.map(item=>item.tree));
    for(const claim of result.packet.claims){expect(listed.has(claim.subject_ref),claim.claim_id).toBe(true);if(typeof claim.object==='string')expect(listed.has(claim.object),claim.claim_id).toBe(true);}
  });
  it('records the digest of every Tree contract it was grafted from',()=>{
    for(const id of ids){
      const {manifest}=loadBonsai(root,id);
      expect(manifest.graft.tree_revision).toMatch(/^[0-9a-f]{40}$/);
      for(const [name,entry] of Object.entries(manifest.graft.contracts)){expect(readdirSync(new URL('../../../../ToS/contracts/',import.meta.url)),name).toContain(entry.path.split('/').at(-1));expect(entry.sha256,name).toMatch(/^[0-9a-f]{64}$/);}
    }
  });
  it('marks every lens of its stand as a measure or a grouping, and says what',()=>{
    for(const id of ids){
      const {manifest}=loadBonsai(root,id);
      for(const [lens,declared] of Object.entries(manifest.lenses)){
        expect(['measure','group'],`${id}/${lens}`).toContain(declared.positions);
        const said=declared.positions==='measure'?declared.measures:declared.groups;
        expect(said?.ru,`${id}/${lens}`).toBeTruthy();expect(said?.en,`${id}/${lens}`).toBeTruthy();
      }
    }
  });
});

describe('the places module contract',()=>{
  it('names what is wrong with a malformed places module',()=>{
    expect(placesIssues(null)).toEqual(['not an object']);
    const chronology={situations:{'tos.historical-process.a.w-composition':{type:'historical-process'},'tos.historical-process.a.x-composition':{type:'historical-process'}}};
    const module={schema:'tos.bonsai.places.v1',sources:{s:{short:'S',title:'Source',url:'https://example.org/'}},
      places:{athens:{label:{ru:'Афины',en:'Athens'},gazetteer:[{scheme:'pleiades',id:'579885',url:'https://pleiades.stoa.org/places/579885'}],point:[23.72,37.97],point_source:'https://pleiades.stoa.org/places/579885/json'},
        nowhere:{label:{ru:'Нигде',en:'Nowhere'},gazetteer:[{scheme:'osm',id:'1',url:'http://example.org'}],point:[200,0],point_source:'x'}},
      situations:{'tos.historical-process.a.w-composition':{place:'athens',basis:'activity',statement:{ru:'Да.',en:'Yes.'},sources:['s'],footing:'high'},
        'tos.historical-process.a.x-composition':{place:'athens',basis:'unknown',statement:{ru:'Нет.',en:'No.'},sources:['t'],footing:'high'},
        'tos.historical-process.a.y-composition':{place:'athens',basis:'event',statement:{ru:'Нет.',en:'No.'},sources:['s'],footing:'firm'}}};
    const issues=placesIssues(module,chronology);
    for(const expected of ['place nowhere: gazetteer entry','place nowhere: point','place nowhere: point_source','place nowhere: no situation stands there',
      'situation tos.historical-process.a.x-composition: an unknown place names no place','situation tos.historical-process.a.x-composition: unknown source t',
      'situation tos.historical-process.a.y-composition: not a situation of the chronology','situation tos.historical-process.a.y-composition: footing'])expect(issues).toContain(expected);
    expect(issues.some(issue=>issue.startsWith('situation tos.historical-process.a.w-composition'))).toBe(false);
  });
  it('reads a sourced place, and an open one without a point',()=>{
    const places=readPlaces(bonsaiFor('antiquity').places);
    expect(places.at('tos.historical-process.cicero.de-officiis-composition')).toMatchObject({place:'puteoli',basis:'composition',point:[14.121741499999999,40.825838]});
    expect(places.at('tos.historical-process.plato.apology-composition')).toBeNull();
    expect(places.entry('tos.historical-process.plato.apology-composition')).toMatchObject({basis:'unknown'});
    expect(places.sources('tos.historical-process.cicero.de-officiis-composition').map(source=>source.short)).toEqual(['Holden','Loeb · Winstedt']);
    const zarathustra=readPlaces(bonsaiFor('zarathustra').places);
    expect(zarathustra.at('tos.historical-event.friedrich-nietzsche.recurrence-thought')).toMatchObject({place:'surlej',basis:'event'});
    expect(zarathustra.at('tos.historical-process.friedrich-nietzsche.zarathustra-part-1-composition')).toMatchObject({place:'rapallo',basis:'composition'});
  });
});

describe('the chronology module contract',()=>{
  it('names what is wrong with a malformed chronology',()=>{
    expect(chronologyIssues(null)).toEqual(['not an object']);
    const issues=chronologyIssues({schema:'x',sources:{},situations:{'not-a-tree-id':{type:'historical-process'}},lives:{plato:'tos.biographical-phase.plato.lifetime'}});
    expect(issues).toContain('schema');
    expect(issues.some(issue=>issue.includes('not a Tree identity'))).toBe(true);
    expect(issues.some(issue=>issue.startsWith('life plato'))).toBe(true);
  });
  it('reads the bonsai the stand reads',()=>{
    const json=JSON.parse(readFileSync(new URL('../../../../bonsai/antiquity/chronology.json',import.meta.url),'utf8'));
    expect(bonsaiFor('antiquity').chronology).toEqual(json);
  });
});

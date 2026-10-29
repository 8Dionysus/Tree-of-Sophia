import {readFileSync,readdirSync} from 'node:fs';
import {join} from 'node:path';
import {describe,expect,it} from 'vitest';
import {alignRun,alignSameLanguage,alignWitnesses,chapterRows,prepare,sentenceSpans,triangle} from '../../../../bonsai/lib/align.mjs';
import {citeIssues,chapterIssues,displayText,fragmentOf,readChapter,readingText,witnessesIssues} from '../../../../bonsai/lib/witnesses.mjs';
import {realignWitnesses} from '../../../../bonsai/lib/realign.mjs';

const root=new URL('../../../../',import.meta.url).pathname;
const home=join(root,'bonsai','zarathustra');
const manifest=JSON.parse(readFileSync(join(home,'witnesses.json'),'utf8'));
const files=Object.fromEntries(readdirSync(join(home,'witnesses')).filter(name=>name.endsWith('.json')).map(name=>[name.slice(0,-5),JSON.parse(readFileSync(join(home,'witnesses',name),'utf8'))]));
const chapter=key=>readChapter(files[key]);
const shown=fragment=>fragment.units.map(unit=>unit.text.slice(unit.from,unit.to)).join(' ');

describe('the witnesses of Zarathustra',()=>{
  it('hold every chapter of the original, each unit in exactly one row, with the rights of every witness',()=>{
    expect(witnessesIssues(manifest,files)).toEqual([]);
    expect(Object.keys(files)).toHaveLength(81);
    expect(manifest.pivot).toBe('de');
    for(const witness of Object.values(manifest.witnesses))expect(['public-domain','public-domain+licence']).toContain(witness.rights.basis);
    // The censored Russian edition of 1913 lacks three chapters; the alignment says so instead of inventing them.
    expect(manifest.chapters.filter(entry=>entry.omitted).map(entry=>entry.key)).toEqual(['II.04','IV.06','IV.18']);
  });
  it('find a place of the original in every translation through the alignment, never by hand',()=>{
    const vision=chapter('III.02'),unit=vision.units.de.find(item=>item.raw.includes('Siehe diesen Thorweg'));
    const s=unit.raw.indexOf('Siehe diesen Thorweg'),e=unit.raw.indexOf('Gesichter.',s)+'Gesichter.'.length;
    const cite={w:'za',c:'III.02',u:[unit.id,unit.id],s,e};
    expect(citeIssues(cite,vision)).toEqual([]);
    expect(shown(fragmentOf(cite,vision,'de'))).toBe('Siehe diesen Thorweg! Zwerg! sprach ich weiter: der hat zwei Gesichter.');
    expect(shown(fragmentOf(cite,vision,'en'))).toBe('“Look at this gateway! Dwarf!” I continued, “it hath two faces.');
    expect(shown(fragmentOf(cite,vision,'ru'))).toBe('«Взгляни на эти ворота, карлик! продолжал я: у них два лица.');
  });
  it('show what a translation leaves out as left out: a whole chapter, or the rest of one',()=>{
    const priests=chapter('II.04');
    expect(fragmentOf({w:'za',c:'II.04',u:[priests.units.de[3].id,priests.units.de[3].id]},priests,'ru')).toMatchObject({units:[],omitted:true});
    const awakening=chapter('IV.17');
    // The litany of the ass is not in the edition of 1913.
    const litany=awakening.units.de.findIndex(unit=>unit.raw.includes('Amen'));
    expect(fragmentOf({w:'za',c:'IV.17',u:[awakening.units.de[litany].id,awakening.units.de[litany].id]},awakening,'ru')).toMatchObject({omitted:true});
    const run=fragmentOf({w:'za',c:'IV.17',u:[awakening.units.de[16].id,awakening.units.de[18].id]},awakening,'ru');
    expect(run).toMatchObject({omitted:false,partial:true});
  });
  it('refuse a place that is not in the chapter',()=>{
    const vision=chapter('III.02'),first=vision.units.de[0];
    expect(citeIssues({w:'za',c:'III.02',u:[first.id,'tos.text-unit.sid-unknown']},vision)).toHaveLength(1);
    expect(citeIssues({w:'za',c:'III.01',u:[first.id,first.id]},vision)).toHaveLength(1);
    expect(citeIssues({w:'za',c:'III.02',u:[first.id,first.id],s:5,e:2},vision)).toHaveLength(1);
    expect(fragmentOf({w:'za',u:['x','y']},vision,'de')).toBeNull();
  });
  it('are aligned exactly as the mechanism aligns their stored texts',()=>{
    const result=realignWitnesses(manifest,files);
    expect(result.changed).toEqual([]);
    expect(result.alignment.triangle).toEqual(manifest.alignment.triangle);
    expect(result.alignment.pairs).toEqual(manifest.alignment.pairs);
  },240_000);
});

describe('what a reader sees of a unit',()=>{
  it('joins a word broken at a line end, makes a printed line end a space and keeps the lines of verse',()=>{
    const view=displayText('Als es unter den Schiffs¬\nleuten ruchbar wurde,\ndass','p');
    expect(view.text).toBe('Als es unter den Schiffsleuten ruchbar wurde, dass');
    expect(view.at(17)).toBe(17);expect(view.at(26)).toBe(24);expect(view.at(999)).toBe(view.text.length);
    expect(displayText('Eins!\nOh Mensch!  Gieb Acht!\n','v').text).toBe('Eins!\nOh Mensch! Gieb Acht!');
    expect(readingText('Eins!\nOh Mensch!','v')).toBe('Eins! / Oh Mensch!');
    expect(readingText('ver¬\nstehen\nund','p')).toBe('verstehen und');
  });
  it('splits sentences on their own ends, not at abbreviations or inside a sentence',()=>{
    const raw='Also sprach Zarathustra. Und er ging. „Wohin?“ fragte er.';
    expect(sentenceSpans(raw).map(([a,b])=>raw.slice(a,b))).toEqual(['Also sprach Zarathustra.','Und er ging.','„Wohin?“ fragte er.']);
  });
});

describe('the alignment mechanism',()=>{
  const units=(texts,language)=>prepare(texts.map((text,index)=>({id:`${language}${index}`,text,section:null})),language);
  it('pairs paragraphs one to one, merges where a translation joins two, and reads a long omission as one',()=>{
    const de=units(['Kurz.','Ein längerer Absatz mit vielen Worten, der weitergeht und weitergeht.','Noch ein Satz hier.','Und dann der letzte Satz des Kapitels.'],'de');
    const en=units(['Short.','A longer paragraph with many words, which goes on and goes on.','Yet another sentence here.','And then the last sentence of the chapter.'],'en');
    const groups=alignRun(de,en,1,2);
    expect(groups.map(group=>group.shape)).toEqual(['1:1','1:1','1:1','1:1']);
    const merged=alignRun(de,units(['Short.','A longer paragraph with many words, which goes on and goes on. Yet another sentence here.','And then the last sentence of the chapter.'],'en'),1,2);
    expect(merged.map(group=>group.shape)).toEqual(['1:1','2:1','1:1']);
    // Determinism: the same units give the same groups.
    expect(alignRun(de,en,1,2)).toEqual(groups);
  });
  it('joins the rows of every pairing around the pivot and checks them against each other',()=>{
    const pivot=[{id:'d0'},{id:'d1'},{id:'d2'}],en=[{id:'e0'},{id:'e1'}],ru=[{id:'r0'},{id:'r1'},{id:'r2'}];
    const rows=chapterRows({de:pivot,en,ru},{en:[{s:['d0'],t:['e0']},{s:['d1','d2'],t:['e1']}],ru:[{s:['d0'],t:['r0']},{s:['d1'],t:['r1']},{s:['d2'],t:['r2']}]},'de');
    expect(rows).toEqual([{de:[0],en:[0],ru:[0]},{de:[1,2],en:[1],ru:[1,2]}]);
    expect(triangle([{s:['d0'],t:['e0']}],[{s:['d0'],t:['r0']}],[{s:['e0'],t:['r0']}])).toMatchObject({agree:1,disagree:0});
    expect(triangle([{s:['d0'],t:['e0']}],[{s:['d0'],t:['r1']}],[{s:['e0'],t:['r0']}])).toMatchObject({agree:0,disagree:1,disagreements:['d0']});
  });
  it('maps two electronic texts of one edition onto each other by their letters',()=>{
    const groups=alignSameLanguage([{id:'a',text:'Als Zarathustra dreissig Jahr alt war'},{id:'b',text:'verliess er seine Heimat'}],[{id:'x',text:'Als Zarathustra dreißig Jahr alt war, verließ er seine Heimat'}]);
    expect(groups).toEqual([{s:['a','b'],t:['x'],shape:'2:1',similarity:expect.any(Number)}]);
  });
  it('learns which words translate each other from confident pairs',()=>{
    const de={c:[{id:'d0',text:'Der Hund läuft.',raw:'Der Hund läuft.'},{id:'d1',text:'Die Katze schläft.',raw:'Die Katze schläft.'}]};
    const en={c:[{id:'e0',text:'The dog runs.',raw:'The dog runs.'},{id:'e1',text:'The cat sleeps.',raw:'The cat sleeps.'}]};
    const result=alignWitnesses(de,en,'de','en');
    expect(result.chapters.c.groups.map(group=>[group.s,group.t])).toEqual([[['d0'],['e0']],[['d1'],['e1']]]);
    expect(result.chapters.c.groups[0].sentences).toEqual([[[[0,0,15]],[[0,0,13]]]]);
  });
});

describe('a chapter file',()=>{
  it('is refused when a unit sits in two rows or a sentence leaves its unit',()=>{
    const file=JSON.parse(JSON.stringify(files['I.02']));
    file.rows[1].de=[...file.rows[0].de];
    expect(chapterIssues(file).some(issue=>issue.includes('is in 2 rows'))).toBe(true);
    const other=JSON.parse(JSON.stringify(files['I.02']));
    const row=other.rows.find(item=>item.sentences?.en);row.sentences.en[0][1][0][2]=1e6;
    expect(chapterIssues(other).some(issue=>issue.includes('outside unit'))).toBe(true);
  });
});

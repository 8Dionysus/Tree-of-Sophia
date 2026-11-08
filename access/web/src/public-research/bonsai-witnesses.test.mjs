import {readFileSync,readdirSync} from 'node:fs';
import {join} from 'node:path';
import {describe,expect,it} from 'vitest';
import {alignRun,alignSameLanguage,alignWitnesses,chapterRows,prepare,sentenceSpans,triangle} from '../../../../bonsai/lib/align.mjs';
import {citeIssues,chapterIssues,displayText,fragmentOf,readChapter,readingText,witnessesIssues} from '../../../../bonsai/lib/witnesses.mjs';
import {realignWitnesses} from '../../../../bonsai/lib/realign.mjs';
import {chapterName} from './fragments.mjs';

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
    // A reader of Russian finds each of them under the stand's own Russian name, and every other chapter under its edition's title.
    expect(manifest.chapters.filter(entry=>entry.omitted).map(entry=>chapterName(entry,'ru'))).toEqual(['О священниках','Удалившийся от дел','Праздник осла']);
    expect(manifest.chapters.map(entry=>chapterName(entry,'ru')).filter(name=>/[A-Z]/.test(name))).toEqual([]);
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
  it('shows the scan-corrected Russian at the aligned place, retaining the 1913 wording',()=>{
    const redemption=chapter('II.20');
    const fragment=id=>shown(fragmentOf({w:'za',c:'II.20',u:[id,id]},redemption,'ru'));
    expect(fragment('tos.text-unit.sid-04ae7c3916e9390af927106df7b960ca')).toContain('так учил я вас, мои друзья! А теперь научитесь еще: сама воля еще пленница.');
    expect(fragment('tos.text-unit.sid-3d1d58f8435846038b07514d30c5672a')).toContain('в этом гнев ее;');
    expect(redemption.units.ru.find(unit=>unit.id==='ru1913.II.20.042').raw).toContain('воля избавительница себя самой и вестником радости');
    expect(chapter('IV.05').units.ru.find(unit=>unit.id==='ru1913.IV.05.039').raw).toContain('о, Заратустра?');
    expect(chapter('IV.20').units.ru.find(unit=>unit.id==='ru1913.IV.20.023').raw).toContain('брежжит мой день');
    expect(chapter('IV.08').units.ru.find(unit=>unit.id==='ru1913.IV.08.008').raw).toContain('научиться у этих коров');
    expect(chapter('III.12').units.ru.find(unit=>unit.id==='ru1913.III.12.219').raw).toContain('налагать руку на тысячелетия');
    expect(chapter('IV.10').units.ru.find(unit=>unit.id==='ru1913.IV.10.022').raw).toContain('Вставай, ты, сонливец!');
    expect(chapter('IV.12').units.ru.find(unit=>unit.id==='ru1913.IV.12.007').raw).toContain('импровизованное здоровье');
    expect(chapter('IV.13').units.ru.find(unit=>unit.id==='ru1913.IV.13.096').raw).toContain('Заратустра смеющийся');
    expect(chapter('IV.13').units.ru.find(unit=>unit.id==='ru1913.IV.13.028').raw).toContain('Не всякое слово годится ко всякому рылу');
    expect(chapter('IV.20').units.ru.find(unit=>unit.id==='ru1913.IV.20.011').raw).toContain('он увидел, что у ног его лежал');
    expect(chapter('III.16').units.ru.find(unit=>unit.id==='ru1913.III.16.051').raw).toContain('даже у бога есть есть свой ад');
    expect(chapter('III.12').units.ru.find(unit=>unit.id==='ru1913.III.12.036').raw).toContain('самым дальным: не щади');
    expect(chapter('II.13').units.ru.find(unit=>unit.id==='ru1913.II.13.027').raw).toContain('освободить волю от ноши ее');
    expect(chapter('III.06').units.ru.find(unit=>unit.id==='ru1913.III.06.001').raw).toContain('от ее дружеских рукопожатий');
    expect(chapter('III.12').units.ru.find(unit=>unit.id==='ru1913.III.12.089').raw).toContain('ищут ее ваши корабли');
    expect(chapter('III.05').units.ru.find(unit=>unit.id==='ru1913.III.05.070').raw).toContain('час молнии. О, тайна перед полуднем!');
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
  it('preserves the original sentence correspondence where Russian proofreading changed the text',()=>{
    const result=realignWitnesses(manifest,files);
    // Proofread chapters retain their original pairings, with adjusted offsets.
    // Fresh segmentation can split the restored questions differently.
    const proofread=["I.01","I.09","I.10","I.11","I.12","I.16","I.17","I.18","I.19","I.20","I.21","I.22","I.23","II.01","II.02","II.03","II.05","II.07","II.08","II.09","II.10","II.11","II.12","II.13","II.14","II.15","II.16","II.17","II.18","II.19","II.20","II.21","II.22","III.01","III.02","III.03","III.04","III.05","III.06","III.07","III.08","III.09","III.10","III.11","III.12","III.13","III.14","III.15","III.16","IV.01","IV.02","IV.03","IV.04","IV.05","IV.07","IV.08","IV.09","IV.10","IV.11","IV.12","IV.13","IV.14","IV.15","IV.16","IV.17","IV.19","IV.20"];
    expect(result.changed.filter(key=>!proofread.includes(key))).toEqual([]);
    // Re-aligning the changed Russian can change the model and triangle;
    // the stored correspondences remain in the proofread chapter files.
    expect(result.alignment.pairs['de-en']).toEqual(manifest.alignment.pairs['de-en']);
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
    // A numbered item's number opens its own sentence and is no part of the one before; a year ends a sentence.
    const numbered='The blessed is not troubled. 2. Death is nothing to us. 3.';
    expect(sentenceSpans(numbered).map(([a,b])=>numbered.slice(a,b))).toEqual(['The blessed is not troubled.','2. Death is nothing to us.']);
    const year='He was born in 1844. Then he left.';
    expect(sentenceSpans(year).map(([a,b])=>year.slice(a,b))).toEqual(['He was born in 1844.','Then he left.']);
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

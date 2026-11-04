import {readFileSync} from 'node:fs';
import {expect,it} from 'vitest';
import {INQUIRY_READINGS_ECHOES} from '../../constructor/inquiry-readings-echoes.mjs';

const read=path=>JSON.parse(readFileSync(new URL(`../../../../bonsai/zarathustra/${path}`,import.meta.url),'utf8'));

// A question the book answers in another place is a link between the two places: the answer shows both in the
// book's words, the stand's objects keep the link, and a relation of the core that holds the answer opens from it.
it('joins each riddle the book answers elsewhere to the place of its answer',()=>{
  const answered=Object.entries(INQUIRY_READINGS_ECHOES).filter(([,inquiry])=>inquiry.answer);
  expect(answered.map(([id])=>id).sort()).toEqual(['demo:moment-returns','demo:shepherd-riddle']);
  const links=read('objects.json').links,core=new Map(read('core/links.json').map(link=>[link.id,link]));
  for(const [id,{answer,grounds}] of answered){
    expect(answer.ru&&answer.en,id).toBeTruthy();
    const asked=new Set(grounds.flatMap(ground=>ground.cite).map(cite=>cite.c)),places=new Set(answer.cite.map(cite=>cite.c));
    // The answer shows the place that asks and a place elsewhere in the book that answers.
    expect([...places].some(place=>asked.has(place)),id).toBe(true);
    expect([...places].some(place=>!asked.has(place)),id).toBe(true);
    const link=links.find(item=>item.form===`INQUIRY_READINGS_ECHOES.${id}.answer`);
    expect(link?.kind,id).toBe('answers');
    expect(asked.has(link.to.c)&&!asked.has(link.from.c),id).toBe(true);
    if(answer.relation)expect(core.get(answer.relation)?.admission,id).toBe('map');
  }
});

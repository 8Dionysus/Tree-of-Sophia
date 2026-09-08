import {test,expect} from 'vitest';
import {nodePreview,relationPreview} from './graph-preview.mjs';
const node=(id,kind)=>({id,kind_id:kind,display:{title:{ru:id},kind_label:{ru:kind},summary_state:'metadata-synthesis',summary:{ru:'Техническая заглушка'}}});
test('every supplied kind is displayed without a browser taxonomy or invented description',()=>{
  for(const kind of ['work','concept','expression','editorial-witness','future-kind']){
    const raw=node('Предмет',kind),preview=nodePreview({nodes:[raw],relations:[]},raw);
    expect(preview).toEqual({kind,title:'Предмет',body:''});
  }
});
test('all supplied roles and relationships retain their direction, including unknown predicates',()=>{
  const person=node('Имя','Действующее лицо'),work=node('Произведение','Произведение');
  for(const role of ['автор произведения','редактор','переводчик','оформитель','новая роль']){
    const relation={id:'r',from_id:work.id,to_id:person.id,predicate_id:'opaque',display:{label:{ru:'создано'},inverse_label:{ru:role}}};
    const packet={nodes:[person,work],relations:[relation]};
    expect(nodePreview(packet,person).body).toBe(role+': Произведение');
    expect(nodePreview(packet,work).body).toBe('создано → Имя');
    expect(relationPreview(packet,relation).body).toBe('Произведение → Имя');
    delete relation.display.inverse_label;
    expect(nodePreview(packet,person).body).toBe('Произведение → создано');
  }
});
test('source-provided descriptions have a bounded preview without changing source text',()=>{
  const raw=node('Название','Свидетельство'),text='Содержание '.repeat(70);raw.display.summary={ru:text};raw.display.summary_state='authored';
  const preview=nodePreview({nodes:[raw],relations:[]},raw);
  expect(preview.body.length).toBeLessThanOrEqual(140);expect(preview.body.endsWith('…')).toBe(true);expect(raw.display.summary.ru).toBe(text);
});
test('missing titles remain explicit in star and relationship previews without rewriting their records',()=>{
  const raw=node('claim:tos.claim.opaque','Claim'),other=node('Источник','Произведение');
  raw.display.provenance={title:'identifier-fallback'};
  const relation={id:'r',from_id:raw.id,to_id:other.id,display:{label:{ru:'Связь'},inverse_label:{ru:'Обратная связь'}}};
  const packet={nodes:[raw,other],relations:[relation]},before=structuredClone(packet);
  expect(nodePreview(packet,raw).title).toBe('Claim · Нет читаемого названия');
  expect(nodePreview(packet,other).body).toBe('Обратная связь: Claim · Нет читаемого названия');
  expect(relationPreview(packet,relation).body).toBe('Claim · Нет читаемого названия → Источник');
  expect(packet).toEqual(before);
});

import {beforeAll,expect,it} from 'vitest';
import {createCoreView} from './core-view.mjs';

class Element {
  constructor(tag){this.tagName=tag;this.children=[];this.listeners={};this.dataset={};this.textContent='';this.open=false;}
  append(...items){this.children.push(...items);}
  replaceChildren(...items){this.children=[...items];}
  setAttribute(){}
  addEventListener(name,fn){this.listeners[name]=fn;}
  get childElementCount(){return this.children.length;}
  toggle(){this.open=true;this.listeners.toggle?.();}
}
const all=node=>[node,...node.children.flatMap(all)];
const text=node=>all(node).map(item=>item.textContent).join(' ');
const fold=(node,label)=>all(node).find(item=>item.tagName==='details'&&item.children[0]?.textContent===label);
beforeAll(()=>{globalThis.document={createElement:tag=>new Element(tag)};});

it('renders a reference route with its source states and only its declared branching transitions',()=>{
  const cite={w:'work',c:'I.01',u:['u1','u1']};
  const states=['A','B','C','D'].map(id=>({id,title:{ru:id},reading:{ru:`Чтение ${id}`},cite}));
  const steps=[['A','B'],['A','C'],['C','D']].map(([from,to],i)=>({id:`step-${i}`,from,to,verb:{ru:'сопоставляется с'},sense:{ru:`Сравнение ${from} и ${to}`},cite}));
  const core={records:Object.fromEntries(states.map(state=>[state.id,state])),materialIds:{},links:[],chains:[{id:'branch',title:{ru:'Семь различий'},sense:{ru:'Сопоставление сцен'},states,steps}]};
  const fragments={render(parent,value){if(value?.ru)parent.append(Object.assign(new Element('p'),{textContent:value.ru}));},fragment(){return Object.assign(new Element('p'),{textContent:'Точные слова свидетеля'});}};
  const root=new Element('div'),view=createCoreView({core,locale:()=> 'ru',fragments});
  expect(view.renderReference(root,{kind:'chain',id:'branch'})).toBe(true);
  for(const state of states)expect(text(root)).toContain(state.reading.ru);
  expect(text(root).match(/Точные слова свидетеля/g)).toHaveLength(4);
  const transitions=all(root).filter(item=>item.tagName==='details').map(item=>item.children[0].textContent);
  expect(transitions).toEqual(['A → сопоставляется с → B','A → сопоставляется с → C','C → сопоставляется с → D']);
  fold(root,transitions[1]).toggle();expect(text(root)).toContain('Сравнение A и C');
});

it('keeps proposed families, earlier readings and unassessed occurrences distinct',()=>{
  const cite={w:'work',c:'I.01',u:['u1','u1']};
  const figure={id:'speaker',title:{ru:'Говорящий'},existence:'work-world',work_scope:{roles:['speaks'],status:'stand-proposal'},names:{de:[{form:'Brüder',count:1}]},appearances:[{cite,role:'speaks',line:{ru:'Обращается к слушателям'}}]};
  const relation={id:'r',from:'speaker',to:'other',relation:'develops',verb:{ru:'обращается'},sense:{ru:'Текущее чтение'},relation_family:{family:'attribution-role',family_label:{ru:'атрибуция и роль'},family_reason:{ru:'Указана роль'},family_status:'stand-proposal',relation_type:'tos.relation.unmapped',family_proposed:{family:'event-change',family_reason:{ru:'Прежнее основание'}}},sense_revision_history:[{sense:{ru:'Прежнее чтение'},answer:{ru:'Прежний ответ',cite:[cite],status:'text'}}]};
  const core={records:{speaker:figure,other:{title:{ru:'Другой'}}},materialIds:{speaker:'speaker',other:'other'},links:[relation],chains:[],voices:{'I.01':[{u:['u1','u1'],speaker:'speaker',voice_descriptor:{cite,speaker_designation:'speaker',mode:'speech',layer:'primary',identity_status:'figure-candidate'}}]},semanticUses:{terms:[{lemma:'Bruder',forms:['Brüder'],scope:{ru:'Только указанные формы'},groups:[{label:{ru:'Обращение'},meaning:{ru:'К слушателям'},scope:{ru:'В предисловии'},grounds:[cite],review_status:'stand-proposal'}],occurrences:[{cite,semantic_use:{group_id:null,review_status:'unknown'}}]}]}};
  const fragments={render(parent,value){if(value?.ru)parent.append(Object.assign(new Element('p'),{textContent:value.ru}));},fragment(c){return Object.assign(new Element('p'),{textContent:`${c.c}:${c.u.join('–')}`});}};
  const view=createCoreView({core,locale:()=> 'ru',fragments});
  const rel=new Element('div');view.renderReference(rel,{kind:'relation',id:'r'});
  expect(text(rel)).toMatch(/Говорящий\s+обращается\s+Другой/);expect(text(rel)).not.toContain('develops');
  const family=fold(rel,'Семейство связи');family.toggle();
  expect(text(family)).toContain('атрибуция и роль');
  expect(text(family)).toContain('Точный тип не установлен');
  const history=fold(rel,'История');history.toggle();expect(text(history)).toContain('Прежнее чтение');expect(text(history)).toContain('Прежний ответ');expect(text(history)).toContain('Прежнее основание');
  const node=new Element('div');view.node(node,'speaker',{reference:true});
  expect(text(node)).toContain('Роль в произведении');
  const appearance=fold(node,'I.01 · Обращается к слушателям');appearance.toggle();expect(text(appearance)).toContain('Кандидат на фигуру произведения');expect(text(appearance)).toContain('I.01:u1–u1');
  const uses=fold(node,'Смысловые употребления');uses.toggle();expect(text(uses)).toContain('не все употребления слова автоматически относятся к этому предмету');
  const unknown=fold(uses,'Места без рассмотренного значения · 1');unknown.toggle();expect(text(unknown)).toContain('I.01:u1–u1');expect(text(unknown)).not.toContain('К слушателям');
  const group=fold(uses,'Обращение');group.toggle();expect(text(group)).toContain('К слушателям');expect(text(group)).toContain('Предложение стенда');
});

it('offers a bounded lexical lead even when the subject has no lexicon',()=>{
  const record={id:'going-under',title:{ru:'Закат и гибель'}},core={records:{'going-under':record},materialIds:{},links:[],chains:[],semanticUses:{terms:[{id:'untergehen',lemma:'untergehen',forms:['Untergang'],groups:[],occurrences:[]}]}};
  const view=createCoreView({core,locale:()=> 'ru',fragments:{render(){},fragment(){return new Element('p');}}});
  const root=new Element('div');view.node(root,'going-under',{reference:true});const uses=fold(root,'Смысловые употребления');uses.toggle();expect(text(uses)).toContain('untergehen');expect(text(uses)).toContain('как ориентир для исследования');
});

it('shows a proposal’s actual reading and grounds, and opens an addressed revised step',()=>{
  const cite={w:'also-sprach-zarathustra',c:'I.20',u:['u1','u1']};
  const proposal={id:'snake',title:{ru:'Змея'},title_revision_history:[{ru:'Гадюка'}],reading:{reading:{ru:'Змея лижет рану.'},grounds:[cite],limit:{ru:'Текст не говорит, что она слизывает яд.'},related_existing:'speaker',independent_review:{reviewer:'reader',verdict:'revise'},reading_revision_history:[{reading:{ru:'Слизывает яд.'},grounds:[cite],limit:{ru:'Прежняя гипотеза'}}]}};
  const step={id:'step-stable',from:'speaker',to:'snake',verb:{ru:'обращается к'},sense:{ru:'Текущее различение'},cite,step_revision_history:[{verb:{ru:'учится у'},sense:{ru:'Прежняя атрибуция'},cite}],method_transfer:{title:{ru:'Чья реплика?'},episode:{ru:'Сравнили реплику с ближайшим ответом.'},method:{ru:'Различить говорящего и адресата.'},applications:[{id:'jgb',title:{ru:'JGB §21'},result:{ru:'Два понятия подвергнуты критике.'},limit:{ru:'Это место не разрешает все вопросы о свободе.'},source_url:'https://www.gutenberg.org/cache/epub/7204/pg7204.html',source_label:'JGB §21 · Gutenberg'}]}};
  const core={records:{snake:proposal,speaker:{title:{ru:'Заратустра'}}},materialIds:{snake:'snake',speaker:'speaker'},links:[],chains:[{id:'chain',steps:[step]}]};
  const fragments={render(parent,value){if(value?.ru)parent.append(Object.assign(new Element('p'),{textContent:value.ru}));},fragment(value){return Object.assign(new Element('p'),{textContent:`Источник ${value.c}`});}};
  const actions=[],view=createCoreView({core,locale:()=> 'ru',fragments,onAction:value=>actions.push(value)}),node=new Element('div');
  view.node(node,'snake',{reference:true});
  expect(text(node)).toContain('Змея лижет рану.');expect(text(node)).toContain('Текст не говорит, что она слизывает яд.');expect(text(node)).toContain('Источник I.20');
  expect(text(node).match(/Змея лижет рану\./g)).toHaveLength(1);
  all(node).find(item=>item.tagName==='button'&&item.textContent==='Читать').listeners.click();expect(actions[0].cite).toEqual(cite);
  const original=fold(node,'История');original.toggle();expect(text(original)).toContain('Гадюка');expect(text(original)).toContain('Слизывает яд.');
  const relation=new Element('div');expect(view.renderReference(relation,{kind:'relation',id:'step-stable'})).toBe(true);
  const history=fold(relation,'История');history.toggle();expect(text(history)).toContain('Прежняя атрибуция');expect(text(history)).toContain('учится у');
  const method=fold(relation,'Чья реплика?');method.toggle();const application=fold(method,'JGB §21');application.toggle();
  expect(text(application)).toContain('Два понятия подвергнуты критике.');expect(text(application)).toContain('не разрешает все вопросы о свободе');
  expect(all(application).find(item=>item.tagName==='a').href).toBe('https://www.gutenberg.org/cache/epub/7204/pg7204.html');
});

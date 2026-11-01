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

it('keeps proposed families, earlier readings and unassessed occurrences distinct',()=>{
  const cite={w:'work',c:'I.01',u:['u1','u1']};
  const figure={id:'speaker',title:{ru:'Говорящий'},existence:'work-world',work_scope:{roles:['speaks'],status:'stand-proposal'},names:{de:[{form:'Brüder',count:1}]},appearances:[{cite,role:'speaks',line:{ru:'Обращается к слушателям'}}]};
  const relation={id:'r',from:'speaker',to:'other',relation:'develops',verb:{ru:'обращается'},sense:{ru:'Текущее чтение'},relation_family:{family:'attribution-role',family_label:{ru:'атрибуция и роль'},family_reason:{ru:'Указана роль'},family_status:'stand-proposal',relation_type:'tos.relation.unmapped',family_proposed:{family:'event-change',family_reason:{ru:'Прежнее основание'}}},sense_revision_history:[{sense:{ru:'Прежнее чтение'}}]};
  const core={records:{speaker:figure,other:{title:{ru:'Другой'}}},materialIds:{speaker:'speaker',other:'other'},links:[relation],chains:[],voices:{'I.01':[{u:['u1','u1'],speaker:'speaker',voice_descriptor:{cite,speaker_designation:'speaker',mode:'speech',layer:'primary',identity_status:'figure-candidate'}}]},semanticUses:{terms:[{lemma:'Bruder',forms:['Brüder'],scope:{ru:'Только указанные формы'},groups:[{label:{ru:'Обращение'},meaning:{ru:'К слушателям'},scope:{ru:'В предисловии'},grounds:[cite],review_status:'stand-proposal'}],occurrences:[{cite,semantic_use:{group_id:null,review_status:'unknown'}}]}]}};
  const fragments={render(parent,value){if(value?.ru)parent.append(Object.assign(new Element('p'),{textContent:value.ru}));},fragment(c){return Object.assign(new Element('p'),{textContent:`${c.c}:${c.u.join('–')}`});}};
  const view=createCoreView({core,locale:()=> 'ru',fragments});
  const rel=new Element('div');view.renderReference(rel,{kind:'relation',id:'r'});
  expect(text(rel)).toContain('Отношение · develops');
  expect(text(rel)).toContain('атрибуция и роль');
  expect(text(rel)).toContain('Точный тип не установлен');
  const history=fold(rel,'История');history.toggle();expect(text(history)).toContain('Прежнее чтение');expect(text(history)).toContain('Прежнее основание');
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

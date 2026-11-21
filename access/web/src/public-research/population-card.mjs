// The card of a Population star: a member, a research node or a region, read from the bonsai itself. A member's card
// says what the registers and the line's readers say of the person, each with its source and footing; nothing on it
// is admitted to the Tree, and its verdict stays undecided.
import {batchOfRow,memberMaterial,nodeMaterial,REGION_NAMES} from './population-library.mjs';

// Written as {ru, en} so that the space's catalogs are checked to hold every word in every language.
const COPY={
  author:{ru:'Автор',en:'Author'},
  pseudonymous:{ru:'Псевдоавтор',en:'Pseudonymous author'},
  conventional:{ru:'Традиционный автор',en:'Traditional author'},
  'attributed-voice':{ru:'Голос, которому приписан текст',en:'A voice a text is attributed to'},
  'unnamed-hand':{ru:'Безымянный автор',en:'Unnamed author'},
  collective:{ru:'Коллектив',en:'Collective'},
  node:{ru:'Узел исследования',en:'Research node'},
  region:{ru:'Регион',en:'Region'},
  names:{ru:'Имена',en:'Names'},
  years:{ru:'Годы',en:'Years'},
  noYears:{ru:'Годы неизвестны.',en:'The years are not known.'},
  verdict:{ru:'Вердикт читателя: не вынесен. Ничто здесь ещё не принято в Древо.',en:'Reader’s verdict: not given. Nothing here is admitted to the Tree yet.'},
  low:{ru:'основание низкое: слова реестра',en:'low footing: the register’s words'},
  medium:{ru:'основание среднее: два читателя по источнику',en:'medium footing: two readers from a source'},
  rows:{ru:'Строки реестров',en:'Register rows'},
  founded:{ru:'основная строка',en:'founding row'},
  gaps:{ru:'Чего не хватает',en:'What is missing'},
  'name:latin':{ru:'нет имени латиницей',en:'no name in Latin script'},
  'life:unchecked':{ru:'годы в реестре не проверены',en:'the register has not checked the years'},
  'life:unread':{ru:'запись годов ещё не прочитана',en:'the years are not read yet'},
  'life:none':{ru:'реестр не даёт годов',en:'the register gives no years'},
  'life:rows-differ':{ru:'строки расходятся в годах',en:'the rows differ on the years'},
  'core:ambiguous':{ru:'таблица авторов неоднозначна',en:'the authors table is ambiguous'},
  table:{ru:'Таблица',en:'Table'},
  tableI:{ru:'древность',en:'antiquity'},
  tableII:{ru:'600–1500',en:'600–1500'},
  tableIII:{ru:'Новое время',en:'modern times'},
  formation:{ru:'Формирование',en:'Formation'},
  fixation:{ru:'Письменная фиксация',en:'Written fixation'},
  canonization:{ru:'Канонизация',en:'Canonization'},
  windows:{ru:'Окна времени',en:'Windows of time'},
  regionWords:{ru:'Макрорегион в мастер-таблице',en:'Macro-region in the master table'},
  members:{ru:'Авторы',en:'Authors'},
  nodes:{ru:'Узлы',en:'Nodes'},
  more:{ru:'и ещё',en:'and'},
  moreTail:{ru:'',en:'more'},
};
const STATUS={verified:{ru:'проверено реестром',en:'verified by the register'},provisional:{ru:'предварительно',en:'provisional'},disputed:{ru:'спорно',en:'disputed'}};
const ROLE={core:{ru:'основной',en:'core'},continuation:{ru:'продолжение',en:'continuation'},supporting:{ru:'опора',en:'supporting'},same_tradition:{ru:'та же традиция',en:'same tradition'},antecedent:{ru:'предшественник',en:'antecedent'},transmission:{ru:'передача',en:'transmission'}};
const LIST_LIMIT=60;

const el=(tag,cls,text)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(text!=null)node.textContent=text;return node;};
const kindOfMaterial=material=>String(material?.id??'').split(':')[1];

/** The small line over a Population star's title: what kind of entry it is. */
export function populationEyebrow(material,tr){
  const kind=kindOfMaterial(material),said=kind==='member'?(material.memberKind==='person'?COPY.author:COPY[material.memberKind]??COPY.author):kind==='node'?COPY.node:COPY.region;
  return el('p','eyebrow ps-population-kind',tr(said));
}

/** Fill a card with what the Population says of a member, a node or a region; false when the material is none of them. */
export function renderPopulationCard(into,{material,bonsai,tr,open}){
  const kind=kindOfMaterial(material),population=bonsai?.population,chronology=bonsai?.chronology;
  if(!population)return false;
  const link=(materialId,text)=>{const button=el('button','connection-target',text);button.type='button';button.addEventListener('click',()=>open(materialId));return button;};
  const section=(titleKey,cls='ps-population-section')=>{const node=el('section',cls);node.append(el('h2','minor-title',tr(COPY[titleKey])));into.append(node);return node;};
  const list=(parent,ids,make)=>{const shown=ids.slice(0,LIST_LIMIT),box=el('div','ps-branch-list');for(const id of shown)box.append(make(id));parent.append(box);if(ids.length>shown.length)parent.append(el('p','muted',`${tr(COPY.more)} ${ids.length-shown.length} ${tr(COPY.moreTail)}`.trim()));};
  if(kind==='member'){
    const member=population.members[material.legacyId];if(!member)return false;
    const names=[member.name.ru,member.name.en,member.name.original].filter((name,i,all)=>name&&all.indexOf(name)===i);
    if(names.length>1){const box=section('names');box.append(el('p','body',names.join(' · ')));}
    const years=section('years');
    if(!member.phases.length)years.append(el('p','muted',tr(COPY.noYears)));
    for(const phase of member.phases){
      const situation=chronology?.situations?.[phase];if(!situation)continue;
      years.append(el('p','body',tr(situation.statement)));
      const source=chronology.sources?.[situation.sources?.[0]],line=el('p','ps-source-line');
      if(source?.url){const a=el('a','source-link',`${source.short} ↗`);a.href=source.url;a.target='_blank';a.rel='noopener noreferrer';line.append(a);}
      else if(source?.short)line.append(source.short);
      line.append(` · ${tr(situation.footing==='medium'?COPY.medium:COPY.low)}`);years.append(line);
    }
    into.append(el('p','muted ps-population-verdict',tr(COPY.verdict)));
    const rows=section('rows');
    const byRow=member.register?.rows??{};
    for(const row of member.grew_from){
      const batch=batchOfRow(row),node=population.batches[batch]?.node,said=byRow[row]??{};
      const line=el('div','ps-link-row');
      line.append(link(nodeMaterial(batch),node?tr(node.title):batch));
      const facts=[row.split(':')[1],said.status?tr(STATUS[said.status]??{ru:said.status,en:said.status}):'',said.role?tr(ROLE[said.role]??{ru:said.role,en:said.role}):'',row===member.founded?tr(COPY.founded):''].filter(Boolean).join(' · ');
      line.append(el('span','muted',facts));rows.append(line);
    }
    if(member.gaps.length){const box=section('gaps');box.append(el('p','body',member.gaps.map(gap=>tr(COPY[gap]??{ru:gap,en:gap})).join('; ')));}
    return true;
  }
  if(kind==='node'){
    const batch=material.batch,node=population.batches[batch]?.node;if(!node)return false;
    const about=el('p','ps-source-line',[`${tr(COPY.table)} ${node.table}: ${tr(COPY[`table${node.table}`])}`,tr(REGION_NAMES[node.region]??population.regions[node.region]),batch].join(' · '));
    into.append(about);
    const windows=section('windows');
    for(const key of ['formation','fixation','canonization'])windows.append(el('p','body',`${tr(COPY[key])}: ${node.windows[key].words.split('\n')[0]}`));
    windows.append(el('p','muted',`${tr(COPY.regionWords)}: ${node.region_words}`));
    const members=Object.entries(population.members).filter(([,member])=>member.grew_from.some(row=>batchOfRow(row)===batch)).sort(([,a],[,b])=>tr(a.name).localeCompare(tr(b.name)));
    const box=section('members');box.querySelector('h2').textContent=`${tr(COPY.members)} · ${members.length}`;
    list(box,members.map(([id])=>id),id=>link(memberMaterial(id),tr(population.members[id].name)));
    return true;
  }
  if(kind==='region'){
    const region=material.region,batches=Object.entries(population.batches).filter(([,entry])=>entry.node?.region===region);
    const box=section('nodes');box.querySelector('h2').textContent=`${tr(COPY.nodes)} · ${batches.length}`;
    list(box,batches.map(([batch])=>batch),batch=>link(nodeMaterial(batch),tr(population.batches[batch].node.title)));
    return true;
  }
  return false;
}

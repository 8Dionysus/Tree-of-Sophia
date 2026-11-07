// What an article of the book says, by the kind of its subject. Each kind reads the fields its stand authored —
// a branch its moves and the turns between them, a move its reading and the steps of its reasoning, a passage the
// text itself and the readings that rest on it, a question the answers of its voices, a word its uses — and leaves
// out what is not there. The stand's own words lead; the places they rest on stand beside them.

import {localeValue} from './locale.mjs';
import {roleWord} from './reference-book.mjs';

const make=(tag,cls='',text)=>{const node=document.createElement(tag);if(cls)node.className=cls;if(text!==undefined&&text!==null)node.textContent=text;return node;};
const said=(value,locale)=>{const text=typeof value==='string'?value:localeValue(value,locale);return typeof text==='string'?text.trim():'';};
const cites=value=>Array.isArray(value?.cite)?value.cite.filter(Boolean):value?.cite&&typeof value.cite==='object'?[value.cite]:[];

/**
 * Write the body of the article `id` into `body` and its lead into `head`. `kit` is the book's own tools
 * (claim, section, field, link, done, w); `data` reads the stand's materials.
 */
export function articleBody({head,body,id,item,claim,section,field,link,sign,done,w},data){
  const locale=data.locale(),tr=value=>said(value,locale);
  const lead=(value,cls='rb-lead')=>{if(!value||(!tr(value)&&!cites(value).length))return;const box=make('div',cls);claim(box,value,{className:'rb-lead-text'});head.append(box);};
  const ask=value=>{const text=tr(value);if(text)head.append(make('p','rb-question',text));};
  const places=(parent,value)=>{if(cites(value).length)claim(parent,{cite:cites(value)},{grounds:'places'});};
  // A question a claim leaves stands under its words, beside the places they rest on.
  const after=(said,parent,node)=>(said?.classList?.contains('rb-claim')&&!said.classList.contains('is-places')?said:parent).append(node);
  // Names the article leads to once: a list leaves out what the head and the body already lead to.
  const links=(parent,ids,label)=>{const shown=new Set([...head.querySelectorAll('[data-open]'),...body.querySelectorAll('[data-open]')].map(node=>node.dataset.open)),list=[...new Set(ids.filter(Boolean))].filter(other=>!shown.has(other));if(!list.length){if(parent.classList.contains('rb-section-body'))done(parent);return;}const row=field(parent,label),names=make('p','rb-chips');for(const other of list)names.append(link(other,undefined,'rb-chip'));row.append(names);};

  // A subject of the core: its question, its lead and the book's own formula, then the parts of its card.
  if(item.owner==='bonsai/core'&&data.coreBook&&(item.kind==='chain'||data.coreRecord(id))){
    ask(data.coreBook.question(id));
    const box=make('div','rb-lead');if(data.coreBook.lead(box,id))head.append(box);
    data.coreBook.epigraph(head,id);
    data.coreBook.bookBody(body,id);
    return;
  }
  if(item.kind==='relation'){data.coreBook?.renderReference(body,{kind:'relation',id});return;}
  if(item.kind==='statement'){lead(item.record);if(item.parentId){const row=field(body,'');row.append(link(item.parentId));}return;}
  if(item.kind==='passage'){body.append(data.fragments.fragment(item.record));return;}
  if(item.kind==='bibliographic-pointer')return;

  const material=data.material(id);if(!material)return;
  const route=data.route(id);
  // A branch or a path reads as a chapter: its question, its moves in order and the turns between them. The moves
  // where a branch stays open carry that mark: their questions are where the branch arrives.
  if(route){
    ask(route.question);
    if(tr(route.description))lead({ru:tr(route.description),en:tr(route.description)});
    const steps=route.steps??[],turns=route.transitions??[];
    const open=new Set((data.branch(id)?.openEnds??[]).map(value=>data.passageOf(typeof value==='string'?value:value.ru??value.en)).filter(Boolean));
    const moves=section(body,'moves',w(data.profile==='antiquity'?'moves':'steps')),list=make('ol','rb-moves');
    for(const [at,step] of steps.entries()){
      const stepId=data.idOf(step.nodeId),entry=make('li','rb-move');
      const name=make('h3','rb-move-title');name.append(make('span','rb-move-n',String(at+1)),stepId?link(stepId,tr(step.title)||undefined,'rb-link'):make('span','',tr(step.title)));entry.append(name);
      // The source of a move opens the place itself.
      const witness=stepId?data.sourceLine(stepId):'',place=data.has(step.sourceNodeId)?step.sourceNodeId:null;
      if(witness)entry.append(place?link(place,witness,'rb-move-source'):make('p','rb-move-source',witness));
      const said=claim(entry,step.body);
      if(tr(step.question)){
        const question=make('p','rb-move-question',tr(step.question));
        if(open.has(stepId)){entry.classList.add('is-open');question.prepend(make('span','rb-move-end',w('openEnd')));}
        after(said,entry,question);
      }
      list.append(entry);
      const next=steps[at+1];if(!next)continue;
      const turn=turns.find(item=>(item.fromNodeId??item.from)===step.nodeId&&(item.toNodeId??item.to)===next.nodeId)??turns.find(item=>item.from===step.nodeId&&item.to===next.nodeId);
      if(turn){
        const style=data.turnStyle(turn),row=make('li','rb-turn');row.style.setProperty('--link-color',style.color??'#a5b7cf');row.dataset.dash=style.dash??'solid';
        const word=make('p','rb-turn-kind');word.append(make('span','rb-link-line'),make('span','',style.word??''));if(tr(turn.label))word.append(make('span','rb-turn-label',tr(turn.label)));row.append(word);
        // A turn rests on the places of the two moves it joins; its own words say what the turn does.
        if(tr(turn.body))row.append(make('p','rb-turn-text',tr(turn.body)));list.append(row);
      }
    }
    moves.append(list);
    const questions=data.questionsOf(id);
    if(questions.length){const box=section(body,'questions',w('branchQuestions')),list=make('ul','rb-asks');for(const question of questions){const row=make('li','rb-ask');row.append(link(question.id));if(tr(question.line))row.append(make('p','rb-ask-line',tr(question.line)));list.append(row);}box.append(list);}
    for(const part of body.querySelectorAll(':scope > .rb-section > .rb-section-body'))done(part);
    return;
  }
  // A word of the trunk: the lemma, what it means, where it is used and its kin. A use names its form where the
  // form differs from the lemma, and the place it is used in.
  if(material.lexical||material.lemma){
    const lemma=make('p','rb-lemma',material.lemma);if(material.language)lemma.lang=material.language==='grc'?'grc':material.language;head.append(lemma);
    lead(material.body);ask(material.question);
    if(material.uses?.length){
      const box=section(body,'uses',w('uses')),lemmaKey=String(material.lemma??'').toLocaleLowerCase();
      for(const use of material.uses){
        const entry=make('div','rb-use'),line=make('p','rb-use-head'),where=[data.workTitle(use.work),use.citation].filter(Boolean).join(' · '),place=data.passageOf(use.part);
        if(use.form&&use.form.toLocaleLowerCase()!==lemmaKey){const form=make('span','rb-use-form',use.form);if(lemma.lang)form.lang=lemma.lang;line.append(form);}
        if(where)line.append(place?link(place,where,'rb-use-source'):make('span','rb-use-source',where));
        if(line.childElementCount)entry.append(line);
        claim(entry,use.body);box.append(entry);
      }
    }
    if(material.aliases?.length){const row=field(section(body,'forms',w('otherForms')),'');row.append(make('p','rb-forms',material.aliases.join(', ')));}
    links(body,(material.relatedIds??[]).map(data.catalogId),w('relatedWords'));
    return;
  }
  const voice=tr(material.voice);if(voice)head.append(make('p','rb-voice',voice));
  // The place of a text: the text itself in the original and in translation, then what is read in it.
  if(material.kind==='fragment'){
    const text=section(body,'passage',w('passage'));
    const reading=data.readingsOf(id)[0];
    const cited=cites(material.body).length?material.body:{cite:cites(data.material(reading)?.body)};
    data.fragments.render(text,{cite:cites(cited)});
    if(tr(material.body))claim(text,{ru:tr(material.body),en:tr(material.body)});
    reasoning(body,material);
    ask(material.question);
    const readings=data.readingsOf(id);
    if(readings.length){const box=section(body,'readings',w('readingsOfPlace')),list=make('ul','rb-asks');for(const other of readings){const row=make('li','rb-ask');row.append(link(other));const line=data.glimpse(other);if(line)row.append(make('p','rb-ask-line',line));list.append(row);}box.append(list);}
    links(body,(material.concepts??[]).map(data.catalogId),w('concepts'));
    links(body,(material.about??[]).map(data.catalogId),w('about'));
    if(material.coreId)links(body,[data.idOf(material.coreId)],w('reading'));
    return;
  }
  // A question of the dome: its line, then the answers the voices give, each with the place it rests on.
  if(material.kind==='question'&&material.positions?.length){
    if(typeof material.prompt==='string'&&material.prompt.trim())ask(material.prompt);
    lead(material.body);
    const box=section(body,'voices',w('voices'));
    for(const position of material.positions){
      const row=field(box,tr(position.voice),'rb-voice-answer');
      if(tr(position.title))row.append(make('h3','rb-step-title',tr(position.title)));
      claim(row,position.body);
      const place=data.passageOf(position.passage);if(place)row.append(sign(place,link(place,undefined,'rb-place-link')));
    }
    links(body,(material.concepts??[]).map(data.catalogId),w('concepts'));
    return;
  }
  // A move, a reading, a concept, a thinker, a work, a school: the reading first, then how it goes on.
  const named=tr(material.title).trim();
  if(material.kind!=='interpretation'&&tr(material.question).trim()!==named)ask(material.question);
  const words=material.atlasBody??material.body;
  if(tr(words))lead(words);
  else if(cites(words).length)places(section(body,'places',w('places')),words);
  if(material.workMap){
    const map=material.workMap,box=section(body,'course',w('courseOfWork'));
    if(tr(map.question))box.append(make('p','rb-course-question',tr(map.question)));
    const list=make('ol','rb-moves');
    for(const [at,stage] of (map.stages??[]).entries()){
      const entry=make('li','rb-move'),name=make('h3','rb-move-title');name.append(make('span','rb-move-n',String(at+1)),make('span','',tr(stage.title)));if(stage.span)name.append(make('span','rb-move-span',stage.span));entry.append(name);
      claim(entry,stage.body);
      const parts=(stage.parts??[]).map(data.passageOf).filter(Boolean);if(parts.length){const row=make('div','rb-places');for(const part of parts)row.append(sign(part,link(part,undefined,'rb-place-link')));entry.append(row);}
      list.append(entry);
    }
    box.append(list);
  }
  reasoning(body,material);
  if(material.kind==='interpretation'&&tr(material.question)){const box=body.querySelector('[data-part="reasoning"] > .rb-section-body')??body;const row=field(box,w('openQuestion'),'rb-open');row.append(make('p','rb-open-question',tr(material.question)));}
  // The argument of an inquiry, its answer, another reading of it and the details to follow, under one part;
  // words the lead already says are not said twice.
  const inquiry=material.inquiry,leadText=tr(words).trim();
  if(inquiry||material.perspectives?.length){
    const box=section(body,'argument',w('argument')),argument=inquiry?.thesis??inquiry?.argument;
    if(tr(argument)&&tr(argument).trim()!==leadText)claim(field(box,''),argument);
    if(inquiry?.answer&&tr(inquiry.answer.text??inquiry.answer))claim(field(box,w('answer')),inquiry.answer.text??inquiry.answer);
    if(inquiry?.counterReading?.text)claim(field(box,w('otherReading')),inquiry.counterReading.text);
    for(const perspective of material.perspectives??[])claim(field(box,tr(perspective.title)),perspective.body);
    done(box);
  }
  if(material.kind==='figure'){const works=data.worksOf(material);if(works.length)links(section(body,'works',w('works')),works,'');}
  if(material.kind==='work'){const passages=data.placesOf(id);if(passages.length)links(section(body,'work-places',w('places')),passages,'');}
  links(body,(material.kind==='interpretation'?[]:material.concepts??[]).map(data.catalogId),w('concepts'));
  if(material.coreId&&material.kind!=='fragment')links(body,[data.idOf(material.coreId)],w('reading'));

  // The steps of a reasoning: what each does in the argument, its words and the place it rests on, the question it leaves.
  function reasoning(parent,source){
    const steps=source.argument?.steps?.length?source.argument.steps:source.readingSteps??[];if(!steps.length)return;
    const box=section(parent,'reasoning',w(source.argument?.steps?.length?'reasoning':'readingSteps')),list=make('ol','rb-reasoning');
    for(const step of steps){
      const entry=make('li','rb-step'),role=roleWord(step.role??source.argument?.roles?.[step.id],locale);
      if(role)entry.append(make('p','rb-step-role',role));
      const text=make('div','rb-step-text');if(tr(step.title))text.append(make('h3','rb-step-title',tr(step.title)));
      // A step said in the source's own words shows them whole; a step said in the stand's words shows its grounds beside them.
      const said=tr(step.body)?claim(text,step.body):cites(step.body).length?data.fragments.render(text,{cite:cites(step.body)}):null;
      if(tr(step.question))after(said,text,make('p','rb-step-question',tr(step.question)));
      entry.append(text);list.append(entry);
    }
    box.append(list);
  }
}

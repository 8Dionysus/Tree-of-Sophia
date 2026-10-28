// Presentation rules that keep the public research spaces (Zarathustra and
// Antiquity) readable. They name what the authored data already says; they
// never add a relation, source or claim, and they stay outside the constructor
// library, its fingerprint and saved graphs.

const bi=(ru,en)=>({ru,en});
const lang=locale=>String(locale||'ru').startsWith('en')?'en':'ru';
const pick=(value,locale)=>typeof value==='string'?value:value?.[lang(locale)]??value?.en??value?.ru??'';

// The shared relation kinds, as every other space draws them. Kinds fall into
// three families by line: grounds and continuations are solid, readings and
// resonances dotted, tensions dashed; weight separates kinds inside a family.
// The word and colour stay the shared ones from the constructor.
export const SHARED_RELATION_LINES=Object.freeze({
  contains:Object.freeze({dash:'solid',weight:.8,hint:bi('целое и его часть','a whole and its part')}),
  supports:Object.freeze({dash:'solid',weight:1.15,hint:bi('служит основанием','serves as grounds')}),
  develops:Object.freeze({dash:'solid',weight:1.6,hint:bi('следующий шаг той же мысли','the next step of the same thought')}),
  translates:Object.freeze({dash:'solid',weight:.8,hint:bi('тот же текст на другом языке','the same text in another language')}),
  relates:Object.freeze({dash:'solid',weight:.65,hint:bi('связь без уточнённого вида','a relation of unspecified kind')}),
  interprets:Object.freeze({dash:'dotted',weight:1.15,hint:bi('предлагает прочтение места или мысли','offers a reading of a passage or a thought')}),
  echoes:Object.freeze({dash:'dotted',weight:1.6,hint:bi('сходный мотив в другом материале','a similar motif in other material')}),
  compares:Object.freeze({dash:'dotted',weight:.85,hint:bi('ставит рядом для сравнения','sets side by side for comparison')}),
  questions:Object.freeze({dash:'dashed',weight:1.15,hint:bi('открывает трудность, которую надо решить','opens a difficulty to be worked out')}),
  contrasts:Object.freeze({dash:'dashed',weight:1.9,hint:bi('другой ответ на тот же вопрос','a different answer to the same question')}),
});

// The earlier Antiquity tree authored four kinds of transition. The shared graph
// carries them as generic relation kinds; this table restores their own names.
// Each kind differs by word, line pattern and line weight, never by hue alone.
export const ANTIQUITY_TRANSITIONS=Object.freeze({
  develops:Object.freeze({id:'continuation',...bi('Продолжение','Continuation'),hint:bi('следующий шаг той же мысли','the next step of the same thought'),color:'#d9b66d',dash:'solid',weight:1.15}),
  questions:Object.freeze({id:'qualification',...bi('Уточнение','Qualification'),hint:bi('ограничивает или поправляет предыдущий ход','limits or corrects the previous move'),color:'#86b6e8',dash:'dashed',weight:1.15}),
  contrasts:Object.freeze({id:'contrast',...bi('Контраст','Contrast'),hint:bi('другой ответ на тот же вопрос','a different answer to the same question'),color:'#ef8c62',dash:'solid',weight:1.9}),
  echoes:Object.freeze({id:'analogy',...bi('Аналогия','Analogy'),hint:bi('сходный ход в другом материале','a similar move in other material'),color:'#b9a6ee',dash:'dotted',weight:1.5}),
});

/** An authored step-to-step transition of an Antiquity branch, as projected into the graph. */
export const isAuthoredTransition=edge=>/(?:^|:)antiquity:relation:/.test(String(edge?.id??''));

/** Display style of a relation kind: the profile's own vocabulary where it has one. */
export function relationStyle(profile,kind,base={}){
  const own=profile==='antiquity'?ANTIQUITY_TRANSITIONS[kind]:null;
  if(own)return {...base,...own};
  // Every other kind keeps the shared word and colour, with a line of its family.
  const line=SHARED_RELATION_LINES[kind];
  return {...base,dash:base.dash??line?.dash??'solid',weight:base.weight??line?.weight??1,...(base.hint||!line?{}:{hint:line.hint})};
}

/** "Author · Work · Place" of one authored bibliographic reference. */
export function referenceLine(reference,locale){
  if(!reference)return '';
  return [pick(reference.author,locale),pick(reference.work,locale),pick(reference.locator,locale)].map(value=>String(value??'').trim()).filter(Boolean).join(' · ');
}

/**
 * The witnesses a route step names. In Zarathustra a step belongs to a concept
 * or a thinker shared by several routes, so the place lives on the step's own
 * grounds, never on the node.
 */
export function stepWitness(step,{references={},locale}={}){
  const lines=[];
  for(const ground of step?.grounds??[]){const line=referenceLine(references?.[ground?.ref],locale);if(line&&!lines.includes(line))lines.push(line);}
  return lines;
}

/** Words for the parts of a branch. A route of the earlier tree is a branch; its steps are moves. */
export function partName(profile,material,locale,{moveIds=null}={}){
  if(profile!=='antiquity'||!material)return '';
  if(material.route||String(material.id??'').startsWith('antiquity:route:'))return pick(bi('Ветвь','Branch'),locale);
  // A passage is a move only where a branch actually steps through it.
  if(moveIds?.has(material.id)||(!moveIds&&material.kind==='interpretation'&&String(material.id??'').startsWith('antiquity:reading:')))return pick(bi('Ход','Move'),locale);
  return '';
}

/** "Author · Work · Place" for a move, from the bibliographic source it already cites. */
export function sourceLine(material,{materials,entryByLegacyId,locale}={}){
  if(!material)return '';
  const source=materials?.get?.(material.sourceNodeId)??null;
  // The authored passage names its own work (Epicurus' Letter), even where the
  // public witness is a containing book (Diogenes Laertius X); prefer it.
  const reading=String(material.id??'').startsWith('antiquity:reading:'),passage=reading?entryByLegacyId?.get?.(String(material.id).slice('antiquity:reading:'.length)):null;
  const workId=passage?.workId??material.workId??source?.workId;
  const work=workId?entryByLegacyId?.get?.(workId):null;
  const authorId=passage?.author??material.author??work?.author;
  const author=authorId?entryByLegacyId?.get?.(authorId):null;
  const authorName=pick(author?.title,locale)||pick(work?.authorLabel,locale);
  const workTitle=pick(work?.title,locale);
  const locator=pick(passage?.meta,locale)||(reading||material.kind==='passage'?pick(material.meta,locale):'')||pick(source?.locator,locale);
  if(workTitle||locator)return [authorName,workTitle,locator].filter(Boolean).join(' · ');
  // Fall back to the citation label carried by the move itself.
  const cited=(material.sourceRefs??[]).map(ref=>pick(ref?.label,locale)).find(Boolean)??'';
  return [authorName,cited].filter(Boolean).join(' · ');
}

/** Search order: the object's own name first, then its question, then its prose. */
export function searchScore(item,query,{locale,authorTitle=''}={}){
  const fold=value=>String(value??'').toLocaleLowerCase().replaceAll('ё','е').normalize('NFKD').replace(/[\u0300-\u036f]/g,'').replace(/\s+/g,' ').trim();
  const q=fold(query);if(!q)return 0;
  const words=q.split(' ').filter(Boolean);
  const fields=[[item?.title,6],[item?.question,3],[authorTitle,3],[item?.atlasBody,1],[item?.body,1]];
  let best=0;
  for(const [value,weight]of fields){
    const text=fold(pick(value,locale));if(!text)continue;
    const score=text===q?4:text.startsWith(q)?3:text.includes(q)?2:words.length>1&&words.every(w=>text.includes(w))?1:0;
    best=Math.max(best,score*weight);
  }
  // A branch outranks a material of equal match, since it opens the whole inquiry.
  return best?best+(item?.route?.steps?.length?1:0):0;
}

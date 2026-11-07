// The book of a stand: its contents, its library and the ways through them.
//
// The contents read the tree the way the lens "The tree" stands it up, from the crown to the roots: the branches
// of inquiry and their questions, then the concepts and words of the trunk, then the texts. The library is the
// roots told in full: the eras, who wrote in them, what they wrote and the places the tree rests on. The path above
// an article and the article before and after it are read from these two lists, so the book grows as they grow.
//
// An entry is {key, id?, title, meta?, line?, n?, children?, questions?}: `id` is the research address an article
// opens at, `title`, `meta` and `line` are authored forms ({ru, en}) or plain text, `n` numbers the moves of a
// branch and the steps of a path, which are read in their order.

const bi=(ru,en)=>({ru,en});
const plain=value=>typeof value==='string'?value:value?.ru??value?.en??'';
const said=value=>{if(!value||typeof value!=='object')return typeof value==='string'&&value.trim()?value:null;return Object.entries(value).some(([key,text])=>key!=='cite'&&typeof text==='string'&&text.trim())?value:null;};
const byId=list=>new Map((list??[]).map(item=>[item.id,item]));
// One entry per name: two subjects that share a title are listed once.
const unique=entries=>{const seen=new Set();return entries.filter(entry=>{const name=plain(entry.title);if(seen.has(name))return false;seen.add(name);return true;});};

export const BOOK_PARTS=Object.freeze({
  crown:bi('Ветви и вопросы','Branches and questions'),paths:bi('Пути и вопросы','Paths and questions'),
  concepts:bi('Понятия','Concepts'),words:bi('Слова','Words'),images:bi('Образы','Images'),figures:bi('Фигуры','Figures'),
  proposed:bi('Предложенные предметы','Proposed subjects'),questions:bi('Вопросы','Questions'),
  texts:bi('Тексты','Texts'),book:bi('Книга','The book'),
});

/**
 * The book of Antiquity. `routes` are the stand's branches as the space walks them, `collection` its bonsai
 * collection, `materialOf(id)` the research address of a collection entry, `areaTitles` the regions of the crown in
 * the order the tree stands them, `schoolEra` the era of each school and `lifeStart(authorId)` the first year of an
 * author's life, for the order of the authors in their era.
 */
export function antiquityBook({routes=[],collection={},materialOf=()=>null,areaTitles={},schoolEra={},lifeStart=()=>null}={}){
  const routeById=byId(routes),questionById=byId(collection.questions),entities=collection.entities??[];
  const kind=value=>entities.filter(item=>item.kind===value);
  const homeOfQuestion=new Map();
  const areas=Object.entries(areaTitles).map(([area,title])=>({key:area,title,children:(collection.branches??[]).filter(branch=>branch.area===area).map(branch=>{
    const route=routeById.get(branch.id),id=`antiquity:route:${branch.id}`;
    const children=(route?.steps??[]).map((step,index)=>({key:`${branch.id}:move:${index}`,id:step.nodeId,title:step.title,line:said(step.body),n:index+1}));
    const questions=(branch.questions??[]).map(plain).map(questionId=>{
      const question=questionById.get(questionId),address=materialOf(questionId);if(!question||!address)return null;
      if(!homeOfQuestion.has(address))homeOfQuestion.set(address,id);
      return {key:`${branch.id}:question:${questionId}`,id:address,title:question.title,line:said(question.body)};
    }).filter(Boolean);
    return {key:branch.id,id,title:route?.title??branch.rootTitle,meta:branch.title,line:branch.question,children,questions,walk:'moves'};
  })})).filter(area=>area.children.length);
  // The questions no branch asks stand after the branches, as the tree's dome holds them.
  const loose=(collection.questions??[]).map(question=>({key:`question:${question.id}`,id:materialOf(question.id),title:question.title,line:said(question.body)})).filter(entry=>entry.id&&!homeOfQuestion.has(entry.id));
  if(loose.length)areas.push({key:'questions',title:BOOK_PARTS.questions,children:loose});
  const concepts=kind('concept').map(concept=>({key:`concept:${concept.id}`,id:materialOf(concept.id),title:concept.title,line:said(concept.question)})).filter(entry=>entry.id);
  const words=(collection.terms??[]).map(term=>({key:`word:${term.id}`,id:materialOf(`lexical:${term.id}`),title:term.lemma||term.title,meta:term.title,line:said(term.question)})).filter(entry=>entry.id);
  // The roots: eras in their order, the authors of each in the order of their lives, their works and the places.
  const passagesOf=new Map();for(const passage of collection.passages??[]){if(!passagesOf.has(passage.workId))passagesOf.set(passage.workId,[]);passagesOf.get(passage.workId).push(passage);}
  const works=kind('work'),authors=kind('author');
  const workEntry=work=>({key:`work:${work.id}`,id:materialOf(work.id),title:work.title,meta:work.meta,children:(passagesOf.get(work.id)??[]).map(passage=>({key:`passage:${passage.id}`,id:materialOf(passage.id),title:passage.title,meta:passage.meta})).filter(entry=>entry.id)});
  const library=(collection.eras??[]).map(era=>({key:era.id,title:era.title,meta:era.meta,line:said(era.body),children:[
    ...authors.filter(author=>author.period===era.period).sort((a,b)=>(lifeStart(a.id)??0)-(lifeStart(b.id)??0)).map(author=>({key:`author:${author.id}`,id:materialOf(author.id),title:author.title,meta:author.meta,children:works.filter(work=>work.author===author.id).map(workEntry)})),
    ...kind('school').filter(school=>schoolEra[school.id]===era.period).map(school=>({key:`school:${school.id}`,id:materialOf(school.id),title:school.title,meta:school.meta,school:true})),
  ]})).filter(era=>era.children.length);
  const parts=[
    {key:'crown',title:BOOK_PARTS.crown,stratum:'crown',children:areas},
    {key:'concepts',title:BOOK_PARTS.concepts,stratum:'concepts',children:concepts,flat:true},
    {key:'words',title:BOOK_PARTS.words,stratum:'words',children:words,flat:true},
    {key:'texts',title:BOOK_PARTS.texts,stratum:'texts',children:library.map(era=>({...era,children:era.children.filter(entry=>!entry.school).map(author=>({...author,children:[]}))})),library:true},
  ].filter(part=>part.children.length);
  return createBook({parts,library});
}

/**
 * The book of Zarathustra. `core` is the stand's core, `routes` the paths of its atlas, `questions` the library's
 * questions, `chapters` the book's chapters in order, `partTitles` the names of its four parts and `idOf(materialId)`
 * the research address of a library material.
 */
export function zarathustraBook({core={},routes=[],questions=[],chapters=[],partTitles={},idOf=id=>id,citing=()=>[]}={}){
  const records=core.records??{},title=id=>records[id]?.title;
  const line=record=>said(record?.conception?.thesis??record?.essence?.who??record?.reading?.reading??record?.question);
  const subject=record=>({key:record.id,id:record.id,title:record.title,line:line(record)});
  const chains=new Map();for(const chain of core.chains??[]){if(chain.admission!=='map')continue;if(!chains.has(chain.from))chains.set(chain.from,[]);chains.get(chain.from).push(chain);}
  const withChains=record=>({...subject(record),children:(chains.get(record.id)??[]).map(chain=>({key:chain.id,id:chain.id,title:chain.title,line:said(chain.question??chain.sense)}))});
  const concepts=(core.concepts??[]).filter(record=>record.type==='tos.entity.conception'),images=[...(core.concepts??[]),...(core.proposed??[])].filter(record=>record.type==='tos.entity.thought-image');
  const proposed=(core.proposed??[]).filter(record=>record.type!=='tos.entity.thought-image');
  const paths=routes.map(route=>({key:`path:${route.id}`,id:null,route:route.id,title:route.title,line:said(route.question??route.description),children:(route.steps??[]).map((step,index)=>({key:`path:${route.id}:${index}`,id:idOf(step.nodeId),title:step.title??title(idOf(step.nodeId)),line:said(step.body),n:index+1}))}));
  const asked=questions.map(question=>({key:`question:${question.id}`,id:idOf(question.id),title:question.title,line:said(question.inquiry?.argument)}));
  const byTitle=(a,b)=>plain(a.title).localeCompare(plain(b.title),'ru');
  const parts=[
    {key:'paths',title:BOOK_PARTS.paths,stratum:'crown',children:[...paths,...(asked.length?[{key:'questions',title:BOOK_PARTS.questions,children:asked}]:[])]},
    {key:'concepts',title:BOOK_PARTS.concepts,stratum:'concepts',children:concepts.map(withChains).sort(byTitle)},
    {key:'images',title:BOOK_PARTS.images,stratum:'images',children:images.map(withChains).sort(byTitle),flat:true},
    {key:'figures',title:BOOK_PARTS.figures,stratum:'figures',children:(core.figures??[]).map(withChains).sort(byTitle),flat:true},
    {key:'proposed',title:BOOK_PARTS.proposed,stratum:'crown',children:proposed.map(subject).sort(byTitle),flat:true},
  ].filter(part=>part.children.length);
  // The book's parts and chapters, each chapter with the subjects that rest on it. A subject that rests on a quarter
  // of the chapters or more runs through the book: it is named once above them, and a chapter lists the subjects
  // that rest on it in particular, the rarest first.
  const cited=new Map(chapters.map(chapter=>[chapter.key,unique(citing(chapter.key).map(id=>({key:`${chapter.key}:${id}`,id,title:title(id)})).filter(entry=>entry.title))]));
  const count=new Map();for(const list of cited.values())for(const entry of list)count.set(entry.id,(count.get(entry.id)??0)+1);
  const through=[...count].filter(([,n])=>n>=chapters.length/4).sort((a,b)=>b[1]-a[1]).map(([id])=>({key:`through:${id}`,id,title:title(id)}));
  const common=new Set(through.map(entry=>entry.id));
  const library=['I','II','III','IV'].map(part=>({key:`part:${part}`,title:partTitles[part]??part,children:chapters.filter(chapter=>chapter.key.startsWith(`${part}.`)).map(chapter=>({key:`chapter:${chapter.key}`,chapter:chapter.key,title:chapter.titles,children:(cited.get(chapter.key)??[]).filter(entry=>!common.has(entry.id)).sort((a,b)=>count.get(a.id)-count.get(b.id))}))}));
  parts.push({key:'book',title:BOOK_PARTS.book,stratum:'texts',children:library.map(part=>({...part,children:[]})),library:true});
  return createBook({parts,library,through});
}

/** The ways through a book: where an address stands in it, and the entries before and after it. `through` names the
 * subjects that run through the whole library. */
export function createBook({parts=[],library=[],through=[]}={}){
  // An address is at home where the contents list it as itself, then where a branch or a path steps through it,
  // then in the library. An article's path and its neighbours come from its home.
  const homes=new Map(),steps=new Map(),shelves=new Map();
  const visit=(entries,trail,source)=>{for(const [index,entry] of entries.entries()){
    if(entry.id){const place={entry,trail,siblings:entries,index,source},own=source==='library'?shelves:Number.isInteger(entry.n)?steps:homes;if(!own.has(entry.id))own.set(entry.id,place);}
    if(entry.children?.length)visit(entry.children,[...trail,entry],source);
    if(entry.questions?.length)visit(entry.questions,[...trail,entry],source);
  }};
  for(const part of parts)if(!part.library)visit(part.children,[part],'contents');
  visit(library,[],'library');
  const home=id=>homes.get(id)??steps.get(id)??shelves.get(id)??null;
  return {parts,library,through,home,
    /** The entries above an address, from its part of the book down to its own parent. */
    trail(id){const found=home(id);return found?found.trail:[];},
    /** The entry before and after an address among its siblings that have articles; a numbered sequence says its place. */
    neighbours(id){
      const found=home(id);if(!found)return null;
      const list=found.siblings.filter(entry=>entry.id),at=list.indexOf(found.entry);
      return {previous:list[at-1]??null,next:list[at+1]??null,index:at,total:list.length,numbered:Number.isInteger(found.entry.n),parent:found.trail.at(-1)??null};
    },
    /** Every address an entry holds, its own and those under it. */
    addresses(entry){const out=[];const walk=item=>{if(item.id)out.push(item.id);for(const child of [...(item.children??[]),...(item.questions??[])])walk(child);};walk(entry);return out;},
  };
}

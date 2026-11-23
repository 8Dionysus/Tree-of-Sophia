// The surface an agent shapes on this page while the person watches (В7, TOS-D-0089): blocks in the dock beside the
// space, a strip above it, the end of a star's card and light on the sky, bound to what the page already reads (the
// act journal's folds, the research index, the selection, the view, the agents' field). An agent works it like clay:
// one patch of small operations, applied all or none by the shared Rust rules (tos-web-rules, surface.rs), drawn at
// once and answered with what was drawn, so the next patch can follow without a screenshot. Where the person's bridge
// serves the page, the bridge holds the surface and agents shape it over MCP; the page mirrors it, draws it and sends
// back what it drew.
//
// The surface is presentation only: it writes no source, review, rights or canon, carries text and addresses but no
// markup, styles, scripts or outside links, and is framed as the work of who shaped it. The person freezes, undoes,
// folds or clears it at any time and keeps it as an act of their journal when it earns a place.
import './surface.css';
import {localeText,localeValue,resolveLocale} from './locale.mjs';
import {fieldOf} from './agent-field.mjs';

let Rules=null;
const encoder=new TextEncoder(),decoder=new TextDecoder();
const bytes=value=>encoder.encode(JSON.stringify(value));
const parse=value=>JSON.parse(decoder.decode(value));
const EMPTY=new Uint8Array();

/** Installed by the browser entry once the generated rules have initialized. */
export function installSurfaceRules(runtime){
  if(typeof runtime?.SurfaceV1!=='function')throw new TypeError('The surface rules are unavailable.');
  Rules=runtime.SurfaceV1;
}

/** A refused patch, with the rules' code, message, op and place. */
export class SurfaceRefusal extends Error{
  constructor(value){
    const detail=typeof value==='object'&&value?value:{error:'surface_refused',message:String(value)};
    super(detail.message??detail.error);this.name='SurfaceRefusal';Object.assign(this,detail);
  }
  toJSON(){return {error:this.error,message:this.message,...(this.op!==undefined?{op:this.op}:{}),...(this.at?{at:this.at}:{})};}
}
const refusal=error=>{
  if(error instanceof SurfaceRefusal)return error;
  if(error&&!(error instanceof Error)&&typeof error==='object'&&error.error)return new SurfaceRefusal(error);
  const text=String(error?.message??error);
  try{return new SurfaceRefusal(JSON.parse(text));}catch{return new SurfaceRefusal({error:error?.code??'surface_refused',message:text});}
};

/** The surface's contract as the rules carry it. */
export function surfaceContract(){
  if(!Rules)throw new TypeError('The surface rules are unavailable.');
  return parse(Rules.contract());
}

/**
 * The surface held in this page (tier 0): the rules' machine, kept for the tab's session so a reload keeps the clay.
 * Every method answers with the parsed result; a refusal throws SurfaceRefusal.
 */
export function createLocalSurface({storage=globalThis.sessionStorage,key='tos.surface.v1'}={}){
  if(!Rules)throw new TypeError('The surface rules are unavailable.');
  const machine=new Rules();
  try{const saved=storage?.getItem(key);if(saved)machine.mirror(encoder.encode(saved));}catch{}
  const listeners=new Set();
  const changed=state=>{try{storage?.setItem(key,JSON.stringify(state));}catch{}for(const listener of listeners)listener(state);return state;};
  const run=fn=>{try{return fn();}catch(error){throw refusal(error);}};
  return {
    tier:'browser',machine,
    state:async()=>parse(machine.state()),
    patch:async request=>{const out=run(()=>parse(machine.patch(bytes(request))));changed(out.state);return out;},
    open:async(content,author)=>changed(run(()=>parse(machine.open(bytes(content),bytes(author))))),
    freeze:async frozen=>changed(parse(machine.freeze(Boolean(frozen)))),
    undo:async()=>changed(parse(machine.undo())),
    clear:async()=>changed(parse(machine.clear())),
    receipt:async()=>{},
    subscribe(listener){listeners.add(listener);return ()=>listeners.delete(listener);},
    dispose(){listeners.clear();machine.free?.();},
  };
}

/**
 * The surface the person's bridge holds (tier 1): the bridge applies every patch with the same rules and announces
 * each revision; this page mirrors the state into its own machine to draw it and sends back what it drew.
 */
export function createBridgeSurface(channel){
  if(!Rules)throw new TypeError('The surface rules are unavailable.');
  const machine=new Rules();
  const listeners=new Set();
  const take=state=>{machine.mirror(bytes(state));for(const listener of listeners)listener(state);return state;};
  const call=async body=>{try{return await channel.apply(body);}catch(error){throw refusal(error.detail??error);}};
  let stop=()=>{};
  const ready=channel.state().then(take).catch(()=>null);
  stop=channel.listen(()=>{void channel.state().then(state=>{if(state.revision!==parse(machine.state()).revision)take(state);}).catch(()=>{});});
  return {
    tier:'bridge',machine,ready,
    state:async()=>{await ready;return parse(machine.state());},
    patch:async request=>{const out=await call({patch:request});take(out.state);return out;},
    open:async(content,author)=>take(await call({open:{content,author}})),
    freeze:async frozen=>take(await call({freeze:Boolean(frozen)})),
    undo:async()=>take(await call({undo:true})),
    clear:async()=>take(await call({clear:true})),
    receipt:receipt=>channel.receipt(receipt).catch(()=>{}),
    subscribe(listener){listeners.add(listener);return ()=>listeners.delete(listener);},
    dispose(){listeners.clear();stop();machine.free?.();},
  };
}

/**
 * The page's surface: the bridge's when the person's bridge serves the page and keeps the journal, this tab's
 * otherwise. The choice follows the act journal's (bridge.mjs openActJournal); every method waits for it.
 */
export function openSurface({acts=null,storage=globalThis.sessionStorage,key='tos.surface.v1'}={}){
  let current=null;const pending=new Set();
  const chosen=Promise.resolve(acts?.chosen).catch(()=>null).then(journal=>{
    const channel=journal?.surfaceChannel?.();
    current=channel?createBridgeSurface(channel):createLocalSurface({storage,key});
    for(const listener of pending)listener.stop=current.subscribe(listener.fn);
    return current;
  });
  const wait=name=>(...args)=>chosen.then(surface=>surface[name](...args));
  return {
    get tier(){return current?.tier??null;},
    get machine(){return current?.machine??null;},
    ready:chosen.then(surface=>surface.ready),
    state:wait('state'),patch:wait('patch'),open:wait('open'),freeze:wait('freeze'),undo:wait('undo'),clear:wait('clear'),
    receipt:receipt=>current?.receipt(receipt),
    subscribe(fn){if(current)return current.subscribe(fn);const listener={fn,stop:null};pending.add(listener);return ()=>{pending.delete(listener);listener.stop?.();};},
    dispose(){void chosen.then(surface=>surface.dispose());},
  };
}

const labels={
  surface:['Поверхность агента','The agent\'s surface'],
  shapedBy:['лепит','shaped by'],
  fold:['Свернуть','Fold'],
  unfold:['Развернуть','Unfold'],
  freeze:['Заморозить: агент больше не меняет','Freeze: the agent changes nothing more'],
  thaw:['Разморозить','Let it change again'],
  frozen:['заморожено','frozen'],
  undo:['Вернуть, как было','Undo'],
  clear:['Убрать поверхность','Clear the surface'],
  keep:['Оставить в журнале','Keep it in the journal'],
  kept:['Поверхность оставлена в журнале','The surface is kept in the journal'],
  empty:['Пусто','Nothing yet'],
  unreadable:['не прочитано','not read'],
  surfaceSky:['Свет агента','The agent\'s light'],
  menu:['Поверхность агента','The agent\'s surface'],
  untitled:['Без названия','Untitled'],
};

const sourceDefaults={journal:{limit:1000}};

/**
 * Draw the surface on the page and give agents its tools. `surface` is a local or bridge surface; `acts` the act
 * journal; `space` the public space; `registry()` the page command registry; `viewState()` the action layer's view.
 */
export function mountSurface({shell,surface,acts=null,space=null,registry=()=>null,viewState=()=>null,layer=()=>null,actTier=()=> 'propose',locale=()=> 'ru',now=()=>Date.now()}){
  const doc=shell.ownerDocument;
  const ru=()=>resolveLocale(locale())==='ru';
  const t=key=>ru()?labels[key][0]:localeText(labels[key][1],locale());
  const el=(tag,cls='',value)=>{const node=doc.createElement(tag);if(cls)node.className=cls;if(value!==undefined)node.textContent=value;return node;};
  const button=(value,cls='text-button',title)=>{const node=el('button',cls,value);node.type='button';if(title){node.title=title;node.setAttribute('aria-label',title);}return node;};
  const nameOf=id=>{const item=space?.index?.get?.(id);return item?localeValue(item.title,locale()):id;};

  const dock=el('aside','sf-dock');dock.hidden=true;dock.setAttribute('aria-label',t('surface'));
  const head=el('div','sf-head'),titleNode=el('strong','sf-title'),byline=el('span','sf-by'),tools=el('div','sf-tools');
  let body=el('div','sf-body');
  const foldButton=button('▾','icon sf-fold',t('fold')),freezeButton=button('❄','icon sf-freeze',t('freeze')),undoButton=button('↶','icon sf-undo',t('undo')),keepButton=button('✓','icon sf-keep',t('keep')),clearButton=button('×','icon sf-clear',t('clear'));
  tools.append(foldButton,undoButton,freezeButton,keepButton,clearButton);
  const label=el('div','sf-label');label.append(titleNode,byline);head.append(label,tools);dock.append(head,body);
  const strip=el('div','sf-strip');strip.hidden=true;
  shell.append(dock,strip);
  // On a phone the dock starts folded to its head, so the sky stays the person's; the agent reads `folded` in the answer.
  let folded=Boolean(doc.defaultView?.matchMedia?.('(max-width:760px)')?.matches),state=null,lastReceipt=null,drawing=null,again=false,disposed=false,cardHost=null,cardId=null,skyKey='null';
  const waiters=[];

  // ------------------------------------------------------------------ data
  const selectedId=()=>space?.selection?.()?.id??null;
  const resolve=(value,selected)=>value==='$selected'?selected:value;
  async function records(limit=2000){const rows=[];let before=Number.MAX_SAFE_INTEGER;while(rows.length<limit){const batch=await acts.recent({before,limit:500});if(!batch.length)break;rows.push(...batch.map(row=>row.record));before=batch.at(-1).seq;if(batch.length<500)break;}return rows;}
  const subject=id=>{const item=space?.index?.get?.(id);return item?{id:item.id,kind:item.kind,label:localeValue(item.title,locale())}:{id,kind:'unknown',label:id};};
  /** The answer of a bind's source, before the rules shape it. */
  async function read(bind,selected){
    switch(bind.source){
      case 'journal':{
        if(!acts)throw new Error('the act journal is unavailable here');
        const request={view:bind.view,now_ms:now(),...sourceDefaults.journal};
        for(const key of ['address','id','window_ms'])if(bind[key]!==undefined)request[key]=resolve(bind[key],selected);
        if(request.view==='target'&&!request.address)request.address=selected;
        return acts.query(request);
      }
      case 'search':return (space?.index?.search?.(resolve(bind.query,selected),{limit:50})??[]).map(item=>({id:item.id,kind:item.kind,label:localeValue(item.title,locale())}));
      case 'item':{
        const id=resolve(bind.id,selected);const item=id?space?.index?.inspect?.(id):null;if(!item)return null;
        return {id:item.id,kind:item.kind,label:localeValue(item.title,locale()),owner:item.owner,
          dependencies:item.dependencies.map(row=>({...subject(row.id),kind:row.kind})),
          backlinks:item.referencedBy.map(row=>({id:row.id,kind:row.kind,label:localeValue(row.title,locale())}))};
      }
      case 'selected':return selected?subject(selected):null;
      case 'view':return viewState();
      case 'field':{
        if(!acts)throw new Error('the act journal is unavailable here');
        const field=fieldOf(await records());
        return field.items.filter(item=>(!bind.shape||item.shape===bind.shape)&&(!bind.kind||item.kind===bind.kind));
      }
      default:return null;
    }
  }

  // ------------------------------------------------------------------ drawing
  const select=target=>{if(!target)return;const commands=registry();if(commands)void commands.invoke('tos.page.select',{item_id:target}).catch(()=>layer()?.say?.(`${t('unreadable')}: ${target}`));};
  const clickable=(node,target)=>{if(!target)return node;node.classList.add('sf-target');node.tabIndex=0;node.setAttribute('role','button');
    node.addEventListener('click',()=>select(target));node.addEventListener('keydown',event=>{if(event.key==='Enter'||event.key===' '){event.preventDefault();select(target);}});return node;};
  function drawBlock(view,into){
    const box=el('div',`sf-block sf-${view.kind}`);box.dataset.block=view.id;
    switch(view.kind){
      case 'heading':box.append(el('h3','',view.text));break;
      case 'text':box.dataset.tone=view.tone;box.append(el('p','',view.text));break;
      case 'metric':box.append(el('span','sf-value',view.value),el('span','sf-metric-label',view.label));break;
      case 'list':{const list=el('ul','sf-list');for(const item of view.items??[]){const row=clickable(el('li'),item.target);row.append(el('span','sf-item-title',item.title));if(item.meta)row.append(el('span','sf-meta',item.meta));list.append(row);}box.append(list);break;}
      case 'table':{const table=el('table','sf-table'),head=el('tr');for(const title of view.head??[])head.append(el('th','',title));table.append(head);
        for(const line of view.rows??[]){const row=clickable(el('tr'),line.target);for(const cell of line.cells)row.append(el('td','',cell));table.append(row);}box.append(table);break;}
      case 'bars':for(const item of view.items??[]){const row=clickable(el('div','sf-bar'),item.target),fill=el('span','sf-bar-fill');fill.style.setProperty('--share',String(item.share));row.append(el('span','sf-bar-label',item.label),fill,el('span','sf-value',item.value));box.append(row);}break;
      case 'chips':for(const item of view.items??[]){const chip=button(item.label===item.target?nameOf(item.target):item.label,'sf-chip');chip.addEventListener('click',()=>select(item.target));box.append(chip);}break;
      case 'button':{const run=button(view.label,'primary sf-run');run.addEventListener('click',()=>{const commands=registry();if(commands)void commands.invoke(view.command,view.input??{}).catch(error=>layer()?.say?.(String(error?.message??error)));});box.append(run);break;}
      default:break;
    }
    if(view.empty&&!(view.items?.length||view.rows?.length))box.append(el('p','sf-empty',view.empty));
    into.append(box);return box;
  }
  /** Draw a list of blocks; groups recurse. Returns each block's receipt line. */
  async function drawList(blocks,into,place,data,lines){
    for(const block of blocks){
      const line={id:block.id,kind:block.kind,place};
      try{
        const view=parse(surface.machine.view(block.id,data.get(block.id)??EMPTY));
        if(view.count!==undefined){line.count=view.count;line.total=view.total;}
        if(view.notes)line.notes=view.notes;
        if(block.kind==='group'){
          const group=el('details','sf-block sf-group');group.dataset.block=block.id;group.open=view.open;group.append(el('summary','',view.title));
          into.append(group);await drawList(block.children??[],group,place,data,lines);
        }else if(place==='sky')line.view=view;
        else drawBlock(view,into);
      }catch(error){line.error=String(error?.message??error);const box=el('p','sf-block sf-error',`${block.id}: ${t('unreadable')}`);box.dataset.block=block.id;into.append(box);}
      lines.push(line);
    }
  }
  /**
   * Read every bind's source once. An answer is kept while what it depends on stands: the journal's length for the
   * journal and the field (and a five-second window for folds that read the clock), the selection and the language for
   * the index. So a patch that only changes how a block looks draws without reading its source again. A failed read
   * is named on its block.
   */
  const answers=new Map();
  const CLOCKED=new Set(['presence','squads','leases']);
  const epochOf=(bind,selected)=>{
    const journal=bind.source==='journal'||bind.source==='field';
    const usesSelected=Object.values(bind).includes('$selected')||(bind.source==='journal'&&bind.view==='target'&&!bind.address);
    return [journal?acts?.size??0:'',journal&&CLOCKED.has(bind.view)?Math.floor(now()/5000):'',usesSelected||bind.source==='selected'?selected:'',bind.source==='journal'?'':locale()].join('|');
  };
  async function gather(places,selected,lines){
    const data=new Map();
    const binds=parse(surface.machine.binds());
    await Promise.all(binds.map(async({id,place,bind})=>{
      if(place==='card'&&!selected)return;
      if(bind.source==='rows'){data.set(id,EMPTY);return;}
      const key=JSON.stringify([bind.source,bind.view,bind.address,bind.id,bind.query,bind.window_ms,bind.shape,bind.kind]),epoch=epochOf(bind,selected);
      let held=answers.get(key);
      if(bind.source==='view'||held?.epoch!==epoch){held={epoch,answer:read(bind,selected).then(value=>bytes(value??null))};answers.set(key,held);}
      try{data.set(id,await held.answer);}
      catch(error){if(answers.get(key)===held)answers.delete(key);lines.push({id,place,error:`source ${bind.source}: ${String(error?.message??error)}`});}
    }));
    if(answers.size>64)for(const key of [...answers.keys()].slice(0,answers.size-64))answers.delete(key);
    return data;
  }
  function paintCard(host,id){
    cardHost=host;cardId=id;
    host.querySelector(':scope > .sf-card')?.remove();
    if(!state||!state.places.card.length)return;
    const box=el('section','sf-card');box.setAttribute('aria-label',`${t('surface')}: ${state.title||t('untitled')}`);
    box.append(el('p','sf-card-by',`${t('shapedBy')} ${who(state)}`));host.append(box);
    void draw();
  }
  const who=value=>(value.authors??[]).slice(0,2).map(author=>[author.id,author.model].filter(Boolean).join(' · ')).join(', ')||'—';
  const removeCard=space?.actions?.addCardBlock?.((host,id)=>paintCard(host,space?.selection?.()?.id??id))??(()=>{});

  async function drawNow(){
    const started=performance.now();
    state=await surface.state();
    const lines=[];
    const selected=selectedId();
    const data=await gather(state.places,selected,lines);
    // The dock: title, who shapes it and the person's controls, then the blocks.
    const dockBlocks=state.places.dock;
    dock.hidden=!dockBlocks.length;dock.dataset.folded=String(folded);dock.dataset.frozen=String(state.frozen);
    titleNode.textContent=state.title||t('surface');
    byline.textContent=`${t('shapedBy')} ${who(state)}${state.frozen?` · ${t('frozen')}`:''}`;
    foldButton.textContent=folded?'▸':'▾';foldButton.title=t(folded?'unfold':'fold');
    freezeButton.setAttribute('aria-pressed',String(state.frozen));freezeButton.title=t(state.frozen?'thaw':'freeze');
    undoButton.disabled=!state.can_undo;keepButton.disabled=!acts||!state.blocks;
    const dockBody=el('div','sf-body');await drawList(dockBlocks,dockBody,'dock',data,lines);body.replaceWith(dockBody);body=dockBody;body.hidden=folded;
    // The strip above the space.
    const stripBody=doc.createDocumentFragment();await drawList(state.places.strip,stripBody,'strip',data,lines);
    strip.replaceChildren(stripBody);strip.hidden=!state.places.strip.length;
    if(state.places.strip.length)shell.dataset.surfaceStrip='';else delete shell.dataset.surfaceStrip;
    // The card of the selected star.
    const card=cardHost?.isConnected?cardHost.querySelector(':scope > .sf-card'):null;
    if(card&&cardId){const cardBody=el('div','sf-card-body');await drawList(state.places.card,cardBody,'card',data,lines);card.querySelector('.sf-card-body')?.remove();card.append(cardBody);}
    else if(state.places.card.length&&cardHost?.isConnected===false)cardHost=null;
    if(state.places.card.length&&!card)space?.actions?.refreshCard?.();
    // Light on the sky: the union of its highlights and links.
    const skyLines=[];await drawList(state.places.sky,doc.createDocumentFragment(),'sky',data,skyLines);lines.push(...skyLines);
    const views=skyLines.map(line=>line.view).filter(Boolean);
    const members=views.flatMap(view=>view.members??[]),relations=views.flatMap(view=>view.relations??[]);
    // The sky is painted again only when its light changed: a patch elsewhere leaves it be.
    const light=members.length||relations.length?{title:`${t('surfaceSky')}: ${views.map(view=>view.title).filter(Boolean).join(' · ')||state.title||who(state)}`,members,relations,dimRest:views.some(view=>view.dim_rest)}:null;
    const lightKey=JSON.stringify(light);
    if(lightKey!==skyKey){skyKey=lightKey;space?.actions?.setSurfaceSky?.(light,{off:()=>{skyKey=null;void surface.patch({author:{id:'person',type:'human'},ops:[{op:'clear',in:'sky'}]}).catch(()=>{});}});}
    // The sky's chip stands in the strip when there is one, so neither covers the heading.
    const chip=space?.actions?.surfaceSkyChip?.();
    if(chip){if(state.places.strip.length&&!chip.hidden)strip.append(chip);else if(chip.parentNode===strip)shell.append(chip);}
    for(const line of skyLines){if(line.view){line.count=line.count??(line.view.members??line.view.relations??[]).length;delete line.view;}}
    lastReceipt={revision:state.revision,ms:Math.round(performance.now()-started),drawn_at:new Date(now()).toISOString(),
      watching:!doc.hidden,folded,blocks:lines,...(selected?{selected}:{})};
    void surface.receipt(lastReceipt);
    for(let i=waiters.length-1;i>=0;i--)if(waiters[i].revision<=state.revision){waiters[i].resolve(lastReceipt);waiters.splice(i,1);}
    return lastReceipt;
  }
  /** One drawing at a time; a change during one draws once more after it. */
  function draw(){
    if(disposed)return Promise.resolve(lastReceipt);
    if(drawing){again=true;return drawing;}
    drawing=drawNow().catch(error=>{console.error('Surface',error);return lastReceipt;}).finally(()=>{drawing=null;if(again){again=false;void draw();}});
    return drawing;
  }
  /** What was drawn at or after a revision. */
  function drawnAt(revision,timeoutMs=3000){
    if(lastReceipt&&lastReceipt.revision>=revision&&!drawing)return Promise.resolve(lastReceipt);
    return new Promise(resolve=>{const waiter={revision,resolve};waiters.push(waiter);
      setTimeout(()=>{const at=waiters.indexOf(waiter);if(at>=0){waiters.splice(at,1);resolve(lastReceipt);}},timeoutMs);});
  }

  // ------------------------------------------------------------------ the person's controls
  const say=text=>layer()?.say?.(text);
  foldButton.addEventListener('click',()=>{folded=!folded;void draw();});
  freezeButton.addEventListener('click',()=>void surface.freeze(!state?.frozen).then(()=>draw()));
  undoButton.addEventListener('click',()=>void surface.undo().then(()=>draw()));
  clearButton.addEventListener('click',()=>void surface.clear().then(()=>draw()));
  async function keep(author={type:'human',id:'local-person'},title){
    const current=await surface.state();
    const content={title:title||current.title||t('untitled'),places:current.places};
    const id=(content.title.toLowerCase().replace(/[^a-z0-9а-яё._-]+/gi,'-').replace(/^-+|-+$/g,'')||'surface').slice(0,48);
    // A later keep under the same title replaces the earlier one.
    const earlier=(await records(2000)).find(record=>record.kind==='surface'&&record.targets?.includes(`surface:${id}`));
    return acts.act({kind:'surface',author,targets:[`surface:${id}`],intent:content.title.slice(0,200),body:{title:content.title,surface:content},...(earlier?{replaces:earlier.id}:{})});
  }
  keepButton.addEventListener('click',()=>void keep().then(()=>say(t('kept'))).catch(error=>say(String(error?.message??error))));

  const stopSurface=surface.subscribe(()=>void draw());
  let journalTimer=null;
  const stopJournal=acts?.subscribe?.(()=>{if(!surface.machine||!parse(surface.machine.binds()).some(item=>item.bind.source==='journal'||item.bind.source==='field'))return;clearTimeout(journalTimer);journalTimer=setTimeout(()=>void draw(),300);})??(()=>{});
  let removeAction=null;
  void Promise.resolve(surface.ready).then(()=>draw());

  // ------------------------------------------------------------------ agents' tools
  const schema=(properties,required=[])=>({type:'object',properties,required,additionalProperties:false});
  const text=value=>({content:[{type:'text',text:JSON.stringify(value)}]});
  const writes=()=>actTier()!=='read';
  const agent=input=>({type:'agent',id:String(input.author?.id??'page-agent').slice(0,128),...(input.author?.model?{model:String(input.author.model).slice(0,128)}:{})});
  const refuse=error=>({...text(refusal(error).toJSON()),isError:true});
  const notAllowed={...text({error:'surface_tier',message:'the person lets agents only read; patches wait until they allow drafts or proposals'}),isError:true};
  const authorSchema={type:'object',properties:{id:{type:'string'},model:{type:'string'}},required:['id'],additionalProperties:false};
  const tool=(name,description,inputSchema,execute,readOnlyHint=false)=>({name,description,inputSchema,annotations:{readOnlyHint,untrustedContentHint:true},execute});
  const agentTools=[
    tool('tos.surface.describe','Read first: the places, blocks, sources, transforms, templates and operations of the surface you shape on this page, its limits, the current surface with its revision, and what was last drawn. Shape it with tos.surface.patch; the answer says what each block drew, its rows, and the fields its templates asked for that no row had.',
      schema({}),async()=>text({contract:surfaceContract(),state:await surface.state(),drawn:lastReceipt,tier:surface.tier,may_patch:writes(),selected:selectedId()}),true),
    tool('tos.surface.patch','Change the surface with a list of operations applied all or none: put {block, in?, parent?, after?|before?|at?}, set {id, field, value}, move {id, in?|parent?|after?|before?|at?}, remove {id}, clear {in?}, title {text}. The answer comes after the page drew it: the revision, what changed, and per block its rows, total and notes. A refusal names the op, the place and what to write instead. Pass expect_revision when several agents shape the same surface.',
      schema({ops:{type:'array',minItems:1,maxItems:64,items:{type:'object'}},author:authorSchema,expect_revision:{type:'integer',minimum:0}},['ops']),
      async input=>{if(!writes())return notAllowed;try{const out=await surface.patch({ops:input.ops,author:agent(input),...(input.expect_revision!==undefined?{expect_revision:input.expect_revision}:{})});void draw();const drawn=await drawnAt(out.revision);return text({revision:out.revision,changed:out.changed,removed:out.removed,drawn});}catch(error){return refuse(error);}}),
    tool('tos.surface.try','Try a bind without placing it: the first rows of its source as the bind shapes them, the fields they have, the total and notes. Use it to find a path, a field or a filter before a patch.',
      schema({bind:{type:'object'}},['bind']),async input=>{try{const data=input.bind?.source==='rows'?null:await read(input.bind??{},selectedId());return text(parse(Rules.tryBind(bytes(input.bind??{}),data===null?EMPTY:bytes(data))));}catch(error){return refuse(error);}},true),
    tool('tos.surface.state','Read the surface as it stands and what the page drew last.',schema({}),async()=>text({state:await surface.state(),drawn:lastReceipt}),true),
    tool('tos.surface.undo','Return the surface to the state before the last change.',schema({}),async()=>{if(!writes())return notAllowed;const state=await surface.undo().catch(refuse);if(state?.isError)return state;void draw();return text({state,drawn:await drawnAt(state.revision)});}),
    tool('tos.surface.keep','Keep the surface as an act of the person\'s journal (kind surface), to open again later; a later keep under the same title replaces it. A kept surface is an agent\'s act: name your model in author.model.',
      schema({title:{type:'string',maxLength:200},author:authorSchema}),async input=>{if(!writes()||!acts)return notAllowed;if(!input.author?.model)return refuse({error:'act_invalid:author.model',message:'a kept surface is an agent\'s act: name your model in author.model'});try{const row=await keep(agent(input),input.title);return text({kept:row?.record?.id??row?.id??null,seq:row?.seq??null});}catch(error){return refuse(error);}}),
    tool('tos.surface.open','Open a kept surface (an act of kind surface) on the page; undo returns to the one before.',
      schema({act:{type:'string'},author:authorSchema},['act']),async input=>{if(!writes()||!acts)return notAllowed;try{const record=await acts.record(input.act);if(record?.kind!=='surface')throw new SurfaceRefusal({error:'surface_unknown',message:`no surface act ${input.act}`});const state=await surface.open(record.body.surface,agent(input));void draw();return text({state,drawn:await drawnAt(state.revision)});}catch(error){return refuse(error);}}),
  ];

  return {
    tools:agentTools,draw,drawnAt,keep,
    /** The menu's way to the surface, once the action layer stands. */
    attach(actionLayer){removeAction=actionLayer?.addActions?.([{id:'surface',text:()=>t('menu'),for:['any'],when:()=>Boolean(state?.blocks),run:()=>{folded=false;void draw().then(()=>dock.querySelector('button')?.focus());}}])??null;},
    receipt:()=>lastReceipt,
    surface,
    destroy(){disposed=true;clearTimeout(journalTimer);stopSurface();stopJournal();removeCard();removeAction?.();space?.actions?.setSurfaceSky?.(null);dock.remove();strip.remove();cardHost?.querySelector(':scope > .sf-card')?.remove();delete shell.dataset.surfaceStrip;surface.dispose?.();},
  };
}

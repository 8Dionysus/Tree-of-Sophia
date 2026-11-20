// What a ticket or a proposal can be checked for on the device, before any agent: that its subject resolves,
// that it rests on grounds, that the word it would change is there to change, and whether it repeats one already
// filed. The checks only report; the person decides. Each check is {id, level: 'ok'|'warn'|'block', detail}.

const SKIP=new Set(['grounds','cite','u','q','w','c','s','e','id','from','to','path','statement_ids','maker','admission','relation','relation_family','order','status','proposed','card']);

/** Every string the subject says in its own words (titles, verbs, senses, answers, counter-readings), in any language. */
export function wordsOf(record,depth=0,out=[]){
  if(depth>4||record==null)return out;
  if(typeof record==='string'){out.push(record);return out;}
  if(Array.isArray(record)){for(const item of record)wordsOf(item,depth+1,out);return out;}
  if(typeof record==='object')for(const [key,value] of Object.entries(record))if(!SKIP.has(key))wordsOf(value,depth+1,out);
  return out;
}

const normal=value=>String(value??'').normalize('NFC').toLocaleLowerCase().replace(/ё/gu,'е').trim();

/**
 * `target` {type, id, item?, passage?, stand?}; `draft` {category, change?, observation}; `grounds` the addresses
 * the proposal would rest on; `known(id)` whether the research index holds an address; `similar` acts already on
 * the target ({kind, category, withdrawn}); `termWord` the Tree's word for the chosen term, when one is chosen.
 */
export function checkTicket({target,draft,grounds=[],known=()=>false,similar=[],termWord=null}){
  const out=[];
  if(target.type==='star'||target.type==='link')out.push({id:'address',level:known(target.id)?'ok':'block'});
  else if(target.type==='passage')out.push({id:'address',level:target.passage?.reference||target.stand?'ok':'block'});
  else out.push({id:'address',level:'block'});
  if(!String(draft.observation??'').trim())out.push({id:'observation',level:'block'});
  const change=draft.change;
  if(change){
    out.push({id:'grounds',level:grounds.length?'ok':'block',detail:String(grounds.length)});
    const from=normal(change.from),to=normal(change.to);
    if('from' in change||'to' in change){
      if(!from||!to)out.push({id:'change-empty',level:'block'});
      else if(from===to)out.push({id:'change-same',level:'block'});
      else{
        const own=[...wordsOf(target.item?.record??target.item),target.passage?.quote,target.quote].filter(Boolean).map(normal);
        out.push({id:'from-found',level:own.some(text=>text.includes(from))?'ok':'warn'});
      }
    }
    if(termWord&&change.to_lemma)out.push({id:'tree-word',level:normal(change.to_lemma)===normal(termWord)?'ok':'warn',detail:termWord});
  }
  const repeats=similar.filter(act=>!act.withdrawn&&act.kind==='ticket'&&(!draft.category||act.category===draft.category)).length;
  out.push({id:'repeat',level:repeats?'warn':'ok',detail:String(repeats)});
  return out;
}

/** Whether checks let the act be saved: no blocking check. */
export const checksPass=checks=>!checks.some(check=>check.level==='block');

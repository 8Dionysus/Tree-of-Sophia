// Listening to the original (H2): a chapter's German is read aloud sentence by sentence, and the sentence being read
// is lit together with its partner in the translation beside it. The partners are the sentence pairs the bonsai's
// aligner proposed (witnesses.json says they are unreviewed); a unit no pair covers is read and lit whole. Speech is
// the browser's own synthesis; where it has no German voice, the light still moves at the pace of reading.
import {displayText} from '../../../../bonsai/lib/witnesses.mjs';

const span=([unit,start,end])=>({unit,start,end});

/** The order of reading: each sentence of `pivot` with its partner in `target`, or a whole unit where no pair names it. */
export function readingPlan(data,{pivot='de',target=null}={}){
  const plan=[];
  (data?.rows??[]).forEach((row,rowIndex)=>{
    const units=row[pivot]??[];if(!units.length)return;
    const own=target?row.sentences?.[target]??[]:[];
    // Without pairs in the shown translation, another translation's pairs still cut the original into sentences.
    const cut=own.length?own:Object.values(row.sentences??{}).find(pairs=>pairs?.length)??[];
    const sentences=cut.map(([source,partner])=>({row:rowIndex,source:source.map(span),target:own.length?partner.map(span):[]}));
    const covered=new Set(sentences.flatMap(sentence=>sentence.source.map(part=>part.unit)));
    for(const unit of units)if(!covered.has(unit))sentences.push({row:rowIndex,source:[{unit,start:0,end:data.units[pivot]?.[unit]?.raw?.length??0}],target:[]});
    sentences.sort((a,b)=>a.source[0].unit-b.source[0].unit||a.source[0].start-b.source[0].start);
    plan.push(...sentences);
  });
  return plan;
}

/** What is said for one sentence: its shown text, broken words joined and line ends as spaces. */
export function sentenceText(data,sentence,pivot='de'){
  return sentence.source.map(({unit,start,end})=>{
    const record=data.units[pivot]?.[unit];if(!record)return '';
    const shown=displayText(record.raw,record.kind);return shown.text.slice(shown.at(start),shown.at(end));
  }).join(' ').replace(/\s+/g,' ').trim();
}

/** How long reading a sentence takes without a voice: about 14 characters a second, never under a second. */
export const silentDuration=(text,rate=1)=>Math.max(1000,Math.round(String(text).length*72/Math.max(.25,rate)));

/**
 * One reader of one chapter window. `light(sentence)` lights a sentence (null puts the light out); `onState` hears
 * {state:'idle'|'playing'|'paused', index, total, voiced}. `speech` is the browser's speechSynthesis or a stand-in.
 */
export function createChapterVoice({speech=globalThis.speechSynthesis??null,Utterance=globalThis.SpeechSynthesisUtterance??null,language='de-DE',light=()=>{},onState=()=>{},timers=globalThis}={}){
  let data=null,plan=[],index=0,state='idle',rate=1,timer=0,current=null,token=0;
  const voiceFor=()=>{try{return speech?.getVoices?.().find(voice=>voice.lang?.toLowerCase().startsWith(language.slice(0,2).toLowerCase()))??null;}catch{return null;}};
  const voiced=()=>Boolean(speech&&Utterance&&voiceFor());
  const report=()=>onState({state,index,total:plan.length,voiced:voiced()});
  function clear(){token++;timers.clearTimeout?.(timer);timer=0;if(current){current.onend=current.onerror=null;current=null;}try{speech?.cancel?.();}catch{}}
  function step(){
    if(state!=='playing')return;
    if(index>=plan.length){state='idle';index=0;light(null);report();return;}
    const sentence=plan[index],text=sentenceText(data,sentence),mine=++token;light(sentence);report();
    const next=()=>{if(mine!==token||state!=='playing')return;index++;step();};
    const voice=voiceFor();
    if(voice&&Utterance){
      const utterance=new Utterance(text);utterance.lang=language;utterance.voice=voice;utterance.rate=rate;
      utterance.onend=next;utterance.onerror=next;current=utterance;speech.speak(utterance);
    }else timer=timers.setTimeout(next,silentDuration(text,rate));
  }
  return {
    load(chapter,{target=null,start=0}={}){clear();data=chapter;plan=readingPlan(chapter,{target});index=Math.max(0,plan.findIndex(sentence=>sentence.row>=start));state='idle';light(null);report();},
    play(){if(!plan.length)return;state='playing';clear();step();},
    pause(){if(state!=='playing')return;state='paused';clear();report();},
    toggle(){if(state==='playing')this.pause();else this.play();},
    stop(){clear();state='idle';light(null);report();},
    setRate(value){rate=Math.min(2,Math.max(.5,Number(value)||1));if(state==='playing'){clear();step();}return rate;},
    get rate(){return rate;},
    state:()=>({state,index,total:plan.length,voiced:voiced(),sentence:plan[index]??null}),
    dispose(){clear();state='idle';light(null);data=null;plan=[];},
  };
}

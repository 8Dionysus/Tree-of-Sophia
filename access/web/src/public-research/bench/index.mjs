// The workbench of a stand: its store, its views and its words, mounted over the space. The stand gives it what only
// the stand knows (how a card is drawn, what a star is called, where the Tree's records stand); the bench gives back
// a place to work on one question with several subjects at once.

import {createBenchStore} from './store.mjs';
import {mountBench} from './view.mjs';
import {benchWords} from './words.mjs';
import {chapterOfUnit,hasWitnesses,loadChapter} from './witnesses.mjs';
import {inTray,putFragment,subjectKey,trayAdd} from './model.mjs';
import {PIVOT,citeOfSelection,fingerprintOf,labelOf,rawOffset,snapToWords,witnessWords} from './place.mjs';
import {fragmentOf} from '../../../../../bonsai/lib/witnesses.mjs';
import {receiveOnBench} from './bridge.mjs';
import {cardView} from './views/card.mjs';
import {textView} from './views/text.mjs';
import {wordView} from './views/word.mjs';
import {fragmentView} from './views/fragment.mjs';
import {thoughtView} from './views/thought.mjs';

export const BENCH_VIEWS=Object.freeze({card:cardView,text:textView,word:wordView,fragment:fragmentView,thought:thoughtView});

export function createBench({host,profile,locale,stand,onOpenChange}){
  const t=benchWords(locale);
  const store=createBenchStore({profile});
  const bench=mountBench({host,store,views:BENCH_VIEWS,t,onOpenChange,
    ctxBase:{...stand,locale,profile,canRead:hasWitnesses(profile)}});
  const api={
    ...bench,store,t,
    /** Lays a subject in the tray of the active bench. */
    add(subject,title){bench.ctx.tray(subject,title);},
    holds:subject=>inTray(store.state,subject),
    /**
     * A passage read elsewhere becomes a fragment in the tray, cited in the original whatever witness it was read in:
     * a selection {language, chapter?, unitId, endUnitId?, start, end, display?}, or a quotation the stand already
     * cites in the original {cite, language?}. Resolves to {id, label} or null.
     */
    async layPassage({cite=null,language,chapter:key=null,unitId,endUnitId=unitId,start,end,display=false}){
      if(!hasWitnesses(profile))return null;
      if(cite?.c&&cite.u?.length)return layCite(cite,language);
      if(!language||!unitId)return null;
      const chapterKey=key||await chapterOfUnit(language,unitId);if(!chapterKey)return null;
      const chapter=await loadChapter(chapterKey),index=chapter.index[language],list=chapter.units[language];
      const a=index?.get(unitId),b=index?.get(endUnitId);if(a===undefined||b===undefined)return null;
      let s=display?rawOffset(list[a],start):start,e=display?rawOffset(list[b],end):end;
      // As on the bench itself, a passage reaches whole words.
      if(a===b)({start:s,end:e}=snapToWords(list[a].raw,s,e));
      else{s=snapToWords(list[a].raw,s,list[a].raw.length).start;e=snapToWords(list[b].raw,0,e).end;}
      if(a===b&&!(e>s))return null;
      const made=citeOfSelection(chapter,language,{index:a,offset:s},{index:b,offset:e});if(!made)return null;
      return put(made,chapter);
    },
    key:subjectKey,
    dispose(){stopReceiving();bench.dispose();store.dispose();},
  };
  /** The fragment goes into the tray, labelled with the words of the witness it was read in. */
  function put(made,chapter){
    const words=witnessWords(made,chapter),label=labelOf(words);
    let id=null;
    store.update(state=>{const next=putFragment(state,{...made,label,fingerprint:fingerprintOf(words)});id=next.id;return id?trayAdd(next.state,{kind:'fragment',id},{title:label}):next.state;});
    return id?{id,label}:null;
  }
  /** A quotation the stand cites in the original, read in `language` where that witness has the place. */
  async function layCite(cite,language=PIVOT){
    const chapter=await loadChapter(cite.c);
    let witness={language:PIVOT,u:cite.u,s:cite.s,e:cite.e};
    if(language&&language!==PIVOT){
      const part=fragmentOf(cite,chapter,language),list=chapter.units[language];
      if(part&&!part.omitted&&part.units.length){
        const first=part.units[0],last=part.units.at(-1);
        witness={language,u:[first.id,last.id],s:rawOffset(list[first.index],first.from),e:rawOffset(list[last.index],last.to)};
      }
    }
    return put({cite,witness},chapter);
  }
  // Passages laid from the Library reader and the stand's quotations arrive here.
  const stopReceiving=hasWitnesses(profile)?receiveOnBench(passage=>api.layPassage(passage)):()=>{};
  return api;
}

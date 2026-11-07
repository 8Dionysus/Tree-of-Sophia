// The workbench of a stand: its store, its views and its words, mounted over the space. The stand gives it what only
// the stand knows (how a card is drawn, what a star is called, where the Tree's records stand); the bench gives back
// a place to work on one question with several subjects at once.

import {createBenchStore} from './store.mjs';
import {mountBench} from './view.mjs';
import {benchWords} from './words.mjs';
import {hasWitnesses} from './witnesses.mjs';
import {inTray,subjectKey} from './model.mjs';
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
  return {
    ...bench,store,t,
    /** Lays a subject in the tray of the active bench. */
    add(subject,title){bench.ctx.tray(subject,title);},
    holds:subject=>inTray(store.state,subject),
    key:subjectKey,
    dispose(){bench.dispose();store.dispose();},
  };
}

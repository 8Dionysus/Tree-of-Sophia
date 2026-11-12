// Reproduce the bounded Latin/Russian sentence alignment reviewed alongside
// the 1884 journal passages. Context sentences are transcribed from the scans.
import {readFileSync,writeFileSync} from 'node:fs';
import {alignWitnesses,sentenceSpans} from '../../lib/align.mjs';

const places=JSON.parse(readFileSync(new URL('../../antiquity/fragments.json',import.meta.url),'utf8')).places.letters;
const join=(keys,lang)=>keys.map(key=>places[key].texts[lang]).join(' ');
const windows={
  '1.1–3': {
    la:join(['1.1','1.2','1.3'],'la')+' Interrogabis fortasse, quid ego faciam, qui tibi ista praecipio.',
    ru:join(['1.1','1.2','1.3'],'ru')+' Ты, быть можетъ, спросишь, что дѣлаю я-то самъ, дающій такія наставленія?',
  },
  '9.1': {
    la:places['9.1'].texts.la+' In ambiguitatem incidendum est, si exprimere ἀπάθειαν uno verbo cito voluerimus et inpatientiam dicere.',
    ru:places['9.1'].texts.ru+' Но мы не избѣжимъ впасть въ двусмысленность, если захотимъ греческое слово ἀπάθεια передать посредствомъ латинскаго impatientia, нечувствительность.',
  },
  '9.8': {
    la:'Fructuosior est adulescentia liberorum, sed infantia dulcior. '+places['9.8'].texts.la+' Hae sunt amicitiae, quas temporarias populus appellat; qui utilitatis causa adsumptus est, tamdiu placebit, quamdiu utilis fuerit.',
    ru:'Дѣти, выросши, дѣлаютъ больше полезнаго; но они милѣе, пока малы. '+places['9.8'].texts.ru+' Вотъ дружба, которую люди называютъ благовременною. Другъ, котораго мы приблизили къ сердцу изъ-за выгоды, будетъ нравиться намъ дотолѣ, доколѣ будетъ полезнымъ.',
  },
  '9.10': {
    la:places['9.10'].texts.la+' Non dubie habet aliquid simile amicitiae affectus amantium; possis dicere illam esse insanam amicitiam.',
    ru:places['9.10'].texts.ru+' Нѣтъ сомнѣнія, душевное настроеніе любящихъ имѣетъ нѣчто сходное съ дружбой: можно сказать, что чувство это есть вышедшая за предѣлы здраво-спокойнаго состоянія дружба.',
  },
  '9.13': {
    la:places['9.13'].texts.la+' Volo tibi Chrysippi quoque distinctionem indicare.',
    ru:places['9.13'].texts.ru+' При этомъ случаѣ я желаю указать тебѣ и на различіе Хризиппа.',
  },
  '9.19': {
    la:'Hic enim capta patria, amissis liberis, amissa uxore cum ex incendio publico solus et tamen beatus exiret, interroganti Demetrio, cui cognomen ab exitio urbium Poliorcetes fuit, numquid perdidisset, Omnia, inquit, bona mea mecum sunt. '+places['9.19'].texts.la+' Ne existimes nos solos generosa verba iactare; et ipse Stilbonis obiurgator Epicurus similem illi vocem emisit, quam tu boni consule, etiam si hunc diem iam expunxi.',
    ru:'Этотъ философъ, вмѣстѣ съ завоеваніемъ города потерявши дѣтей и супругу, одинъ, какъ перстъ, являясь невредимымъ на всеобщемъ пожарищѣ, и тѣмъ не менѣе счастливымъ, — на вопросъ Димитрія, прозваннаго за разрушеніе городовъ Поліоркетомъ: „неужели онъ ничего не потерялъ?“ отвѣтилъ: „все добро мое находится при мнѣ!“ '+places['9.19'].texts.ru+' Не подумай, что только мы, стоики, роняемъ такія животворныя слова: даже оспаривающій Стильпона Эпикуръ вымолвилъ нѣчто, похожее на это; прими сказанное имъ съ добромъ, хотя долгъ на сегодня я уже погасилъ.',
  },
  '28.1': {
    la:places['28.1'].texts.la+' Hoc idem querenti cuidam Socrates ait: Quid miraris nihil tibi peregrinationes prodesse, cum te circumferas?',
    ru:places['28.1'].texts.ru+' Тоже самое говоритъ на вопросъ одного человѣка Сократъ: „что-же удивительнаго въ томъ, что твое путешествіе не принесло тебѣ пользы, когда ты повсюду путешествуешь съ самимъ собою?“',
  },
};
// Inspect each numbered boundary in its own continuous neighbourhood as well.
for(const [key,previous,next] of [['1.1',null,'1.2'],['1.2','1.1','1.3'],['1.3','1.2',null]]){
  const neighbour=(place,language,which)=>{
    if(!place)return '';
    const text=places[place].texts[language],spans=sentenceSpans(text,language),[a,b]=which==='last'?spans.at(-1):spans[0];
    return Array.from(text).slice(a,b).join('');
  };
  const context={la:'Interrogabis fortasse, quid ego faciam, qui tibi ista praecipio.',ru:'Ты, быть можетъ, спросишь, что дѣлаю я-то самъ, дающій такія наставленія?'};
  windows[key]=Object.fromEntries(['la','ru'].map(lang=>[lang,[neighbour(previous,lang,'last'),places[key].texts[lang],next?neighbour(next,lang,'first'):context[lang]].filter(Boolean).join(' ')]));
}
// Shorter continuous windows expose the boundaries when a longer run drifts.
for(const key of ['1.2','1.3','9.8','9.19']){
  const wider=windows[key];
  for(const side of ['start','end']){
    windows[`${key}-${side}`]=Object.fromEntries(['la','ru'].map(lang=>{
      const text=wider[lang], selected=places[key].texts[lang], at=text.indexOf(selected);
      if(at<0)throw new Error(`selected text absent: ${key} ${lang}`);
      const from=Array.from(text.slice(0,at)).length,to=from+Array.from(selected).length;
      const spans=sentenceSpans(text,lang);
      const inside=spans.filter(([a,b])=>a>=from&&b<=to);
      const before=spans.filter(([,b])=>b<=from),after=spans.filter(([a])=>a>=to);
      const selectedSpans=side==='start'?[...before.slice(-1),...inside.slice(0,2)]:[...inside.slice(-2),...after.slice(0,1)];
      const a=selectedSpans[0][0],b=selectedSpans.at(-1)[1];
      return [lang,Array.from(text).slice(a,b).join('')];
    }));
  }
}
const candidates={};
for(const [name,{la,ru}] of Object.entries(windows)){
  const unit=(id,text)=>({id,text,raw:text});
  const result=alignWitnesses({[name]:[unit('la',la)]},{[name]:[unit('ru',ru)]},'la','ru');
  const split=(text,language)=>sentenceSpans(text,language).map(([a,b],i)=>unit(String(i),Array.from(text).slice(a,b).join('')));
  const latin=split(la,'la'),russian=split(ru,'ru');
  const sentenceGroups=alignWitnesses({[name]:latin},{[name]:russian},'la','ru',{sentences:false});
  candidates[name]={la,ru,result,latin,russian,sentenceGroups};
  const chars=x=>Array.from(x),cut=(x,spans)=>spans.map(([,a,b])=>chars(x).slice(a,b).join('')).join(' / ');
  console.log(`\n${name}`);
  for(const group of result.chapters[name].groups)for(const [a,b] of group.sentences??[])console.log('LA:',cut(la,a),'\nRU:',cut(ru,b));
  console.log('SENTENCE-UNIT GROUPS');
  for(const group of sentenceGroups.chapters[name].groups)console.log('LA:',group.s.map(id=>latin[+id].text).join(' / '),'\nRU:',group.t.map(id=>russian[+id].text).join(' / '));
}
const output=new URL('./seneca-letters1-9-28-alignment.json',import.meta.url);
writeFileSync(output,JSON.stringify({schema:'tos_bonsai_bounded_alignment_candidate_v1',candidates},null,2)+'\n');

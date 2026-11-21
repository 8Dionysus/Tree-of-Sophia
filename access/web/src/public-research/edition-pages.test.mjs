import {expect,test} from 'vitest';
import {Window} from 'happy-dom';
import {pageMap,pagesStream,scanPages,skeleton,teiStream,unitPages,dtaImage,commonsPdfPage} from '../../../../bonsai/lib/pages.mjs';
import {editionPagesSection,pagesOf} from './edition-pages.mjs';

const tei=`<TEI><teiHeader><title>Also sprach</title></teiHeader><text><body>
<pb facs="#f0011" n="5"/><fw type="header" place="top">Vorrede.</fw>
<p>Als Zarathustra dreissig Jahr alt war, verliess er<lb/>seine Heimat und den See seiner Heimat und gieng in<lb/>das Gebirge.</p>
<p>Endlich aber verwandelte sich sein Herz, — und eines<lb/>Morgens stand er mit der Morgenröthe auf, ge¬<lb/><pb facs="#f0012" n="6"/><fw type="catch">wor-</fw>worden sein, ohne mich.</p>
<p>Siehe! Ich bin meiner Weisheit überdrüssig &amp; müde.</p>
</body></text></TEI>`;
const units=[
  {id:'u1',raw:'Als Zarathustra dreissig Jahr alt war, verliess er\nseine Heimat und den See seiner Heimat und gieng in\ndas Gebirge.'},
  {id:'u2',raw:'Endlich aber verwandelte sich sein Herz, — und eines\nMorgens stand er mit der Morgenröthe auf, ge¬\nworden sein, ohne mich.'},
  {id:'u3',raw:'Siehe! Ich bin meiner Weisheit überdrüssig & müde.'},
  {id:'u4',raw:'Ein Satz, den das Buch nicht hat.'},
];

test('a unit\'s pages follow from where its letters stand in the holder\'s TEI: running heads and catchwords aside',()=>{
  const record=teiStream(tei);
  expect(record.breaks.map(page=>[page.n,page.facs])).toEqual([['5','f0011'],['6','f0012']]);
  expect(record.stream.includes('vorrede')).toBe(false);
  const {units:spans,missing}=unitPages(units,record);
  expect(spans).toEqual({u1:[0,0],u2:[0,1],u3:[1,1]});
  expect(missing).toEqual(['u4']);
  const map=pageMap({witness:'de',units,record,rights:{statement:'CC BY-SA 4.0',checked:false},source:{kind:'dta'},image:dtaImage('nietzsche_zarathustra01_1883')});
  expect(map.pages[0]).toEqual({n:'5',facs:'f0011',image:'https://media.dwds.de/dta/images/nietzsche_zarathustra01_1883/nietzsche_zarathustra01_1883_0011_800px.jpg'});
});

test('the old Russian letters and hard signs do not keep a unit from its scan page',()=>{
  expect(skeleton('Заратустрѣ хвалили одного мудреца, — сказалъ онъ.')).toBe(skeleton('Заратустре хвалили одного мудреца, сказал он'));
  const record=pagesStream([{scan:21,n:'17',text:'<noinclude>{{колонтитул}}</noinclude>Заратустрѣ хвалили одного мудреца, который умѣлъ хорошо говорить о снѣ'},{scan:22,n:'18',text:"и о добродѣтели: за это его высоко чтили и награждали, и всѣ юноши садились передъ каѳедрой его."}]);
  const {units:spans}=unitPages([{id:'r1',raw:'Заратустре хвалили одного мудреца, который умел хорошо говорить о сне и о добродетели: за это его высоко чтили и награждали, и все юноши садились перед кафедрой его.'}],record);
  expect(spans).toEqual({r1:[0,1]});
  expect(commonsPdfPage('Ницше Так говорил Заратустра 1913.pdf')({scan:21})).toContain('page=21');
  // A MediaWiki API answer reads as scan pages directly.
  expect(scanPages({query:{pages:{7:{title:'Страница:Ницше Так говорил Заратустра 1913.pdf/21',revisions:[{slots:{main:{'*':'Текст'}}}]}}}})).toEqual([{scan:21,text:'Текст'}]);
});

test('the chapter window shows a witness\'s pages behind the technical door, and a witness without a map shows nothing',async()=>{
  const window=new Window(),document=window.document;
  const map={schema:'tos.bonsai.pages.v1',witness:'de',rights:{statement:'Public domain',url:'https://example.org/rights'},pages:[{n:'5',image:'https://media.dwds.de/p5.jpg'},{n:'6',image:'https://media.dwds.de/p6.jpg'}],units:{u1:[0,0],u2:[0,1]}};
  expect(pagesOf(map,['u2','u1']).map(page=>page.n)).toEqual(['5','6']);
  const loaders={'x/pages/de.json':async()=>map};
  const data={units:{de:[{id:'u1'},{id:'u2'}],ru:[{id:'r1'}]}};
  const section=await editionPagesSection(document,()=> 'en',{data,languages:['de','ru'],loaders});
  expect(section.tagName).toBe('DETAILS');expect(section.open).toBe(false);
  expect(section.querySelector('summary').textContent).toBe('Pages of the edition');
  expect(section.querySelectorAll('.ep-page')).toHaveLength(2);
  // The holder's images are asked for only when the door opens.
  expect(section.querySelector('img').getAttribute('src')).toBeNull();
  section.open=true;section.dispatchEvent(new window.Event('toggle'));
  expect(section.querySelector('img').getAttribute('src')).toBe('https://media.dwds.de/p5.jpg');
  expect(await editionPagesSection(document,()=> 'en',{data,languages:['ru'],loaders})).toBeNull();
});

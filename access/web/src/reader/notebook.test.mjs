import {test,expect} from 'vitest';
import {createReaderNotebook,readerVersionKey,readerAnchorKey} from './notebook.mjs';

const anchor=(patch={})=>({documentId:'passage:one',version:'ru',sourceRevision:'edition:1911',textSha256:'a'.repeat(64),paragraph:2,...patch});
function storage(initial=null){
  let text=initial,writes=0;
  return {getItem:()=>text,setItem:(_key,value)=>{text=value;writes++;},external:value=>{text=value;},get text(){return text;},get writes(){return writes;}};
}
const create=()=>{const disk=storage();return {disk,book:createReaderNotebook({storage:disk})};};

test('positions and notes bind exact document, version, source revision and text digest',()=>{
  const {book,disk}=create(),ru=anchor(),en=anchor({version:'en'}),edited=anchor({textSha256:'b'.repeat(64)});
  book.savePosition(ru,.25);book.savePosition(en,.75);book.saveNote(ru,'Русская заметка');book.saveNote(en,'English note');
  expect(book.positionFor(ru)).toEqual({anchor:ru,fraction:.25});
  expect(book.positionFor(edited)).toBeNull();expect(book.positionFor(anchor({sourceRevision:'edition:other'}))).toBeNull();
  expect(book.positionFor(anchor({documentId:'another'}))).toBeNull();
  book.savePosition(anchor({paragraph:7}),.5);
  expect(book.getState().positions).toHaveLength(2);expect(book.positionFor(ru).anchor.paragraph).toBe(7);
  expect(book.positionFor(en).fraction).toBe(.75);
  book.saveNote(ru,'Changed note');expect(book.getState().notes).toHaveLength(2);
  expect(book.getState().notes.find(item=>item.anchor.version==='en').text).toBe('English note');
  const restored=createReaderNotebook({storage:disk});expect(restored.getState()).toEqual(book.getState());
  expect(readerVersionKey(ru)).toBe(readerVersionKey(anchor({paragraph:99})));
  expect(readerAnchorKey(ru)).not.toBe(readerAnchorKey(anchor({paragraph:99})));
});

test('malformed saved state stays byte-for-byte untouched while new user work remains exportable',()=>{
  for(const malformed of ['{broken','null',JSON.stringify({schema:'wrong'})]){
    const disk=storage(malformed),book=createReaderNotebook({storage:disk});
    expect(book.status()).toEqual({writable:false,error:'invalid-storage'});
    book.saveNote(anchor(),'Still mine');expect(disk.text).toBe(malformed);expect(disk.writes).toBe(0);
    expect(JSON.parse(book.exportData()).notes[0].text).toBe('Still mine');
  }
});

test('an external tab change blocks overwrites and retains the new local note for export',()=>{
  const disk=storage(),first=createReaderNotebook({storage:disk}),second=createReaderNotebook({storage:disk});
  first.saveNote(anchor(),'First tab');const remote=disk.text;
  second.saveNote(anchor({paragraph:4}),'Second tab');
  expect(second.status()).toEqual({writable:false,error:'conflict'});expect(disk.text).toBe(remote);
  expect(JSON.parse(second.exportData()).notes[0].text).toBe('Second tab');
  second.setPreferences({theme:'paper'});expect(disk.text).toBe(remote);
});

test('unavailable storage and failed writes retain working session state',()=>{
  const absent=createReaderNotebook({storage:null});absent.saveNote(anchor(),'Session note');
  expect(absent.status()).toEqual({writable:false,error:'unavailable'});expect(absent.getState().notes).toHaveLength(1);
  const failed=createReaderNotebook({storage:{getItem:()=>null,setItem:()=>{throw Error('quota');}}});
  failed.toggleBookmark(anchor());expect(failed.getState().bookmarks).toHaveLength(1);
  expect(failed.status()).toEqual({writable:false,error:'unavailable'});
});

test('invalid imports and mutations are atomic, including duplicate identities and source payload injection',()=>{
  const {book,disk}=create();book.saveNote(anchor(),'Existing');const original=book.exportData(),writes=disk.writes;
  const malformed=[];
  for(const change of [s=>{s.preferences.fontSize=25;},s=>{s.positions=[{anchor:anchor(),fraction:2}];},
    s=>{s.notes.push(structuredClone(s.notes[0]));},s=>{s.notes[0].anchor.text='Private source';},
    s=>{s.paragraphs=['Private source'];},s=>{s.notes[0].updatedAt='yesterday';}]){
    const next=JSON.parse(original);change(next);malformed.push(JSON.stringify(next));
  }
  for(const text of ['broken',...malformed]){expect(()=>book.importData(text)).toThrow('invalid-input');expect(book.exportData()).toBe(original);}
  expect(()=>book.savePosition(anchor({paragraph:-1}))).toThrow('invalid-input');
  expect(()=>book.setPreferences({lineHeight:Infinity})).toThrow('invalid-input');
  expect(()=>book.saveNote(anchor(),'x'.repeat(4001))).toThrow('limit');
  expect(()=>book.setActive({documentId:'x',version:'ru',paragraphs:['source']})).toThrow('invalid-input');
  expect(book.exportData()).toBe(original);expect(disk.writes).toBe(writes);
});

test('bookmarks toggle, notes delete, and returned values cannot mutate notebook state',()=>{
  const {book}=create(),a=anchor();book.toggleBookmark(a);book.toggleBookmark(a);expect(book.getState().bookmarks).toEqual([]);
  book.saveNote(a,'Keep');const copy=book.getState();copy.notes[0].text='Outside';expect(book.getState().notes[0].text).toBe('Keep');
  const saved=book.savePosition(a,.3);saved.positions[0].anchor.paragraph=999;
  const found=book.positionFor(a);found.anchor.paragraph=888;expect(book.positionFor(a).anchor.paragraph).toBe(2);
  book.deleteNote(a);expect(book.getState().notes).toEqual([]);
});

test('the bounded state preserves recent exact positions and refuses extra bookmarks or notes',()=>{
  const {book}=create();
  for(let i=0;i<65;i++)book.savePosition(anchor({documentId:'doc:'+i}));
  expect(book.getState().positions).toHaveLength(64);expect(book.positionFor(anchor({documentId:'doc:0'}))).toBeNull();
  const seed=book.getState(),stamp='2026-09-12T12:00:00.000Z';
  seed.bookmarks=Array.from({length:256},(_,paragraph)=>({anchor:anchor({paragraph}),createdAt:stamp}));
  seed.notes=Array.from({length:200},(_,paragraph)=>({anchor:anchor({paragraph}),text:'note',updatedAt:stamp}));
  book.importData(JSON.stringify(seed));const before=book.exportData();
  expect(()=>book.toggleBookmark(anchor({paragraph:999}))).toThrow('limit');
  expect(()=>book.saveNote(anchor({paragraph:999}),'One too many')).toThrow('limit');
  expect(book.exportData()).toBe(before);
  book.saveNote(anchor({paragraph:0}),'Update at capacity');expect(book.getState().notes).toHaveLength(200);
});

test('UTF-8 import size and note text bounds are enforced without changing existing state',()=>{
  const {book}=create(),before=book.exportData();
  expect(()=>book.importData(' '.repeat(1_500_001))).toThrow('limit');
  const large=book.getState(),stamp='2026-09-12T12:00:00.000Z';
  large.notes=Array.from({length:200},(_,paragraph)=>({anchor:anchor({paragraph}),text:'Ж'.repeat(4000),updatedAt:stamp}));
  expect(()=>book.importData(JSON.stringify(large))).toThrow('limit');expect(book.exportData()).toBe(before);
});

test('roundtrip contains only anchors, explicit user notes and preferences, never a supplied paragraph',()=>{
  const {book,disk}=create(),source='Never persist supplied source wording';
  expect(()=>book.savePosition({...anchor(),paragraphs:[source]})).toThrow('invalid-input');
  book.setActive({documentId:'passage:one',version:'ru'});book.setPreferences({fontSize:24,lineHeight:2.2,width:'wide',mode:'parallel',sidebar:false,inspector:true,theme:'paper'});
  book.savePosition(anchor(),1);book.saveNote(anchor(),'My observation');book.toggleBookmark(anchor());
  expect(disk.text).not.toContain(source);expect(disk.text).not.toContain('paragraphs');
  const other=createReaderNotebook({storage:null});other.importData(book.exportData());expect(other.getState()).toEqual(book.getState());
});

import {expect,test} from 'vitest';
import {antiquityBook,createBook,zarathustraBook} from './reference-outline.mjs';

const bi=(ru,en)=>({ru,en});

test('the contents read a branch as its moves in order, and a move is at home in its branch before the library',()=>{
  const collection={
    branches:[{id:'b',area:'area-k',title:bi('От уверенности','From confidence'),question:bi('Когда?','When?'),questions:[bi('q1','q1')]}],
    questions:[{id:'q1',title:bi('Вопрос','Question'),body:bi('Тело','Body')},{id:'q2',title:bi('Одинокий','Alone')}],
    entities:[{id:'plato',kind:'author',period:'classical',title:bi('Платон','Plato')},{id:'meno',kind:'work',author:'plato',title:bi('Менон','Meno')}],
    passages:[{id:'p1',workId:'meno',title:bi('Место','Place')},{id:'p2',workId:'meno',title:bi('Другое','Other')}],
    eras:[{id:'era-classical',period:'classical',title:bi('Классика','Classical')}],
  };
  const routes=[{id:'b',title:bi('Знание','Knowledge'),steps:[{nodeId:'r:p1',title:bi('Первый','First')},{nodeId:'r:p2',title:bi('Второй','Second')}]}];
  const ids={q1:'c:q1',q2:'c:q2',plato:'c:plato',meno:'c:meno',p1:'r:p1',p2:'r:p2'};
  const book=antiquityBook({routes,collection,materialOf:id=>ids[id]??null,areaTitles:{'area-k':bi('Знание','Knowledge')}});
  expect(book.parts.map(part=>part.key)).toEqual(['crown','texts']);
  // The second move stands in its branch, numbered, between the first move and nothing.
  expect(book.trail('r:p2').map(entry=>entry.key)).toEqual(['crown','area-k','b']);
  expect(book.neighbours('r:p2')).toMatchObject({index:1,total:2,numbered:true,previous:{id:'r:p1'},next:null});
  // A question asked by a branch stands under it; one no branch asks stands among the questions of the crown.
  expect(book.trail('c:q1').map(entry=>entry.key)).toEqual(['crown','area-k','b']);
  expect(book.trail('c:q2').map(entry=>entry.key)).toEqual(['crown','questions']);
  // An author and a work are at home in the library.
  expect(book.home('c:meno').source).toBe('library');
  expect(book.trail('c:meno').map(entry=>entry.key)).toEqual(['era-classical','author:plato']);
});

test('a subject of the core is at home among its kind, and a path only lists it as a step',()=>{
  const core={
    concepts:[{id:'recurrence',type:'tos.entity.conception',title:bi('Возвращение','Recurrence')},{id:'gate',type:'tos.entity.thought-image',title:bi('Ворота','Gate')}],
    figures:[{id:'dwarf',title:bi('Карлик','Dwarf')}],proposed:[],
    chains:[{id:'recurrence:chain:01',from:'recurrence',admission:'map',title:bi('Путь','Route')},{id:'held',from:'recurrence',admission:'held',title:bi('Ждёт','Held')}],
    records:{recurrence:{title:bi('Возвращение','Recurrence')},gate:{title:bi('Ворота','Gate')},dwarf:{title:bi('Карлик','Dwarf')}},
  };
  const routes=[{id:'moment',title:bi('У ворот','At the gate'),steps:[{nodeId:'gate'},{nodeId:'recurrence'}]}];
  const book=zarathustraBook({core,routes,chapters:[{key:'III.02',titles:bi('О призраке','On the vision')}],partTitles:{III:bi('Часть третья','Part three')},citing:key=>key==='III.02'?['gate','dwarf','gate']:[]});
  expect(book.trail('recurrence').map(entry=>entry.key)).toEqual(['concepts']);
  expect(book.neighbours('recurrence').numbered).toBe(false);
  // Only the routes the map admits stand under their subject.
  expect(book.parts.find(part=>part.key==='concepts').children[0].children.map(entry=>entry.id)).toEqual(['recurrence:chain:01']);
  // A chapter lists each subject that rests on it once.
  const chapter=book.library.find(part=>part.key==='part:III').children[0];
  expect(chapter.children.map(entry=>entry.id)).toEqual(['gate','dwarf']);
});

test('every address of an entry is gathered for the figure of the tree',()=>{
  const book=createBook({parts:[{key:'p',children:[{key:'a',id:'a',children:[{key:'b',id:'b',n:1}],questions:[{key:'c',id:'c'}]}]}]});
  expect(book.addresses(book.parts[0].children[0])).toEqual(['a','b','c']);
});

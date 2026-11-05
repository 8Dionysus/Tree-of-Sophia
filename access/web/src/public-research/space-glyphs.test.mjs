import {expect,it} from 'vitest';
import {STAR_KINDS,signElement,signFamily,signMarkup,starKind} from './space-glyphs.mjs';
import {TREE_LABEL_FROM} from './space-tree.mjs';

it('gives every role the tree grows by in either bonsai a kind with a sign',()=>{
  for(const role of Object.keys(TREE_LABEL_FROM)){expect(starKind({role}),role).toBe(role);expect(signMarkup(role),role).toMatch(/^<svg [^>]*><path d="M[^"]+"\/><\/svg>$/);}
  for(const kind of STAR_KINDS)expect(signFamily(kind),kind).toBeTruthy();
});

it('reads each bonsai’s own kinds of material as the same kinds of star',()=>{
  // Antiquity's thinkers and Zarathustra's characters are persons; both bonsais' passages and excerpts are places of a text.
  expect([starKind({kind:'figure'}),starKind({kind:'character'})].map(signFamily)).toEqual(['person','person']);
  expect([starKind({kind:'fragment'}),starKind({kind:'excerpt'}),starKind({kind:'chapter'})].map(signFamily)).toEqual(['text','text','text']);
  expect(['edition','translation','original'].map(kind=>starKind({kind}))).toEqual(['version','version','version']);expect(starKind({kind:'passage'})).toBe('passage');
  expect(starKind({kind:'symbol'})).toBe('image');expect(starKind({kind:'tradition'})).toBe('school');expect(starKind({kind:'dossier'})).toBe('era');
  // The tree's role wins over the material's kind: a symbol that walks the book as a figure is a person.
  expect(starKind({role:'figure',kind:'symbol'})).toBe('figure');expect(starKind({role:'translator',kind:'figure'})).toBe('translator');
  expect(starKind({kind:'concept',personal:true})).toBe('personal');expect(starKind({kind:'unknown'})).toBeNull();
});

it('draws a sign as one element marked with its kind and family, hidden from the screen reader beside its name',()=>{
  const made=[];const doc={createElement:tag=>{const node={tagName:tag,dataset:{},attributes:{},setAttribute(name,value){this.attributes[name]=value;}};made.push(node);return node;}};
  const sign=signElement(doc,'translator');
  expect(sign.className).toBe('ps-sign');expect(sign.dataset).toEqual({sign:'person',kind:'translator'});expect(sign.attributes['aria-hidden']).toBe('true');
  expect(sign.innerHTML).toBe(signMarkup('author'));
  expect(signElement(doc,'unknown')).toBeNull();expect(made).toHaveLength(1);
});

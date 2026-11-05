// One sign for each kind of star, the same in every bonsai. Each bonsai's own kinds of material fall into the kinds the
// tree grows by (its roles), and each kind is drawn by one small line figure wherever a star is named: on its card, in a
// list of links, in search and in its hint. Kinds of one family share a sign: thinkers, figures and translators are
// persons; works and editions are books; chapters and passages are places of a text; branches and paths are ways.

const SIGNS={
  era:'M4.5 2.5h7M4.5 13.5h7M5.5 2.5c0 3 5 3.2 5 5.5s-5 2.5-5 5.5M10.5 2.5c0 3-5 3.2-5 5.5s5 2.5 5 5.5',
  way:'M5 13.5v-11M5 10.5c0-4 6.5-3.5 6.5-8M5 2.5v0M11.5 2.5v0M5 13.5v0',
  step:'M3.5 2.5v6.5h8.5M9.5 6.5l2.5 2.5-2.5 2.5',
  person:'M8 2.5a2.4 2.4 0 1 1 0 4.8a2.4 2.4 0 1 1 0-4.8M3.5 13.5c.6-2.8 2.4-4.3 4.5-4.3s3.9 1.5 4.5 4.3',
  school:'M2 6l6-3.5L14 6M3.5 7.5v5M6.5 7.5v5M9.5 7.5v5M12.5 7.5v5M2 13.5h12',
  book:'M8 4.5c-1.8-1.3-3.8-1.6-5.5-1v9.5c1.7-.6 3.7-.3 5.5 1M8 4.5c1.8-1.3 3.8-1.6 5.5-1v9.5c-1.7-.6-3.7-.3-5.5 1M8 4.5v9.5',
  text:'M3.5 3.5v9M6 4h6.5M6 6.7h6.5M6 9.4h6.5M6 12h4',
  concept:'M8 2l6 6-6 6-6-6zM8 5.5L10.5 8 8 10.5 5.5 8z',
  term:'M2.5 8.5l5.5-6h5.5v5.5l-6 5.5zM10.5 5.5v0',
  image:'M8 2v3M8 11v3M2 8h3M11 8h3M3.8 3.8l2.1 2.1M10.1 10.1l2.1 2.1M12.2 3.8l-2.1 2.1M5.9 10.1l-2.1 2.1',
  question:'M5.6 5.6a2.5 2.5 0 1 1 3.6 2.2c-.8.4-1.2 1-1.2 1.8v.6M8 12.8v.2',
  personal:'M3 13l.8-3L10.6 3.2a1.4 1.4 0 0 1 2 2L5.8 12z',
};
// The kinds of stars and the family of signs each belongs to.
const FAMILY={era:'era',branch:'way',path:'way',step:'step',author:'person',figure:'person',translator:'person',school:'school',work:'book',version:'book',chapter:'text',passage:'text',concept:'concept',term:'term',image:'image',question:'question',personal:'personal'};
// A bonsai's own kinds of material, for a star the tree gives no role of its own.
// The public collection's own kinds (its passages, editions and translations) are read the same way.
const BY_KIND={work:'work',fragment:'passage',excerpt:'passage',passage:'passage',chapter:'chapter',part:'chapter',question:'question',interpretation:'step',figure:'author',translator:'translator',character:'figure',symbol:'image',tradition:'school',concept:'concept',dossier:'era',era:'era',edition:'version',translation:'version',original:'version',thought:'personal',note:'personal'};

export const STAR_KINDS=Object.freeze(Object.keys(FAMILY));

/** The kind of a star across bonsais: its role on the tree, else its bonsai's kind of material. */
export function starKind({role=null,kind=null,personal=false}={}){
  if(personal)return 'personal';
  if(role&&FAMILY[role])return role;
  return BY_KIND[kind]??null;
}

/** The family of signs a kind belongs to. */
export const signFamily=kind=>FAMILY[kind]??null;

/** The sign of a kind as a small line drawing that takes the colour of its text. */
export function signMarkup(kind){
  const path=SIGNS[FAMILY[kind]];
  return path?`<svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" stroke-width="1.35" stroke-linecap="round" stroke-linejoin="round"><path d="${path}"/></svg>`:'';
}

/** The sign of a kind as an element, or null for a star of no known kind. */
export function signElement(doc,kind){
  const markup=signMarkup(kind);if(!markup||!doc)return null;
  const node=doc.createElement('span');node.className='ps-sign';node.dataset.sign=FAMILY[kind];node.dataset.kind=kind;node.setAttribute('aria-hidden','true');node.innerHTML=markup;
  return node;
}

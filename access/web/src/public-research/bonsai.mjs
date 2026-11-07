// The stands read their bonsai through this one door.
//
// A bonsai (`bonsai/<stand>/` at the repository root) is a stand's own
// mini-Tree: its manifest and modules. The stand presents them and never
// writes them; what it shows on a lens comes from here, with the sources and
// footing each module keeps.

import antiquityManifest from '../../../../bonsai/antiquity/bonsai.json' with {type:'json'};
import zarathustraManifest from '../../../../bonsai/zarathustra/bonsai.json' with {type:'json'};

// A stand loads its own bonsai when it opens, and nothing of another's: the
// manifests are small and always here, the modules come with their stand.
// Vite transforms dynamic JSON imports into JavaScript modules in development.
const json=module=>module.default;
// The Antiquity collection also serves the shared library of another stand, which needs nothing else of Antiquity's.
let collectionLoad=null;
/** The Antiquity collection alone, loaded once. */
export function loadAntiquityCollection(){
  collectionLoad??=import('../../../../bonsai/antiquity/collection.json').then(json).catch(error=>{collectionLoad=null;throw error;});
  return collectionLoad;
}
const LOADERS=Object.freeze({
  antiquity:()=>Promise.all([
    loadAntiquityCollection().then(collection=>({default:collection})),
    import('../../../../bonsai/antiquity/first-branches.json'),
    import('../../../../bonsai/antiquity/chronology.json'),
    import('../../../../bonsai/antiquity/places.json'),
    import('../../../../bonsai/antiquity/identities.json'),
    import('../../../../bonsai/shared/coastlines.json'),
  ]).then(([collection,firstBranches,chronology,places,identities,basemap])=>({collection:json(collection),firstBranches:json(firstBranches),chronology:json(chronology),places:json(places),identities:json(identities),basemap:json(basemap)})),
  zarathustra:()=>Promise.all([
    import('../../../../bonsai/zarathustra/chronology.json'),
    import('../../../../bonsai/zarathustra/places.json'),
    import('../../../../bonsai/zarathustra/identities.json'),
    import('../../../../bonsai/shared/coastlines.json'),
    Promise.all([
      import('../../../../bonsai/zarathustra/core/core.json'),
      import('../../../../bonsai/zarathustra/core/concepts.json'),
      import('../../../../bonsai/zarathustra/core/figures.json'),
      import('../../../../bonsai/zarathustra/core/links.json'),
      import('../../../../bonsai/zarathustra/core/chains.json'),
      import('../../../../bonsai/zarathustra/core/proposed.json'),
      import('../../../../bonsai/zarathustra/core/voices.json'),
      import('../../../../bonsai/zarathustra/core/place-relations.json'),
      import('../../../../bonsai/zarathustra/core/semantic-uses.json'),
    ]).then(([manifest,concepts,figures,links,chains,proposed,voices,placeRelations,semanticUses])=>({manifest:json(manifest),concepts:json(concepts),figures:json(figures),links:json(links),chains:json(chains),proposed:json(proposed),voices:json(voices),placeRelations:json(placeRelations),semanticUses:json(semanticUses)})),
    import('../../../../bonsai/zarathustra/renderings.json'),
  ]).then(([chronology,places,identities,basemap,core,renderings])=>({chronology:json(chronology),places:json(places),identities:json(identities),basemap:json(basemap),core,renderings:json(renderings)})),
});
const BONSAI={
  antiquity:Object.freeze({manifest:antiquityManifest}),
  zarathustra:Object.freeze({manifest:zarathustraManifest}),
};
const loading=new Map();
// A stand's philological core (bonsai/<stand>/terms.json) comes with it where the stand keeps one.
const TERMS=import.meta.glob('../../../../bonsai/*/terms.json',{import:'default'});
const termsOf=profile=>TERMS[`../../../../bonsai/${profile}/terms.json`]?.()??Promise.resolve(null);

/** Load a stand's bonsai modules once; later calls share the same load. */
export function loadBonsai(profile){
  if(!LOADERS[profile])return Promise.resolve(null);
  if(!loading.has(profile))loading.set(profile,Promise.all([LOADERS[profile](),termsOf(profile)]).then(([modules,terms])=>(BONSAI[profile]=Object.freeze({...BONSAI[profile],...modules,terms}))).catch(error=>{loading.delete(profile);throw error;}));
  return loading.get(profile);
}

/** The bonsai a stand grows from, or null: its manifest always, its modules once `loadBonsai` has brought them. */
export const bonsaiFor=profile=>BONSAI[profile]??null;

/**
 * What depth means on the lenses that group: the steps it orders stars by, deep to near, with their names,
 * and the pose the space rests in so that its depth shows; null where the bonsai declares none.
 */
export function depthOf(profile){
  const declared=BONSAI[profile]?.manifest?.depth;
  return Array.isArray(declared?.steps)&&declared.steps.length?declared:null;
}

/** How a lens's positions read: a measure (with what it measures) or a grouping (with what it groups by). */
export function lensPositions(profile,lensId){
  const declared=BONSAI[profile]?.manifest?.lenses?.[lensId];
  return declared??{positions:'group'};
}

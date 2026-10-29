// The stands read their bonsai through this one door.
//
// A bonsai (`bonsai/<stand>/` at the repository root) is a stand's own
// mini-Tree: its manifest and modules. The stand presents them and never
// writes them; what it shows on a lens comes from here, with the sources and
// footing each module keeps.

import antiquityManifest from '../../../../bonsai/antiquity/bonsai.json' with {type:'json'};
import antiquityCollection from '../../../../bonsai/antiquity/collection.json' with {type:'json'};
import antiquityFirstBranches from '../../../../bonsai/antiquity/first-branches.json' with {type:'json'};
import antiquityChronology from '../../../../bonsai/antiquity/chronology.json' with {type:'json'};
import antiquityPlaces from '../../../../bonsai/antiquity/places.json' with {type:'json'};
import antiquityIdentities from '../../../../bonsai/antiquity/identities.json' with {type:'json'};
import zarathustraManifest from '../../../../bonsai/zarathustra/bonsai.json' with {type:'json'};
import zarathustraChronology from '../../../../bonsai/zarathustra/chronology.json' with {type:'json'};
import zarathustraPlaces from '../../../../bonsai/zarathustra/places.json' with {type:'json'};
import zarathustraIdentities from '../../../../bonsai/zarathustra/identities.json' with {type:'json'};
import coastlines from '../../../../bonsai/shared/coastlines.json' with {type:'json'};

export {antiquityCollection,antiquityFirstBranches};

const BONSAI=Object.freeze({
  antiquity:Object.freeze({manifest:antiquityManifest,collection:antiquityCollection,firstBranches:antiquityFirstBranches,chronology:antiquityChronology,places:antiquityPlaces,basemap:coastlines,identities:antiquityIdentities}),
  zarathustra:Object.freeze({manifest:zarathustraManifest,chronology:zarathustraChronology,places:zarathustraPlaces,basemap:coastlines,identities:zarathustraIdentities}),
});

/** The bonsai a stand grows from, or null. */
export const bonsaiFor=profile=>BONSAI[profile]??null;

/** How a lens's positions read: a measure (with what it measures) or a grouping (with what it groups by). */
export function lensPositions(profile,lensId){
  const declared=BONSAI[profile]?.manifest?.lenses?.[lensId];
  return declared??{positions:'group'};
}

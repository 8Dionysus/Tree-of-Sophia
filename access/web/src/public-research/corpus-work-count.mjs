import {corpusProfile} from './bonsai.mjs';

/** Count this stand's selected works, retaining its scope when it reads via a fallback profile. */
export function publicCorpusWorkCount(profile, manifest, identities = {}) {
  const ownIds = manifest?.profiles?.[profile]?.documentIds;
  if (Array.isArray(ownIds)) return ownIds.length;

  const fallbackIds = manifest?.profiles?.[corpusProfile(profile)]?.documentIds;
  if (!Array.isArray(fallbackIds)) return null;
  if (profile !== 'aesthetics') return fallbackIds.length;

  const selectedWorks = new Set(Object.values(identities ?? {}).map(item => item?.tree).filter(id => typeof id === 'string'));
  return fallbackIds.filter(id => selectedWorks.has(id)).length;
}

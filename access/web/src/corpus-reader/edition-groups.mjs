function canonicalSourceUrl(value) {
  if (typeof value !== 'string' || !value.trim()) return '';
  try {
    const url = new URL(value);
    if (url.protocol !== 'https:') return '';
    if (url.hostname === 'raw.githubusercontent.com') {
      const parts = url.pathname.split('/').filter(Boolean);
      if (parts.length < 4) return '';
      url.hostname = 'github.com';
      url.pathname = `/${parts[0]}/${parts[1]}/blob/${parts.slice(2).join('/')}`;
    }
    if (url.hostname !== 'github.com') return url.href;
    url.hash = '';
    return `${url.origin}${url.pathname}`;
  } catch {
    return '';
  }
}

/**
 * Group reader representations only when their source URL identifies the
 * same upstream file. Each version stays independently addressable.
 */
export function groupEditionRepresentations(versions = []) {
  const groups = new Map();
  for (const version of versions) {
    const sourceUrl = canonicalSourceUrl(version?.sourceUrl);
    const key = sourceUrl ? `${version.language || ''}\u0000${sourceUrl}` : `version:${version?.id || groups.size}`;
    if (!groups.has(key)) groups.set(key, {key, versions: []});
    groups.get(key).versions.push(version);
  }
  return [...groups.values()];
}

/** Human-readable label for the preserved reading layer inside a group. */
export function editionRepresentationKind(version) {
  const path = version?.source?.textLayerRef || '';
  if (path.includes('/editions/retained-reading/')) return 'retained';
  if (path.includes('/editions/digital-source/')) return 'source';
  return 'text';
}

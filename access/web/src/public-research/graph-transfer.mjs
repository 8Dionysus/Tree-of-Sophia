export const PUBLIC_GRAPH_SPACE_SCHEMA = 'tos.public_research.graph_space.v1';
export const PUBLIC_GRAPH_SPACE_VERSION = 1;
export const MAX_GRAPH_SPACE_BYTES = 8_000_000;
const MAX_GRAPH_PACKET_BYTES = 1_000_000;
const MAX_GRAPH_ENTRIES = 64;
const text = value => new TextEncoder().encode(value).byteLength;
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const only = (value, fields) => record(value) && Object.keys(value).length === fields.length && fields.every(field => Object.hasOwn(value, field));
const blockedObjectKeys = new Set(['__proto__', 'prototype', 'constructor']);

/** Copy an explicitly compatible predecessor; preserve the original namespace. */
export function upgradePublicGraph(storage,{profile,fingerprint,predecessors=[],validate,pristineGraph=null}={}) {
  if(!storage||typeof validate!=='function')return {upgraded:false};
  const key=`tos.public-research.graph.v1.${profile}.${fingerprint}`;
  const current=storage.getItem(key);
  // An earlier archive may be imported after this release has seeded a page.
  // Only that exact owner-produced seed may be replaced, never personal edits.
  if(current!==null&&current!==pristineGraph)return {upgraded:false};
  for(const predecessor of predecessors){
    if(!/^[a-f0-9]{64}$/.test(predecessor.fingerprint)||predecessor.fingerprint===fingerprint)continue;
    const previousKey=`tos.public-research.graph.v1.${profile}.${predecessor.fingerprint}`;
    const original=storage.getItem(previousKey);if(original===null)continue;
    const packet=JSON.parse(original),allowed=new Set(predecessor.materialIds??[]);
    if(packet.libraryFingerprint!==predecessor.fingerprint||!Array.isArray(packet.nodes)||packet.nodes.some(node=>node.materialId&&!allowed.has(node.materialId)))throw new Error('The saved graph does not match its declared predecessor.');
    const candidate=validate(JSON.stringify({...packet,libraryFingerprint:fingerprint}));
    const owners=storage.getItem(`${previousKey}.owners`);
    const currentOwners=storage.getItem(`${key}.owners`);
    if(currentOwners!==null&&currentOwners!=='{}'&&currentOwners!==owners)throw new Error('The current graph has different personal bindings. Both original graphs are preserved.');
    if(storage.getItem(key)!==current)throw new Error('The current graph changed during restoration. Both original graphs are preserved.');
    // The current Rust owner validates every reference before any storage write.
    storage.setItem(key,candidate);
    if(owners!==null)storage.setItem(`${key}.owners`,owners);
    return {upgraded:true,from:previousKey,to:key};
  }
  return {upgraded:false};
}

export function graphKeyInfo(key) {
  if (typeof key !== 'string' || key.length > 256) return null;
  let match = /^tos\.public-research\.graph\.v1\.([a-z0-9][a-z0-9-]{0,63})\.([a-f0-9]{64})$/.exec(key);
  if (match) return {kind: 'graph', profile: match[1], fingerprint: match[2], key};
  match = /^tos\.public-research\.graph\.v1\.([a-z0-9][a-z0-9-]{0,63})\.([a-f0-9]{64})\.owners$/.exec(key);
  if (match) return {kind: 'owners', profile: match[1], fingerprint: match[2], graphKey: key.slice(0, -7), key};
  match = /^tos\.public-research\.graph-positions\.v1\.([a-z0-9][a-z0-9-]{0,63})$/.exec(key);
  if (match) return {kind: 'positions', profile: match[1], key};
  return null;
}

function parseEntryValue(entry, info, graphs) {
  if (typeof entry.value !== 'string' || text(entry.value) > MAX_GRAPH_PACKET_BYTES) throw new TypeError('Invalid or oversized public graph record.');
  let value;
  try { value = JSON.parse(entry.value); } catch { throw new TypeError('A public graph record is not valid JSON.'); }
  if (!record(value)) throw new TypeError('A public graph record must be an object.');
  if (info.kind === 'graph') {
    if (!only(value, ['schema', 'version', 'libraryFingerprint', 'title', 'nodes', 'edges', 'revision'])
      || value.schema !== 'tos_constructor_workspace_v1' || value.version !== 1 || value.libraryFingerprint !== info.fingerprint
      || typeof value.title !== 'string' || value.title.length > 512 || !Array.isArray(value.nodes) || value.nodes.length > 200
      || !Array.isArray(value.edges) || value.edges.length > 600 || !Number.isSafeInteger(value.revision) || value.revision < 0) {
      throw new TypeError('A public graph packet does not match the bounded constructor graph schema.');
    }
    const ids = new Set();
    for (const node of value.nodes) {
      if (!record(node) || typeof node.id !== 'string' || node.id.length < 1 || node.id.length > 256 || ids.has(node.id)) throw new TypeError('A public graph packet has an invalid or duplicate node ID.');
      ids.add(node.id);
    }
    for (const edge of value.edges) if (!record(edge) || typeof edge.id !== 'string' || edge.id.length < 1 || edge.id.length > 256) throw new TypeError('A public graph packet has an invalid edge ID.');
    graphs.set(entry.key, true);
  } else if (info.kind === 'owners') {
    if (Object.keys(value).length > 200) throw new TypeError('A public graph owner map exceeds 200 entries.');
    for (const [ownerId, graphId] of Object.entries(value)) {
      if (blockedObjectKeys.has(ownerId) || ownerId.length < 1 || ownerId.length > 256 || typeof graphId !== 'string' || graphId.length < 1 || graphId.length > 256) throw new TypeError('A public graph owner map contains an invalid binding.');
    }
  } else {
    if (Object.keys(value).length > 200) throw new TypeError('A public graph positions map exceeds 200 entries.');
    for (const [nodeId, position] of Object.entries(value)) {
      if (blockedObjectKeys.has(nodeId) || nodeId.length < 1 || nodeId.length > 256 || !Array.isArray(position) || position.length !== 3
        || !position.every(axis => Number.isFinite(axis) && Math.abs(axis) <= 10_000_000)) throw new TypeError('A public graph positions map contains an invalid position.');
    }
  }
}

/** Missing graphSpace is the v1 legacy format and remains valid. */
export function validateGraphSpace(value) {
  if (value === undefined) return null;
  if (!only(value, ['schema', 'version', 'entries']) || value.schema !== PUBLIC_GRAPH_SPACE_SCHEMA || value.version !== PUBLIC_GRAPH_SPACE_VERSION
    || !Array.isArray(value.entries) || value.entries.length > MAX_GRAPH_ENTRIES) throw new TypeError('Unsupported public graph-space packet.');
  const entries = [], keys = new Set(), graphs = new Map(); let byteCount = 0;
  for (const entry of value.entries) {
    if (!only(entry, ['key', 'value']) || !graphKeyInfo(entry.key) || keys.has(entry.key)) throw new TypeError('A public graph-space entry has an invalid or duplicate storage key.');
    keys.add(entry.key); const info = graphKeyInfo(entry.key); parseEntryValue(entry, info, graphs);
    byteCount += text(entry.key) + text(entry.value);
    entries.push({key: entry.key, value: entry.value});
  }
  for (const entry of entries) {
    const info = graphKeyInfo(entry.key);
    if (info.kind !== 'owners') continue;
    if (!graphs.has(info.graphKey)) throw new TypeError('A public graph owner map requires its graph packet in the same transfer.');
  }
  if (byteCount > MAX_GRAPH_SPACE_BYTES) throw new TypeError('The public graph-space packet exceeds 8 MB.');
  return {schema: PUBLIC_GRAPH_SPACE_SCHEMA, version: PUBLIC_GRAPH_SPACE_VERSION, entries};
}

export function exportGraphSpace(storage) {
  if (!storage) throw new TypeError('Browser graph storage is unavailable.');
  const entries = [];
  for (let index = 0; index < storage.length; index++) {
    const key = storage.key(index);
    if (!graphKeyInfo(key)) continue;
    const value = storage.getItem(key);
    if (value !== null) entries.push({key, value});
  }
  entries.sort((left, right) => left.key.localeCompare(right.key));
  return validateGraphSpace({schema: PUBLIC_GRAPH_SPACE_SCHEMA, version: PUBLIC_GRAPH_SPACE_VERSION, entries});
}

export function inspectGraphSpace(storage, graphSpace, {pristineGraphs = {}} = {}) {
  const packet = validateGraphSpace(graphSpace);
  if (!packet) return {entries: [], snapshot: [], addable: 0, equal: 0, conflicts: []};
  const entries = packet.entries.map(entry => {
    const info = graphKeyInfo(entry.key), current = storage.getItem(entry.key);
    return {...entry, ...info, current, status: current === null ? 'missing' : current === entry.value ? 'equal' : info.kind === 'graph' && current === pristineGraphs[entry.key] ? 'pristine' : 'conflict'};
  });
  const statuses = new Map(entries.filter(item => item.kind === 'graph').map(item => [item.key, item.status]));
  for (const item of entries) {
    if (item.kind === 'owners' && statuses.get(item.graphKey) === 'conflict') item.status = 'blocked';
    if (item.kind === 'positions') {
      const profileGraphs = entries.filter(candidate => candidate.kind === 'graph' && candidate.profile === item.profile);
      if (profileGraphs.length && profileGraphs.every(candidate => candidate.status === 'conflict')) item.status = 'blocked';
    }
  }
  return {entries, snapshot: entries.map(item => [item.key, item.current]),
    addable: entries.filter(item => item.status === 'missing' || item.status === 'pristine').length, equal: entries.filter(item => item.status === 'equal').length,
    conflicts: entries.filter(item => item.status === 'conflict' || item.status === 'blocked')};
}

/** Add absent records only. A changed destination is always preserved. */
export function applyGraphSpace(storage, graphSpace, options = {}) {
  if (!graphSpace) return {added: [], equal: [], conflicts: []};
  const inspection = inspectGraphSpace(storage, graphSpace, options), added = [], equal = [], conflicts = inspection.conflicts.map(item => item.key);
  const order = {graph: 0, owners: 1, positions: 2};
  for (const item of [...inspection.entries].sort((a, b) => order[a.kind] - order[b.kind])) {
    if (item.status === 'equal') { equal.push(item.key); continue; }
    if (item.status !== 'missing' && item.status !== 'pristine') continue;
    const current = storage.getItem(item.key);
    if (current !== null && !(item.status === 'pristine' && current === item.current)) { if (current === item.value) equal.push(item.key); else conflicts.push(item.key); continue; }
    storage.setItem(item.key, item.value); added.push(item.key);
  }
  return {added, equal, conflicts: [...new Set(conflicts)]};
}

/** A newly added graph or companion for the active profile needs remounting. */
export function shouldReloadImportedGraph(addedKeys, profile) {
  return Array.isArray(addedKeys) && addedKeys.some(key => graphKeyInfo(key)?.profile === profile);
}

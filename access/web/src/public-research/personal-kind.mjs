// A shared owner ID, not a profile-local draft ID, is the only safe bridge
// between historical graph copies. Conflicting old copies remain unspecified.
const namespace = /^tos\.public-research\.graph\.v1\.(?:antiquity|zarathustra)\.[a-f0-9]{64}\.owners$/;
const known = new Set(['concept', 'interpretation', 'question', 'note', 'excerpt', 'thought']);

export function readPersonalKinds(storage) {
  const kindsByOwner = new Map();
  try {
    for (let i = 0; i < storage.length; i++) {
      const key = storage.key(i);
      if (!namespace.test(key || '')) continue;
      const owners = JSON.parse(storage.getItem(key) || '{}');
      const packet = JSON.parse(storage.getItem(key.slice(0, -'.owners'.length)) || 'null');
      if (!Array.isArray(packet?.nodes)) continue;
      const nodes = new Map(packet.nodes.map(node => [node.id, node]));
      for (const [ownerId, graphId] of Object.entries(owners)) {
        const kind = nodes.get(graphId)?.kind;
        if (!known.has(kind) || kind === 'thought') continue;
        if (!kindsByOwner.has(ownerId)) kindsByOwner.set(ownerId, new Set());
        kindsByOwner.get(ownerId).add(kind);
      }
    }
    return kindsByOwner;
  } catch { return null; /* Incomplete evidence cannot establish a kind. */ }
}

export function personalKind(hypothesis, evidence, currentKind = null) {
  if (known.has(hypothesis?.kind)) return hypothesis.kind;
  if (!evidence) return 'thought';
  const kinds = new Set(evidence.get(hypothesis?.id) || []);
  if (known.has(currentKind) && currentKind !== 'thought') kinds.add(currentKind);
  return kinds.size === 1 ? [...kinds][0] : 'thought';
}

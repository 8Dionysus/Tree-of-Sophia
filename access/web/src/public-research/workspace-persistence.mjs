export function createCompareAndSwapPersistence(storage, key) {
  if (!storage || typeof storage.getItem !== 'function' || typeof storage.setItem !== 'function'
    || typeof key !== 'string' || !key.trim()) throw new TypeError('A local storage owner and key are required.');
  let lastSaved = null;
  return {
    load() { lastSaved = storage.getItem(key); return lastSaved; },
    save(packet) {
      const current = storage.getItem(key);
      if (current !== lastSaved) throw new Error('Another tab changed this research workspace. Export the unsaved draft before reloading.');
      storage.setItem(key, packet);
      lastSaved = packet;
    },
  };
}

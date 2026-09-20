import {describe,expect,it,vi} from 'vitest';
import {NATIVE_REFERENCE_SCHEMA} from './native-reference.mjs';
import {createNotebookMetadataResolver,notebookFallbackLabel,notebookReferenceWorkId} from './notebook-labels.mjs';

describe('notebook metadata labels', () => {
  it('resolves off-page work metadata with bounded concurrency and an LRU cache', async () => {
    let active = 0;
    let peak = 0;
    const calls = [];
    const readDocument = vi.fn(async documentId => {
      calls.push(documentId);
      active += 1;
      peak = Math.max(peak, active);
      await new Promise(resolve => setTimeout(resolve, 0));
      active -= 1;
      return {id: documentId, title: `Work ${documentId}`, versions: [{id: `${documentId}-ru`, language: 'ru'}]};
    });
    const resolver = createNotebookMetadataResolver({readDocument, maxConcurrent: 2, maxEntries: 2});

    const values = await Promise.all(['off-page-a', 'off-page-b', 'off-page-c'].map(documentId => resolver.resolve(documentId)));
    expect(values.map(value => value?.title)).toEqual(['Work off-page-a', 'Work off-page-b', 'Work off-page-c']);
    expect(peak).toBe(2);
    expect(resolver.stats()).toMatchObject({active: 0, queued: 0, pending: 0, cache: 2});

    await resolver.resolve('off-page-c');
    expect(calls.filter(value => value === 'off-page-c')).toHaveLength(1);
  });

  it('keeps failed metadata lookups generic and never treats native references as corpus works', async () => {
    const readDocument = vi.fn(async () => { throw new Error('metadata unavailable'); });
    const resolver = createNotebookMetadataResolver({readDocument});

    await expect(resolver.resolve('missing-work')).resolves.toBeNull();
    await expect(resolver.resolve('missing-work')).resolves.toBeNull();
    expect(readDocument).toHaveBeenCalledTimes(1);
    expect(notebookReferenceWorkId({target: {workId: 'off-page-work'}})).toBe('off-page-work');
    expect(notebookReferenceWorkId({
      schemaVersion: NATIVE_REFERENCE_SCHEMA,
      target: {workId: 'must-not-leak'},
    })).toBeNull();
    expect(notebookFallbackLabel({kind: 'bookmark', reference: {target: {workId: 'must-not-leak'}}}, {index: 0, passage: 'Passage'})).toBe('Passage · 1');
    expect(notebookFallbackLabel({kind: 'bookmark', reference: {target: {workId: 'must-not-leak'}}}, {index: 1, passage: 'Passage'})).toBe('Passage · 2');
    expect(notebookFallbackLabel({reference: {selector: {start: 3, end: 8}}}, {index: 0, passage: 'Passage', rangeLabel: 'characters'})).toBe('Passage · 1 · 3–8 characters');
  });

  it('settles queued lookups when the reader is disposed', async () => {
    let release;
    const readDocument = vi.fn(() => new Promise(resolve => { release = resolve; }));
    const resolver = createNotebookMetadataResolver({readDocument, maxConcurrent: 1});
    const pending = resolver.resolve('work-a');
    resolver.dispose();
    await expect(pending).resolves.toBeNull();
    release?.({id: 'work-a', title: 'late'});
    expect(resolver.stats()).toMatchObject({disposed: true, queued: 0, pending: 0});
  });
});

import {expect, test} from 'vitest';
import {editionRepresentationKind, groupEditionRepresentations} from './edition-groups.mjs';

test('the two published Meno layers group only when their upstream file and language match', () => {
  const versions = [
    {id: 'perseus-lamb-1924-en', language: 'en', sourceUrl: 'https://raw.githubusercontent.com/PerseusDL/canonical-greekLit/commit/data/tlg024/perseus-eng2.xml', source: {textLayerRef: 'works/meno/editions/digital-source/reading-layer.txt'}},
    {id: 'retained-meno-eng2', language: 'en', sourceUrl: 'https://github.com/PerseusDL/canonical-greekLit/blob/commit/data/tlg024/perseus-eng2.xml', source: {textLayerRef: 'works/meno/editions/retained-reading/reading-layer.txt'}},
    {id: 'perseus-burnet-1903-grc', language: 'grc', sourceUrl: 'https://github.com/PerseusDL/canonical-greekLit/blob/commit/data/tlg024/perseus-grc2.xml'},
    {id: 'unattributed-layer', language: 'en', sourceUrl: ''},
  ];

  const groups = groupEditionRepresentations(versions);
  expect(groups.map(group => group.versions.map(version => version.id))).toEqual([
    ['perseus-lamb-1924-en', 'retained-meno-eng2'],
    ['perseus-burnet-1903-grc'],
    ['unattributed-layer'],
  ]);
  expect(editionRepresentationKind(versions[0])).toBe('source');
  expect(editionRepresentationKind(versions[1])).toBe('retained');
  expect(groups.flatMap(group => group.versions.map(version => version.id))).toEqual(versions.map(version => version.id));
});

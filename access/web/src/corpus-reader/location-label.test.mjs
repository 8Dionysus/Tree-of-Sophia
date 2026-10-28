import {expect, test} from 'vitest';
import {readableUnitLocator} from './location-label.mjs';

test('a canonical anchor object is never shown as the source locator', () => {
  const unit = {reference: {schemaVersion: 'tos.corpus.reader.reference.v1', versionId: 'lysis-en', unitId: 'u212'}, ordinal: 212};
  expect(readableUnitLocator(unit, 'en')).toBe('');
  expect(readableUnitLocator({...unit, label: '212'}, 'en')).toBe('212');
});

test('display locators prefer the source witness string and localize declared labels', () => {
  expect(readableUnitLocator({reference: {unitId: 'u1'}, sourceReference: 'Book 9 · 582'}, 'en')).toBe('Book 9 · 582');
  expect(readableUnitLocator({label: {ru: 'Книга 9 · 582', en: 'Book 9 · 582'}}, 'en')).toBe('Book 9 · 582');
});

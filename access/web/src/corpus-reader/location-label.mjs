/** Return only a provider-supplied display locator, never a canonical anchor. */
export function readableUnitLocator(unit, locale = 'ru') {
  const candidate = typeof unit?.reference === 'string'
    ? unit.reference
    : typeof unit?.sourceReference === 'string'
      ? unit.sourceReference
      : unit?.label;
  if (typeof candidate === 'string') return candidate;
  if (!candidate || typeof candidate !== 'object' || Array.isArray(candidate)) return '';
  const language = String(locale || 'ru').toLowerCase().split('-')[0];
  const keys = [locale, language, 'default', 'original', 'ru', 'en'];
  for (const key of keys) {
    if (typeof candidate[key] === 'string' && candidate[key].trim()) return candidate[key];
  }
  return Object.values(candidate).find(value => typeof value === 'string' && value.trim()) || '';
}

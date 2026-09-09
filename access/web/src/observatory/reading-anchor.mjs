// Page-local DOM anchors may be arbitrary; durable reading accepts only these
// bounded position identities. Source wording never becomes an anchor key.
export const persistentReadingAnchorKey=value=>typeof value==='string'
  &&/^(?:(?:description|statement):\d{1,6}|(?:record-context:\d{1,3}|claim-context|form:(?:name|caption|hover|statement|grounds|history|technical):(?:heading|wording|language|metadata|(?:context|binding):\d{1,3}))(?::part:\d{1,6})?)$(?![\s\S])/.test(value);

// Read the stand's unit-scoped proposals and lexical references.
import uses from '../../../../bonsai/zarathustra/core/semantic-uses.json' with {type:'json'};

const readings=new Map(uses.terms.flatMap(term=>term.occurrences.filter(use=>use.reader_reading).map(use=>[use.cite.u[0],{form:use.form,lemma:use.reader_reading.lemma,ru:use.reader_reading.ru,en:use.reader_reading.en,history:use.reader_reading.history}])));
export function lexicalReading(reference,form){
  if(reference?.versionId!==uses.witness)return null;
  const entry=readings.get(reference.unitId);
  return entry?.form===form?{...entry}:null;
}

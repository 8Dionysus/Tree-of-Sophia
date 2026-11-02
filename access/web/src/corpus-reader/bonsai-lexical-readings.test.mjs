import {expect,test} from 'vitest';
import uses from '../../../../bonsai/zarathustra/core/semantic-uses.json';
import {lexicalReading} from './bonsai-lexical-readings.mjs';

test('passage readings are source-owned proposals with grounds',()=>{
 const occurrence=uses.terms.flatMap(term=>term.occurrences).find(item=>item.cite.u[0]==='tos.text-unit.sid-2a1b29f9c62115040c9a0330cad6d16e'&&item.form==='Gold');
 expect(occurrence.reader_reading.review_status).toBe('stand-proposal');
 expect(occurrence.reader_reading.grounds).toContainEqual(occurrence.cite);
 expect(lexicalReading({versionId:uses.witness,unitId:occurrence.cite.u[0]},occurrence.form).en).toBe(occurrence.reader_reading.en);
});

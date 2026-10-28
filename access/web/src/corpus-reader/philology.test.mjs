import '../observatory/human-forms-wasm-test-runtime.mjs';
import {expect, test} from 'vitest';
import {createReference} from './model.mjs';
import {bindPhilologyResult, philologyMarkIntervals, selectedPhilologyForm, morphologyLabel, isSingleWordSelection} from './philology.mjs';

const source = {
  workId:'work-1', expressionId:'expression-1', editionId:'edition-1', itemId:'item-1', fileId:'file-1',
  fileSha256:'a'.repeat(64), textLayerRef:'source.txt', textLayerSha256:'b'.repeat(64),
};

test('dictionary analysis distinguishes a word from a selected phrase', () => {
  expect(isSingleWordSelection('ἀρετή')).toBe(true);
  expect(isSingleWordSelection('Seele')).toBe(true);
  expect(isSingleWordSelection('e\u0301')).toBe(true);
  expect(isSingleWordSelection('ἆρα διδακτὸν ἡ ἀρετή')).toBe(false);
  expect(isSingleWordSelection('unterredete sich')).toBe(false);
});

test('selected form follows the exact Unicode-code-point occurrence', () => {
  const text = 'Kreis 😀Kreis e\u0301';
  const reference = createReference({source, versionId:'de-v1', unitId:'unit-1', start:7, end:12});
  expect(selectedPhilologyForm(text, reference)).toBe('Kreis');
  const composedRange = createReference({source, versionId:'de-v1', unitId:'unit-1', start:13, end:15});
  expect(selectedPhilologyForm(text, composedRange)).toBe('e\u0301');
});

test('analysis presentation accepts only a versioned packet bound to the exact reference', () => {
  const reference = createReference({source, versionId:'de-v1', unitId:'unit-1', start:3, end:8});
  const response = {
    status:'available', reference, analysisVersion:'fixture-lexicon-2',
    source:{id:'fixture-lexicon',version:'2'},
    candidates:[{lemma:'Kreis',morphology:'noun',sourceRef:'lexicon:Kreis',alternatives:['Kreisel']}],
  };
  expect(bindPhilologyResult(response, reference)).toMatchObject({
    status:'available', analysisVersion:'fixture-lexicon-2', candidates:response.candidates,
  });
  expect(bindPhilologyResult({status:'unavailable',reference,reason:'unrecognized',source:{id:'fixture-lexicon'},
    dictionaryLinks:[{label:'Dictionary',url:'https://example.test/word'}]},reference)).toMatchObject({
    status:'unavailable',reason:'unrecognized',source:{id:'fixture-lexicon'},dictionaryLinks:[{label:'Dictionary',url:'https://example.test/word'}],
  });
  const otherOccurrence = createReference({source, versionId:'de-v1', unitId:'unit-1', start:4, end:8});
  expect(bindPhilologyResult(response, otherOccurrence).status).toBe('mismatch');
  expect(bindPhilologyResult({...response, analysisVersion:undefined}, reference).status).toBe('unavailable');
});

test('translation proposals stay on their exact source occurrence and preserve other analysis state', () => {
  const reference=createReference({source,versionId:'de-v1',unitId:'unit-1',start:3,end:7});
  const other=createReference({source,versionId:'de-v1',unitId:'unit-1',start:9,end:13});
  const targetSource={...source,editionId:'en-v1',itemId:'item-en',fileId:'file-en',fileSha256:'c'.repeat(64),textLayerRef:'target.txt',textLayerSha256:'d'.repeat(64)};
  const mapping={status:'proposed',reviewStatus:'model_proposal_owner_review_pending',sourceReference:reference,sourceQuote:'Gold',targetReference:createReference({source:targetSource,versionId:'en-v1',unitId:'target-unit',start:0,end:6}),targetQuote:'Golden',reviewBasis:'contextual proposal'};
  const response={status:'available',reference,analysisVersion:'fixture-2',candidates:[{lemma:'gold'}],mappings:[mapping],mappingStatus:'available'};
  expect(bindPhilologyResult(response,reference,'Gold')).toMatchObject({status:'available',mappingStatus:'available',mappings:[mapping]});
  expect(bindPhilologyResult({...response,mappings:[{...mapping,sourceReference:other}]},reference,'Gold'))
    .toMatchObject({status:'available',mappingStatus:'none',mappings:[],candidates:response.candidates});
});

test('the selected second occurrence owns its mark and search highlights do not overlap it', () => {
  const text = 'Gold · Gold';
  const intervals = philologyMarkIntervals(Array.from(text).length,[{start:0,end:4},{start:7,end:11}],{start:7,end:11},true);
  expect(intervals).toEqual([
    {start:0,end:4,selected:false},
    {start:7,end:11,selected:true},
  ]);
  expect(intervals.filter(item=>item.selected)).toHaveLength(1);
  const points=Array.from(text),rendered=[];let cursor=0;
  for(const interval of intervals){rendered.push(points.slice(cursor,interval.start).join(''),points.slice(interval.start,interval.end).join(''));cursor=interval.end;}
  rendered.push(points.slice(cursor).join(''));
  expect(rendered.join('')).toBe(text);
});

test('morphology renders readable language labels without changing the supplied analysis', () => {
  expect(morphologyLabel('noun · accusative · feminine · singular', 'ru')).toBe('существительное · винительный падеж · женский род · единственное число');
  expect(morphologyLabel('Gold<NN><Neut><Nom><Sg>', 'en')).toBe('noun · neuter · nominative · singular');
});

test('tagged morphology keeps alternative boundaries, order, and unfamiliar source tags',()=>{
  const original='d<~>ie<REL><Subst><Fem><Dat><Sg><St> / d<~>ie<REL><Subst><Masc><Nom><Sg><St> / d<~>ie<DEM><Subst><Fem><Dat><Sg><St>';
  expect(morphologyLabel(original,'en').split(' / ')).toEqual([
    '<~> · <REL> · <Subst> · feminine · dative · singular · <St>',
    '<~> · <REL> · <Subst> · masculine · nominative · singular · <St>',
    '<~> · <DEM> · <Subst> · feminine · dative · singular · <St>',
  ]);
  expect(morphologyLabel(original,'ru').split(' / ')).toEqual([
    '<~> · <REL> · <Subst> · женский род · дательный падеж · единственное число · <St>',
    '<~> · <REL> · <Subst> · мужской род · именительный падеж · единственное число · <St>',
    '<~> · <DEM> · <Subst> · женский род · дательный падеж · единственное число · <St>',
  ]);
  expect(morphologyLabel('x<Nom><Nom> / unrecognized','en')).toBe('nominative · nominative / unrecognized');
  expect(morphologyLabel('noun · provider-specific','ru')).toBe('существительное · provider-specific');
});

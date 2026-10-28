import {describe, expect, it} from 'vitest';
import {selectedSourceContext} from './source-context.mjs';

const revision = '48d5e7c0cbd364402d2e8d70a3bd1e30eaf1bb9e44c7dd2458d174da743a3652';
const version = {id:'retained-athenian-constitution-grc2-6e0c298a511e',revision,source:{textLayerSha256:revision}};
const unitId = 'tos.text-unit.sid-6276729d215947f182b03878e4bd2193';
const reference = (start,end) => ({versionId:version.id,unitId,target:{textLayerSha256:revision},selector:{start,end}});

describe('selected source context', () => {
  it('bounds a late Unicode selection around its actual location', () => {
    const text = '🌳'.repeat(2300) + 'target' + '🌳'.repeat(2200);
    const result = selectedSourceContext({version,unit:{id:unitId,text},reference:reference(2300,2306)});
    expect(Array.from(result.context)).toHaveLength(2000);
    expect(result.context).toContain('target');
    expect(result.contextRange.start).toBeLessThan(2300);
    expect(result.contextRange.end).toBeGreaterThan(2306);
    expect(result.contextTruncated).toBe(true);
    expect(result.editorialAnnotations).toEqual([]);
  });

  it('keeps projected roles bound to the exact version, hash, unit and matching text', () => {
    const unit = {id:unitId,text:'[…] κατηγοροῦντος Μύρωνος'};
    const result = selectedSourceContext({version,unit,reference:reference(18,25)});
    expect(result.context).toBe(unit.text);
    expect(result.contextRange).toEqual({start:0,end:25,positionUnit:'unicode_code_point',interval:'half_open'});
    expect(result.editorialAnnotations).toEqual([{kind:'gap',start:0,end:3},{kind:'addition',start:4,end:17}]);
    expect(selectedSourceContext({version,unit,reference:{...reference(4,17),target:{textLayerSha256:'wrong'}}})).toBeNull();
    expect(selectedSourceContext({version,unit:{...unit,text:'[…] Χατηγοροῦντος Μύρωνος'},reference:reference(4,17)}).editorialAnnotations)
      .toEqual([{kind:'gap',start:0,end:3}]);
  });

  it('starts a whole-unit point at the beginning and clips roles to context', () => {
    const unit = {id:unitId,text:'[…] κατηγοροῦντος Μύρωνος'};
    const result = selectedSourceContext({version,unit,reference:reference(0,0),limit:9});
    expect(result.contextRange).toMatchObject({start:0,end:9});
    expect(result.editorialAnnotations).toEqual([{kind:'gap',start:0,end:3},{kind:'addition',start:4,end:9}]);
    expect(result.contextTruncated).toBe(true);
  });

  it('exposes a whole-unit publication role only for its exact layer', () => {
    const sha='4f71adfeeb39e64055acc2fc3f58964cf39f03d27bf2264d25313bc45d8658ec';
    const version={id:'gutenberg-7205-de',revision:sha,source:{textLayerSha256:sha}};
    const unit={id:'license-unit',ordinal:3694,text:'Publication notice'};
    const reference={versionId:version.id,unitId:unit.id,target:{textLayerSha256:sha},selector:{start:0,end:0}};
    expect(selectedSourceContext({version,unit,reference}).sourceLayerKind).toBe('publication');
    expect(selectedSourceContext({version,unit:{...unit,ordinal:3693},reference})).not.toHaveProperty('sourceLayerKind');
  });
});

import {describe,it,expect} from 'vitest';
import {inclusionSummary} from './inclusion-summary.mjs';

describe('query inclusion explanations',()=>{
  it('separates a selected origin from a traversal and preserves the exact input',()=>{
    const reason={kind:'traversal',via_node_id:'opaque:n',via_relation_id:'opaque:r',depth:1,
      query:{direction:'incoming',max_depth:1,predicate_ids:['has_object']},unknown:{source:'unchanged'}};
    const before=structuredClone(reason);
    const lines=inclusionSummary(reason,{nodeLabel:()=>'<Исходный предмет>',relationLabel:()=> 'Имеет объект',predicateLabel:()=> 'имеет объект'});
    expect(lines).toContain('Материал найден при переходе по связям.');
    expect(lines).toContain('Переход от: «<Исходный предмет>».');
    expect(lines).toContain('Шагов от начала: 1.');
    expect(lines).toContain('Направление запроса: входящие.');
    expect(lines).toContain('Фильтр связей: имеет объект.');
    expect(reason).toEqual(before);
    expect(inclusionSummary({kind:'origin'})).toEqual(['Материал выбран началом этого раскрытия.']);
  });
  it('does not treat identity carriers as semantic hops or assert a missing label',()=>{
    const text=inclusionSummary({kind:'identity-carrier',via_node_id:'private:opaque',depth:0},{language:'en'}).join(' ');
    expect(text).toContain('same declared identity, not a new semantic neighbor');
    expect(text).not.toContain('private:opaque');expect(text).not.toContain('Hops');
  });
  it('makes future and malformed reasons visible without guessing their meaning',()=>{
    for(const kind of ['future-kind','__proto__',null])expect(inclusionSummary({kind})[0]).toContain('не поддерживается');
    expect(inclusionSummary({kind:'traversal',depth:NaN,query:{direction:'future',max_depth:-1}})).toEqual(['Материал найден при переходе по связям.']);
  });
  it('distinguishes relation-origin endpoints from returned relation context',()=>{
    expect(inclusionSummary({kind:'origin-endpoint'})[0]).toContain('выбранной исходной связи');
    expect(inclusionSummary({kind:'context-endpoint'},{language:'en'})[0]).toContain('returned relation');
    expect(inclusionSummary({kind:'focus'},{language:'en'})[0]).toContain('selected as the origin');
  });
});

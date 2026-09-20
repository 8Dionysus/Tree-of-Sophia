import {beforeEach,expect,test,vi} from 'vitest';
import {renderInspectorForms} from './knowledge-ui.mjs';
import {readableContextFor} from './readable-context.mjs';
import {essentialContext} from './record-context.mjs';
import {renderHumanForms,renderEssentialContext} from './human-forms-view.mjs';

// Reader validation and DOM rendering have their own contract suites. This
// protects the ordinary card's wiring to both validated context consumers.
vi.mock('./readable-context.mjs',()=>({readableContextFor:vi.fn()}));
vi.mock('./record-context.mjs',()=>({essentialContext:vi.fn()}));
vi.mock('./human-forms-view.mjs',()=>({renderHumanForms:vi.fn(),renderEssentialContext:vi.fn()}));
beforeEach(()=>vi.resetAllMocks());

test.each([null,{state:'complete',contexts:[]},{state:'requires-exact-context',contexts:[]}])(
  'ordinary inspector forwards the exact validated context posture %j',context=>{
    const raw={id:'fixture'},essential={state:'available',items:[]};
    readableContextFor.mockReturnValue(context);essentialContext.mockReturnValue(essential);
    renderHumanForms.mockReturnValue('forms');renderEssentialContext.mockReturnValue('record');
    expect(renderInspectorForms(raw)).toEqual(['forms','record']);
    expect(readableContextFor).toHaveBeenCalledWith(raw);
    const presentation=renderHumanForms.mock.calls[0][1].presentation;
    expect(renderHumanForms).toHaveBeenCalledWith(raw,{readableContext:context,presentation});
    expect(renderEssentialContext).toHaveBeenCalledWith(essential,context,presentation);
  });
test('unverified or changed presentation cannot silently fall back to raw card context',()=>{
  readableContextFor.mockImplementation(()=>{throw new Error('unverified');});
  expect(()=>renderInspectorForms({id:'fixture'})).toThrow('unverified');
  expect(renderHumanForms).not.toHaveBeenCalled();expect(renderEssentialContext).not.toHaveBeenCalled();
});

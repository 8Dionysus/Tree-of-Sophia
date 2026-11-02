import {expect,test,vi} from 'vitest';
import {configureResearchOperations,runResearchOperation} from './research-operations.mjs';
test('routes every research operator through the native rule',()=>{
 const decoder=new TextDecoder(),encoder=new TextEncoder();
 const kernel=vi.fn(bytes=>encoder.encode(JSON.stringify({request:JSON.parse(decoder.decode(bytes)),result:{native:true}})));
 configureResearchOperations({research_operation_wasm_v1:kernel});
 for(const operation of ['distribution','directed_path','common_grounds','interpretation_comparison','argument_structure','dependency_impact','time_place','signed_support','reading_distance','commitment_order']){
  const request={operation,scope:'selected',versions:{projection:'v1'},input:{}};
  expect(runResearchOperation(request)).toEqual({request,result:{native:true}});
 }
 expect(kernel).toHaveBeenCalledTimes(10);
});

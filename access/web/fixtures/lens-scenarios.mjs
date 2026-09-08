// Explicit UI fixtures, never a ToS source or a production registry.
import baseSchema from '../../contracts/lens-spec.v1.schema.json';
export const boundary={is_source:false,is_canon:false,writes_to_tree:false};
export const fixtureRevision='a'.repeat(64);
export function lensContext({properties=true,revision=fixtureRevision}={}){
  const schema=structuredClone(baseSchema);
  // Shape observed in the Foundation owner's lens-spec.v1.schema.json,
  // 2026-09-07. Integration against its completed backend remains separate.
  if(properties){const filter=schema.$defs.nodeFilter;filter.required=['op','value'];filter.oneOf=[{required:['field']},{required:['property_id']}];
    filter.properties.property_id={type:'string',pattern:'^tos\\.property\\.[a-z0-9-]+$(?![\\s\\S])'};}
  const property=(id,label,valueType,operators)=>({property_id:'tos.property.fixture-'+id,field:'attributes.fixture_'+id,labels:{ru:label,default:label},definition:'Искусственное свойство для проверки интерфейса; не утверждение о Древе.',value_type:valueType,applies_to:['tos.entity.fixture'],inherited:true,unit:null,language:null,operators});
  const catalog={schema:'tos_knowledge_catalog_v1',source_revision:revision,authority_boundary:boundary,
    node_kinds:[{kind_id:'fixture-material',display:{ru:'Проверочный материал'}}],predicates:[{predicate_id:'fixture-related',display:{ru:'Проверочная связь'}}],
    semantic_registries:{entity_types:{entries:[{type_id:'tos.entity.fixture',labels:{ru:'Проверочный материал'}}]},properties:[
      property('title','Название проверки','string',['eq','contains','prefix','exists']),property('number','Число проверки','number',['gte','lt','eq','exists']),
      property('flag','Признак проверки','boolean',['eq','exists']),property('tags','Метки проверки','string-array',['in','contains','exists'])]},
    capabilities:{sources:['philosophy'],filter_operators:['eq','neq','in','contains','prefix','exists','gt','gte','lt','lte'],
      operator_value_contracts:{eq:'scalar',neq:'scalar',in:'scalar-or-scalar-array',contains:'scalar-or-scalar-array',prefix:'string',exists:'boolean',gt:'number',gte:'number',lt:'number',lte:'number'},
      node_fields:['kind_id','display.title.default','epistemic.review_posture'],relation_fields:['predicate_id','display.label.default','epistemic.review_posture'],
      maximums:{nodes:1000,relations:2000,groups:200,traversal_depth:5},neighborhood_profiles:[{profile:'all'},{profile:'overview'}],
      inclusion:{authority:'query-execution-not-semantic-proof'},
      facets:{nodes:{'epistemic.review_posture':[{value:'fixture-reviewed',count:2},{value:'fixture-open',count:1}]},relations:{}},
      ...(properties?{property_filters:{selector:'property_id',scope:'node-query-and-path-node-query',binding:'same-graph-snapshot',operators:'declared-per-property'}}:{})}};
  return {catalog,schema};
}

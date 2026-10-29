// A compact validator for the JSON Schema 2020-12 keywords the Tree's contracts use.
//
// A bonsai checks what it would hand to the Tree against the Tree's own
// contracts as they stand in the checkout, not against a copy, so a contract
// that has grown or changed shows up as a failed check rather than as silent
// drift. Supported: $ref (absolute $id and local pointers), type, enum, const,
// pattern, minLength, minimum, maximum, items, minItems, uniqueItems,
// properties, required, additionalProperties, minProperties, allOf, anyOf,
// oneOf, not, if/then/else and unevaluatedProperties. `format` is an
// annotation here; any other keyword is refused rather than ignored.

const ANNOTATIONS=new Set(['$schema','$id','$defs','$comment','title','description','format','default','examples','deprecated','readOnly','writeOnly']);
const KEYWORDS=new Set(['$ref','type','enum','const','pattern','minLength','minimum','maximum','items','minItems','uniqueItems','properties','required','additionalProperties','minProperties','allOf','anyOf','oneOf','not','if','then','else','unevaluatedProperties']);
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const typeOf=value=>value===null?'null':Array.isArray(value)?'array':Number.isInteger(value)?'integer':typeof value;
const pointerEscape=key=>String(key).replaceAll('~','~0').replaceAll('/','~1');

function equal(a,b){
  if(a===b)return true;
  if(typeof a!==typeof b||a===null||b===null||typeof a!=='object')return false;
  if(Array.isArray(a)!==Array.isArray(b))return false;
  if(Array.isArray(a))return a.length===b.length&&a.every((item,index)=>equal(item,b[index]));
  const keys=Object.keys(a);return keys.length===Object.keys(b).length&&keys.every(key=>Object.hasOwn(b,key)&&equal(a[key],b[key]));
}

function matchesType(value,type){
  const actual=typeOf(value);
  return (Array.isArray(type)?type:[type]).some(item=>item===actual||(item==='number'&&(actual==='integer'||(actual==='number'&&Number.isFinite(value)))));
}

const patterns=new Map();
function regex(source){
  if(!patterns.has(source)){let compiled;try{compiled=new RegExp(source,'u');}catch{compiled=new RegExp(source);}patterns.set(source,compiled);}
  return patterns.get(source);
}

/** A validator over a set of schemas, addressed by their `$id`. */
export function createValidator(schemas){
  const documents=new Map();
  for(const schema of schemas)if(object(schema)&&typeof schema.$id==='string')documents.set(schema.$id.replace(/#$/,''),schema);
  function resolve(ref,document){
    const [address,fragment='']=String(ref).split('#');
    const base=address?documents.get(new URL(address,document?.$id??undefined).href):document;
    if(!base)throw new Error(`Unresolved schema reference: ${ref}`);
    let target=base;
    for(const raw of fragment.split('/').slice(1)){const key=decodeURIComponent(raw).replaceAll('~1','/').replaceAll('~0','~');if(!object(target)&&!Array.isArray(target)||!(key in target))throw new Error(`Unresolved schema pointer: ${ref}`);target=target[key];}
    return {schema:target,document:base};
  }
  function evaluate(schema,value,path,document){
    if(schema===true)return {errors:[],seen:new Set()};
    if(schema===false)return {errors:[{path,message:'no value is allowed here'}],seen:new Set()};
    if(!object(schema))throw new Error(`Not a schema at ${path}`);
    for(const key of Object.keys(schema))if(!KEYWORDS.has(key)&&!ANNOTATIONS.has(key))throw new Error(`Unsupported schema keyword ${key} at ${path}`);
    const errors=[],seen=new Set(),fail=message=>errors.push({path,message});
    const take=result=>{errors.push(...result.errors);if(!result.errors.length)for(const key of result.seen)seen.add(key);return !result.errors.length;};
    if(schema.$ref!==undefined){const target=resolve(schema.$ref,document);take(evaluate(target.schema,value,path,target.document));}
    if(schema.type!==undefined&&!matchesType(value,schema.type))fail(`expected ${[].concat(schema.type).join(' or ')}`);
    if(schema.enum!==undefined&&!schema.enum.some(item=>equal(item,value)))fail(`not one of ${JSON.stringify(schema.enum)}`);
    if(schema.const!==undefined&&!equal(schema.const,value))fail(`expected ${JSON.stringify(schema.const)}`);
    if(typeof value==='string'){
      if(schema.minLength!==undefined&&[...value].length<schema.minLength)fail(`shorter than ${schema.minLength}`);
      if(schema.pattern!==undefined&&!regex(schema.pattern).test(value))fail(`does not match ${schema.pattern}`);
    }
    if(typeof value==='number'){
      if(schema.minimum!==undefined&&value<schema.minimum)fail(`below ${schema.minimum}`);
      if(schema.maximum!==undefined&&value>schema.maximum)fail(`above ${schema.maximum}`);
    }
    if(Array.isArray(value)){
      if(schema.minItems!==undefined&&value.length<schema.minItems)fail(`fewer than ${schema.minItems} items`);
      if(schema.uniqueItems===true&&value.some((item,index)=>value.findIndex(other=>equal(other,item))!==index))fail('items are not unique');
      if(schema.items!==undefined)value.forEach((item,index)=>{errors.push(...evaluate(schema.items,item,`${path}/${index}`,document).errors);});
    }
    if(object(value)){
      const keys=Object.keys(value);
      if(schema.required!==undefined)for(const key of schema.required)if(!Object.hasOwn(value,key))fail(`missing ${key}`);
      if(schema.minProperties!==undefined&&keys.length<schema.minProperties)fail(`fewer than ${schema.minProperties} properties`);
      const declared=object(schema.properties)?schema.properties:{};
      // A property this schema declares counts as evaluated even when it fails, so its error is reported once.
      for(const key of keys)if(Object.hasOwn(declared,key)){take(evaluate(declared[key],value[key],`${path}/${pointerEscape(key)}`,document));seen.add(key);}
      if(schema.additionalProperties!==undefined)for(const key of keys)if(!Object.hasOwn(declared,key)){take(evaluate(schema.additionalProperties,value[key],`${path}/${pointerEscape(key)}`,document));seen.add(key);}
    }
    if(schema.allOf!==undefined)for(const item of schema.allOf)take(evaluate(item,value,path,document));
    if(schema.anyOf!==undefined){
      const results=schema.anyOf.map(item=>evaluate(item,value,path,document)),passing=results.filter(result=>!result.errors.length);
      if(!passing.length)fail('matches none of anyOf');else for(const result of passing)for(const key of result.seen)seen.add(key);
    }
    if(schema.oneOf!==undefined){
      const results=schema.oneOf.map(item=>evaluate(item,value,path,document)),passing=results.filter(result=>!result.errors.length);
      if(passing.length!==1)fail(`matches ${passing.length} of oneOf, not exactly one`);else for(const key of passing[0].seen)seen.add(key);
    }
    if(schema.not!==undefined&&!evaluate(schema.not,value,path,document).errors.length)fail('matches a forbidden schema');
    if(schema.if!==undefined){
      const condition=evaluate(schema.if,value,path,document);
      if(!condition.errors.length){for(const key of condition.seen)seen.add(key);if(schema.then!==undefined)take(evaluate(schema.then,value,path,document));}
      else if(schema.else!==undefined)take(evaluate(schema.else,value,path,document));
    }
    if(schema.unevaluatedProperties!==undefined&&object(value)){
      for(const key of Object.keys(value))if(!seen.has(key)){const result=evaluate(schema.unevaluatedProperties,value[key],`${path}/${pointerEscape(key)}`,document);if(result.errors.length)errors.push(schema.unevaluatedProperties===false?{path:`${path}/${pointerEscape(key)}`,message:'property is not allowed'}:result.errors[0]);else seen.add(key);}
    }
    return {errors,seen};
  }
  return {
    /** Errors of `value` against the schema at `ref` (an `$id`, optionally with a `#/pointer`). */
    validate(ref,value){const target=resolve(ref,null);return evaluate(target.schema,value,'',target.document).errors;},
    has:id=>documents.has(id),
  };
}

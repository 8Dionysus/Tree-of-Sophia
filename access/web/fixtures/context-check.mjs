import {renderEssentialContext} from '../src/observatory/human-forms-view.mjs';
import {setUiLanguage,uiLanguage} from '../src/observatory/ui-i18n.mjs';

// Run in the development fixture. Test data is intentionally not ToS content.
export function checkReadableContext(root){
  let checks=0;const require=(condition,message)=>{checks++;if(!condition)throw new Error(message);};
  const value={notes:'Possibly <img src=x onerror=alert(1)>\nKeep this qualification.',source_refs:['fixture:source'],
    constraints:{unknown_key:null,negated:false,allowed:true,zero:0,empty:'',list:[],object:{}},
    alternatives:['first','second'],unknown_field:{'a/b~c':'Untranslated source value'}};
  const input={state:'incomplete',items:[{pointer:'/fixture/context',state:'available',value},
    {pointer:'/fixture/unavailable',state:'unavailable'}]},before=JSON.stringify(input);
  const section=renderEssentialContext(input);root.append(section);const previous=uiLanguage(),labels=[];
  try{
    for(const language of ['ru','en','es']){
      setUiLanguage(language);labels.push(section.querySelector('h5').textContent);
      require(section.querySelector('[data-source-key=notes]').nextElementSibling.textContent===value.notes,'Source wording changed.');
      require(section.querySelector('[data-source-key=unknown_field]'),'An unknown field disappeared.');
      require(section.querySelector('[data-source-key="a/b~c"]').nextElementSibling.textContent==='Untranslated source value','An unknown value changed.');
      require(!section.querySelector('img'),'Source markup became active HTML.');
      const exact=JSON.parse(section.querySelector('.sc-context-exact pre').textContent);
      require(JSON.stringify(exact)===JSON.stringify(value),'Exact context lost a member, order, type or value.');
      require(section.querySelectorAll('[data-value-type=null]').length===1,'Null disappeared.');
      require(section.querySelectorAll('[data-value-type=boolean]').length===2,'A boolean disappeared.');
      require(section.querySelector('[data-value-type=number]').textContent==='0','Zero disappeared.');
      require(section.querySelectorAll('.sc-reader-gap').length===1,'Missing context was silently substituted.');
    }
    require(new Set(labels).size===3,'Context heading did not follow all interface languages.');
    require(JSON.stringify(input)===before,'Rendering mutated the supplied context.');
    return {status:'PASS',languages:3,checks};
  }finally{section.remove();setUiLanguage(previous);}
}

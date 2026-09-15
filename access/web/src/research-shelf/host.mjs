import {mountResearchShelf} from './view.mjs';
import {validateMaterialTarget,validateFormTarget,validateTarget} from './model.mjs';
import {readingSnapshot,readingDocument} from '../observatory/reader-model.mjs';
import {validateHumanForms,sameFormRef,FORM_ROLES} from '../observatory/human-forms.mjs';
import {RevisionError} from '../observatory/knowledge-client.mjs';
import {NATIVE_REFERENCE_SCHEMA} from '../corpus-reader/native-reference.mjs';

export function shelfMaterial(snapshot){
  return validateMaterialTarget({kind:snapshot.kind,id:snapshot.raw.id,sourceRevision:snapshot.sourceRevision,
    contentRevision:snapshot.raw.content_revision,...(snapshot.claimReference?{claimReference:snapshot.claimReference}:{})});
}
export function shelfForm(snapshot,role){
  const selected=validateHumanForms(snapshot.raw)?.roles[role];
  const reference=selected?.state==='ready'?selected.packet.form:snapshot.exactForms?.[role]?.form;
  return reference?validateFormTarget({material:shelfMaterial(snapshot),role,form:reference}):null;
}
export async function readShelfMaterial(client,target,{signal,language='ru',form=null}={}){
  const exact=validateMaterialTarget(target);
  const material=exact.claimReference?await client.readClaimReference(exact.claimReference,signal,{language,expected:exact.sourceRevision}):
    await client.readMaterial(exact.kind,exact.id,signal,exact.sourceRevision,exact.contentRevision,{language});
  if(material.match.content_revision!==exact.contentRevision)throw new RevisionError();
  const snapshot=readingSnapshot(material,exact.kind);
  if(form){const current=shelfForm(snapshot,form.role);if(!current||!sameFormRef(current.form,form.form))throw new RevisionError();}
  return snapshot;
}

// One local shelf entry point in both hosts. Personal content remains in its
// own store; source access still re-enters the exact shared read consumers.
export function mountResearchShelfEntry({root,client,corpus,locale=()=> 'ru',onMaterial,onLens,onRoute,onError,onViewChange}={}){
  const word=(ru,en)=>locale()==='en'?en:ru;
  const toolbar=root.querySelector('.main-tools,.sc-header-actions');if(!toolbar)throw new Error('The research toolbar is missing.');
  let disposed=false,reading=null;
  const report=error=>onError?.(error);
  const shelf=mountResearchShelf({host:document.body,locale,notebook:corpus.notebook,onError:report,
    onOpenView:()=>{root.inert=true;onViewChange?.(true);},onCloseView:()=>{root.inert=false;onViewChange?.(false);},
    async onOpen(record){
      reading?.abort();reading=new AbortController();const signal=reading.signal;
      const target=validateTarget(record.type,record.target);
      if(record.type==='material'||record.type==='form'){
        const material=record.type==='form'?target.material:target;
        const snapshot=await readShelfMaterial(client,material,{signal,language:locale(),form:record.type==='form'?target:null});
        if(disposed||signal.aborted)return;
        shelf.close();await onMaterial?.({snapshot,target:material,form:record.type==='form'?target:null});
      }else if(record.type==='text'){
        shelf.close();const reference=target.reference;
        if(reference.schemaVersion===NATIVE_REFERENCE_SCHEMA)await corpus.native.open({reference,note:record.note});
        else await corpus.open({documentId:reference.target.workId,versionId:reference.versionId,reference});
      }else {shelf.close();if(record.type==='lens')await onLens?.(target.draft);else await onRoute?.(target,record);}
    },
  });
  const opener=document.createElement('button');opener.type='button';opener.className='corpus-open sc-control';opener.dataset.researchShelf='true';
  const label=()=>{opener.textContent=word('Моя полка','My shelf');};label();toolbar.append(opener);
  opener.onclick=()=>{label();void shelf.open();};
  const narrow=root.querySelector('.header')?document.createElement('button'):null;
  if(narrow){narrow.type='button';narrow.className='research-shelf-narrow';narrow.textContent='☆';narrow.setAttribute('aria-label',word('Моя полка','My shelf'));narrow.onclick=()=>void shelf.open();root.querySelector('.header').append(narrow);}
  const save=async input=>{
    const result=await shelf.save(input);if(!disposed)root.dispatchEvent(new CustomEvent('sophia-research-saved',{detail:{record:result}}));return result;
  };
  const saveEvent=event=>void save(event.detail).catch(report);
  const openEvent=()=>void shelf.open();root.addEventListener('sophia-save-research',saveEvent);root.addEventListener('sophia-open-research',openEvent);
  function addReadingActions(container,snapshot){
    const title=String(readingDocument(snapshot,locale()).title?.text??word('Сохранённый материал','Saved material')).slice(0,256);
    const action=(label,work)=>{const button=document.createElement('button');button.type='button';button.className='text-button sc-builder-link';button.textContent=label;
      button.onclick=async()=>{button.disabled=true;try{await work();button.textContent=word('На вашей полке','On your shelf');}catch(error){button.disabled=false;report(error);}};return button;};
    container.prepend(action(word('Сохранить материал','Save material'),()=>save({type:'material',title,target:shelfMaterial(snapshot)})));
    for(const role of FORM_ROLES){const target=shelfForm(snapshot,role);if(!target)continue;
      const section=container.querySelector(`[data-form-role="${role}"]`);if(section)section.append(action(word('Сохранить эту форму','Save this form'),()=>save({type:'form',title,target})));
    }
  }
  return {shelf,save,open:()=>shelf.open(),addReadingActions,
    destroy(){if(disposed)return;disposed=true;reading?.abort();root.removeEventListener('sophia-save-research',saveEvent);root.removeEventListener('sophia-open-research',openEvent);opener.remove();narrow?.remove();void shelf.destroy();},
  };
}

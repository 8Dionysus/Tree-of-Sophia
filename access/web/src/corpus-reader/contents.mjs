// Bounded source-supplied structure. The browser never infers a book's chapters
// from paragraph ordinals, labels, or the shape of the graph.
import {corpusSectionLabel} from '../observatory/human-presentation.mjs';
export function mountCorpusContents({reader,provider,locale=()=> 'ru'}) {
  const root=reader.element;
  if(!root)throw new TypeError('Reader element is required.');
  let opened=false,disposed=false,request=null,generation=0,page=null,back=[],currentCursor=null,currentKey='',selectedVersionId=null;
  const english=()=>String(locale()||'ru').toLowerCase().startsWith('en');
  const words=()=>english()?{
    title:'Contents',close:'Close contents',empty:'Choose a work first.',unavailable:'The source does not provide a table of contents yet.',
    loading:'Loading sections…',failed:'These sections could not be loaded.',more:'More sections',back:'Previous sections',start:'Beginning',retry:'Retry',edition:'Contents for edition',
  }:{title:'Оглавление',close:'Закрыть оглавление',empty:'Сначала выберите произведение.',unavailable:'Источник пока не предоставляет оглавление.',
    loading:'Загружаются разделы…',failed:'Не удалось загрузить эти разделы.',more:'Следующие разделы',back:'Предыдущие разделы',start:'В начало',retry:'Повторить',edition:'Оглавление издания'};
  const state=()=>{const s=reader.state(),versions=reader.visibleVersions?.(s)??[];
    if(!versions.some(version=>version.id===selectedVersionId))selectedVersionId=s.activeVersionId;
    return {documentId:s.document?.id,versionId:selectedVersionId,versions};};
  const key=()=>{const {documentId,versionId}=state();return JSON.stringify({documentId,versionId});};
  const el=(tag,text='')=>{const node=document.createElement(tag);node.textContent=text;return node;};
  const button=(label,action)=>{const node=el('button',label);node.type='button';node.onclick=action;return node;};
  const close=()=>{opened=false;generation++;request?.abort();root.querySelector('.cr-contents')?.remove();const opener=root.querySelector('.cr-contents-button');opener?.setAttribute('aria-expanded','false');opener?.focus();};
  function render(message=null) {
    if(!opened||disposed)return;
    let panel=root.querySelector('.cr-contents');
    if(!panel){panel=el('aside');panel.className='cr-contents';panel.setAttribute('aria-label',words().title);panel.onkeydown=event=>{if(event.key==='Escape'){event.stopPropagation();event.preventDefault();close();}};root.append(panel);}
    const restoreEditionFocus=panel.querySelector('.cr-contents-edition select')===document.activeElement;
    panel.replaceChildren();
    const top=el('div');top.className='cr-contents-top';top.append(el('h2',words().title),button('×',close));top.lastChild.setAttribute('aria-label',words().close);panel.append(top);
    const selection=state();
    if(selection.versions.length>1){
      const label=el('label',words().edition);label.className='cr-contents-edition';
      const select=el('select');select.setAttribute('aria-label',words().edition);
      for(const version of selection.versions){const option=el('option',version.label);option.value=version.id;select.append(option);}
      select.value=selection.versionId;select.addEventListener('change',()=>{selectedVersionId=select.value;void load();});label.append(select);panel.append(label);
      if(restoreEditionFocus)select.focus({preventScroll:true});
    } else if(selection.versions[0])panel.append(el('p',`${words().edition}: ${selection.versions[0].label}`));
    if(message){const status=el('p',message);status.setAttribute('role','status');panel.append(status);}
    if(page){
      const list=el('ol');list.className='cr-contents-list';
      for(const item of page.items){const row=el('li');row.style.paddingInlineStart=`${Math.min(item.level??0,6)*.6}rem`;
        row.append(button(corpusSectionLabel(item.label,locale(),page.versionId),()=>{const address={documentId:page.documentId,versionId:page.versionId,unitId:item.unitId,revision:page.revision};close();void (reader.openPane ? reader.openPane(address) : reader.open(address));}));list.append(row);}
      panel.append(list);
      const controls=el('nav');controls.setAttribute('aria-label',words().title);
      const previous=button(words().back,()=>{const cursor=back.pop();void load(cursor,false);});previous.disabled=!back.length;
      const next=button(words().more,()=>{back.push(currentCursor);if(back.length>32)back.shift();void load(page.nextCursor,false);});next.disabled=!page.nextCursor;
      controls.append(previous,next,button(words().start,()=>{back=[];void load(null,false);}));panel.append(controls);
    }
  }
  async function load(cursor=null,reset=true) {
    const {documentId,versionId}=state(),selection={documentId,versionId};currentKey=key();
    request?.abort();request=new AbortController();const token=++generation;
    if(reset){back=[];page=null;}currentCursor=cursor;
    if(!selection.documentId){render(words().empty);return;}
    if(provider.capabilities?.structure!==true||typeof provider.structure!=='function'){render(words().unavailable);return;}
    render(words().loading);
    try{
      const value=await provider.structure({...selection,cursor,limit:24,signal:request.signal});
      if(disposed||token!==generation||currentKey!==key())return;
      page=value;render();
    }catch(error){if(disposed||token!==generation||error.name==='AbortError')return;render(words().failed);
      root.querySelector('.cr-contents')?.append(button(words().retry,()=>void load(cursor,false)));}
  }
  const ensure=()=>{
    if(disposed)return;
    const toolbar=root.querySelector('.cr-toolbar');
    if(toolbar&&!toolbar.querySelector('.cr-contents-button')){
      const opener=button(words().title,()=>{opened=!opened;opener.setAttribute('aria-expanded',String(opened));if(opened){selectedVersionId=null;void load();}else close();});opener.className='cr-contents-button';opener.setAttribute('aria-expanded',String(opened));toolbar.append(opener);
    }
    if(opened&&currentKey!==key()){void load();return;}
    if(opened&&!root.querySelector('.cr-contents'))render();
  };
  const observer=new MutationObserver(ensure);observer.observe(root,{childList:true,subtree:true});ensure();
  return {close,destroy(){disposed=true;generation++;request?.abort();observer.disconnect();root.querySelector('.cr-contents')?.remove();root.querySelector('.cr-contents-button')?.remove();}};
}

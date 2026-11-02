import {afterEach,expect,test,vi} from 'vitest';
import {mountCognitiveSession} from './cognitive-session.mjs';

class Element{
  constructor(tag){this.tagName=tag;this.children=[];this.listeners={};this.attributes={};this.dataset={};this.value='';this.textContent='';}
  append(...items){this.children.push(...items);}
  replaceChildren(...items){this.children=[...items];}
  setAttribute(key,value){this.attributes[key]=value;}
  addEventListener(key,fn){this.listeners[key]=fn;}
  get childElementCount(){return this.children.length;}
}
const all=root=>[root,...root.children.flatMap(all)];
const text=root=>all(root).map(item=>item.textContent).join(' ');
const active=[];
afterEach(()=>{for(const session of active.splice(0))session.destroy();vi.unstubAllGlobals();});
function fixture(){
  vi.stubGlobal('document',{createElement:tag=>new Element(tag)});vi.stubGlobal('location',{href:'http://localhost/zarathustra/'});
  const saved=new Map(),copy=value=>value===undefined?undefined:structuredClone(value);
  const notebook={getPreference:async key=>copy(saved.get(key)),setPreference:async(key,value,options)=>{if(options&&JSON.stringify(options.expectedValue)!==JSON.stringify(saved.get(key)))throw Object.assign(new Error('conflict'),{code:'conflict'});saved.set(key,copy(value));},exportPacket:async()=>({preferences:[...saved].map(([key,value])=>({key,value:copy(value)}))})};
  let question='Первый вопрос',questionReady=true,saveAllowed=true;
  const api={getQuestion:()=>question,ensureCurrentQuestion:async()=>questionReady,setQuestion:vi.fn(async value=>{if(!saveAllowed)return false;question=value;return true;})};
  const index={revision:'r1',get:id=>['one','source'].includes(id)?{id,title:{ru:id},kind:id==='source'?'passage':'conception'}:null,inspect:id=>index.get(id)};
  const mount=()=>{const root=new Element('div'),session=mountCognitiveSession({root,notebook,index,api,profile:'test',getSelection:()=> 'one'});active.push(session);return {root,session};};
  return {saved,api,index,mount,setReady:value=>{questionReady=value;},setSaveAllowed:value=>{saveAllowed=value;}};
}

test('queues research requests while preserving navigation as a separate trace, and resumes exact replies',async()=>{
  const f=fixture(),first=f.mount();await first.session.ready;
  await first.session.record('open',{id:'one'});await first.session.record('mode');
  const thought=await first.session.record('thought',{text:'Моя обоснованная мысль.'});
  const next=await first.session.dispatch('next-event',{timeout_ms:0});expect(next.event.id).toBe(thought.id);
  const log=all(first.root).find(item=>item.className==='cognitive-log'),trace=all(first.root).find(item=>item.className==='cognitive-trace');
  expect(text(log)).toContain('Моя обоснованная мысль.');expect(text(log)).not.toContain('Открыт предмет');expect(text(trace)).toContain('Открыт предмет');expect(text(trace)).not.toContain('Ожидает ответа');
  const response={event_id:thought.id,question_revision:thought.question_revision,index_revision:thought.index_revision,actor:'agent',kind:'feedback',body:'Ответ по выбранному месту.',source_ids:['source'],next_step:'Проверить контекст.'};
  const delivered=await first.session.dispatch('respond',response);expect(delivered.replayed).toBe(false);
  first.session.destroy();active.splice(active.indexOf(first.session),1);
  const reopened=f.mount();await reopened.session.ready;expect((await reopened.session.context()).session_id).toBe(thought.session_id);
  expect((await reopened.session.dispatch('next-event',{timeout_ms:0})).event).toBeNull();
  expect((await reopened.session.dispatch('respond',response)).replayed).toBe(true);
  await expect(reopened.session.dispatch('respond',{...response,next_step:'Другой следующий шаг.'})).rejects.toThrow('already preserved');
  const second=await reopened.session.record('thought',{text:'Следующая мысль.'});expect((await reopened.session.dispatch('next-event',{timeout_ms:0})).event.id).toBe(second.id);
});

test('saves the question through its existing owner and retains an unsaved draft on conflict',async()=>{
  const f=fixture(),{root,session}=f.mount();await session.ready;
  const question=all(root).find(item=>item.attributes['aria-label']==='Вопрос исследования'),save=all(root).find(item=>item.tagName==='button'&&item.textContent==='Сохранить вопрос');
  question.value='Новый вопрос';question.listeners.input();save.listeners.click();
  await vi.waitFor(()=>expect(text(root)).toContain('Вопрос сохранён.'));expect(f.api.setQuestion).toHaveBeenCalledWith('Новый вопрос');
  const event=(await session.dispatch('next-event',{timeout_ms:0})).event;expect(event.type).toBe('question');expect(event.question).toBe('Новый вопрос');expect(event.question_revision).toBe(1);
  f.setSaveAllowed(false);question.value='Несохранённый черновик';question.listeners.input();save.listeners.click();
  await vi.waitFor(()=>expect(text(root)).toContain('Вопрос не сохранён'));expect(question.value).toBe('Несохранённый черновик');
  f.setReady(false);await expect(session.record('thought',{text:'Не сохранять поверх конфликта.'})).rejects.toThrow('конфликт');
  expect([...f.saved.values()].filter(value=>value?.schema==='tos_cognitive_event_v1')).toHaveLength(1);
});

test('skips stale replies and proceeds to the next request',async()=>{
  const f=fixture(),{session}=f.mount();await session.ready;
  const thought=await session.record('thought',{text:'First'});
  await f.api.setQuestion('Changed');const later=await session.record('question',{text:'Changed'});
  const reply={event_id:thought.id,question_revision:thought.question_revision,index_revision:thought.index_revision,body:'Historical reply'};
  expect((await session.dispatch('respond',reply)).response.stale).toBe(true);
  expect((await session.dispatch('next-event',{timeout_ms:0})).event.id).toBe(later.id);
});

test('two tabs see persisted events and responses',async()=>{
  const f=fixture(),human=f.mount();await human.session.ready;const agent=f.mount();await agent.session.ready;
  const thought=await human.session.record('thought',{text:'Across tabs'});
  expect((await agent.session.dispatch('next-event',{timeout_ms:0})).event.id).toBe(thought.id);
  await agent.session.dispatch('respond',{event_id:thought.id,question_revision:thought.question_revision,index_revision:thought.index_revision,body:'Reply across tabs'});
  await vi.waitFor(()=>expect(text(human.root)).toContain('Reply across tabs'),{timeout:2500});
});

test('keeps a thought draft when paused',async()=>{
  const f=fixture(),{root,session}=f.mount();await session.ready;await session.dispatch('pause');
  const answer=all(root).find(item=>item.attributes['aria-label']==='Моя мысль или ответ агенту'),form=all(root).find(item=>item.tagName==='form');
  answer.value='Keep this';form.listeners.submit({preventDefault(){}});
  await vi.waitFor(()=>expect(all(root).find(item=>item.tagName==='button'&&item.textContent==='Отправить мысль').disabled).toBe(false));
  expect(answer.value).toBe('Keep this');
});

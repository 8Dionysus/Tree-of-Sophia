// A connected host supplies the intelligence. This owner retains the exact
// question, human actions and attributed replies in the existing notebook.
import {localeValue,copyLocale} from './locale.mjs';
import './cognitive-session.css';
const PREFIX='tos.public-research.cognitive.v1:';
const uid=()=>crypto.randomUUID();
const clone=value=>structuredClone(value);
const bounded=(value,max,name)=>{if(typeof value!=='string'||!value.trim()||value.length>max)throw new TypeError(`Invalid ${name}`);return value.trim();};

export function mountCognitiveSession({root,notebook,index,api,locale=()=> 'ru',profile,getSelection=()=>null,onOpen=()=>{},workspace,isActive=()=>true}){
  let header,expected,events=[],responses=[],disposed=false,serial=Promise.resolve(),waiters=new Set();
  const key=`${PREFIX}active:${profile}`,sessionKey=id=>`${PREFIX}session:${id}`,eventPrefix=id=>`${PREFIX}event:${id}:`,responsePrefix=id=>`${PREFIX}response:${id}:`;
  const el=(tag,value,cls='')=>{const node=document.createElement(tag);node.className=cls;if(value!==undefined)node.textContent=value;return node;};
  const t=(ru,en)=>copyLocale(locale)==='en'?en:ru;
  const status=el('p',undefined,'cognitive-status');status.setAttribute('role','status');
  const controls=el('div',undefined,'cognitive-controls'),level=document.createElement('select'),connect=el('button'),pause=el('button'),newStep=el('button');
  level.setAttribute('aria-label',t('Уровень помощи','Level of help'));for(const [value,ru,en]of [['light','Лёгкий','Light'],['medium','Средний','Medium'],['heavy','Тяжёлый','Heavy']]){const option=el('option',t(ru,en));option.value=value;level.append(option);}
  const intro=el('p'),launch=el('textarea',undefined,'cognitive-launch');launch.readOnly=true;launch.rows=5;launch.setAttribute('aria-label',t('Задание подключённому агенту','Connected agent instruction'));launch.hidden=true;
  const copy=el('button'),question=el('p'),selection=el('p'),log=el('div',undefined,'cognitive-log'),form=el('form'),answer=el('textarea'),send=el('button'),message=el('p');
  answer.rows=4;answer.maxLength=4000;answer.setAttribute('aria-label',t('Моя мысль или ответ агенту','My thought or answer to the agent'));send.type='submit';
  for(const b of [connect,pause,newStep,copy])b.type='button';
  controls.append(level,connect,pause,newStep);form.append(answer,send);root.append(el('h2',t('Познание с агентом','Learning with an agent')),intro,controls,status,launch,copy,question,selection,log,form,message);
  const notify=()=>{for(const wake of waiters)wake();};
  const ready=(async()=>{
    const active=await notebook.getPreference(key);let id=typeof active==='string'?active:null;
    if(id)header=await notebook.getPreference(sessionKey(id));
    if(!header){id=uid();header={schema:'tos_cognitive_session_v1',id,profile,help:'light',sequence:0,last_ack:null,paused:false,connected_at:null,question_revision:0,question:api.getQuestion(),created_at:new Date().toISOString()};await notebook.setPreference(sessionKey(id),header,{expectedValue:undefined});await notebook.setPreference(key,id,{expectedValue:active});}
    expected=clone(header);const packet=await notebook.exportPacket();
    events=packet.preferences.filter(item=>item.key.startsWith(eventPrefix(header.id))).map(item=>item.value).sort((a,b)=>a.sequence-b.sequence||a.id.localeCompare(b.id));
    responses=packet.preferences.filter(item=>item.key.startsWith(responsePrefix(header.id))).map(item=>item.value);
    render();return header;
  })().catch(error=>{message.textContent=t('Сессия не сохранилась: ','Session storage failed: ')+error.message;throw error;});
  const enqueue=run=>{const result=serial.catch(()=>{}).then(()=>ready).then(run);serial=result.catch(error=>{message.textContent=error.message;});return result;};
  async function update(change){
    for(let attempt=0;attempt<3;attempt++){
      const next=change(clone(header));
      try{await notebook.setPreference(sessionKey(header.id),next,{expectedValue:expected});header=next;expected=clone(next);return;}
      catch(error){if(error.code!=='conflict'||attempt===2)throw error;header=await notebook.getPreference(sessionKey(header.id));expected=clone(header);}
    }
  }
  function context(){
    const selected=getSelection(),id=typeof selected==='string'?selected:selected?.id;
    return {schema:'tos_cognitive_context_v1',active:isActive(),session_id:header?.id??null,profile,help:header?.help??'light',question:api.getQuestion(),question_revision:header?.question_revision??0,index_revision:index.revision,selected:id?index.inspect(id):null,last_ack:header?.last_ack??null,paused:header?.paused??false,
      experience:{actions:events.filter(item=>item.actor==='human').length,disclosed_source_ids:[...new Set(responses.flatMap(item=>item.source_ids??[]))]},
      task:'Read exact sources, distinguish the work from interpretations, respond to the human action at the selected help level, preserve alternatives and explain your grounds.'};
  }
  function instruction(){return `Сопровождай мою исследовательскую сессию Бонсая ${header.id}. Открой ${location.href} в видимой вкладке IAB основного агента; продолжим работу в этой вкладке. Прочитай WebMCP tools и tos.bonsai.session-context, затем tos.bonsai.session-join. Одной активной задачей читай tos.bonsai.session-next (timeout_ms:20000), исследуй точные основания через tos.bonsai.research-item и tos.bonsai.read-source, отвечай через tos.bonsai.session-respond. После каждого ответа продолжай ждать следующее действие до моей остановки. Уровень помощи ${header.help}; вопрос: ${api.getQuestion()}. При возобновлении используй тот же session_id и last_ack; исходные мысли, альтернативы и все источники сохраняй с их собственными адресами. Ответ относится к event_id, question_revision и index_revision полученного события. Пустое ожидание означает продолжить цикл. Для другая вкладки того же происхождения используется общий notebook; session-join принимает этот session_id.`;}
  function render(){
    if(disposed||!header)return;const age=header.connected_at?Date.now()-Date.parse(header.connected_at):Infinity;
    status.textContent=header.paused?t('Пауза · исследование сохранено','Paused · research saved'):age>60000?t('Ожидает подключения агента','Waiting for agent connection'):t('Агент подключён','Agent connected');
    intro.textContent=t('Читайте и формулируйте собственное прочтение. Подключённый агент отвечает по вашим действиям и точным основаниям.','Read and develop your own interpretation. Your connected agent responds to your actions and exact grounds.');
    connect.textContent=t('Подключить / продолжить с агентом','Connect / continue with agent');copy.textContent=t('Скопировать задание','Copy instruction');pause.textContent=header.paused?t('Продолжить','Resume'):t('Пауза','Pause');newStep.textContent=t('Исследовать выбранный предмет','Explore selected item');send.textContent=t('Отправить мысль','Send thought');answer.placeholder=t('Моя формулировка, вопрос, различение или возражение…','My interpretation, question, distinction or objection…');level.value=header.help;
    const selected=getSelection(),item=index.get(typeof selected==='string'?selected:selected?.id);selection.textContent=item?`${t('Предмет','Item')}: ${localeValue(item.title,locale())||item.id}`:'';question.textContent=`${t('Вопрос','Question')}: ${api.getQuestion()}`;
    launch.value=instruction();log.replaceChildren();
    for(const event of events.slice(-60)){
      const row=el('article',undefined,'cognitive-event');row.dataset.eventId=event.id;
      row.append(el('small',`${t('Действие','Action')} ${event.sequence} · ${event.type}`));
      if(event.text)row.append(el('p',event.text));
      if(event.item_id){const open=el('button',localeValue(index.get(event.item_id)?.title,locale())||event.item_id);open.type='button';open.addEventListener('click',()=>onOpen(event.item_id));row.append(open);}
      const replies=responses.filter(response=>response.event_id===event.id);
      for(const response of replies){const reply=el('section',undefined,'cognitive-response');reply.append(el('small',`${response.actor} · ${response.kind}${response.stale?' · '+t('предыдущий вопрос','earlier question'):''}`),el('p',response.body));
        if(response.next_step)reply.append(el('p',response.next_step,'cognitive-next'));
        for(const id of response.source_ids??[]){const open=el('button',localeValue(index.get(id)?.title,locale())||id);open.type='button';open.addEventListener('click',()=>onOpen(id));reply.append(open);}
        if(workspace){const save=el('button',t('Сохранить в моём исследовании','Save in my research'));save.type='button';save.addEventListener('click',()=>{try{workspace.addNote({id:`cognitive-${response.id}`,body:response.body.slice(0,4000),targetId:event.item_id??undefined});message.textContent=t('Результат сохранён с адресом предмета.','Result saved with its item address.');}catch(error){message.textContent=error.message;}});reply.append(save);}row.append(reply);
      }
      if(!replies.length)row.append(el('small',t('Ожидает ответа','Awaiting response')));log.append(row);
    }
  }
  function record(type,detail={}){return enqueue(async()=>{
    if(type==='mode'&&!isActive()){notify();return null;}
    if((header.paused&&type!=='resume')||!isActive())return null;
    const selected=getSelection(),item_id=detail.id??(typeof selected==='string'?selected:selected?.id)??null;
    if(item_id&&!index.get(item_id))throw new TypeError('Unknown research item');
    const question=api.getQuestion();await update(next=>({...next,sequence:next.sequence+1,question_revision:next.question===question?next.question_revision:next.question_revision+1,question}));
    const event={schema:'tos_cognitive_event_v1',id:uid(),session_id:header.id,sequence:header.sequence,actor:'human',type,item_id,help:header.help,question,question_revision:header.question_revision,index_revision:index.revision,text:typeof detail.text==='string'?detail.text.slice(0,4000):'',created_at:new Date().toISOString()};
    await notebook.setPreference(eventPrefix(header.id)+event.id,event,{expectedValue:undefined});events.push(event);render();notify();return clone(event);
  });}
  async function nextEvent(input={},signal){
    await ready;if(input.session_id&&input.session_id!==header.id)throw new TypeError('Session does not match this document');
    if(input.peek)return {context:context()};
    const after=input.after_event_id??header.last_ack,timeout=Math.min(20000,Math.max(0,Number(input.timeout_ms??20000)));
    const find=()=>{if(header.paused||!isActive())return null;const at=after?events.findIndex(event=>event.id===after):-1;if(after&&at<0)throw new TypeError('Unknown event cursor');return events.slice(at+1).find(event=>!responses.some(response=>response.event_id===event.id&&!response.stale))??null;};
    await enqueue(()=>update(next=>({...next,connected_at:new Date().toISOString()})));render();let event=find();if(event||!timeout)return {event,context:context()};
    await new Promise((resolve,reject)=>{let timer;const done=()=>{waiters.delete(wake);clearTimeout(timer);signal?.removeEventListener('abort',abort);resolve();};const wake=()=>done();const abort=()=>{waiters.delete(wake);clearTimeout(timer);reject(signal.reason??new DOMException('Cancelled','AbortError'));};waiters.add(wake);timer=setTimeout(done,timeout);signal?.addEventListener('abort',abort,{once:true});if(signal?.aborted)abort();});
    event=find();return {event,context:context(),waiting:!event};
  }
  async function dispatch(action,input={},signal){
    await ready;
    if(action==='context')return context();if(action==='next-event')return nextEvent(input,signal);
    return enqueue(async()=>{
      if(input.session_id&&input.session_id!==header.id)throw new TypeError('Session does not match this document');
      if(action==='join'||action==='resume'){await update(next=>({...next,paused:false,connected_at:new Date().toISOString()}));render();notify();return context();}
      if(action==='pause'){await update(next=>({...next,paused:true}));render();notify();return context();}
      if(action!=='respond')throw new TypeError('Unknown cognitive action');
      const event=events.find(event=>event.id===input.event_id);if(!event)throw new TypeError('Unknown cognitive event');
      if(input.question_revision!==event.question_revision||input.index_revision!==event.index_revision)throw new TypeError('Response must bind the exact event question and source revision');
      const source_ids=input.source_ids??[];if(!Array.isArray(source_ids)||source_ids.length>32||source_ids.some(id=>!index.get(id)))throw new TypeError('Response source IDs must resolve in this research index');
      const response={schema:'tos_cognitive_response_v1',id:`response-${event.id}`,event_id:event.id,session_id:header.id,question_revision:event.question_revision,index_revision:event.index_revision,actor:bounded(input.actor??'connected-agent',128,'actor'),kind:['question','hint','finding','feedback'].includes(input.kind)?input.kind:'feedback',body:bounded(input.body,12000,'response'),source_ids,next_step:typeof input.next_step==='string'?input.next_step.slice(0,2000):'',stale:event.question!==api.getQuestion()||event.index_revision!==index.revision,created_at:new Date().toISOString()};
      const prior=responses.find(value=>value.event_id===event.id);
      if(prior){if(prior.body!==response.body||JSON.stringify(prior.source_ids)!==JSON.stringify(source_ids))throw new TypeError('A response for this event is already preserved; create a new research event to continue');return {response:prior,replayed:true};}
      await notebook.setPreference(responsePrefix(header.id)+response.id,response,{expectedValue:undefined});responses.push(response);
      await update(next=>({...next,last_ack:response.stale?next.last_ack:event.id,connected_at:new Date().toISOString()}));render();notify();return {response,replayed:false,context:context()};
    });
  }
  connect.addEventListener('click',()=>{launch.hidden=false;render();launch.focus();launch.select();});copy.addEventListener('click',()=>void navigator.clipboard.writeText(instruction()).then(()=>message.textContent=t('Задание скопировано. Запустите его в Codex и продолжайте здесь.','Instruction copied. Start it in Codex and continue here.')).catch(()=>{launch.hidden=false;launch.select();}));
  pause.addEventListener('click',()=>void dispatch(header?.paused?'resume':'pause').catch(error=>message.textContent=error.message));
  newStep.addEventListener('click',()=>void record('research').catch(error=>message.textContent=error.message));
  level.addEventListener('change',()=>void enqueue(async()=>{await update(next=>({...next,help:level.value}));render();}).then(()=>record('help-level')).catch(error=>message.textContent=error.message));
  form.addEventListener('submit',event=>{event.preventDefault();const value=answer.value;if(!value.trim())return;send.disabled=true;void record('thought',{text:value}).then(()=>{if(answer.value===value)answer.value='';}).catch(error=>message.textContent=error.message).finally(()=>send.disabled=false);});
  const unsub=api.subscribeQuestion?.(()=>{render();});
  const timer=setInterval(()=>{if(!disposed)render();},15000);
  return {ready,context,dispatch,record,refreshLocale:render,destroy(){disposed=true;unsub?.();clearInterval(timer);notify();root.replaceChildren();}};
}
